#[cfg(feature = "ssr")]
use bcrypt::{DEFAULT_COST, hash, verify};
#[cfg(feature = "ssr")]
use leptos::prelude::ServerFnError;
#[cfg(feature = "ssr")]
use std::sync::{Arc, OnceLock};
#[cfg(feature = "ssr")]
use tower_sessions::Session;

#[cfg(feature = "ssr")]
use super::models::UserSession;

#[cfg(feature = "ssr")]
pub fn hash_password(password: &str) -> Result<String, bcrypt::BcryptError> {
    hash(password, DEFAULT_COST)
}

#[cfg(feature = "ssr")]
pub fn verify_password(password: &str, hash: &str) -> Result<bool, bcrypt::BcryptError> {
    verify(password, hash)
}

#[cfg(feature = "ssr")]
fn password_workers() -> &'static Arc<tokio::sync::Semaphore> {
    static WORKERS: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
    WORKERS.get_or_init(|| Arc::new(tokio::sync::Semaphore::new(4)))
}

#[cfg(feature = "ssr")]
pub async fn verify_password_bounded(password: String, hash: String) -> Result<bool, ()> {
    let permit = password_workers()
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| ())?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        verify_password(&password, &hash).unwrap_or(false)
    })
    .await
    .map_err(|_| ())
}

#[cfg(feature = "ssr")]
pub async fn hash_password_bounded(password: String) -> Result<String, ()> {
    let permit = password_workers()
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| ())?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        hash_password(&password).map_err(|_| ())
    })
    .await
    .map_err(|_| ())?
}

#[cfg(feature = "ssr")]
pub async fn get_user_from_session(session: &Session) -> Option<UserSession> {
    session.get::<UserSession>("user").await.ok().flatten()
}

#[cfg(feature = "ssr")]
pub async fn set_user_in_session(
    session: &Session,
    user: &UserSession,
) -> Result<(), tower_sessions::session::Error> {
    session.insert("user", user).await
}

#[cfg(feature = "ssr")]
pub async fn clear_session(session: &Session) -> Result<(), tower_sessions::session::Error> {
    // `flush` (not just `delete`) also drops the session ID, so the middleware
    // expires the session cookie in the browser instead of leaving the dead
    // session ID behind.
    session.flush().await
}

#[cfg(feature = "ssr")]
pub async fn require_auth() -> Result<UserSession, ServerFnError> {
    use leptos::prelude::expect_context;
    use leptos_axum::extract;
    use sqlx::SqlitePool;

    let session = extract::<Session>()
        .await
        .map_err(|_| ServerFnError::new("Authentication error"))?;

    let stored = get_user_from_session(&session)
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    let pool = expect_context::<SqlitePool>();
    let current = sqlx::query_as::<_, (i64, String, i64)>(
        "SELECT id, username, is_admin FROM users WHERE id = ?",
    )
    .bind(stored.id)
    .fetch_optional(&pool)
    .await
    .map_err(|_| ServerFnError::new("Authentication error"))?
    .ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    Ok(UserSession {
        id: current.0,
        username: current.1,
        is_admin: current.2 == 1,
    })
}

#[cfg(feature = "ssr")]
pub async fn ensure_admin_user(pool: &sqlx::SqlitePool) -> Result<(), sqlx::Error> {
    let existing = sqlx::query!("SELECT id FROM users WHERE is_admin = 1 LIMIT 1")
        .fetch_optional(pool)
        .await?;

    if existing.is_some() {
        return Ok(());
    }

    let admin_username = std::env::var("ADMIN_USERNAME").unwrap_or_else(|_| "admin".to_string());
    let admin_password = std::env::var("ADMIN_PASSWORD").unwrap_or_else(|_| "admin123".to_string());
    let admin_email = std::env::var("ADMIN_EMAIL").ok();

    let password_hash = hash_password(&admin_password)
        .map_err(|e| sqlx::Error::Protocol(format!("bcrypt error: {e}")))?;

    sqlx::query!(
        r#"
        INSERT INTO users (username, password_hash, email, is_admin)
        VALUES (?, ?, ?, 1)
        ON CONFLICT(username) DO UPDATE SET
            password_hash = excluded.password_hash,
            email = excluded.email,
            is_admin = 1,
            updated_at = datetime('now')
        "#,
        admin_username,
        password_hash,
        admin_email
    )
    .execute(pool)
    .await?;

    Ok(())
}
