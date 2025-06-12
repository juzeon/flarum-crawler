use sqlx::SqlitePool;

pub async fn get_connection_pool(path: &str) -> anyhow::Result<SqlitePool> {
    Ok(SqlitePool::connect(path).await?)
}
