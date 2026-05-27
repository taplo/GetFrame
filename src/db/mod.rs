pub mod streams;
pub mod tasks;

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

pub async fn init_pool(url: &str, max_connections: u32) -> Result<PgPool, sqlx::Error> {
    let pool = PgPoolOptions::new()
        .max_connections(max_connections)
        .connect(url)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}
