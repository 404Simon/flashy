use leptos::prelude::*;
#[cfg(feature = "ssr")]
use leptos_axum::extract;
#[cfg(feature = "ssr")]
use tower_sessions::Session;

use super::models::UserSession;
#[cfg(feature = "ssr")]
use super::models::User;
#[cfg(feature = "ssr")]
use super::utils::{clear_session, get_user_from_session, hash_password, set_user_in_session, verify_password};

#[server(RegisterUser)]
pub async fn register_user(
    invite_token: String,
    username: String,
    password: String,
    email: Option<String>,
) -> Result<UserSession, ServerFnError> {
    use sqlx::SqlitePool;

    if username.trim().len() < 3 {
        return Err(ServerFnError::new("Username must be at least 3 characters"));
    }
    if password.len() < 8 {
        return Err(ServerFnError::new("Password must be at least 8 characters"));
    }

    let pool = expect_context::<SqlitePool>();

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let invite = sqlx::query!(
        r#"
        SELECT id, is_reusable, duration_days, created_at, uses
        FROM invites
        WHERE token = ?
        "#,
        invite_token
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let invite = invite.ok_or_else(|| ServerFnError::new("Invalid invite token"))?;

    let valid = sqlx::query_scalar!(
        r#"
        SELECT CASE
            WHEN datetime(created_at, '+' || duration_days || ' days') > datetime('now')
            THEN 1
            ELSE 0
        END as "valid!: i64"
        FROM invites
        WHERE id = ?
        "#,
        invite.id
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
        == 1;

    if !valid {
        return Err(ServerFnError::new("Invite has expired"));
    }

    if invite.is_reusable == 0 && invite.uses >= 1 {
        return Err(ServerFnError::new("Invite has already been used"));
    }

    let existing = sqlx::query!("SELECT id FROM users WHERE username = ?", username)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if existing.is_some() {
        return Err(ServerFnError::new("Username already exists"));
    }

    let password_hash = hash_password(&password).map_err(|e| ServerFnError::new(e.to_string()))?;

    let result = sqlx::query!(
        "INSERT INTO users (username, password_hash, email, is_admin) VALUES (?, ?, ?, 0)",
        username,
        password_hash,
        email
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let user_id = result.last_insert_rowid();

    sqlx::query!(
        "UPDATE invites SET uses = uses + 1, updated_at = datetime('now') WHERE id = ?",
        invite.id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let session = extract::<Session>()
        .await
        .map_err(|_| ServerFnError::new("Authentication error"))?;

    let user_session = UserSession {
        id: user_id,
        username: username.clone(),
        is_admin: false,
    };

    set_user_in_session(&session, &user_session)
        .await
        .map_err(|_| ServerFnError::new("Authentication error"))?;

    Ok(user_session)
}

#[server(LoginUser)]
pub async fn login_user(username: String, password: String) -> Result<UserSession, ServerFnError> {
    use sqlx::SqlitePool;

    let username = username.trim().to_string();
    if username.is_empty() || password.is_empty() {
        return Err(ServerFnError::new("Username and password are required"));
    }

    let pool = expect_context::<SqlitePool>();

    let user = sqlx::query_as::<_, User>(
        "SELECT id, username, password_hash, email, is_admin FROM users WHERE username = ?",
    )
    .bind(&username)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let (valid, user_session) = match user {
        Some(user) => {
            let valid = verify_password(&password, &user.password_hash).unwrap_or(false);
            (
                valid,
                Some(UserSession {
                    id: user.id,
                    username: user.username,
                    is_admin: user.is_admin == 1,
                }),
            )
        }
        None => {
            let _ = verify_password(
                &password,
                "$2b$12$dummy.hash.to.prevent.timing.attacks.abcdefghijklmnopqr",
            );
            (false, None)
        }
    };

    if !valid {
        return Err(ServerFnError::new("Invalid username or password"));
    }

    let user_session = user_session.ok_or_else(|| ServerFnError::new("Authentication error"))?;

    let session = extract::<Session>()
        .await
        .map_err(|_| ServerFnError::new("Authentication error"))?;

    set_user_in_session(&session, &user_session)
        .await
        .map_err(|_| ServerFnError::new("Authentication error"))?;

    Ok(user_session)
}

#[server(LogoutUser)]
pub async fn logout_user() -> Result<(), ServerFnError> {
    let session = extract::<Session>()
        .await
        .map_err(|_| ServerFnError::new("Authentication error"))?;

    clear_session(&session)
        .await
        .map_err(|_| ServerFnError::new("Logout failed"))?;
    Ok(())
}

#[server(GetUser)]
pub async fn get_user() -> Result<Option<UserSession>, ServerFnError> {
    let session = extract::<Session>()
        .await
        .map_err(|_| ServerFnError::new("Authentication error"))?;

    Ok(get_user_from_session(&session).await)
}
