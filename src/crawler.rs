use crate::api::{ApiError, GetDiscussionOptions, GetDiscussionOptionsBuilder, get_discussion};
use crate::config::Config;
use crate::entity::{Job, JobStatus};
use async_channel::{Receiver, Sender};
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tracing::{error, info, instrument};

#[derive(Clone)]
pub struct Crawler {
    config: Config,
    receiver: Receiver<u64>, // discussion id
    get_discussion_options: GetDiscussionOptions,
    sem: Arc<Semaphore>,
    conn: SqlitePool,
}
impl Crawler {
    pub async fn new(config: Config, conn: SqlitePool) -> anyhow::Result<(Self, Sender<u64>)> {
        let get_discussion_options = GetDiscussionOptionsBuilder::default()
            .base_url(config.base_url.to_string())
            .concurrency(config.concurrency)
            .build()?;
        let (sender, receiver) = async_channel::bounded::<u64>(1);
        Ok((
            Self {
                sem: Arc::new(Semaphore::new(config.concurrency)),
                config,
                receiver,
                get_discussion_options,
                conn,
            },
            sender,
        ))
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
            let discussion_res = get_discussion(
                id,
                self.get_discussion_options.clone(),
                Some(self.sem.clone()),
            )
            .await;
            match discussion_res {
                Ok(discussion) => {
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
                Err(err) => {
                    error!(id, "Cannot get discussion: {:#}", err);
                    Job {
                        entity: "discussion".to_string(),
                        entity_id: id,
                        status: if matches!(
                            err.root_cause().downcast_ref(),
                            Some(ApiError::ImpossibleDiscussion)
                        ) {
                            JobStatus::Impossible
                        } else {
                            JobStatus::Failed
                        },
                    }
                    .save(&self.conn)
                    .await;
                }
            }
        }
    }
}
