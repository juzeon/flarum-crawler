use crate::api::get_index_page;
use crate::config::Config;
use crate::crawler::Crawler;
use crate::entity::{Job, JobStatus};
use crate::server::{AppState, run_server};
use sqlx::SqlitePool;
use std::collections::HashSet;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, instrument};

#[derive(Clone)]
pub struct Cmd {
    config: Config,
    conn: SqlitePool,
}
impl Cmd {
    pub fn new(config: Config, conn: SqlitePool) -> Self {
        Self { config, conn }
    }
    pub async fn server(&self, addr: String, port: u16) {
        let state = AppState {
            conn: self.conn.clone(),
            config: self.config.clone(),
        };
        run_server(addr, port, state).await
    }
    #[instrument(skip_all)]
    pub async fn cron(&self, page: usize) -> anyhow::Result<()> {
        let (crawler, sender) = Crawler::new(self.config.clone(), self.conn.clone()).await;
        let set = crawler.launch().await;
        let mut ids = vec![];
        for i in 1..=page {
            ids.extend(get_index_page(self.config.base_url.as_str(), i, None).await?)
        }
        let len = ids.len();
        for (ix, id) in ids.into_iter().enumerate() {
            info!(
                current = ix + 1,
                total = len,
                id,
                "Start to crawl discussion"
            );
            sender.send(id).await?;
        }
        drop(sender);
        set.join_all().await;
        Ok(())
    }
    #[instrument(skip_all)]
    pub async fn retry(&self) {
        let mut retry_discussion_jobs =
            Job::find_by_entity_status("discussion", JobStatus::Failed, &self.conn).await;
        retry_discussion_jobs
            .extend(Job::find_by_entity_status("discussion", JobStatus::Partial, &self.conn).await);
        let (crawler, sender) = Crawler::new(self.config.clone(), self.conn.clone()).await;
        let set = crawler.launch().await;
        for job in retry_discussion_jobs {
            sender.send(job.entity_id).await.unwrap();
        }
        drop(sender);
        set.join_all().await;
    }
    #[instrument(skip_all)]
    pub async fn full(&self, page_start: usize, ignore_existed: bool) {
        let mut ignore_ids =
            Job::find_by_entity_status("discussion", JobStatus::Impossible, &self.conn)
                .await
                .into_iter()
                .map(|x| x.entity_id)
                .collect::<HashSet<_>>();
        if ignore_existed {
            ignore_ids.extend(
                Job::find_by_entity_status("discussion", JobStatus::Success, &self.conn)
                    .await
                    .into_iter()
                    .map(|x| x.entity_id),
            );
        }
        let (crawler, sender) = Crawler::new(self.config.clone(), self.conn.clone()).await;
        let set = crawler.launch().await;
        let mut current_page = page_start;
        loop {
            info!(
                current_page,
                offset = (current_page - 1) * 20,
                "Processing index page"
            );
            let ids = loop {
                match get_index_page(
                    self.config.base_url.as_str(),
                    current_page,
                    Some("createdAt"),
                )
                .await
                {
                    Ok(res) => break res,
                    Err(err) => {
                        error!("Error get index page: {:#}", err);
                        sleep(Duration::from_secs(1)).await;
                    }
                }
            };
            if ids.is_empty() {
                break;
            }
            for id in ids {
                if ignore_ids.contains(&id) {
                    continue;
                }
                sender.send(id).await.unwrap();
            }
            current_page += 1;
        }
        drop(sender);
        set.join_all().await;
    }
}
