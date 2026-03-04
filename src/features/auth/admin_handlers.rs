use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(feature = "ssr")]
use super::utils::require_auth;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserListItem {
    pub id: i64,
    pub username: String,
    pub email: Option<String>,
    pub is_admin: bool,
    pub created_at: String,
}

#[server(ListUsers)]
pub async fn list_users() -> Result<Vec<UserListItem>, ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    if !user.is_admin {
        return Err(ServerFnError::new("Unauthorized"));
    }

    let pool = expect_context::<SqlitePool>();
    let users = sqlx::query!(
        r#"
        SELECT id, username, email, is_admin, created_at
        FROM users
        ORDER BY created_at DESC
        "#
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(users
        .into_iter()
        .map(|u| UserListItem {
            id: u.id,
            username: u.username,
            email: u.email,
            is_admin: u.is_admin == 1,
            created_at: u.created_at,
        })
        .collect())
}

#[server(ToggleUserAdmin)]
pub async fn toggle_user_admin(user_id: i64) -> Result<bool, ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    if !user.is_admin {
        return Err(ServerFnError::new("Unauthorized"));
    }

    // Prevent admins from removing their own admin status
    if user.id == user_id {
        return Err(ServerFnError::new("Cannot modify your own admin status"));
    }

    let pool = expect_context::<SqlitePool>();

    // Get current admin status
    let current_user = sqlx::query!("SELECT is_admin FROM users WHERE id = ?", user_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| ServerFnError::new("User not found"))?;

    let new_admin_status = if current_user.is_admin == 1 { 0 } else { 1 };

    sqlx::query!(
        "UPDATE users SET is_admin = ?, updated_at = datetime('now') WHERE id = ?",
        new_admin_status,
        user_id
    )
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(new_admin_status == 1)
}

#[server(DeleteUser)]
pub async fn delete_user(user_id: i64) -> Result<(), ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    if !user.is_admin {
        return Err(ServerFnError::new("Unauthorized"));
    }

    // Prevent admins from deleting themselves
    if user.id == user_id {
        return Err(ServerFnError::new("Cannot delete your own account"));
    }

    let pool = expect_context::<SqlitePool>();

    // Check if user exists
    let target_user = sqlx::query!("SELECT id, is_admin FROM users WHERE id = ?", user_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| ServerFnError::new("User not found"))?;

    // Ensure at least one admin remains
    if target_user.is_admin == 1 {
        let admin_count =
            sqlx::query_scalar!("SELECT COUNT(*) as count FROM users WHERE is_admin = 1")
                .fetch_one(&pool)
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;

        if admin_count <= 1 {
            return Err(ServerFnError::new("Cannot delete the last admin user"));
        }
    }

    sqlx::query!("DELETE FROM users WHERE id = ?", user_id)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}
