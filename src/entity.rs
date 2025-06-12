use anyhow::anyhow;
use chrono::{FixedOffset, Utc};
use sqlx::{FromRow, SqlitePool, query};

#[derive(Debug, Clone, Default, FromRow)]
pub struct Post {
    pub id: u64,
    pub user_id: u64,
    pub discussion_id: u64,
    pub reply_to_id: u64,
    pub username: String,
    pub content: String,
    pub created_at: chrono::DateTime<FixedOffset>,
}
#[derive(Debug, Clone, Default, FromRow)]
pub struct Discussion {
    pub id: u64,
    pub user_id: u64,
    pub username: String,
    pub title: String,
    #[sqlx(json)]
    pub tags: Vec<String>,
    #[sqlx(skip)]
    pub posts: Vec<Post>,
    pub is_frontpage: bool,
    pub created_at: chrono::DateTime<FixedOffset>,
}
impl Discussion {
    pub async fn save(&self, pool: &SqlitePool) {
        // TODO upsert
        query("insert into discussions (id,user_id,username,title,tags,is_frontpage,created_at) values (?,?,?,?,?,?,?)")
            .bind(self.id as i64)
            .bind(self.user_id as i64)
            .bind(self.username.as_str())
            .bind(self.title.as_str())
            .bind(serde_json::to_string(&self.tags).unwrap())
            .bind(self.is_frontpage)
            .bind(self.created_at)
            .execute(pool).await.unwrap();
    }
}
#[derive(Debug, Clone, FromRow)]
pub struct Job {
    pub id: u64,
    pub entity: String,
    pub entity_id: u64,
    #[sqlx(try_from = "String")]
    pub status: JobStatus,
}
#[derive(Debug, Clone)]
pub enum JobStatus {
    Failed,
    Success,
}
impl TryFrom<String> for JobStatus {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "failed" => Ok(JobStatus::Failed),
            "success" => Ok(JobStatus::Success),
            _ => Err(anyhow!("")),
        }
    }
}
