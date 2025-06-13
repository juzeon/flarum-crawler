use anyhow::anyhow;
use chrono::{FixedOffset, Utc};
use sqlx::{Executor, FromRow, QueryBuilder, Sqlite, SqlitePool, query, query_as};
use std::fmt;
use std::fmt::Display;

#[derive(Debug, Clone, Default, FromRow)]
pub struct Post {
    pub id: u64,
    pub user_id: u64,
    pub discussion_id: u64,
    pub reply_to_id: u64,
    pub username: String,
    pub user_display_name: String,
    pub content: String,
    pub created_at: chrono::DateTime<FixedOffset>,
}
impl Post {
    pub async fn save(&self, pool: &SqlitePool) {
        query(
            r#"
            INSERT INTO posts (id, user_id, discussion_id, reply_to_id, username, user_display_name, content, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT (id) DO UPDATE SET
                user_id = EXCLUDED.user_id,
                discussion_id = EXCLUDED.discussion_id,
                reply_to_id = EXCLUDED.reply_to_id,
                username = EXCLUDED.username,
                user_display_name = EXCLUDED.user_display_name,
                content = EXCLUDED.content,
                created_at = EXCLUDED.created_at
            "#,
        )
            .bind(self.id as i64)
            .bind(self.user_id as i64)
            .bind(self.discussion_id as i64)
            .bind(self.reply_to_id as i64)
            .bind(&self.username)
            .bind(&self.user_display_name)
            .bind(&self.content)
            .bind(self.created_at)
            .execute(pool)
            .await
            .unwrap();
    }
}

#[derive(Debug, Clone, Default, FromRow)]
pub struct Discussion {
    pub id: u64,
    pub user_id: u64,
    pub username: String,
    pub user_display_name: String,
    pub title: String,
    #[sqlx(json)]
    pub tags: Vec<String>,
    #[sqlx(skip)]
    pub posts: Vec<Post>,
    pub is_frontpage: bool,
    pub created_at: chrono::DateTime<FixedOffset>,
}
impl Discussion {
    pub async fn save_with_posts(&self, pool: &SqlitePool) {
        let mut tx = pool.begin().await.unwrap();
        query(
            r#"
            INSERT INTO discussions (id, user_id, username, user_display_name, title, tags, is_frontpage, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT (id) DO UPDATE SET
                user_id = EXCLUDED.user_id,
                username = EXCLUDED.username,
                user_display_name = EXCLUDED.user_display_name,
                title = EXCLUDED.title,
                tags = EXCLUDED.tags,
                is_frontpage = EXCLUDED.is_frontpage,
                created_at = EXCLUDED.created_at
            "#,
        )
            .bind(self.id as i64)
            .bind(self.user_id as i64)
            .bind(&self.username)
            .bind(&self.user_display_name)
            .bind(&self.title)
            .bind(serde_json::to_string(&self.tags).unwrap())
            .bind(self.is_frontpage)
            .bind(self.created_at)
            .execute(&mut *tx)
            .await
            .unwrap();
        if !self.posts.is_empty() {
            let mut query_builder: QueryBuilder<Sqlite> = QueryBuilder::new(
                r#"
            INSERT INTO posts (id, user_id, discussion_id, reply_to_id, username, user_display_name, content, created_at)
            "#,
            );
            query_builder.push_values(&self.posts, |mut b, post| {
                b.push_bind(post.id as i64)
                    .push_bind(post.user_id as i64)
                    .push_bind(post.discussion_id as i64)
                    .push_bind(post.reply_to_id as i64)
                    .push_bind(&post.username)
                    .push_bind(&post.user_display_name)
                    .push_bind(&post.content)
                    .push_bind(post.created_at);
            });
            query_builder.push(
                r#"
            ON CONFLICT (id) DO UPDATE SET
                user_id = EXCLUDED.user_id,
                discussion_id = EXCLUDED.discussion_id,
                reply_to_id = EXCLUDED.reply_to_id,
                username = EXCLUDED.username,
                user_display_name = EXCLUDED.user_display_name,
                content = EXCLUDED.content,
                created_at = EXCLUDED.created_at
            "#,
            );
            query_builder.build().execute(&mut *tx).await.unwrap();
        }
        tx.commit().await.unwrap();
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct Job {
    pub entity: String,
    pub entity_id: u64,
    #[sqlx(try_from = "String")]
    pub status: JobStatus,
}
#[derive(Debug, Clone)]
pub enum JobStatus {
    Failed,
    Success,
    Impossible,
}
impl TryFrom<String> for JobStatus {
    type Error = anyhow::Error;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "failed" => Ok(JobStatus::Failed),
            "success" => Ok(JobStatus::Success),
            "impossible" => Ok(JobStatus::Impossible),
            _ => Err(anyhow!("Unknown JobStatus: {}", value)),
        }
    }
}
impl Display for JobStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            JobStatus::Failed => write!(f, "failed"),
            JobStatus::Success => write!(f, "success"),
            JobStatus::Impossible => write!(f, "impossible"),
        }
    }
}
impl Job {
    pub async fn find_by_entity_status(
        entity: &str,
        status: JobStatus,
        pool: &SqlitePool,
    ) -> Vec<Self> {
        query_as(r"select * from jobs where entity=? and status=?")
            .bind(entity)
            .bind(status.to_string())
            .fetch_all(pool)
            .await
            .unwrap()
    }
    pub async fn save(&self, pool: &SqlitePool) {
        query(
            r#"
            INSERT INTO jobs (entity, entity_id, status)
            VALUES (?, ?, ?)
            ON CONFLICT (entity, entity_id) DO UPDATE SET
                status = EXCLUDED.status
            "#,
        )
        .bind(&self.entity)
        .bind(self.entity_id as i64)
        .bind(self.status.to_string())
        .execute(pool)
        .await
        .unwrap();
    }
}
