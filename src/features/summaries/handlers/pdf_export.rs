#[cfg(feature = "ssr")]
use std::io::Write;

#[cfg(feature = "ssr")]
pub async fn download_summary_pdf(
    axum::extract::Path(summary_id): axum::extract::Path<i64>,
    axum::extract::State(app_state): axum::extract::State<crate::app_state::AppState>,
    session: tower_sessions::Session,
) -> Result<([(axum::http::HeaderName, String); 2], Vec<u8>), axum::http::StatusCode> {
    use axum::http::{header, HeaderName};

    use crate::features::auth::utils::get_user_from_session;
    use crate::features::flashcards::markdown::markdown_to_html;
    use crate::features::summaries::models::Summary;

    let user = get_user_from_session(&session)
        .await
        .ok_or(axum::http::StatusCode::UNAUTHORIZED)?;

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
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(axum::http::StatusCode::NOT_FOUND)?;

    if summary.status != "completed" {
        return Err(axum::http::StatusCode::UNPROCESSABLE_ENTITY);
    }

    let content_html = markdown_to_html(&summary.content_markdown);
    let full_html = build_pdf_html(&summary.title, &content_html);

    let mut temp_file = tempfile::NamedTempFile::with_suffix(".html").map_err(|e| {
        tracing::error!("Failed to create temp file: {e}");
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;
    temp_file.write_all(full_html.as_bytes()).map_err(|e| {
        tracing::error!("Failed to write temp HTML: {e}");
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;
    temp_file.flush().map_err(|e| {
        tracing::error!("Failed to flush temp HTML: {e}");
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let html_path = temp_file.path().to_path_buf();

    let pdf_bytes = tokio::task::spawn_blocking(move || render_html_to_pdf(&html_path))
        .await
        .map_err(|e| {
            tracing::error!("Blocking task join error: {e}");
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?
        .map_err(|err_msg| {
            tracing::error!("PDF render failed: {err_msg}");
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // temp_file is dropped here, cleaning up the HTML file

    let filename = sanitize_pdf_filename(&summary.title);

    let content_disposition = format!("attachment; filename=\"{filename}\"");
    Ok((
        [
            (header::CONTENT_TYPE, "application/pdf".to_string()),
            (
                HeaderName::from_static("content-disposition"),
                content_disposition,
            ),
        ],
        pdf_bytes,
    ))
}

#[cfg(feature = "ssr")]
fn build_pdf_html(title: &str, content_html: &str) -> String {
    let title_escaped = title
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;");

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>{title}</title>
<script>
MathJax = {{
  tex: {{
    inlineMath: [['$', '$'], ['\\(', '\\)']],
    displayMath: [['$$', '$$'], ['\\[', '\\]']],
    processEscapes: true
  }},
  svg: {{ fontCache: 'global' }}
}};
</script>
<script src="https://cdn.jsdelivr.net/npm/mathjax@3/es5/tex-mml-chtml.js" async></script>
<style>
  * {{ margin: 0; padding: 0; box-sizing: border-box; }}
  body {{
    font-family: 'Liberation Serif', 'Georgia', 'Times New Roman', serif;
    font-size: 12pt;
    line-height: 1.7;
    color: #1a1a1a;
    max-width: 100%;
    padding: 0.75in 1in;
  }}
  h1 {{ font-size: 20pt; margin: 0.5em 0 0.3em; page-break-after: avoid; }}
  h2 {{ font-size: 16pt; margin: 0.5em 0 0.3em; page-break-after: avoid; }}
  h3 {{ font-size: 14pt; margin: 0.4em 0 0.2em; page-break-after: avoid; }}
  h4 {{ font-size: 12pt; margin: 0.4em 0 0.2em; page-break-after: avoid; }}
  p {{ margin: 0.3em 0; }}
  pre, code {{ font-family: 'Liberation Mono', 'Courier New', monospace; font-size: 10pt; }}
  pre {{
    background: #f5f5f5;
    padding: 0.8em;
    margin: 0.5em 0;
    border-radius: 4px;
    overflow-x: auto;
    white-space: pre-wrap;
    word-break: break-word;
  }}
  code {{ background: #f0f0f0; padding: 0.1em 0.3em; border-radius: 2px; }}
  pre code {{ background: none; padding: 0; }}
  table {{ border-collapse: collapse; margin: 0.5em 0; width: 100%; }}
  th, td {{ border: 1px solid #ccc; padding: 0.4em 0.6em; text-align: left; }}
  th {{ background: #f0f0f0; font-weight: 600; }}
  blockquote {{
    border-left: 3px solid #ccc;
    margin: 0.5em 0;
    padding-left: 1em;
    color: #555;
  }}
  ul, ol {{ margin: 0.3em 0; padding-left: 2em; }}
  li {{ margin: 0.15em 0; }}
  img {{ max-width: 100%; }}
  .page-break {{ page-break-before: always; }}
  @media print {{
    body {{ padding: 0; }}
  }}
</style>
</head>
<body>
<div id="mathjax-content">
{content}
</div>
<script>
(function() {{
  function signalReady() {{
    var el = document.createElement('div');
    el.id = 'mathjax-done';
    el.style.display = 'none';
    document.body.appendChild(el);
  }}
  function typesetWhenReady() {{
    if (window.MathJax && MathJax.typesetPromise) {{
      MathJax.typesetPromise().then(signalReady).catch(signalReady);
    }} else {{
      signalReady();
    }}
  }}
  if (document.readyState === 'complete') {{
    typesetWhenReady();
  }} else {{
    window.addEventListener('load', typesetWhenReady);
  }}
  setTimeout(signalReady, 15000);
}})();
</script>
</body>
</html>"#,
        title = title_escaped,
        content = content_html,
    )
}

#[cfg(feature = "ssr")]
fn render_html_to_pdf(html_path: &std::path::Path) -> Result<Vec<u8>, String> {
    use std::ffi::OsString;

    use headless_chrome::types::PrintToPdfOptions;
    use headless_chrome::{Browser, LaunchOptions};

    let chrome_args = [
        OsString::from("--no-sandbox"),
        OsString::from("--disable-gpu"),
        OsString::from("--disable-dev-shm-usage"),
        OsString::from("--allow-file-access-from-files"),
    ];
    let chrome_args_refs: Vec<&std::ffi::OsStr> =
        chrome_args.iter().map(|a| a.as_os_str()).collect();

    let mut builder = LaunchOptions::default_builder();
    builder.headless(true);
    builder.sandbox(false);
    builder.window_size(Some((1200, 1600)));
    builder.args(chrome_args_refs);
    let launch_options = builder
        .build()
        .map_err(|e| format!("Failed to build launch options: {e}"))?;

    let browser =
        Browser::new(launch_options).map_err(|e| format!("Failed to launch browser: {e}"))?;

    let tab = browser
        .new_tab()
        .map_err(|e| format!("Failed to create tab: {e}"))?;

    let file_url = format!("file://{}", html_path.display());
    tab.navigate_to(&file_url)
        .map_err(|e| format!("Failed to navigate to HTML: {e}"))?;

    tab.wait_until_navigated()
        .map_err(|e| format!("Failed waiting for page load: {e}"))?;

    tab.wait_for_element_with_custom_timeout("#mathjax-done", std::time::Duration::from_secs(20))
        .map_err(|e| format!("MathJax render timeout: {e}"))?;

    tab.wait_for_element_with_custom_timeout("body", std::time::Duration::from_secs(2))
        .map_err(|_| "Body not found after MathJax".to_string())?;

    let pdf_options = Some(PrintToPdfOptions {
        landscape: None,
        display_header_footer: None,
        print_background: Some(true),
        scale: None,
        paper_width: None,
        paper_height: None,
        margin_top: Some(0.5),
        margin_bottom: Some(0.5),
        margin_left: Some(0.5),
        margin_right: Some(0.5),
        page_ranges: None,
        header_template: None,
        footer_template: None,
        prefer_css_page_size: Some(true),
        transfer_mode: None,
        generate_document_outline: None,
        generate_tagged_pdf: None,
        ignore_invalid_page_ranges: None,
    });

    let pdf_bytes = tab
        .print_to_pdf(pdf_options)
        .map_err(|e| format!("Failed to print PDF: {e}"))?;

    Ok(pdf_bytes)
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
