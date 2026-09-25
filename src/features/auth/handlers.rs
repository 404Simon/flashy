use leptos::prelude::*;
#[cfg(feature = "ssr")]
use leptos_axum::extract;
#[cfg(feature = "ssr")]
use tower_sessions::Session;

#[cfg(feature = "ssr")]
use super::models::User;
use super::models::UserSession;
#[cfg(feature = "ssr")]
use super::utils::{
    clear_session, get_user_from_session, hash_password_bounded, set_user_in_session,
    verify_password_bounded,
};
#[cfg(feature = "ssr")]
use crate::validation::{validate_email, validate_password, validate_username};

#[server(RegisterUser)]
pub async fn register_user(
    invite_token: String,
    username: String,
    password: String,
    email: Option<String>,
) -> Result<UserSession, ServerFnError> {
    use sqlx::SqlitePool;

    // Validate username
    let username = validate_username(&username)?;

    // Validate password
    validate_password(&password)?;

    // Validate email if provided
    let email = match email {
        Some(ref e) if !e.trim().is_empty() => Some(validate_email(e)?),
        _ => None,
    };

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

    let password_hash = hash_password_bounded(password.clone())
        .await
        .map_err(|_| ServerFnError::new("Could not secure password"))?;

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

    session
        .cycle_id()
        .await
        .map_err(|_| ServerFnError::new("Authentication error"))?;
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
            let password = password.clone();
            let hash = user.password_hash.clone();
            let valid = verify_password_bounded(password, hash)
                .await
                .map_err(|_| ServerFnError::new("Authentication error"))?;
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
            let password = password.clone();
            let _ = verify_password_bounded(
                password,
                "$2b$12$abcdefghijklmnopqrstuuP7K3QF6nNbs7Q9Y3pQ1Kc0f9QF1Ge".into(),
            )
            .await;
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

    session
        .cycle_id()
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

    let Some(stored) = get_user_from_session(&session).await else {
        return Ok(None);
    };
    use sqlx::SqlitePool;
    let pool = expect_context::<SqlitePool>();
    let current = sqlx::query_as::<_, (i64, String, i64)>(
        "SELECT id, username, is_admin FROM users WHERE id = ?",
    )
    .bind(stored.id)
    .fetch_optional(&pool)
    .await
    .map_err(|_| ServerFnError::new("Authentication error"))?;
    Ok(current.map(|u| UserSession {
        id: u.0,
        username: u.1,
        is_admin: u.2 == 1,
    }))
}

#[server(ChangePassword)]
pub async fn change_password(
    current_password: String,
    new_password: String,
) -> Result<(), ServerFnError> {
    use sqlx::SqlitePool;

    // Require authentication
    let session = extract::<Session>()
        .await
        .map_err(|_| ServerFnError::new("Authentication error"))?;

    let user_session = get_user_from_session(&session)
        .await
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;

    // Validate new password
    validate_password(&new_password)?;

    // Prevent setting the same password
    if current_password == new_password {
        return Err(ServerFnError::new(
            "New password must be different from current password",
        ));
    }

    let pool = expect_context::<SqlitePool>();

    // Fetch current user with password hash
    let user = sqlx::query_as::<_, User>(
        "SELECT id, username, password_hash, email, is_admin FROM users WHERE id = ?",
    )
    .bind(user_session.id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("User not found"))?;

    // Verify current password
    let valid = verify_password_bounded(current_password.clone(), user.password_hash)
        .await
        .map_err(|_| ServerFnError::new("Authentication error"))?;

    if !valid {
        return Err(ServerFnError::new("Current password is incorrect"));
    }

    // Hash new password
    let new_password_hash = hash_password_bounded(new_password)
        .await
        .map_err(|_| ServerFnError::new("Could not secure password"))?;

    super::service::AuthService::new(pool)
        .change_password_and_revoke_oauth(user_session.id, &new_password_hash)
        .await
        .map_err(|_| ServerFnError::new("Could not change password"))?;

    Ok(())
}

#[server(ListConnectedApplications)]
pub async fn list_connected_applications()
-> Result<Vec<super::models::ConnectedApplication>, ServerFnError> {
    let user = super::utils::require_auth().await?;
    let state = expect_context::<crate::app_state::AppState>();
    state
        .oauth
        .connected_apps(user.id)
        .await
        .map_err(|_| ServerFnError::new("Could not load connected applications"))
}

#[server(RevokeConnectedApplication)]
pub async fn revoke_connected_application(client_id: String) -> Result<(), ServerFnError> {
    let user = super::utils::require_auth().await?;
    if client_id.is_empty() || client_id.len() > 2048 {
        return Err(ServerFnError::new("Invalid client"));
    }
    let state = expect_context::<crate::app_state::AppState>();
    state
        .oauth
        .revoke_client(user.id, &client_id)
        .await
        .map_err(|_| ServerFnError::new("Could not disconnect application"))
}
