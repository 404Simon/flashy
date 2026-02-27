use leptos::prelude::*;

use super::super::models::ProjectFile;
#[cfg(feature = "ssr")]
use crate::features::auth::utils::require_auth;

#[server(ListProjectFiles)]
pub async fn list_project_files(project_id: i64) -> Result<Vec<ProjectFile>, ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();

    let allowed = sqlx::query_scalar!(
        "SELECT id FROM study_projects WHERE id = ? AND user_id = ?",
        project_id,
        user.id
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    if allowed.is_none() {
        return Err(ServerFnError::new("Project not found"));
    }

    let rows = sqlx::query!(
        r#"
        SELECT
            id as "id!: i64",
            project_id as "project_id!: i64",
            original_filename as "original_filename!: String",
            file_size as "file_size!: i64",
            created_at as "created_at!: String",
            CAST(COALESCE(substr(extracted_text, 1, 400), '') AS TEXT) as "text_preview!: String"
        FROM project_files
        WHERE project_id = ?
        ORDER BY created_at DESC
        "#,
        project_id
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|row| ProjectFile {
            id: row.id,
            project_id: row.project_id,
            original_filename: row.original_filename,
            file_size: row.file_size,
            created_at: row.created_at,
            text_preview: if row.text_preview.trim().is_empty() {
                None
            } else {
                Some(row.text_preview)
            },
        })
        .collect())
}

#[server(GetProjectFileText)]
pub async fn get_project_file_text(file_id: i64) -> Result<String, ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();

    let row = sqlx::query!(
        r#"
        SELECT CAST(COALESCE(pf.extracted_text, '') AS TEXT) as "extracted_text!: String"
        FROM project_files pf
        INNER JOIN study_projects sp ON sp.id = pf.project_id
        WHERE pf.id = ? AND sp.user_id = ?
        "#,
        file_id,
        user.id
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("File not found"))?;

    Ok(row.extracted_text)
}
