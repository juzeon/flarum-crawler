#[derive(Debug, Clone, Default)]
pub struct Post {
    pub id: u64,
    pub reply_to_id: Option<u64>,
    pub user_id: u64,
    pub user: String,
    pub content: String,
    pub created_at: String,
}
#[derive(Debug, Clone, Default)]
pub struct Discussion {
    pub id: u64,
    pub title: String,
    pub tags: Vec<String>,
    pub posts: Vec<Post>,
}
