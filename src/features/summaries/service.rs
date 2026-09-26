#![cfg(feature = "ssr")]

use crate::{
    features::summaries::models::{Summary, SummaryListItem},
    services::{Actor, Page, PageRequest, ServiceError, decode_cursor, encode_cursor},
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct SummaryService {
    pool: SqlitePool,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SummaryCursor {
    project_id: i64,
    before_created_at: String,
    before_id: i64,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ContentCursor {
    summary_id: i64,
    revision: String,
    byte_offset: usize,
}

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct SummaryChunk {
    pub id: i64,
    pub project_id: i64,
    pub title: String,
    pub description: Option<String>,
    pub segment_label: Option<String>,
    pub status: String,
    pub content_markdown: String,
    pub next_cursor: Option<String>,
}

impl SummaryService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list_all(
        &self,
        actor: &Actor,
        project_id: i64,
    ) -> Result<Vec<SummaryListItem>, ServiceError> {
        self.query(actor, project_id, None, None).await
    }
    pub async fn list(
        &self,
        actor: &Actor,
        project_id: i64,
        page: PageRequest,
    ) -> Result<Page<SummaryListItem>, ServiceError> {
        let limit = page.limit()?;
        let cursor: Option<SummaryCursor> = decode_cursor(page.cursor.as_deref())?;
        if cursor.as_ref().is_some_and(|c| c.project_id != project_id) {
            return Err(ServiceError::InvalidInput(
                "cursor does not belong to this project",
            ));
        }
        let mut items = self
            .query(actor, project_id, cursor, Some(limit + 1))
            .await?;
        let more = items.len() > limit as usize;
        items.truncate(limit as usize);
        let next_cursor = if more {
            items
                .last()
                .map(|x| {
                    encode_cursor(&SummaryCursor {
                        project_id,
                        before_created_at: x.created_at.clone(),
                        before_id: x.id,
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
        project_id: i64,
        before: Option<SummaryCursor>,
        limit: Option<u32>,
    ) -> Result<Vec<SummaryListItem>, ServiceError> {
        let rows = sqlx::query_as::<_, SummaryListItem>(r#"SELECT s.id, s.project_id, s.title, s.description, s.segment_label, s.status, s.created_at
            FROM summaries s JOIN study_projects sp ON sp.id = s.project_id
            WHERE s.project_id = ? AND sp.user_id = ?
              AND (? IS NULL OR s.created_at < ? OR (s.created_at = ? AND s.id < ?))
            ORDER BY s.created_at DESC, s.id DESC LIMIT COALESCE(?, -1)"#)
            .bind(project_id).bind(actor.user_id())
            .bind(before.as_ref().map(|cursor| cursor.before_created_at.as_str()))
            .bind(before.as_ref().map(|cursor| cursor.before_created_at.as_str()))
            .bind(before.as_ref().map(|cursor| cursor.before_created_at.as_str()))
            .bind(before.as_ref().map(|cursor| cursor.before_id))
            .bind(limit).fetch_all(&self.pool).await?;
        if rows.is_empty()
            && sqlx::query_scalar::<_, i64>(
                "SELECT 1 FROM study_projects WHERE id = ? AND user_id = ?",
            )
            .bind(project_id)
            .bind(actor.user_id())
            .fetch_optional(&self.pool)
            .await?
            .is_none()
        {
            return Err(ServiceError::NotFound);
        }
        Ok(rows)
    }
    pub async fn get(&self, actor: &Actor, summary_id: i64) -> Result<Summary, ServiceError> {
        sqlx::query_as::<_, Summary>(r#"SELECT s.id, s.project_id, s.title, s.description, s.content_markdown,
            CASE WHEN owner_file.id IS NOT NULL THEN s.file_id ELSE NULL END AS file_id, s.segment_label, s.status, s.error_message, s.created_at, s.updated_at
            FROM summaries s JOIN study_projects sp ON sp.id = s.project_id
            LEFT JOIN project_files owner_file ON owner_file.id = s.file_id AND owner_file.project_id IN (SELECT id FROM study_projects WHERE user_id = ?)
            WHERE s.id = ? AND sp.user_id = ?"#).bind(actor.user_id()).bind(summary_id).bind(actor.user_id()).fetch_optional(&self.pool).await?.ok_or(ServiceError::NotFound)
    }
    pub async fn chunk(
        &self,
        actor: &Actor,
        summary_id: i64,
        cursor: Option<&str>,
        max_chars: u32,
    ) -> Result<SummaryChunk, ServiceError> {
        if max_chars == 0 || max_chars > 20_000 {
            return Err(ServiceError::InvalidInput(
                "max_chars must be between 1 and 20000",
            ));
        }
        let summary = self.get(actor, summary_id).await?;
        let parsed: Option<ContentCursor> = decode_cursor(cursor)?;
        if parsed
            .as_ref()
            .is_some_and(|c| c.summary_id != summary_id || c.revision != summary.updated_at)
        {
            return Err(ServiceError::Conflict(
                "content changed; restart from the beginning",
            ));
        }
        let offset = parsed.map_or(0, |c| c.byte_offset);
        if offset > summary.content_markdown.len()
            || !summary.content_markdown.is_char_boundary(offset)
        {
            return Err(ServiceError::InvalidInput("cursor"));
        }
        let end = summary.content_markdown[offset..]
            .char_indices()
            .nth(max_chars as usize)
            .map_or(summary.content_markdown.len(), |(i, _)| offset + i);
        let next_cursor = (end < summary.content_markdown.len())
            .then(|| {
                encode_cursor(&ContentCursor {
                    summary_id,
                    revision: summary.updated_at.clone(),
                    byte_offset: end,
                })
            })
            .transpose()?;
        Ok(SummaryChunk {
            id: summary.id,
            project_id: summary.project_id,
            title: summary.title,
            description: summary.description,
            segment_label: summary.segment_label,
            status: summary.status,
            content_markdown: summary.content_markdown[offset..end].to_owned(),
            next_cursor,
        })
    }
}
