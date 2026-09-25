use leptos::prelude::*;

use super::super::models::ProjectFile;
#[cfg(feature = "ssr")]
use crate::features::auth::utils::require_auth;

#[server(ListProjectFiles)]
pub async fn list_project_files(project_id: i64) -> Result<Vec<ProjectFile>, ServerFnError> {
    use crate::{
        features::projects::files_service::FileService,
        services::{Actor, to_server_error},
    };
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();

    let actor = Actor::new(user.id).map_err(to_server_error)?;
    FileService::new(pool)
        .list_all(&actor, project_id)
        .await
        .map_err(to_server_error)
}

#[server(GetProjectFileText)]
pub async fn get_project_file_text(file_id: i64) -> Result<String, ServerFnError> {
    use crate::{
        features::projects::files_service::FileService,
        services::{Actor, to_server_error},
    };
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();

    let actor = Actor::new(user.id).map_err(to_server_error)?;
    FileService::new(pool)
        .full_text(&actor, file_id)
        .await
        .map_err(to_server_error)
}

#[server(DeleteProjectFile)]
pub async fn delete_project_file(file_id: i64) -> Result<(), ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();

    // Get file info and verify ownership
    let file_info = sqlx::query!(
        r#"
        SELECT pf.s3_bucket as "s3_bucket!: String", pf.s3_key as "s3_key!: String"
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
    .ok_or_else(|| ServerFnError::new("File not found or access denied"))?;

    // Delete from S3
    let app_state = expect_context::<crate::app_state::AppState>();
    app_state
        .minio_client
        .delete_object()
        .bucket(&file_info.s3_bucket)
        .key(&file_info.s3_key)
        .send()
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to delete file from storage: {}", e)))?;

    // Delete from database (will cascade delete flashcards referencing this file)
    sqlx::query!("DELETE FROM project_files WHERE id = ?", file_id)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[server(GetFileName)]
pub async fn get_file_name(file_id: i64) -> Result<String, ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();

    let row = sqlx::query!(
        r#"
        SELECT pf.original_filename as "original_filename!: String"
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

    Ok(row.original_filename)
}
