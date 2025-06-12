use crate::api::GetDiscussionOptionsBuilder;
use crate::config::Config;
use crate::entity::{Discussion, Post};
use derive_builder::Builder;
use sqlx::{Executor, SqlitePool, query, query_as};

#[derive(Debug, Clone, Builder)]
pub struct FullOptions {
    pub config: Config,
    pub conn: SqlitePool,
}
pub async fn full(options: FullOptions) -> anyhow::Result<()> {
    let get_discussion_options = GetDiscussionOptionsBuilder::default()
        .base_url(options.config.base_url.to_string())
        .concurrency(options.config.concurrency)
        .build()?;
    // let res=query("insert into posts (id,user_id,discussion_id,reply_to_id,username,content,created_at) values (?,?,?,?,?,?,?)")
    //     .bind(1).execute(&options.conn).await?;
    let res: Discussion = query_as("select * from discussions")
        .fetch_one(&options.conn)
        .await?;
    dbg!(res);
    Ok(())
}
