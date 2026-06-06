use async_trait::async_trait;
use sqlx::{SqliteConnection, sqlite::SqlitePool};
use time::OffsetDateTime;
use tower_sessions_core::{
    SessionStore,
    session::{Id, Record},
    session_store::{self, ExpiredDeletion},
};

use crate::session_store::SqlxStoreError;

#[derive(Clone, Debug)]
pub struct SqliteStore {
    pool: SqlitePool,
    table_name: String,
}

impl SqliteStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            table_name: "tower_sessions".into(),
        }
    }

    pub fn with_table_name(mut self, table_name: impl AsRef<str>) -> Result<Self, String> {
        let table_name = table_name.as_ref();
        if !is_valid_table_name(table_name) {
            return Err(format!(
                "Invalid table name '{}'. Table names must be alphanumeric and may contain \
                 hyphens or underscores.",
                table_name
            ));
        }

        table_name.clone_into(&mut self.table_name);
        Ok(self)
    }

    pub async fn migrate(&self) -> sqlx::Result<()> {
        let mut q = sqlx::QueryBuilder::new("create table if not exists ");
        q.push(&self.table_name);
        q.push(" (id text primary key not null, data blob not null, expiry_date integer not null)");
        q.build().execute(&self.pool).await?;
        Ok(())
    }

    async fn try_create_with_conn(
        &self,
        conn: &mut SqliteConnection,
        record: &Record,
    ) -> session_store::Result<bool> {
        let mut q = sqlx::QueryBuilder::new("insert or abort into ");
        q.push(&self.table_name);
        q.push(" (id, data, expiry_date) values (");
        q.push_bind(record.id.to_string());
        q.push(", ");
        q.push_bind(rmp_serde::to_vec(record).map_err(SqlxStoreError::Encode)?);
        q.push(", ");
        q.push_bind(record.expiry_date);
        q.push(")");
        let res = q.build().execute(conn).await;

        match res {
            Ok(_) => Ok(true),
            Err(sqlx::Error::Database(e)) if e.is_unique_violation() => Ok(false),
            Err(e) => Err(SqlxStoreError::Sqlx(e).into()),
        }
    }

    async fn save_with_conn(
        &self,
        conn: &mut SqliteConnection,
        record: &Record,
    ) -> session_store::Result<()> {
        let mut q = sqlx::QueryBuilder::new("insert into ");
        q.push(&self.table_name);
        q.push(" (id, data, expiry_date) values (");
        q.push_bind(record.id.to_string());
        q.push(", ");
        q.push_bind(rmp_serde::to_vec(record).map_err(SqlxStoreError::Encode)?);
        q.push(", ");
        q.push_bind(record.expiry_date);
        q.push(") on conflict(id) do update set data = excluded.data, expiry_date = excluded.expiry_date");
        q.build()
            .execute(conn)
            .await
            .map_err(SqlxStoreError::Sqlx)?;

        Ok(())
    }
}

#[async_trait]
impl ExpiredDeletion for SqliteStore {
    async fn delete_expired(&self) -> session_store::Result<()> {
        let mut q = sqlx::QueryBuilder::new("delete from ");
        q.push(&self.table_name);
        q.push(" where datetime(expiry_date) < datetime('now')");
        q.build()
            .execute(&self.pool)
            .await
            .map_err(SqlxStoreError::Sqlx)?;
        Ok(())
    }
}

#[async_trait]
impl SessionStore for SqliteStore {
    async fn create(&self, record: &mut Record) -> session_store::Result<()> {
        let mut tx = self.pool.begin().await.map_err(SqlxStoreError::Sqlx)?;

        while !self.try_create_with_conn(&mut tx, record).await? {
            record.id = Id::default();
        }

        tx.commit().await.map_err(SqlxStoreError::Sqlx)?;

        Ok(())
    }

    async fn save(&self, record: &Record) -> session_store::Result<()> {
        let mut conn = self.pool.acquire().await.map_err(SqlxStoreError::Sqlx)?;
        self.save_with_conn(&mut conn, record).await
    }

    async fn load(&self, session_id: &Id) -> session_store::Result<Option<Record>> {
        let mut q = sqlx::QueryBuilder::new("select data from ");
        q.push(&self.table_name);
        q.push(" where id = ");
        q.push_bind(session_id.to_string());
        q.push(" and expiry_date > ");
        q.push_bind(OffsetDateTime::now_utc());
        let data: Option<(Vec<u8>,)> = q
            .build_query_as()
            .fetch_optional(&self.pool)
            .await
            .map_err(SqlxStoreError::Sqlx)?;

        if let Some((data,)) = data {
            Ok(Some(
                rmp_serde::from_slice(&data).map_err(SqlxStoreError::Decode)?,
            ))
        } else {
            Ok(None)
        }
    }

    async fn delete(&self, session_id: &Id) -> session_store::Result<()> {
        let mut q = sqlx::QueryBuilder::new("delete from ");
        q.push(&self.table_name);
        q.push(" where id = ");
        q.push_bind(session_id.to_string());
        q.build()
            .execute(&self.pool)
            .await
            .map_err(SqlxStoreError::Sqlx)?;

        Ok(())
    }
}

fn is_valid_table_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}
