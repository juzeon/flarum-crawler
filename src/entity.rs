use chrono::{FixedOffset, Utc};
use sqlx::FromRow;

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
