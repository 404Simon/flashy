use leptos::prelude::*;

use super::models::{Project, ProjectFile, ProjectSummary};
#[cfg(feature = "ssr")]
use crate::features::auth::utils::require_auth;

#[server(CreateProject)]
pub async fn create_project(
    name: String,
    description: Option<String>,
) -> Result<Project, ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let name = name.trim().to_string();

    if name.len() < 3 {
        return Err(ServerFnError::new(
            "Project name must be at least 3 characters",
        ));
    }

    let description = description
        .map(|d| d.trim().to_string())
        .filter(|d| !d.is_empty());

    let pool = expect_context::<SqlitePool>();

    let result = sqlx::query!(
        "INSERT INTO study_projects (user_id, name, description) VALUES (?, ?, ?)",
        user.id,
        name,
        description
    )
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let project_id = result.last_insert_rowid();

    let project = sqlx::query_as!(
        Project,
        r#"
        SELECT id, user_id, name, description, created_at, updated_at
        FROM study_projects
        WHERE id = ?
        "#,
        project_id
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(project)
}

#[server(ListProjects)]
pub async fn list_projects() -> Result<Vec<ProjectSummary>, ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();

    let rows = sqlx::query!(
        r#"
        SELECT
            sp.id as "id!: i64",
            sp.name as "name!: String",
            CAST(COALESCE(sp.description, '') AS TEXT) as "description!: String",
            sp.created_at as "created_at!: String",
            CAST(COALESCE((
                SELECT COUNT(*)
                FROM project_files pf
                WHERE pf.project_id = sp.id
            ), 0) AS INTEGER) as "file_count!: i64"
        FROM study_projects sp
        WHERE sp.user_id = ?
        ORDER BY sp.created_at DESC
        "#,
        user.id
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|row| ProjectSummary {
            id: row.id,
            name: row.name,
            description: if row.description.trim().is_empty() {
                None
            } else {
                Some(row.description)
            },
            created_at: row.created_at,
            file_count: row.file_count,
        })
        .collect())
}

#[server(GetProject)]
pub async fn get_project(project_id: i64) -> Result<Project, ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();

    let project = sqlx::query_as!(
        Project,
        r#"
        SELECT id, user_id, name, description, created_at, updated_at
        FROM study_projects
        WHERE id = ? AND user_id = ?
        "#,
        project_id,
        user.id
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("Project not found"))?;

    Ok(project)
}

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

#[cfg(feature = "ssr")]
pub async fn get_project_pdf(
    axum::extract::State(state): axum::extract::State<crate::app_state::AppState>,
    axum::extract::Path((project_id, file_id)): axum::extract::Path<(i64, i64)>,
    session: tower_sessions::Session,
) -> Result<axum::response::Response, (axum::http::StatusCode, String)> {
    use axum::response::IntoResponse;
    use sqlx::SqlitePool;

    use crate::features::auth::utils::get_user_from_session;

    let user = get_user_from_session(&session)
        .await
        .ok_or((axum::http::StatusCode::UNAUTHORIZED, "Not authenticated".to_string()))?;

    let pool: &SqlitePool = &state.db_pool;
    let row = sqlx::query!(
        r#"
        SELECT pf.s3_key as "s3_key!: String", pf.s3_bucket as "s3_bucket!: String"
        FROM project_files pf
        INNER JOIN study_projects sp ON sp.id = pf.project_id
        WHERE pf.id = ? AND pf.project_id = ? AND sp.user_id = ?
        "#,
        file_id,
        project_id,
        user.id
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((
        axum::http::StatusCode::NOT_FOUND,
        "File not found".to_string(),
    ))?;

    let object = state
        .minio_client
        .objects()
        .get(&row.s3_bucket, &row.s3_key)
        .send()
        .await
        .map_err(|e| (axum::http::StatusCode::BAD_GATEWAY, e.to_string()))?;

    let bytes = object
        .bytes()
        .await
        .map_err(|e| (axum::http::StatusCode::BAD_GATEWAY, e.to_string()))?;

    Ok((
        [(axum::http::header::CONTENT_TYPE, "application/pdf")],
        bytes,
    )
        .into_response())
}

#[cfg(feature = "ssr")]
pub async fn upload_project_file(
    axum::extract::State(state): axum::extract::State<crate::app_state::AppState>,
    axum::extract::Path(project_id): axum::extract::Path<i64>,
    session: tower_sessions::Session,
    mut multipart: axum::extract::Multipart,
) -> Result<axum::response::Redirect, (axum::http::StatusCode, String)> {
    use sqlx::SqlitePool;

    use crate::features::auth::utils::get_user_from_session;
    use crate::features::projects::processing::extract_text_with_pdftotext;

    let user = get_user_from_session(&session)
        .await
        .ok_or((axum::http::StatusCode::UNAUTHORIZED, "Not authenticated".to_string()))?;

    let pool: &SqlitePool = &state.db_pool;
    let project_exists = sqlx::query_scalar!(
        "SELECT id FROM study_projects WHERE id = ? AND user_id = ?",
        project_id,
        user.id
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if project_exists.is_none() {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            "Project not found".to_string(),
        ));
    }

    let mut file_name = None;
    let mut content_type = None;
    let mut file_bytes = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, e.to_string()))?
    {
        if field.name() != Some("file") {
            continue;
        }

        file_name = Some(
            field
                .file_name()
                .map(|name| name.to_string())
                .unwrap_or_else(|| "slides.pdf".to_string()),
        );
        content_type = Some(
            field
                .content_type()
                .unwrap_or("application/octet-stream")
                .to_string(),
        );
        file_bytes = Some(
            field
                .bytes()
                .await
                .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, e.to_string()))?,
        );
        break;
    }

    let file_name = file_name.ok_or((
        axum::http::StatusCode::BAD_REQUEST,
        "Missing file upload".to_string(),
    ))?;
    let content_type = content_type.unwrap_or_else(|| "application/octet-stream".to_string());
    let bytes = file_bytes.ok_or((
        axum::http::StatusCode::BAD_REQUEST,
        "Missing file upload".to_string(),
    ))?;

    let file_name_lower = file_name.to_lowercase();

    let is_pdf = file_name_lower.ends_with(".pdf")
        || matches!(
            content_type.as_str(),
            "application/pdf" | "application/x-pdf"
        );

    if !is_pdf {
        return Err((
            axum::http::StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "Only PDF uploads are supported".to_string(),
        ));
    }

    if bytes.is_empty() {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            "Uploaded file was empty".to_string(),
        ));
    }

    let file_size = bytes.len() as i64;
    let extracted_text = extract_text_with_pdftotext(bytes.to_vec())
        .await
        .map_err(|e| (axum::http::StatusCode::UNPROCESSABLE_ENTITY, e))?;

    let sanitized_name = sanitize_filename(&file_name);
    let key = format!(
        "{}/{}/{}-{}",
        state.object_key_prefix,
        project_id,
        uuid::Uuid::new_v4(),
        sanitized_name
    );

    state
        .minio_client
        .objects()
        .put(state.bucket_name.as_str(), &key)
        .content_type("application/pdf")
        .body_bytes(bytes)
        .send()
        .await
        .map_err(|e| (axum::http::StatusCode::BAD_GATEWAY, e.to_string()))?;

    sqlx::query!(
        r#"
        INSERT INTO project_files
            (project_id, original_filename, content_type, file_size, s3_bucket, s3_key, extracted_text)
        VALUES
            (?, ?, ?, ?, ?, ?, ?)
        "#,
        project_id,
        file_name,
        content_type,
        file_size,
        state.bucket_name,
        key,
        extracted_text
    )
    .execute(pool)
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(axum::response::Redirect::to(&format!(
        "/projects/{project_id}?uploaded=1"
    )))
}

#[cfg(feature = "ssr")]
fn sanitize_filename(filename: &str) -> String {
    let mut cleaned: String = filename
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        .collect();

    if cleaned.is_empty() {
        cleaned = "slides.pdf".to_string();
    }

    if !cleaned.to_lowercase().ends_with(".pdf") {
        cleaned.push_str(".pdf");
    }

    cleaned
}
