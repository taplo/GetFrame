use sqlx::mysql::MySqlPoolOptions;
use sqlx::MySqlPool;
use std::sync::atomic::{AtomicU64, Ordering};

static DB_COUNTER: AtomicU64 = AtomicU64::new(0);

fn base_url() -> String {
    std::env::var("DATABASE_URL_TEST")
        .unwrap_or_else(|_| "mysql://root:root@127.0.0.1:3306/getframe_test".into())
}

fn admin_url_from(url: &str) -> String {
    url.rfind('/')
        .map(|i| url[..i].to_string())
        .unwrap_or_else(|| "mysql://root:root@127.0.0.1:3306".into())
}

pub async fn setup_db() -> MySqlPool {
    let counter = DB_COUNTER.fetch_add(1, Ordering::Relaxed);
    let base = base_url();
    let admin_url = admin_url_from(&base);
    let db_name = format!("getframe_test_{}_{}", std::process::id(), counter);
    let test_url = format!("{}/{}", admin_url, db_name);

    let admin_pool = MySqlPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .expect("Failed to connect to MySQL for test DB setup");

    sqlx::query(&format!("DROP DATABASE IF EXISTS `{}`", db_name))
        .execute(&admin_pool)
        .await
        .expect("Failed to drop test database");
    sqlx::query(&format!("CREATE DATABASE IF NOT EXISTS `{}`", db_name))
        .execute(&admin_pool)
        .await
        .expect("Failed to create test database");
    admin_pool.close().await;

    let pool = MySqlPoolOptions::new()
        .max_connections(2)
        .connect(&test_url)
        .await
        .expect("Failed to connect to per-test database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Migration failed");

    pool
}

pub async fn cleanup_tables(pool: &MySqlPool) {
    sqlx::query("DELETE FROM task_events")
        .execute(pool).await.ok();
    sqlx::query("DELETE FROM metrics_history")
        .execute(pool).await.ok();
    sqlx::query("DELETE FROM tasks")
        .execute(pool).await.ok();
    sqlx::query("DELETE FROM streams")
        .execute(pool).await.ok();
}
