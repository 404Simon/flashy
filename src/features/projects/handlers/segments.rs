use leptos::prelude::*;

use crate::features::projects::models::{PdfOutlineResponse, SegmentRange, SegmentStats};

#[cfg(feature = "ssr")]
use crate::features::projects::models::PdfTocEntry;

#[cfg(feature = "ssr")]
use crate::features::auth::utils::require_auth;

#[server(GetProjectFileOutline)]
pub async fn get_project_file_outline(file_id: i64) -> Result<PdfOutlineResponse, ServerFnError> {
    use sqlx::SqlitePool;
    use std::time::Instant;
    use tokio::time::{timeout, Duration};

    let user = require_auth().await?;
    tracing::info!(
        "TOC: starting outline extraction for file_id={} user_id={}",
        file_id,
        user.id
    );
    let pool = expect_context::<SqlitePool>();
    let app_state = expect_context::<crate::app_state::AppState>();

    let row = sqlx::query!(
        r#"
        SELECT pf.s3_key as "s3_key!: String", pf.s3_bucket as "s3_bucket!: String"
        FROM project_files pf
        INNER JOIN study_projects sp ON sp.id = pf.project_id
        WHERE pf.id = ? AND sp.user_id = ?
        "#,
        file_id,
        user.id
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        tracing::error!(
            "TOC: database lookup failed for file_id={} user_id={} err={}",
            file_id,
            user.id,
            e
        );
        ServerFnError::new(e.to_string())
    })?
    .ok_or_else(|| {
        tracing::info!(
            "TOC: file not found or access denied for file_id={} user_id={}",
            file_id,
            user.id
        );
        ServerFnError::new("File not found or access denied")
    })?;

    tracing::debug!(
        "TOC: resolved storage location for file_id={} bucket={} key={}",
        file_id,
        row.s3_bucket,
        row.s3_key
    );

    let download_start = Instant::now();
    let (temp_path, _size) = crate::features::projects::processing::download_pdf_to_temp(
        &app_state.minio_client,
        &row.s3_bucket,
        &row.s3_key,
    )
    .await
    .map_err(|e| {
        tracing::error!("TOC: download failed for file_id={} err={}", file_id, e);
        ServerFnError::new(e)
    })?;
    tracing::info!(
        "TOC: download complete for file_id={} in {:?}",
        file_id,
        download_start.elapsed()
    );

    let extract_start = Instant::now();
    let temp_path_for_extract = temp_path.to_path_buf();
    let outline = timeout(
        Duration::from_secs(20),
        tokio::task::spawn_blocking(move || {
            extract_outline_from_pdf(temp_path_for_extract.as_ref())
        }),
    )
    .await
    .map_err(|_| {
        tracing::error!(
            "TOC: outline extraction timed out for file_id={} after {:?}",
            file_id,
            extract_start.elapsed()
        );
        ServerFnError::new("Outline extraction timed out")
    })?
    .map_err(|e| {
        tracing::error!(
            "TOC: outline extraction task failed for file_id={} err={}",
            file_id,
            e
        );
        ServerFnError::new(format!("Outline extraction task failed: {e}"))
    })?
    .map_err(|e| {
        tracing::error!(
            "TOC: outline extraction failed for file_id={} err={}",
            file_id,
            e
        );
        ServerFnError::new(e)
    })?;
    tracing::info!(
        "TOC: outline extracted for file_id={} entries={} total_pages={} in {:?}",
        file_id,
        outline.entries.len(),
        outline.total_pages,
        extract_start.elapsed()
    );

    Ok(outline)
}

#[server(GetSegmentStats)]
pub async fn get_segment_stats(
    file_id: i64,
    ranges: Option<Vec<SegmentRange>>,
) -> Result<SegmentStats, ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();
    let app_state = expect_context::<crate::app_state::AppState>();

    let row = sqlx::query!(
        r#"
        SELECT pf.s3_key as "s3_key!: String", pf.s3_bucket as "s3_bucket!: String",
               pf.extracted_text as "extracted_text: String"
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

    let merged = match ranges {
        Some(ranges) => merge_ranges(&ranges),
        None => Vec::new(),
    };

    // If no ranges provided, use the full extracted text
    if merged.is_empty() {
        let extracted_text = row
            .extracted_text
            .ok_or_else(|| ServerFnError::new("File has no extracted text"))?;
        let word_count = extracted_text.split_whitespace().count() as i64;

        // For page count, we need to download and check the PDF
        let (temp_path, _pdf_size) = crate::features::projects::processing::download_pdf_to_temp(
            &app_state.minio_client,
            &row.s3_bucket,
            &row.s3_key,
        )
        .await
        .map_err(ServerFnError::new)?;

        // Get page count from the PDF
        let page_count = tokio::task::spawn_blocking(move || -> Result<i64, String> {
            use lopdf::Document;
            use std::path::Path;
            let doc = Document::load(temp_path.as_ref() as &Path)
                .map_err(|e| format!("Failed to read PDF: {e}"))?;
            Ok(doc.get_pages().len() as i64)
        })
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .map_err(ServerFnError::new)?;

        return Ok(SegmentStats {
            page_count,
            word_count,
        });
    }

    let (temp_path, pdf_size) = crate::features::projects::processing::download_pdf_to_temp(
        &app_state.minio_client,
        &row.s3_bucket,
        &row.s3_key,
    )
    .await
    .map_err(ServerFnError::new)?;

    let text = crate::features::projects::processing::extract_text_for_ranges_with_pdftotext(
        temp_path.as_ref(),
        pdf_size,
        &merged,
    )
    .await
    .map_err(ServerFnError::new)?;

    let word_count = text.split_whitespace().count() as i64;
    let page_count = merged
        .iter()
        .map(|range| range.end_page - range.start_page + 1)
        .sum();

    Ok(SegmentStats {
        page_count,
        word_count,
    })
}

#[server(SaveSegmentPdf)]
pub async fn save_segment_pdf(
    project_id: i64,
    file_id: i64,
    ranges: Vec<SegmentRange>,
    name: String,
) -> Result<i64, ServerFnError> {
    use crate::config::Config;
    use crate::features::projects::processing::{
        build_segment_pdf_bytes, download_pdf_to_temp, process_file_async, sanitize_filename,
    };
    use sqlx::SqlitePool;
    use tokio::io::AsyncWriteExt;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();
    let app_state = expect_context::<crate::app_state::AppState>();

    let trimmed_name = name.trim();
    if trimmed_name.is_empty() {
        return Err(ServerFnError::new("File name is required"));
    }
    let cleaned_name = sanitize_filename(trimmed_name);

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
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("File not found or access denied"))?;

    let (temp_path, _pdf_size) =
        download_pdf_to_temp(&app_state.minio_client, &row.s3_bucket, &row.s3_key)
            .await
            .map_err(ServerFnError::new)?;

    let ranges_clone = ranges.clone();
    let pdf_bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, String> {
        let path: &std::path::Path = temp_path.as_ref();
        build_segment_pdf_bytes(path, &ranges_clone)
    })
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .map_err(ServerFnError::new)?;

    let file_size = pdf_bytes.len() as u64;
    let config = Config::global();
    if file_size == 0 || file_size > config.max_pdf_bytes {
        return Err(ServerFnError::new("Generated PDF exceeds size limits"));
    }

    let temp_file = tempfile::NamedTempFile::with_suffix(".pdf")
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let (temp_std_file, temp_path_handle) = temp_file.into_parts();
    let mut temp_file = tokio::fs::File::from_std(temp_std_file);
    temp_file
        .write_all(&pdf_bytes)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    temp_file
        .flush()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let key = format!(
        "{}/{}/{}-{}",
        app_state.object_key_prefix,
        project_id,
        uuid::Uuid::new_v4(),
        cleaned_name
    );

    let temp_path_buf =
        <tempfile::TempPath as AsRef<std::path::Path>>::as_ref(&temp_path_handle).to_path_buf();

    let upload_file = tokio::fs::File::open(&temp_path_buf)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let body = aws_sdk_s3::primitives::ByteStream::read_from()
        .file(upload_file)
        .build()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    app_state
        .minio_client
        .put_object()
        .bucket(app_state.bucket_name.as_str())
        .key(&key)
        .content_type("application/pdf")
        .content_length(file_size as i64)
        .body(body)
        .send()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let file_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO project_files
            (project_id, original_filename, content_type, file_size, s3_bucket, s3_key, processing_status)
        VALUES
            (?, ?, ?, ?, ?, ?, 'pending')
        RETURNING id
        "#,
    )
    .bind(project_id)
    .bind(&cleaned_name)
    .bind("application/pdf")
    .bind(file_size as i64)
    .bind(&app_state.bucket_name)
    .bind(&key)
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let temp_path_buf = temp_path_handle
        .keep()
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let pool_clone = pool.clone();
    tokio::spawn(async move {
        if let Err(e) = process_file_async(file_id, temp_path_buf, pool_clone).await {
            eprintln!("Error processing file {}: {}", file_id, e);
        }
    });

    Ok(file_id)
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
fn extract_outline_from_pdf(path: &std::path::Path) -> Result<PdfOutlineResponse, String> {
    use lopdf::{Document, Object, ObjectId};
    use std::collections::{HashMap, HashSet};

    let doc = Document::load(path).map_err(|e| format!("Failed to read PDF: {e}"))?;
    let pages = doc.get_pages();
    let total_pages = pages.len() as i64;

    let mut page_map: HashMap<ObjectId, u32> = HashMap::new();
    for (page_number, obj_id) in pages {
        page_map.insert(obj_id, page_number);
    }

    let catalog = doc
        .catalog()
        .map_err(|e| format!("Failed to read PDF catalog: {e}"))?;

    let outlines_obj = match catalog.get(b"Outlines") {
        Ok(obj) => obj,
        Err(_) => {
            return Ok(PdfOutlineResponse {
                total_pages,
                entries: Vec::new(),
            });
        }
    };

    let outlines_resolved = resolve_object(&doc, outlines_obj)?;
    let outlines_dict = outlines_resolved
        .as_dict()
        .map_err(|_| "Invalid outline structure".to_string())?;

    let first = match outlines_dict.get(b"First") {
        Ok(Object::Reference(id)) => *id,
        _ => {
            return Ok(PdfOutlineResponse {
                total_pages,
                entries: Vec::new(),
            });
        }
    };

    let mut raw_entries: Vec<RawOutlineEntry> = Vec::new();
    let mut visited: HashSet<ObjectId> = HashSet::new();
    walk_outline(&doc, first, 0, &page_map, &mut raw_entries, &mut visited)?;

    let mut entries: Vec<PdfTocEntry> = Vec::new();
    for idx in 0..raw_entries.len() {
        let current = &raw_entries[idx];
        let mut end_page = total_pages;

        for next in raw_entries.iter().skip(idx + 1) {
            if next.level <= current.level {
                end_page = (next.start_page - 1).max(current.start_page);
                break;
            }
        }

        entries.push(PdfTocEntry {
            id: current.id.clone(),
            title: current.title.clone(),
            level: current.level,
            start_page: current.start_page,
            end_page,
        });
    }

    Ok(PdfOutlineResponse {
        total_pages,
        entries,
    })
}

#[cfg(feature = "ssr")]
#[derive(Clone, Debug)]
struct RawOutlineEntry {
    id: String,
    title: String,
    level: i64,
    start_page: i64,
}

#[cfg(feature = "ssr")]
fn walk_outline(
    doc: &lopdf::Document,
    first: lopdf::ObjectId,
    level: i64,
    page_map: &std::collections::HashMap<lopdf::ObjectId, u32>,
    entries: &mut Vec<RawOutlineEntry>,
    visited: &mut std::collections::HashSet<lopdf::ObjectId>,
) -> Result<(), String> {
    use lopdf::Object;

    let mut current = Some(first);
    while let Some(obj_id) = current {
        if !visited.insert(obj_id) {
            // Prevent cycles in malformed outline trees.
            break;
        }

        let obj = doc
            .get_object(obj_id)
            .map_err(|e| format!("Failed to read outline entry: {e}"))?;
        let dict = obj
            .as_dict()
            .map_err(|_| "Invalid outline entry".to_string())?;

        let title = dict
            .get(b"Title")
            .ok()
            .and_then(|obj| resolve_object(doc, obj).ok())
            .and_then(|obj| match obj {
                Object::String(_, _) => lopdf::decode_text_string(obj).ok(),
                Object::Name(bytes) => Some(String::from_utf8_lossy(bytes).to_string()),
                _ => None,
            })
            .unwrap_or_else(|| "Untitled".to_string());

        let start_page = dict
            .get(b"Dest")
            .ok()
            .and_then(|obj| resolve_object(doc, obj).ok())
            .and_then(|dest| resolve_dest_page(doc, dest, page_map))
            .or_else(|| {
                dict.get(b"A")
                    .ok()
                    .and_then(|obj| resolve_object(doc, obj).ok())
                    .and_then(|obj| obj.as_dict().ok())
                    .and_then(|action| {
                        let action_type = action.get(b"S").ok();
                        if matches!(
                            action_type,
                            Some(Object::Name(name)) if name == b"GoTo"
                        ) {
                            action
                                .get(b"D")
                                .ok()
                                .and_then(|dest| resolve_object(doc, dest).ok())
                                .and_then(|dest| resolve_dest_page(doc, dest, page_map))
                        } else {
                            None
                        }
                    })
            });

        if let Some(page) = start_page {
            if page as i64 <= 0 {
                // Skip invalid page references
            } else {
                entries.push(RawOutlineEntry {
                    id: format!("{}-{}", obj_id.0, obj_id.1),
                    title,
                    level,
                    start_page: page as i64,
                });
            }
        }

        if let Ok(Object::Reference(child_id)) = dict.get(b"First") {
            walk_outline(doc, *child_id, level + 1, page_map, entries, visited)?;
        }

        current = match dict.get(b"Next") {
            Ok(Object::Reference(next_id)) => Some(*next_id),
            _ => None,
        };
    }

    Ok(())
}

#[cfg(feature = "ssr")]
fn resolve_dest_page(
    doc: &lopdf::Document,
    dest: &lopdf::Object,
    page_map: &std::collections::HashMap<lopdf::ObjectId, u32>,
) -> Option<u32> {
    use lopdf::Object;

    match dest {
        Object::Reference(obj_id) => page_map.get(obj_id).copied(),
        Object::Array(items) => resolve_dest_array(doc, items, page_map),
        Object::Integer(page) => Some((*page as u32).saturating_add(1)),
        Object::Name(name) => resolve_named_destination(doc, name, page_map),
        Object::String(name, _) => resolve_named_destination(doc, name, page_map),
        _ => None,
    }
}

#[cfg(feature = "ssr")]
fn resolve_dest_array(
    doc: &lopdf::Document,
    items: &[lopdf::Object],
    page_map: &std::collections::HashMap<lopdf::ObjectId, u32>,
) -> Option<u32> {
    use lopdf::Object;

    let first = items.first()?;
    match first {
        Object::Reference(obj_id) => page_map.get(obj_id).copied(),
        Object::Integer(page) => Some((*page as u32).saturating_add(1)),
        Object::Name(name) => resolve_named_destination(doc, name, page_map),
        Object::String(name, _) => resolve_named_destination(doc, name, page_map),
        _ => None,
    }
}

#[cfg(feature = "ssr")]
fn resolve_named_destination(
    doc: &lopdf::Document,
    name: &[u8],
    page_map: &std::collections::HashMap<lopdf::ObjectId, u32>,
) -> Option<u32> {
    use lopdf::Object;

    let dest_obj = resolve_named_destination_from_dests(doc, name)
        .or_else(|| resolve_named_destination_from_names(doc, name))?;

    match dest_obj {
        Object::Array(items) => resolve_dest_array(doc, &items, page_map),
        Object::Dictionary(dict) => dict
            .get(b"D")
            .ok()
            .and_then(|dest| resolve_object(doc, dest).ok())
            .and_then(|dest| resolve_dest_page(doc, dest, page_map)),
        _ => None,
    }
}

#[cfg(feature = "ssr")]
fn resolve_object<'a>(
    doc: &'a lopdf::Document,
    obj: &'a lopdf::Object,
) -> Result<&'a lopdf::Object, String> {
    match obj {
        lopdf::Object::Reference(obj_id) => doc
            .get_object(*obj_id)
            .map_err(|e| format!("Failed to resolve PDF object: {e}")),
        _ => Ok(obj),
    }
}

#[cfg(feature = "ssr")]
fn resolve_named_destination_from_dests(
    doc: &lopdf::Document,
    name: &[u8],
) -> Option<lopdf::Object> {
    let catalog = doc.catalog().ok()?;
    let dests_obj = catalog.get(b"Dests").ok()?;
    let dests_obj = resolve_object(doc, dests_obj).ok()?;
    let dests_dict = dests_obj.as_dict().ok()?;
    let dest_obj = dests_dict.get(name).ok()?;
    resolve_object(doc, dest_obj).ok().cloned()
}

#[cfg(feature = "ssr")]
fn resolve_named_destination_from_names(
    doc: &lopdf::Document,
    name: &[u8],
) -> Option<lopdf::Object> {
    let catalog = doc.catalog().ok()?;
    let names_obj = catalog.get(b"Names").ok()?;
    let names_obj = resolve_object(doc, names_obj).ok()?;
    let names_dict = names_obj.as_dict().ok()?;
    let dests_node = names_dict.get(b"Dests").ok()?;
    let dests_node = resolve_object(doc, dests_node).ok()?;
    find_in_name_tree(doc, dests_node, name)
}

#[cfg(feature = "ssr")]
fn find_in_name_tree(
    doc: &lopdf::Document,
    node: &lopdf::Object,
    name: &[u8],
) -> Option<lopdf::Object> {
    use lopdf::Object;

    let dict = node.as_dict().ok()?;

    if let Ok(Object::Array(items)) = dict.get(b"Names") {
        let mut iter = items.iter();
        while let Some(key_obj) = iter.next() {
            let value_obj = iter.next()?;
            let key_bytes = match key_obj {
                Object::String(bytes, _) => bytes.as_slice(),
                Object::Name(bytes) => bytes.as_slice(),
                _ => continue,
            };
            if key_bytes == name {
                return resolve_object(doc, value_obj).ok().cloned();
            }
        }
    }

    if let Ok(Object::Array(kids)) = dict.get(b"Kids") {
        for kid in kids {
            if let Ok(resolved) = resolve_object(doc, kid) {
                if let Some(found) = find_in_name_tree(doc, resolved, name) {
                    return Some(found);
                }
            }
        }
    }

    None
}
