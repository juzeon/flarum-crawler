use crate::config::Config;
use crate::crawler::Crawler;
use crate::entity::{Job, JobStatus};
use derive_builder::Builder;
use sqlx::SqlitePool;
use std::collections::HashSet;
use tracing::instrument;

#[derive(Debug, Clone, Builder)]
pub struct FullOptions {
    pub config: Config,
    pub conn: SqlitePool,
    pub max_id: u64,
    pub min_id: u64,
}
#[instrument(skip_all)]
pub async fn full(options: FullOptions) -> anyhow::Result<()> {
    let impossible_discussion_ids =
        Job::find_by_entity_status("discussion", JobStatus::Impossible, &options.conn)
            .await
            .into_iter()
            .map(|x| x.entity_id)
            .collect::<HashSet<_>>();
    let (crawler, sender) = Crawler::new(options.config, options.conn).await?;
    let set = crawler.launch().await;
    for id in options.min_id..=options.max_id {
        if impossible_discussion_ids.contains(&id) {
            continue;
        }
        sender.send(id).await?;
    }
    drop(sender);
    set.join_all().await;
    Ok(())
}
