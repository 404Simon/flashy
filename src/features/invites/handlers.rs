use super::models::InviteListItem;
#[cfg(feature = "ssr")]
use crate::features::auth::utils::require_auth;
use leptos::prelude::*;

#[server(CreateInvite)]
pub async fn create_invite(
    is_reusable: Option<bool>,
    duration_days: i64,
) -> Result<String, ServerFnError> {
    use sqlx::SqlitePool;
    use uuid::Uuid;

    if !(1..=30).contains(&duration_days) {
        return Err(ServerFnError::new("Duration must be between 1 and 30 days"));
    }

    let user = require_auth().await?;
    if !user.is_admin {
        return Err(ServerFnError::new("Unauthorized"));
    }

    let pool = expect_context::<SqlitePool>();

    let token = Uuid::new_v4().to_string();
    let is_reusable_int = i64::from(is_reusable.unwrap_or(false));

    sqlx::query!(
        "INSERT INTO invites (token, created_by, is_reusable, duration_days) VALUES (?, ?, ?, ?)",
        token,
        user.id,
        is_reusable_int,
        duration_days
    )
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(token)
}

#[server(ListInvites)]
pub async fn list_invites() -> Result<Vec<InviteListItem>, ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    if !user.is_admin {
        return Err(ServerFnError::new("Unauthorized"));
    }

    let pool = expect_context::<SqlitePool>();
    let invites = sqlx::query!(
        r#"
        SELECT token, is_reusable, duration_days, uses, created_at
        FROM invites
        ORDER BY created_at DESC
        "#
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(invites
        .into_iter()
        .map(|inv| InviteListItem {
            token: inv.token,
            is_reusable: inv.is_reusable == 1,
            duration_days: inv.duration_days,
            uses: inv.uses,
            created_at: inv.created_at,
        })
        .collect())
}
