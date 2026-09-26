use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use time::OffsetDateTime;

use super::service::OAuthError;

const WINDOW_SECONDS: i64 = 60;

/// Applies process-independent fixed-window limits. The upsert is a single
/// atomic SQLite statement, so concurrent workers cannot overrun the limit.
pub(super) async fn enforce(
    pool: &SqlitePool,
    operation: &str,
    source: &str,
    peer_limit: i64,
    global_limit: i64,
) -> Result<(), OAuthError> {
    let source_digest = URL_SAFE_NO_PAD.encode(Sha256::digest(source.as_bytes()));
    increment(
        pool,
        &format!("{operation}:peer:{source_digest}"),
        peer_limit,
    )
    .await?;
    increment(pool, &format!("{operation}:global"), global_limit).await
}

async fn increment(pool: &SqlitePool, rate_key: &str, limit: i64) -> Result<(), OAuthError> {
    let timestamp = OffsetDateTime::now_utc().unix_timestamp();
    let window_start = timestamp - timestamp.rem_euclid(WINDOW_SECONDS);
    let accepted = sqlx::query_scalar::<_, i64>(
        r#"INSERT INTO oauth_rate_limits (rate_key, window_start, request_count)
           VALUES (?, ?, 1)
           ON CONFLICT(rate_key, window_start) DO UPDATE
           SET request_count = request_count + 1
           WHERE request_count < ?
           RETURNING request_count"#,
    )
    .bind(rate_key)
    .bind(window_start)
    .bind(limit)
    .fetch_optional(pool)
    .await?;
    accepted.map(|_| ()).ok_or(OAuthError::RateLimited)
}
