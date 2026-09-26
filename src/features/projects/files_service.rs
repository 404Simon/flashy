#![cfg(feature = "ssr")]

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};

use crate::{
    features::projects::models::ProjectFile,
    services::{Actor, Page, PageRequest, ServiceError, decode_cursor, encode_cursor},
};

#[derive(Clone)]
pub struct FileService {
    pool: SqlitePool,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FileCursor {
    project_id: i64,
    before_created_at: String,
    before_id: i64,
}

#[derive(FromRow)]
struct FileRow {
    id: i64,
    project_id: i64,
    original_filename: String,
    file_size: i64,
    processing_status: String,
    created_at: String,
    text_preview: Option<String>,
    word_count: Option<i64>,
    extracted_text: Option<String>,
}

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct FileTextChunk {
    pub file_id: i64,
    pub status: String,
    pub text: String,
    pub next_cursor: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TextCursor {
    file_id: i64,
    revision: String,
    byte_offset: usize,
}

impl FileService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list_all(
        &self,
        actor: &Actor,
        project_id: i64,
    ) -> Result<Vec<ProjectFile>, ServiceError> {
        self.query(actor, project_id, None, None, true).await
    }

    pub async fn list(
        &self,
        actor: &Actor,
        project_id: i64,
        page: PageRequest,
    ) -> Result<Page<ProjectFile>, ServiceError> {
        let limit = page.limit()?;
        let cursor: Option<FileCursor> = decode_cursor(page.cursor.as_deref())?;
        if cursor.as_ref().is_some_and(|c| c.project_id != project_id) {
            return Err(ServiceError::InvalidInput(
                "cursor does not belong to this project",
            ));
        }
        let mut items = self
            .query(actor, project_id, cursor, Some(limit + 1), false)
            .await?;
        let has_more = items.len() > limit as usize;
        items.truncate(limit as usize);
        let next_cursor = if has_more {
            items
                .last()
                .map(|x| {
                    encode_cursor(&FileCursor {
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

    pub async fn full_text(&self, actor: &Actor, file_id: i64) -> Result<String, ServiceError> {
        sqlx::query_scalar::<_, String>(
            r#"SELECT COALESCE(pf.extracted_text, '')
            FROM project_files pf JOIN study_projects sp ON sp.id = pf.project_id
            WHERE pf.id = ? AND sp.user_id = ?"#,
        )
        .bind(file_id)
        .bind(actor.user_id())
        .fetch_optional(&self.pool)
        .await?
        .ok_or(ServiceError::NotFound)
    }

    async fn query(
        &self,
        actor: &Actor,
        project_id: i64,
        before: Option<FileCursor>,
        limit: Option<u32>,
        include_word_count: bool,
    ) -> Result<Vec<ProjectFile>, ServiceError> {
        let rows = if include_word_count {
            sqlx::query_as::<_, FileRow>(r#"SELECT pf.id, pf.project_id, pf.original_filename, pf.file_size, pf.processing_status, pf.created_at,
                NULLIF(substr(pf.extracted_text, 1, 400), '') AS text_preview,
                NULL AS word_count, pf.extracted_text
                FROM project_files pf JOIN study_projects sp ON sp.id = pf.project_id
                WHERE pf.project_id = ? AND sp.user_id = ?
                  AND (? IS NULL OR pf.created_at < ? OR (pf.created_at = ? AND pf.id < ?))
                ORDER BY pf.created_at DESC, pf.id DESC LIMIT COALESCE(?, -1)"#)
                .bind(project_id).bind(actor.user_id())
                .bind(before.as_ref().map(|cursor| cursor.before_created_at.as_str()))
                .bind(before.as_ref().map(|cursor| cursor.before_created_at.as_str()))
                .bind(before.as_ref().map(|cursor| cursor.before_created_at.as_str()))
                .bind(before.as_ref().map(|cursor| cursor.before_id))
                .bind(limit).fetch_all(&self.pool).await?
        } else {
            sqlx::query_as::<_, FileRow>(r#"SELECT pf.id, pf.project_id, pf.original_filename, pf.file_size, pf.processing_status, pf.created_at,
                NULLIF(substr(pf.extracted_text, 1, 400), '') AS text_preview, NULL AS word_count, NULL AS extracted_text
                FROM project_files pf JOIN study_projects sp ON sp.id = pf.project_id
                WHERE pf.project_id = ? AND sp.user_id = ?
                  AND (? IS NULL OR pf.created_at < ? OR (pf.created_at = ? AND pf.id < ?))
                ORDER BY pf.created_at DESC, pf.id DESC LIMIT COALESCE(?, -1)"#)
                .bind(project_id).bind(actor.user_id())
                .bind(before.as_ref().map(|cursor| cursor.before_created_at.as_str()))
                .bind(before.as_ref().map(|cursor| cursor.before_created_at.as_str()))
                .bind(before.as_ref().map(|cursor| cursor.before_created_at.as_str()))
                .bind(before.as_ref().map(|cursor| cursor.before_id))
                .bind(limit).fetch_all(&self.pool).await?
        };
        if rows.is_empty() {
            let exists = sqlx::query_scalar::<_, i64>(
                "SELECT 1 FROM study_projects WHERE id = ? AND user_id = ?",
            )
            .bind(project_id)
            .bind(actor.user_id())
            .fetch_optional(&self.pool)
            .await?;
            if exists.is_none() {
                return Err(ServiceError::NotFound);
            }
        }
        Ok(rows
            .into_iter()
            .map(|r| ProjectFile {
                id: r.id,
                project_id: r.project_id,
                original_filename: r.original_filename,
                file_size: r.file_size,
                processing_status: r.processing_status,
                created_at: r.created_at,
                text_preview: r.text_preview,
                word_count: if include_word_count {
                    r.extracted_text
                        .as_ref()
                        .map(|text| text.split_whitespace().count() as i64)
                } else {
                    r.word_count
                },
            })
            .collect())
    }

    pub async fn text(
        &self,
        actor: &Actor,
        file_id: i64,
        cursor: Option<&str>,
        max_chars: u32,
    ) -> Result<FileTextChunk, ServiceError> {
        if max_chars == 0 || max_chars > 20_000 {
            return Err(ServiceError::InvalidInput(
                "max_chars must be between 1 and 20000",
            ));
        }
        let row = sqlx::query_as::<_, (String, String, String)>(r#"SELECT pf.processing_status, COALESCE(pf.extracted_text, ''), pf.updated_at
            FROM project_files pf JOIN study_projects sp ON sp.id = pf.project_id WHERE pf.id = ? AND sp.user_id = ?"#)
            .bind(file_id).bind(actor.user_id()).fetch_optional(&self.pool).await?.ok_or(ServiceError::NotFound)?;
        let parsed: Option<TextCursor> = decode_cursor(cursor)?;
        if parsed
            .as_ref()
            .is_some_and(|c| c.file_id != file_id || c.revision != row.2)
        {
            return Err(ServiceError::Conflict(
                "content changed; restart from the beginning",
            ));
        }
        let offset = parsed.map_or(0, |c| c.byte_offset);
        if offset > row.1.len() || !row.1.is_char_boundary(offset) {
            return Err(ServiceError::InvalidInput("cursor"));
        }
        let end = row.1[offset..]
            .char_indices()
            .nth(max_chars as usize)
            .map_or(row.1.len(), |(i, _)| offset + i);
        let next_cursor = (end < row.1.len())
            .then(|| {
                encode_cursor(&TextCursor {
                    file_id,
                    revision: row.2.clone(),
                    byte_offset: end,
                })
            })
            .transpose()?;
        Ok(FileTextChunk {
            file_id,
            status: row.0,
            text: row.1[offset..end].to_owned(),
            next_cursor,
        })
    }
}
