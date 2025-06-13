use crate::api::get_index_page;
use crate::config::Config;
use crate::crawler::Crawler;
use crate::entity::{Job, JobStatus};
use sqlx::SqlitePool;
use std::collections::HashSet;
use tracing::instrument;

#[derive(Clone)]
pub struct Cmd {
    config: Config,
    conn: SqlitePool,
}
impl Cmd {
    pub fn new(config: Config, conn: SqlitePool) -> Self {
        Self { config, conn }
    }
    #[instrument(skip_all)]
    pub async fn cron(&self, page: usize) -> anyhow::Result<()> {
        let (crawler, sender) = Crawler::new(self.config.clone(), self.conn.clone()).await;
        let set = crawler.launch().await;
        let mut ids = vec![];
        for i in 1..=page {
            ids.extend(get_index_page(self.config.base_url.as_str(), i).await?)
        }
        for id in ids {
            sender.send(id).await?;
        }
        drop(sender);
        set.join_all().await;
        Ok(())
    }
    #[instrument(skip_all)]
    pub async fn retry(&self) {
        let failed_discussion_jobs =
            Job::find_by_entity_status("discussion", JobStatus::Failed, &self.conn).await;
        let (crawler, sender) = Crawler::new(self.config.clone(), self.conn.clone()).await;
        let set = crawler.launch().await;
        for job in failed_discussion_jobs {
            sender.send(job.entity_id).await.unwrap();
        }
        drop(sender);
        set.join_all().await;
    }
    #[instrument(skip_all)]
    pub async fn full(&self, min_id: u64, max_id: u64, ignore_existed: bool) {
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
        for id in min_id..=max_id {
            if ignore_ids.contains(&id) {
                continue;
            }
            sender.send(id).await.unwrap();
        }
        drop(sender);
        set.join_all().await;
    }
}
