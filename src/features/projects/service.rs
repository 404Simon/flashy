#![cfg(feature = "ssr")]

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::{
    features::projects::models::{Project, ProjectSummary},
    services::{Actor, Page, PageRequest, ServiceError, decode_cursor, encode_cursor},
};

#[derive(Clone)]
pub struct ProjectService {
    pool: SqlitePool,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ProjectCursor {
    before_created_at: String,
    before_id: i64,
}

impl ProjectService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list_all(&self, actor: &Actor) -> Result<Vec<ProjectSummary>, ServiceError> {
        self.query(actor, None, None).await
    }

    pub async fn list(
        &self,
        actor: &Actor,
        page: PageRequest,
    ) -> Result<Page<ProjectSummary>, ServiceError> {
        let limit = page.limit()?;
        let cursor: Option<ProjectCursor> = decode_cursor(page.cursor.as_deref())?;
        let mut items = self.query(actor, cursor, Some(limit + 1)).await?;
        let has_more = items.len() > limit as usize;
        items.truncate(limit as usize);
        let next_cursor = if has_more {
            items
                .last()
                .map(|item| {
                    encode_cursor(&ProjectCursor {
                        before_created_at: item.created_at.clone(),
                        before_id: item.id,
                    })
                })
                .transpose()?
        } else {
            None
        };
        Ok(Page { items, next_cursor })
    }

    async fn query(
        &self,
        actor: &Actor,
        before: Option<ProjectCursor>,
        limit: Option<u32>,
    ) -> Result<Vec<ProjectSummary>, ServiceError> {
        let rows = sqlx::query_as::<_, ProjectSummary>(
            r#"SELECT sp.id, sp.name, sp.description, sp.created_at,
                CAST(COUNT(pf.id) AS INTEGER) AS file_count
               FROM study_projects sp
               LEFT JOIN project_files pf ON pf.project_id = sp.id
               WHERE sp.user_id = ?
                 AND (? IS NULL OR sp.created_at < ? OR (sp.created_at = ? AND sp.id < ?))
               GROUP BY sp.id
               ORDER BY sp.created_at DESC, sp.id DESC
               LIMIT COALESCE(?, -1)"#,
        )
        .bind(actor.user_id())
        .bind(
            before
                .as_ref()
                .map(|cursor| cursor.before_created_at.as_str()),
        )
        .bind(
            before
                .as_ref()
                .map(|cursor| cursor.before_created_at.as_str()),
        )
        .bind(
            before
                .as_ref()
                .map(|cursor| cursor.before_created_at.as_str()),
        )
        .bind(before.as_ref().map(|cursor| cursor.before_id))
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get(&self, actor: &Actor, id: i64) -> Result<Project, ServiceError> {
        sqlx::query_as::<_, Project>(
            "SELECT id, user_id, name, description, created_at, updated_at FROM study_projects WHERE id = ? AND user_id = ?",
        )
        .bind(id).bind(actor.user_id()).fetch_optional(&self.pool).await?.ok_or(ServiceError::NotFound)
    }
}
