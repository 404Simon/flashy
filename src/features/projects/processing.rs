#[cfg(feature = "ssr")]
use std::{path::Path, process::Stdio, time::Duration};

#[cfg(feature = "ssr")]
use futures_util::TryStreamExt;

#[cfg(feature = "ssr")]
use crate::features::projects::models::SegmentRange;

#[cfg(feature = "ssr")]
const PDFTOTEXT_TIMEOUT: Duration = Duration::from_secs(20);

#[cfg(feature = "ssr")]
pub async fn extract_text_with_pdftotext(pdf_path: &Path, pdf_size: u64) -> Result<String, String> {
    use crate::config::Config;

    let config = Config::global();
    if pdf_size > config.max_pdf_bytes {
        return Err(format!(
            "Uploaded PDF exceeded the size limit of {} MB",
            config.max_pdf_bytes / 1024 / 1024
        ));
    }

    let mut command = tokio::process::Command::new("pdftotext");
    command
        .arg("-layout")
        .arg("-q")
        .arg(pdf_path)
        .arg("-")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let child = command
        .spawn()
        .map_err(|e| format!("Failed to run pdftotext: {e}"))?;

    let output = tokio::time::timeout(PDFTOTEXT_TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| "pdftotext timed out".to_string())?
        .map_err(|e| format!("Failed to run pdftotext: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("pdftotext failed: {stderr}"));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[cfg(feature = "ssr")]
pub async fn extract_text_for_ranges_with_pdftotext(
    pdf_path: &Path,
    pdf_size: u64,
    ranges: &[SegmentRange],
) -> Result<String, String> {
    use crate::config::Config;

    let config = Config::global();
    if pdf_size > config.max_pdf_bytes {
        return Err(format!(
            "Uploaded PDF exceeded the size limit of {} MB",
            config.max_pdf_bytes / 1024 / 1024
        ));
    }

    let merged = merge_ranges(ranges);
    if merged.is_empty() {
        return Ok(String::new());
    }

    let mut combined = String::new();
    for range in merged {
        let chunk = extract_text_for_range(pdf_path, range.start_page, range.end_page).await?;
        if !combined.is_empty() {
            combined.push('\n');
        }
        combined.push_str(&chunk);
    }

    Ok(combined)
}

#[cfg(feature = "ssr")]
async fn extract_text_for_range(
    pdf_path: &Path,
    start_page: i64,
    end_page: i64,
) -> Result<String, String> {
    let mut command = tokio::process::Command::new("pdftotext");
    command
        .arg("-layout")
        .arg("-q")
        .arg("-f")
        .arg(start_page.to_string())
        .arg("-l")
        .arg(end_page.to_string())
        .arg(pdf_path)
        .arg("-")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let child = command
        .spawn()
        .map_err(|e| format!("Failed to run pdftotext: {e}"))?;

    let output = tokio::time::timeout(PDFTOTEXT_TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| "pdftotext timed out".to_string())?
        .map_err(|e| format!("Failed to run pdftotext: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("pdftotext failed: {stderr}"));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[cfg(feature = "ssr")]
fn merge_ranges(ranges: &[SegmentRange]) -> Vec<SegmentRange> {
    let mut sanitized: Vec<SegmentRange> = ranges
        .iter()
        .map(|range| {
            let start = range.start_page.max(1);
            let end = range.end_page.max(start);
            SegmentRange {
                start_page: start,
                end_page: end,
            }
        })
        .collect();

    sanitized.sort_by_key(|range| (range.start_page, range.end_page));

    let mut merged: Vec<SegmentRange> = Vec::new();
    for range in sanitized {
        if let Some(last) = merged.last_mut() {
            if range.start_page <= last.end_page + 1 {
                last.end_page = last.end_page.max(range.end_page);
            } else {
                merged.push(range);
            }
        } else {
            merged.push(range);
        }
    }

    merged
}

#[cfg(feature = "ssr")]
pub async fn download_pdf_to_temp(
    minio_client: &s3::Client,
    bucket: &str,
    key: &str,
) -> Result<(tempfile::TempPath, u64), String> {
    use tokio::io::AsyncWriteExt;

    let object = minio_client
        .objects()
        .get(bucket, key)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch PDF from storage: {e}"))?;

    let mut size = object.content_length.unwrap_or(0);

    let temp_file = tempfile::NamedTempFile::with_suffix(".pdf")
        .map_err(|e| format!("Failed to create temp file: {e}"))?;
    let (temp_std_file, temp_path) = temp_file.into_parts();
    let mut temp_file = tokio::fs::File::from_std(temp_std_file);

    let mut stream = object.body;
    while let Some(chunk) = stream
        .try_next()
        .await
        .map_err(|e| format!("Failed to read PDF stream: {e}"))?
    {
        temp_file
            .write_all(&chunk)
            .await
            .map_err(|e| format!("Failed to write PDF: {e}"))?;
    }

    temp_file
        .flush()
        .await
        .map_err(|e| format!("Failed to flush PDF: {e}"))?;

    if size == 0 {
        size = tokio::fs::metadata(<tempfile::TempPath as AsRef<std::path::Path>>::as_ref(
            &temp_path,
        ))
        .await
        .map_err(|e| format!("Failed to read PDF size: {e}"))?
        .len();
    }

    Ok((temp_path, size))
}

#[cfg(feature = "ssr")]
pub async fn process_file_async(
    file_id: i64,
    temp_path: std::path::PathBuf,
    pool: sqlx::SqlitePool,
) -> Result<(), String> {
    // Update status to processing
    sqlx::query("UPDATE project_files SET processing_status = 'processing' WHERE id = ?")
        .bind(file_id)
        .execute(&pool)
        .await
        .map_err(|e: sqlx::Error| e.to_string())?;

    // Get file size for extraction
    let file_size = tokio::fs::metadata(&temp_path)
        .await
        .map_err(|e| e.to_string())?
        .len();

    // Extract text
    let result = match extract_text_with_pdftotext(&temp_path, file_size).await {
        Ok(extracted_text) => {
            sqlx::query(
                "UPDATE project_files SET extracted_text = ?, processing_status = 'completed' WHERE id = ?",
            )
            .bind(extracted_text)
            .bind(file_id)
            .execute(&pool)
            .await
            .map_err(|e: sqlx::Error| e.to_string())
        }
        Err(e) => {
            let error_msg = e.to_string();
            sqlx::query("UPDATE project_files SET processing_status = 'failed' WHERE id = ?")
                .bind(file_id)
                .execute(&pool)
                .await
                .map_err(|e: sqlx::Error| e.to_string())?;
            Err(error_msg)
        }
    };

    // Clean up the temporary file
    let _ = tokio::fs::remove_file(&temp_path).await;

    result.map(|_| ())
}

#[cfg(feature = "ssr")]
pub fn sanitize_filename(filename: &str) -> String {
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

#[cfg(feature = "ssr")]
pub fn build_segment_pdf_bytes(
    pdf_path: &Path,
    ranges: &[SegmentRange],
) -> Result<Vec<u8>, String> {
    use lopdf::Document;
    use std::collections::HashSet;

    let mut doc = Document::load(pdf_path).map_err(|e| format!("Failed to read PDF: {e}"))?;
    let total_pages = doc.get_pages().len() as i64;
    if total_pages == 0 {
        return Err("PDF has no pages".to_string());
    }

    let merged = merge_ranges(ranges);
    if merged.is_empty() {
        return Err("No valid pages selected".to_string());
    }

    let mut pages: Vec<u32> = Vec::new();
    for range in merged {
        if range.start_page > total_pages {
            continue;
        }
        let start = range.start_page.max(1).min(total_pages);
        let end = range.end_page.max(start).min(total_pages);
        for page in start..=end {
            pages.push(page as u32);
        }
    }

    pages.sort_unstable();
    pages.dedup();

    if pages.is_empty() {
        return Err("No valid pages selected".to_string());
    }

    let keep: HashSet<u32> = pages.into_iter().collect();
    let delete_pages: Vec<u32> = doc
        .get_pages()
        .keys()
        .copied()
        .filter(|page| !keep.contains(page))
        .collect();

    doc.delete_pages(&delete_pages);
    doc.prune_objects();

    let mut buffer = Vec::new();
    doc.save_to(&mut buffer)
        .map_err(|e| format!("Failed to save PDF: {e}"))?;

    Ok(buffer)
}
