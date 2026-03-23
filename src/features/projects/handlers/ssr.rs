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
        .get_object()
        .bucket(&row.s3_bucket)
        .key(&row.s3_key)
        .send()
        .await
        .map_err(|e| (axum::http::StatusCode::BAD_GATEWAY, e.to_string()))?;

    let content_type = object
        .content_type
        .clone()
        .unwrap_or_else(|| "application/pdf".to_string());
    let content_length = object.content_length();

    // Convert ByteStream to Body by collecting bytes
    let bytes = object
        .body
        .collect()
        .await
        .map_err(|e| (axum::http::StatusCode::BAD_GATEWAY, e.to_string()))?
        .into_bytes();
    let body = Body::from(bytes);

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
#[derive(serde::Deserialize)]
pub struct SegmentPdfQuery {
    ranges: String,
    name: Option<String>,
}

#[cfg(feature = "ssr")]
pub async fn get_project_segment_pdf(
    axum::extract::State(state): axum::extract::State<crate::app_state::AppState>,
    axum::extract::Path((project_id, file_id)): axum::extract::Path<(i64, i64)>,
    axum::extract::Query(query): axum::extract::Query<SegmentPdfQuery>,
    session: tower_sessions::Session,
) -> Result<axum::response::Response, (axum::http::StatusCode, String)> {
    use axum::body::Body;
    use axum::http::header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE};
    use axum::http::HeaderValue;
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
        SELECT pf.s3_key as "s3_key!: String",
               pf.s3_bucket as "s3_bucket!: String",
               pf.original_filename as "original_filename!: String"
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

    let ranges = parse_segment_ranges(&query.ranges)
        .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, e))?;

    let (temp_path, _pdf_size) = crate::features::projects::processing::download_pdf_to_temp(
        &state.minio_client,
        &row.s3_bucket,
        &row.s3_key,
    )
    .await
    .map_err(|e| (axum::http::StatusCode::BAD_GATEWAY, e))?;

    let pdf_bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, String> {
        let path: &std::path::Path = temp_path.as_ref();
        crate::features::projects::processing::build_segment_pdf_bytes(path, &ranges)
    })
    .await
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let filename = query
        .name
        .as_deref()
        .map(crate::features::projects::processing::sanitize_filename)
        .unwrap_or_else(|| build_segment_filename(&row.original_filename));
    let pdf_len = pdf_bytes.len();
    let body = Body::from(pdf_bytes);
    let mut response = body.into_response();
    let headers = response.headers_mut();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/pdf"));
    headers.insert(
        CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{}\"", filename)).map_err(|_| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Invalid content disposition".to_string(),
            )
        })?,
    );
    headers.insert(
        CONTENT_LENGTH,
        HeaderValue::from_str(&pdf_len.to_string()).map_err(|_| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Invalid content length".to_string(),
            )
        })?,
    );

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

    use sqlx::SqlitePool;

    use crate::config::Config;
    use crate::features::auth::utils::get_user_from_session;

    let config = Config::global();
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

    let mut uploaded_count = 0;

    // Process all uploaded files
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, e.to_string()))?
    {
        if field.name() != Some("file") {
            continue;
        }

        let file_name = field
            .file_name()
            .map(|name| name.to_string())
            .unwrap_or_else(|| "slides.pdf".to_string());

        let content_type = field
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_string();

        // Validate file type early
        let file_name_lower = file_name.to_lowercase();
        let is_pdf = file_name_lower.ends_with(".pdf")
            || matches!(
                content_type.as_str(),
                "application/pdf" | "application/x-pdf"
            );

        if !is_pdf {
            continue; // Skip non-PDF files
        }

        let temp_file = tempfile::NamedTempFile::with_suffix(".pdf")
            .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let (temp_std_file, temp_path_handle) = temp_file.into_parts();
        let mut temp_file = tokio::fs::File::from_std(temp_std_file);
        let mut file_size: u64 = 0;

        while let Some(chunk) = field
            .chunk()
            .await
            .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, e.to_string()))?
        {
            file_size = file_size.saturating_add(chunk.len() as u64);
            if file_size > config.max_pdf_bytes {
                continue; // Skip files that are too large
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

        if file_size == 0 || file_size > config.max_pdf_bytes {
            continue; // Skip empty or oversized files
        }

        // Upload to S3 first
        let sanitized_name = crate::features::projects::processing::sanitize_filename(&file_name);
        let key = format!(
            "{}/{}/{}-{}",
            state.object_key_prefix,
            project_id,
            uuid::Uuid::new_v4(),
            sanitized_name
        );

        let temp_path_buf =
            <tempfile::TempPath as AsRef<std::path::Path>>::as_ref(&temp_path_handle).to_path_buf();

        let upload_file = tokio::fs::File::open(&temp_path_buf)
            .await
            .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, e.to_string()))?;

        let body = aws_sdk_s3::primitives::ByteStream::read_from()
            .file(upload_file)
            .build()
            .await
            .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, e.to_string()))?;

        state
            .minio_client
            .put_object()
            .bucket(state.bucket_name.as_str())
            .key(&key)
            .content_type("application/pdf")
            .content_length(file_size as i64)
            .body(body)
            .send()
            .await
            .map_err(|e| (axum::http::StatusCode::BAD_GATEWAY, e.to_string()))?;

        // Insert into database with pending status
        let file_size_i64 = file_size as i64;
        let file_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO project_files
                (project_id, original_filename, content_type, file_size, s3_bucket, s3_key, processing_status)
            VALUES
                (?, ?, ?, ?, ?, ?, 'pending')
            RETURNING id
            "#
        )
        .bind(project_id)
        .bind(&file_name)
        .bind(&content_type)
        .bind(file_size_i64)
        .bind(&state.bucket_name)
        .bind(&key)
        .fetch_one(pool)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        uploaded_count += 1;

        // Keep the temp file from being deleted
        let temp_path_buf = temp_path_handle
            .keep()
            .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        // Spawn background task for text extraction
        let pool_clone = state.db_pool.clone();
        tokio::spawn(async move {
            if let Err(e) = crate::features::projects::processing::process_file_async(
                file_id,
                temp_path_buf,
                pool_clone,
            )
            .await
            {
                eprintln!("Error processing file {}: {}", file_id, e);
            }
        });
    }

    if uploaded_count == 0 {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            "No valid PDF files uploaded".to_string(),
        ));
    }

    Ok(axum::response::Redirect::to(&format!(
        "/projects/{project_id}?uploaded={uploaded_count}"
    )))
}

#[cfg(feature = "ssr")]
fn build_segment_filename(original: &str) -> String {
    let cleaned = crate::features::projects::processing::sanitize_filename(original);
    match cleaned.strip_suffix(".pdf") {
        Some(stem) => format!("{stem}-segment.pdf"),
        None => format!("{cleaned}-segment.pdf"),
    }
}

#[cfg(feature = "ssr")]
fn parse_segment_ranges(
    raw: &str,
) -> Result<Vec<crate::features::projects::models::SegmentRange>, String> {
    use crate::features::projects::models::SegmentRange;

    let mut ranges = Vec::new();
    for part in raw.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((start, end)) = part.split_once('-') {
            let start_page = start
                .trim()
                .parse::<i64>()
                .map_err(|_| format!("Invalid range start: {start}"))?;
            let end_page = end
                .trim()
                .parse::<i64>()
                .map_err(|_| format!("Invalid range end: {end}"))?;
            ranges.push(SegmentRange {
                start_page,
                end_page,
            });
        } else {
            let page = part
                .parse::<i64>()
                .map_err(|_| format!("Invalid page: {part}"))?;
            ranges.push(SegmentRange {
                start_page: page,
                end_page: page,
            });
        }
    }

    if ranges.is_empty() {
        return Err("No ranges provided".to_string());
    }

    Ok(ranges)
}
