#[cfg(feature = "ssr")]
pub async fn get_project_pdf(
    axum::extract::State(state): axum::extract::State<crate::app_state::AppState>,
    axum::extract::Path((project_id, file_id)): axum::extract::Path<(i64, i64)>,
    session: tower_sessions::Session,
) -> Result<axum::response::Response, (axum::http::StatusCode, String)> {
    use axum::body::Body;
    use axum::http::header::{CONTENT_LENGTH, CONTENT_TYPE};
    use axum::http::HeaderValue;
    use axum::response::IntoResponse;
    use futures_util::TryStreamExt;
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

    let content_type = object
        .content_type
        .clone()
        .unwrap_or_else(|| "application/pdf".to_string());
    let content_length = object.content_length;
    let body = Body::from_stream(
        object
            .body
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)),
    );

    let mut response = body.into_response();
    let headers = response.headers_mut();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_str(&content_type).map_err(|_| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Invalid content type".to_string(),
            )
        })?,
    );
    if let Some(length) = content_length {
        headers.insert(
            CONTENT_LENGTH,
            HeaderValue::from_str(&length.to_string()).map_err(|_| {
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    "Invalid content length".to_string(),
                )
            })?,
        );
    }

    Ok(response)
}

#[cfg(feature = "ssr")]
pub async fn upload_project_file(
    axum::extract::State(state): axum::extract::State<crate::app_state::AppState>,
    axum::extract::Path(project_id): axum::extract::Path<i64>,
    session: tower_sessions::Session,
    mut multipart: axum::extract::Multipart,
) -> Result<axum::response::Redirect, (axum::http::StatusCode, String)> {
    use tokio::io::AsyncWriteExt;
    use tokio_util::io::ReaderStream;

    use sqlx::SqlitePool;

    use crate::features::auth::utils::get_user_from_session;
    use crate::features::projects::processing::{extract_text_with_pdftotext, MAX_PDF_BYTES};

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
    let mut temp_path: Option<tempfile::TempPath> = None;
    let mut file_size: u64 = 0;

    while let Some(mut field) = multipart
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

        let temp_file = tempfile::NamedTempFile::with_suffix(".pdf")
            .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let (temp_std_file, temp_path_handle) = temp_file.into_parts();
        let mut temp_file = tokio::fs::File::from_std(temp_std_file);

        while let Some(chunk) = field
            .chunk()
            .await
            .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, e.to_string()))?
        {
            file_size = file_size.saturating_add(chunk.len() as u64);
            if file_size > MAX_PDF_BYTES {
                return Err((
                    axum::http::StatusCode::PAYLOAD_TOO_LARGE,
                    format!(
                        "Uploaded file exceeded the {} MB limit",
                        MAX_PDF_BYTES / (1024 * 1024)
                    ),
                ));
            }
            temp_file
                .write_all(&chunk)
                .await
                .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, e.to_string()))?;
        }
        temp_file
            .flush()
            .await
            .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, e.to_string()))?;
        temp_path = Some(temp_path_handle);
        break;
    }

    let file_name = file_name.ok_or((
        axum::http::StatusCode::BAD_REQUEST,
        "Missing file upload".to_string(),
    ))?;
    let content_type = content_type.unwrap_or_else(|| "application/octet-stream".to_string());
    let temp_path = temp_path.ok_or((
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

    if file_size == 0 {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            "Uploaded file was empty".to_string(),
        ));
    }

    let temp_path_buf =
        <tempfile::TempPath as AsRef<std::path::Path>>::as_ref(&temp_path).to_path_buf();
    let extracted_text = extract_text_with_pdftotext(&temp_path_buf, file_size)
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

    let upload_file = tokio::fs::File::open(&temp_path_buf)
        .await
        .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, e.to_string()))?;
    let upload_stream = ReaderStream::new(upload_file);

    state
        .minio_client
        .objects()
        .put(state.bucket_name.as_str(), &key)
        .content_type("application/pdf")
        .body_stream_sized(upload_stream, file_size)
        .send()
        .await
        .map_err(|e| (axum::http::StatusCode::BAD_GATEWAY, e.to_string()))?;

    let file_size_i64 = file_size as i64;
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
        file_size_i64,
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
