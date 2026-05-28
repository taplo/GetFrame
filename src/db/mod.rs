pub mod metrics_history;
pub mod streams;
pub mod tasks;

use sqlx::mysql::MySqlPoolOptions;
use sqlx::MySqlPool;

pub async fn init_pool(url: &str, max_connections: u32) -> Result<MySqlPool, sqlx::Error> {
    let pool = MySqlPoolOptions::new()
        .max_connections(max_connections)
        .connect(url)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}
