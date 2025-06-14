use crate::api::{
    GetDiscussionOptions, GetDiscussionOptionsBuilder, GetDiscussionResult, get_discussion,
};
use crate::config::Config;
use crate::entity::{Discussion, Job, JobStatus};
use async_channel::{Receiver, Sender};
use sqlx::SqlitePool;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tracing::{error, info, instrument, warn};

#[derive(Clone)]
pub struct Crawler {
    config: Config,
    receiver: Receiver<u64>, // discussion id
    get_discussion_options: GetDiscussionOptions,
    sem: Arc<Semaphore>,
    conn: SqlitePool,
}
impl Crawler {
    pub async fn new(config: Config, conn: SqlitePool) -> (Self, Sender<u64>) {
        let get_discussion_options = GetDiscussionOptionsBuilder::default()
            .base_url(config.base_url.to_string())
            .concurrency(config.concurrency)
            .build()
            .unwrap();
        let (sender, receiver) = async_channel::bounded::<u64>(1);
        (
            Self {
                sem: Arc::new(Semaphore::new(config.concurrency)),
                config,
                receiver,
                get_discussion_options,
                conn,
            },
            sender,
        )
    }
    pub async fn launch(&self) -> JoinSet<()> {
        let mut set = JoinSet::new();
        for i in 1..=self.config.concurrency {
            let self_clone = self.clone();
            set.spawn(async move { self_clone.worker(i).await });
        }
        set
    }
    #[instrument(skip(self))]
    async fn worker(&self, ix: usize) {
        while let Ok(id) = self.receiver.recv().await {
            info!(id, "Getting discussion");
            let mut options = self.get_discussion_options.clone();
            if let Some(discussion) = Discussion::find_by_id(id, &self.conn).await {
                options.existing_post_ids = discussion
                    .posts
                    .into_iter()
                    .map(|x| x.id)
                    .collect::<HashSet<_>>();
            }
            let get_discussion_res = get_discussion(id, options, Some(self.sem.clone())).await;
            match get_discussion_res {
                Ok(discussion_res) => match discussion_res {
                    GetDiscussionResult::Impossible => {
                        warn!(id, "Impossible to get discussion");
                        Job {
                            entity: "discussion".to_string(),
                            entity_id: id,
                            status: JobStatus::Impossible,
                        }
                        .save(&self.conn)
                        .await;
                    }
                    GetDiscussionResult::Ok(discussion) => {
                        discussion.save_with_posts(&self.conn).await;
                        Job {
                            entity: "discussion".to_string(),
                            entity_id: id,
                            status: JobStatus::Success,
                        }
                        .save(&self.conn)
                        .await;
                        info!(id, "Saved discussion");
                    }
                    GetDiscussionResult::PartialError(discussion) => {
                        discussion.save_with_posts(&self.conn).await;
                        Job {
                            entity: "discussion".to_string(),
                            entity_id: id,
                            status: JobStatus::Partial,
                        }
                        .save(&self.conn)
                        .await;
                        info!(id, "Saved discussion (partial)");
                    }
                },
                Err(err) => {
                    error!(id, "Cannot get discussion: {:#}", err);
                    Job {
                        entity: "discussion".to_string(),
                        entity_id: id,
                        status: JobStatus::Failed,
                    }
                    .save(&self.conn)
                    .await;
                }
            }
        }
    }
}
