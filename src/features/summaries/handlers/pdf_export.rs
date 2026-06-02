#[cfg(feature = "ssr")]
use axum::extract::{Path, State};
#[cfg(feature = "ssr")]
use axum::http::{header, HeaderName, StatusCode};
#[cfg(feature = "ssr")]
use markdown2pdf::config::ConfigSource;
#[cfg(feature = "ssr")]
use tower_sessions::Session;

#[cfg(feature = "ssr")]
use crate::app_state::AppState;
#[cfg(feature = "ssr")]
use crate::features::auth::utils::get_user_from_session;

/// Embedded TOML configuration for study summary PDFs.
///
/// Uses the `academic` theme as a baseline and overrides values that
/// produce a clean, scannable layout for AI-generated summaries.
#[cfg(feature = "ssr")]
const PDF_STYLE: &str = r##"
theme = "academic"

[defaults]
font_size_pt = 10.5
line_height = 1.55
text_color = "#1B1F23"

[page]
margins = { top = 20.0, right = 22.0, bottom = 20.0, left = 22.0 }

[headings.h1]
font_size_pt = 20.0
margin_before_pt = 10.0
margin_after_pt = 4.0

[headings.h2]
font_size_pt = 14.0
margin_before_pt = 8.0
margin_after_pt = 3.0

[headings.h3]
font_size_pt = 12.0
margin_before_pt = 6.0

[paragraph]
margin_after_pt = 5.0

[code_block]
font_family = "Courier"
font_size_pt = 8.5
background_color = "#F6F8FA"
padding = { top = 8.0, right = 10.0, bottom = 8.0, left = 10.0 }
margin_before_pt = 5.0
margin_after_pt = 5.0

[code_inline]
font_family = "Courier"
font_size_pt = 9.0
background_color = "#EFF1F3"

[list.common]
indent_per_level_pt = 15.0

[table]
row_gap_pt = 1.5
cell_padding = { top = 3.0, right = 4.0, bottom = 3.0, left = 4.0 }

[table.border.all]
width_pt = 0.4
color = "#D0D7DE"

[math]
scale = 1.05
color = "#1A1A1A"

[footer]
center = "{page} / {total_pages}"

[toc]
enabled = true
title = "Contents"
max_depth = 3
"##;

#[cfg(feature = "ssr")]
pub async fn download_summary_pdf(
    Path(summary_id): Path<i64>,
    State(app_state): State<AppState>,
    session: Session,
) -> Result<([(HeaderName, String); 2], Vec<u8>), StatusCode> {
    #[cfg(feature = "ssr")]
    use crate::features::summaries::models::Summary;

    let user = get_user_from_session(&session)
        .await
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let summary = sqlx::query_as!(
        Summary,
        r#"SELECT id, project_id, title, description, content_markdown,
                  file_id, segment_label, status, error_message, created_at, updated_at
           FROM summaries
           WHERE id = ? AND project_id IN (
               SELECT id FROM study_projects WHERE user_id = ?
           )"#,
        summary_id,
        user.id
    )
    .fetch_optional(&app_state.db_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch summary for PDF: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::NOT_FOUND)?;

    if summary.status != "completed" {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }

    let markdown = convert_math_delimiters(&summary.content_markdown);

    let pdf_bytes =
        markdown2pdf::parse_into_bytes(markdown, ConfigSource::Embedded(PDF_STYLE), None).map_err(
            |e| {
                tracing::error!("PDF generation failed: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            },
        )?;

    let filename = sanitize_pdf_filename(&summary.title);
    let content_disposition = format!("attachment; filename=\"{filename}\"");

    Ok((
        [
            (header::CONTENT_TYPE, "application/pdf".into()),
            (
                HeaderName::from_static("content-disposition"),
                content_disposition,
            ),
        ],
        pdf_bytes,
    ))
}

/// Converts MathJax-style LaTeX delimiters to markdown2pdf-compatible ones.
///
/// markdown2pdf uses `$…$` / `$$…$$` (standard Markdown math), while the
/// AI-generated summaries use `\(…\)` / `\[…\]` (MathJax convention).
/// Also replaces `|` with `\vert` inside math content, since raw pipes
/// break GFM table parsing when math appears in table cells.
#[cfg(feature = "ssr")]
fn convert_math_delimiters(markdown: &str) -> String {
    use regex::Regex;

    // Display math \[...\] — replace pipes, then swap delimiters.
    let display_re = Regex::new(r"(?s)\\\[(.*?)\\\]").unwrap();
    let result = display_re.replace_all(markdown, |caps: &regex::Captures| {
        format!("$${}$$", caps[1].replace('|', r"\vert "))
    });

    // Inline math \(...\).
    let inline_re = Regex::new(r"(?s)\\\((.*?)\\\)").unwrap();
    let result = inline_re.replace_all(&result, |caps: &regex::Captures| {
        format!("${}$", caps[1].replace('|', r"\vert "))
    });

    result.to_string()
}

#[cfg(feature = "ssr")]
fn sanitize_pdf_filename(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' || c == '.' {
                c
            } else {
                '-'
            }
        })
        .collect();

    let mut cleaned = cleaned.trim().to_string();
    if cleaned.is_empty() {
        cleaned = "summary".to_string();
    }
    if !cleaned.to_lowercase().ends_with(".pdf") {
        cleaned.push_str(".pdf");
    }
    cleaned
}
