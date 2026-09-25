#![cfg(feature = "ssr")]

use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AuthService {
    pool: SqlitePool,
}

impl AuthService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn change_password_and_revoke_oauth(
        &self,
        user_id: i64,
        password_hash: &str,
    ) -> Result<(), sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        let updated = sqlx::query(
            "UPDATE users SET password_hash = ?, updated_at = datetime('now') WHERE id = ?",
        )
        .bind(password_hash)
        .bind(user_id)
        .execute(&mut *transaction)
        .await?;
        if updated.rows_affected() != 1 {
            return Err(sqlx::Error::RowNotFound);
        }
        sqlx::query(
            "UPDATE oauth_grants SET revoked_at = COALESCE(revoked_at, unixepoch()) WHERE user_id = ?",
        )
        .bind(user_id)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await
    }
}
