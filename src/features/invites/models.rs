use serde::{Deserialize, Serialize};
#[cfg(feature = "ssr")]
use sqlx::FromRow;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ssr", derive(FromRow))]
pub struct Invite {
    pub id: i64,
    pub token: String,
    pub created_by: i64,
    pub is_reusable: i64,
    pub duration_days: i64,
    pub uses: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InviteListItem {
    pub token: String,
    pub is_reusable: bool,
    pub duration_days: i64,
    pub uses: i64,
    pub created_at: String,
}
