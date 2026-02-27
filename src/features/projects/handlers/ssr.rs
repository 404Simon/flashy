#[cfg(feature = "ssr")]
pub async fn get_project_pdf(
    axum::extract::State(state): axum::extract::State<crate::app_state::AppState>,
    axum::extract::Path((project_id, file_id)): axum::extract::Path<(i64, i64)>,
    session: tower_sessions::Session,
) -> Result<axum::response::Response, (axum::http::StatusCode, String)> {
    use axum::response::IntoResponse;
    use sqlx::SqlitePool;

    use crate::features::auth::utils::get_user_from_session;

    let user = get_user_from_session(&session).await.ok_or((
        axum::http::StatusCode::UNAUTHORIZED,
        "Not authenticated".to_string(),
    ))?;

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

    let user = get_user_from_session(&session).await.ok_or((
        axum::http::StatusCode::UNAUTHORIZED,
        "Not authenticated".to_string(),
    ))?;

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
