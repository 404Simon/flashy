#[cfg(feature = "ssr")]
use bcrypt::{hash, verify, DEFAULT_COST};
#[cfg(feature = "ssr")]
use leptos::prelude::ServerFnError;
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
    session.delete().await
}

#[cfg(feature = "ssr")]
pub async fn require_auth() -> Result<UserSession, ServerFnError> {
    use leptos_axum::extract;

    let session = extract::<Session>()
        .await
        .map_err(|_| ServerFnError::new("Authentication error"))?;

    get_user_from_session(&session)
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))
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
        .map_err(|e| sqlx::Error::Protocol(format!("bcrypt error: {e}").into()))?;

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
