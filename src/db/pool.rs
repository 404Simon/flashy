#[cfg(feature = "ssr")]
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

/// Initialize database connection pool and run migrations.
#[cfg(feature = "ssr")]
pub async fn init_db() -> Result<SqlitePool, sqlx::Error> {
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:flashy.db".to_string());

    // Formula: (cores * 2) + 1 for SQLite (single spindle).
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let max_connections = (cores * 2 + 1).clamp(5, 20);

    let pool = SqlitePoolOptions::new()
        .max_connections(max_connections as u32)
        .connect(&database_url)
        .await?;

    sqlx::query("PRAGMA journal_mode = WAL")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA synchronous = NORMAL")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA cache_size = -64000")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA busy_timeout = 5000")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}
