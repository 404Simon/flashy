use leptos::prelude::*;

use super::super::models::{Summary, SummaryListItem};

#[cfg(feature = "ssr")]
use super::super::models::GeneratedSummary;

#[cfg(feature = "ssr")]
use crate::features::auth::utils::require_auth;

#[cfg(feature = "ssr")]
fn get_max_context_words() -> usize {
    use crate::config::Config;
    Config::global().max_context_words
}

pub const DEFAULT_SUMMARY_PROMPT: &str = r##"# Agent Instructions: Generate Concise Study Summary for $TITLE$

### 1. Output Format
* Write a comprehensive, in-depth explanatory summary in **Markdown** format.
* Use **MathJax** for all mathematical formulas:
  - Inline math: `\(` and `\)`
  - Display math: `\[` and `\]`
  - **CRITICAL:** Escape backslashes in your JSON output. Use `\\(`, `\\)`, `\\[`, `\\]`.
  - Example: `\\(O(n \\log n)\\)`, `\\(E = mc^2\\)`, `\\[\\sum_{i=1}^{n} x_i\\]`
* **IMPORTANT:** Only use LaTeX for mathematical formulas. For all other content (lists, formatting, structure), use standard Markdown syntax.

### 2. Content Structure
Your summary MUST include:
* **Overview:** A concise introduction to the topic and its significance.
* **Key Concepts:** Detailed explanations of core ideas, definitions, and terminology.
* **Formal Definitions:** Precise mathematical or technical definitions where applicable.
* **Algorithms & Methods:** Step-by-step explanations with complexity analysis where relevant.
* **Important Formulas:** All significant equations and theorems, properly typeset.
* **Connections & Insights:** How concepts relate to each other, practical implications, common pitfalls.

### 3. Style Guidelines
* Be concise but thorough — every sentence should carry information.
* Use Markdown headers (##, ###) to organize sections logically.
* Use bullet points and numbered lists for scannability.
* Bold (**term**) key terms on first use.
* Include worked examples where they clarify understanding.
* Aim for depth: don't just list topics, explain the *why* and *how*.

### 4. JSON Escaping Rules
**IMPORTANT:** Your response must be valid JSON. Remember to:
* Escape all backslashes as double backslashes: `\` becomes `\\` in JSON strings
* For LaTeX math delimiters: use `\\(` and `\\)` for inline, `\\[` and `\\]` for display

Here is the document text:

$DOCUMENT_TEXT$

Generate the summary as JSON with "title", "description" (brief one-line description), and "content_markdown" fields."##;

#[server(CreateSummary)]
pub async fn create_summary(
    project_id: i64,
    title: String,
    description: Option<String>,
    content_markdown: String,
    file_id: Option<i64>,
    segment_label: Option<String>,
) -> Result<Summary, ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();
    let title = title.trim().to_string();

    if title.len() < 2 {
        return Err(ServerFnError::new("Title must be at least 2 characters"));
    }

    let project = sqlx::query!(
        "SELECT id FROM study_projects WHERE id = ? AND user_id = ?",
        project_id,
        user.id
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("Project not found or access denied"))?;

    let description = description
        .map(|d| d.trim().to_string())
        .filter(|d| !d.is_empty());

    let result = sqlx::query!(
        r#"INSERT INTO summaries (project_id, title, description, content_markdown, file_id, segment_label, status)
        VALUES (?, ?, ?, ?, ?, ?, 'completed')"#,
        project.id,
        title,
        description,
        content_markdown,
        file_id,
        segment_label,
    )
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let summary_id = result.last_insert_rowid();

    let summary = sqlx::query_as!(
        Summary,
        r#"SELECT id, project_id, title, description, content_markdown, file_id, segment_label, status, error_message, created_at, updated_at
        FROM summaries WHERE id = ?"#,
        summary_id
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(summary)
}

#[server(StartSummaryGeneration)]
pub async fn start_summary_generation(
    project_id: i64,
    file_id: i64,
    title: String,
    segment_label: Option<String>,
    segment_ranges: Option<Vec<crate::features::projects::models::SegmentRange>>,
) -> Result<Summary, ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();

    let project = sqlx::query!(
        "SELECT id FROM study_projects WHERE id = ? AND user_id = ?",
        project_id,
        user.id
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("Project not found or access denied"))?;

    sqlx::query!(
        "SELECT id FROM project_files WHERE id = ? AND project_id = ?",
        file_id,
        project.id
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("File not found or access denied"))?;

    let title = title.trim().to_string();
    if title.len() < 2 {
        return Err(ServerFnError::new("Title must be at least 2 characters"));
    }

    let segment_label = segment_label
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty());
    let segment_ranges_json = segment_ranges
        .filter(|r| !r.is_empty())
        .map(|ranges| {
            serde_json::to_string(&ranges)
                .map_err(|e| ServerFnError::new(format!("Invalid segment ranges: {e}")))
        })
        .transpose()?;

    let result = sqlx::query!(
        r#"INSERT INTO summaries (project_id, title, content_markdown, file_id, segment_label, status)
        VALUES (?, ?, '', ?, ?, 'pending')"#,
        project.id,
        title,
        file_id,
        segment_label,
    )
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let summary_id = result.last_insert_rowid();

    let pool_clone = pool.clone();
    let app_state = expect_context::<crate::app_state::AppState>();
    let app_state_clone = app_state.clone();
    let segment_ranges_for_job = segment_ranges_json.clone();
    tokio::spawn(async move {
        if let Err(e) =
            process_summary_generation(summary_id, pool_clone, app_state_clone, segment_ranges_for_job).await
        {
            eprintln!("Error processing summary generation {}: {}", summary_id, e);
        }
    });

    let summary = sqlx::query_as!(
        Summary,
        r#"SELECT id, project_id, title, description, content_markdown, file_id, segment_label, status, error_message, created_at, updated_at
        FROM summaries WHERE id = ?"#,
        summary_id
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(summary)
}

#[cfg(feature = "ssr")]
async fn process_summary_generation(
    summary_id: i64,
    pool: sqlx::SqlitePool,
    app_state: crate::app_state::AppState,
    segment_ranges_json: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    use sqlx::Row;

    sqlx::query(
        "UPDATE summaries SET status = 'processing', updated_at = datetime('now') WHERE id = ?",
    )
    .bind(summary_id)
    .execute(&pool)
    .await?;

    let row = sqlx::query(
        r#"
        SELECT s.title, s.file_id, s.segment_label,
               pf.extracted_text, pf.original_filename, pf.s3_bucket, pf.s3_key
        FROM summaries s
        INNER JOIN project_files pf ON s.file_id = pf.id
        WHERE s.id = ?
        "#,
    )
    .bind(summary_id)
    .fetch_one(&pool)
    .await?;

    let title: String = row.try_get("title")?;
    let _file_id: i64 = row.try_get("file_id")?;

    let extracted_text = if let Some(ranges_json) = segment_ranges_json {
        let ranges: Vec<crate::features::projects::models::SegmentRange> =
            serde_json::from_str(&ranges_json)?;
        let s3_bucket: String = row.try_get("s3_bucket")?;
        let s3_key: String = row.try_get("s3_key")?;
        let (temp_path, pdf_size) =
            crate::features::projects::processing::download_pdf_to_temp(
                &app_state.minio_client,
                &s3_bucket,
                &s3_key,
            )
            .await
            .map_err(|e| e.to_string())?;

        crate::features::projects::processing::extract_text_for_ranges_with_pdftotext(
            temp_path.as_ref(),
            pdf_size,
            &ranges,
        )
        .await
        .map_err(|e| e.to_string())?
    } else {
        let extracted_text: Option<String> = row.try_get("extracted_text")?;
        extracted_text.ok_or("File has no extracted text")?
    };

    let word_count = extracted_text.split_whitespace().count();
    let max_words = get_max_context_words();
    if word_count > max_words {
        let error_message = format!(
            "Document is too large. Has {} words, maximum is {}.",
            word_count, max_words
        );
        sqlx::query(
            "UPDATE summaries SET status = 'failed', error_message = ?, updated_at = datetime('now') WHERE id = ?"
        )
        .bind(&error_message)
        .bind(summary_id)
        .execute(&pool)
        .await?;
        return Err(error_message.into());
    }

    let prompt = DEFAULT_SUMMARY_PROMPT
        .replace("$TITLE$", &title)
        .replace("$DOCUMENT_TEXT$", &extracted_text);

    match call_llm_for_summary(&prompt).await {
        Ok(generated) => {
            let description = generated.description.filter(|d| !d.trim().is_empty());
            sqlx::query(
                r#"UPDATE summaries
                SET status = 'completed', content_markdown = ?, description = ?, updated_at = datetime('now')
                WHERE id = ?"#
            )
            .bind(&generated.content_markdown)
            .bind(&description)
            .bind(summary_id)
            .execute(&pool)
            .await?;
        }
        Err(e) => {
            let error_message = e.to_string();
            sqlx::query(
                "UPDATE summaries SET status = 'failed', error_message = ?, updated_at = datetime('now') WHERE id = ?"
            )
            .bind(&error_message)
            .bind(summary_id)
            .execute(&pool)
            .await?;
        }
    }

    Ok(())
}

#[cfg(feature = "ssr")]
async fn call_llm_for_summary(prompt: &str) -> Result<GeneratedSummary, ServerFnError> {
    use llm::{
        builder::{LLMBackend, LLMBuilder},
        chat::{ChatMessage, StructuredOutputFormat},
    };
    use std::str::FromStr;

    let config = crate::config::Config::global();

    let api_key = config
        .llm_api_key
        .clone()
        .ok_or_else(|| ServerFnError::new("LLM_API_KEY not set"))?;

    let backend = LLMBackend::from_str(&config.llm_provider).map_err(|e| {
        ServerFnError::new(format!("Invalid LLM_PROVIDER '{}': {}", config.llm_provider, e))
    })?;

    let model = if config.llm_model.is_empty() {
        return Err(ServerFnError::new("LLM_MODEL not set"));
    } else {
        &config.llm_model
    };

    let schema = r#"
        {
            "name": "summary",
            "schema": {
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "The title of the summary"
                    },
                    "description": {
                        "type": "string",
                        "description": "A brief one-line description of the summary"
                    },
                    "content_markdown": {
                        "type": "string",
                        "description": "The full summary content in Markdown format with LaTeX math"
                    }
                },
                "required": ["title", "content_markdown"]
            }
        }
    "#;
    let schema: StructuredOutputFormat = serde_json::from_str(schema)
        .map_err(|e| ServerFnError::new(format!("Failed to parse JSON schema: {}", e)))?;

    let llm = LLMBuilder::new()
        .backend(backend)
        .api_key(api_key)
        .model(model)
        .max_tokens(8192)
        .temperature(0.5)
        .schema(schema)
        .build()
        .map_err(|e| ServerFnError::new(format!("Failed to build LLM: {}", e)))?;

    let messages = vec![ChatMessage::user().content(prompt).build()];

    let response = llm
        .chat(&messages)
        .await
        .map_err(|e| ServerFnError::new(format!("LLM API error: {}", e)))?;

    let content = response
        .text()
        .ok_or_else(|| ServerFnError::new("No text in LLM response"))?;

    let json_content = if content.trim().starts_with("```") {
        content
            .trim()
            .strip_prefix("```json")
            .or_else(|| content.trim().strip_prefix("```"))
            .and_then(|s| s.strip_suffix("```"))
            .unwrap_or(&content)
            .trim()
    } else {
        content.trim()
    };

    let generated: GeneratedSummary = serde_json::from_str(json_content).map_err(|e| {
        ServerFnError::new(format!("Failed to parse LLM response: {}. Response: {}", e, json_content))
    })?;

    if generated.content_markdown.trim().is_empty() {
        return Err(ServerFnError::new("LLM returned empty summary"));
    }

    Ok(generated)
}

#[server(ListSummariesForProject)]
pub async fn list_summaries_for_project(
    project_id: i64,
) -> Result<Vec<SummaryListItem>, ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();

    sqlx::query!(
        "SELECT id FROM study_projects WHERE id = ? AND user_id = ?",
        project_id,
        user.id
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("Project not found or access denied"))?;

    let rows = sqlx::query!(
        r#"SELECT
            id as "id!: i64",
            project_id as "project_id!: i64",
            title as "title!: String",
            CAST(COALESCE(description, '') AS TEXT) as "description!: String",
            segment_label as "segment_label: String",
            status as "status!: String",
            created_at as "created_at!: String"
        FROM summaries
        WHERE project_id = ?
        ORDER BY created_at DESC"#,
        project_id
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|row| SummaryListItem {
            id: row.id,
            project_id: row.project_id,
            title: row.title,
            description: if row.description.trim().is_empty() {
                None
            } else {
                Some(row.description)
            },
            segment_label: row.segment_label,
            status: row.status,
            created_at: row.created_at,
        })
        .collect())
}

#[server(GetSummary)]
pub async fn get_summary(summary_id: i64) -> Result<Summary, ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();

    let summary = sqlx::query_as!(
        Summary,
        r#"SELECT id, project_id, title, description, content_markdown, file_id, segment_label, status, error_message, created_at, updated_at
        FROM summaries
        WHERE id = ? AND project_id IN (
            SELECT id FROM study_projects WHERE user_id = ?
        )"#,
        summary_id,
        user.id
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("Summary not found or access denied"))?;

    Ok(summary)
}

#[server(DeleteSummary)]
pub async fn delete_summary(summary_id: i64) -> Result<(), ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();

    let result = sqlx::query!(
        r#"DELETE FROM summaries
        WHERE id = ? AND project_id IN (
            SELECT id FROM study_projects WHERE user_id = ?
        )"#,
        summary_id,
        user.id
    )
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err(ServerFnError::new("Summary not found or access denied"));
    }

    Ok(())
}

#[server(UpdateSummary)]
pub async fn update_summary(
    summary_id: i64,
    title: String,
    description: Option<String>,
) -> Result<Summary, ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();

    let title = title.trim().to_string();
    if title.len() < 2 {
        return Err(ServerFnError::new("Title must be at least 2 characters"));
    }

    let description = description
        .map(|d| d.trim().to_string())
        .filter(|d| !d.is_empty());

    let result = sqlx::query!(
        r#"UPDATE summaries
        SET title = ?, description = ?, updated_at = datetime('now')
        WHERE id = ? AND project_id IN (
            SELECT id FROM study_projects WHERE user_id = ?
        )"#,
        title,
        description,
        summary_id,
        user.id
    )
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err(ServerFnError::new("Summary not found or access denied"));
    }

    let summary = sqlx::query_as!(
        Summary,
        r#"SELECT id, project_id, title, description, content_markdown, file_id, segment_label, status, error_message, created_at, updated_at
        FROM summaries WHERE id = ?"#,
        summary_id
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(summary)
}

#[server(GetSummaryMarkdown)]
pub async fn get_summary_markdown(summary_id: i64) -> Result<String, ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();

    let row = sqlx::query!(
        r#"SELECT content_markdown as "content!: String"
        FROM summaries
        WHERE id = ? AND project_id IN (
            SELECT id FROM study_projects WHERE user_id = ?
        )"#,
        summary_id,
        user.id
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("Summary not found or access denied"))?;

    Ok(row.content)
}
