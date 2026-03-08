use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(feature = "ssr")]
use crate::features::auth::utils::require_auth;

use super::super::models::{Flashcard, GenerationJob, GenerationJobWithFile, GenerationPrompt};

#[cfg(feature = "ssr")]
fn get_max_context_words() -> usize {
    use crate::config::Config;
    Config::global().max_context_words
}

// Default prompt template
pub const DEFAULT_PROMPT_TEMPLATE: &str = r#"# Agent Instructions: Flashcards for $DECK_TITLE$

### 1. Card Formatting
* **MathJax Requirement:** Use `\\(` and `\\)` for inline math and `\\[` and `\\]` for display math.
    * **CRITICAL:** You MUST escape backslashes in your JSON output. Use double backslashes: `\\(`, `\\)`, `\\[`, `\\]`.
    * *Example:* `\\(O(n \\log n)\\)`, `\\(\\Sigma\\)`, `\\(G=(V,E)\\)`.
* **Structure:**
    * **Front:** Concise question or "Formally define [Topic]."
    * **Back:** Scannable bulleted lists.

### 2. Mandatory Content Tiers
For every algorithm/concept, generate cards covering:
* **Formal Definitions:** Use technical details and mathematical formalism.
* **I/O Specifications:** Explicitly state:
    * **Input:** [Formal parameters]
    * **Output:** [Formal result]
* **Conceptual Depth:** Explain "how/why," techniques (DP, Greedy), and trade-offs.

### 3. Execution Logic
1. Parse `pdftotext` output.
2. Ensure all mathematical symbols are wrapped in **MathJax** delimiters with **properly escaped backslashes** (`\\(`, `\\)`, `\\[`, `\\]`).
3. Generate comprehensive flashcards covering all major concepts, definitions, algorithms, and formulas.

### 4. JSON Escaping Rules
**IMPORTANT:** Your response must be valid JSON. Remember to:
* Escape all backslashes as double backslashes: `\\` becomes `\\\\` in JSON strings
* For LaTeX math delimiters: use `\\(` and `\\)` for inline, `\\[` and `\\]` for display
* Example valid JSON: `{"front": "What is \\\\(O(n \\\\log n)\\\\)?", "back": "Linearithmic complexity"}`

Here is the document text:

$DOCUMENT_TEXT$

Generate flashcards as an array in the "cards" field. Each card should have "front" and "back" fields."#;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeneratedCard {
    pub front: String,
    pub back: String,
}

#[server(GenerateFlashcards)]
pub async fn generate_flashcards(
    deck_id: i64,
    file_id: i64,
    prompt_template: Option<String>,
) -> Result<Vec<Flashcard>, ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();

    // Verify user owns the deck
    let deck = sqlx::query!(
        r#"
        SELECT fd.id, fd.name
        FROM flashcard_decks fd
        INNER JOIN study_projects sp ON fd.project_id = sp.id
        WHERE fd.id = ? AND sp.user_id = ?
        "#,
        deck_id,
        user.id
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("Deck not found or access denied"))?;

    // Get the file and verify ownership
    let file = sqlx::query!(
        r#"
        SELECT pf.extracted_text, pf.original_filename
        FROM project_files pf
        INNER JOIN study_projects sp ON pf.project_id = sp.id
        WHERE pf.id = ? AND sp.user_id = ?
        "#,
        file_id,
        user.id
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("File not found or access denied"))?;

    let extracted_text = file
        .extracted_text
        .ok_or_else(|| ServerFnError::new("File has no extracted text"))?;

    // Check word count limit
    let word_count = extracted_text.split_whitespace().count();
    let max_words = get_max_context_words();
    if word_count > max_words {
        return Err(ServerFnError::new(format!(
            "Document is too large for AI generation. Document has {} words, but the maximum is {} words. Please use a smaller document or split it into multiple files.",
            word_count, max_words
        )));
    }

    // Prepare the prompt
    let prompt_template = prompt_template.unwrap_or_else(|| DEFAULT_PROMPT_TEMPLATE.to_string());
    let prompt = prompt_template
        .replace("$DECK_TITLE$", &deck.name)
        .replace("$DOCUMENT_TEXT$", &extracted_text);

    // Call LLM to generate flashcards
    let generated_cards = call_llm_for_flashcards(&prompt).await?;

    // Insert all generated cards
    let mut created_cards = Vec::new();
    for card in generated_cards {
        let result = sqlx::query!(
            "INSERT INTO flashcards (deck_id, front, back, file_id) VALUES (?, ?, ?, ?)",
            deck_id,
            card.front,
            card.back,
            file_id
        )
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

        let card_id = result.last_insert_rowid();

        let flashcard = sqlx::query_as!(
            Flashcard,
            r#"
            SELECT id, deck_id, front, back, document_reference, file_id, created_at, updated_at
            FROM flashcards
            WHERE id = ?
            "#,
            card_id
        )
        .fetch_one(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

        created_cards.push(flashcard);
    }

    Ok(created_cards)
}

#[cfg(feature = "ssr")]
async fn call_llm_for_flashcards(prompt: &str) -> Result<Vec<GeneratedCard>, ServerFnError> {
    use llm::{
        builder::{LLMBackend, LLMBuilder},
        chat::{ChatMessage, StructuredOutputFormat},
    };

    println!("🤖 Starting LLM call for flashcard generation");

    // Get API key from env
    let api_key = std::env::var("DEEPSEEK_API_KEY")
        .map_err(|_| ServerFnError::new("DEEPSEEK_API_KEY not set in environment"))?;

    // Define JSON schema for flashcard array
    let schema = r#"
        {
            "name": "flashcards",
            "schema": {
                "type": "object",
                "properties": {
                    "cards": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "front": {
                                    "type": "string",
                                    "description": "The question or prompt side of the flashcard"
                                },
                                "back": {
                                    "type": "string",
                                    "description": "The answer or explanation side of the flashcard"
                                }
                            },
                            "required": ["front", "back"]
                        }
                    }
                },
                "required": ["cards"]
            }
        }
    "#;
    let schema: StructuredOutputFormat = serde_json::from_str(schema)
        .map_err(|e| ServerFnError::new(format!("Failed to parse JSON schema: {}", e)))?;

    println!("🤖 Building LLM client with structured output schema");

    let llm = LLMBuilder::new()
        .backend(LLMBackend::DeepSeek)
        .api_key(api_key)
        .model("deepseek-chat")
        .max_tokens(4096)
        .temperature(0.7)
        .schema(schema)
        .build()
        .map_err(|e| ServerFnError::new(format!("Failed to build LLM: {}", e)))?;

    let messages = vec![ChatMessage::user().content(prompt).build()];

    println!("🤖 Sending request to DeepSeek API...");

    let response = llm
        .chat(&messages)
        .await
        .map_err(|e| ServerFnError::new(format!("LLM API error: {}", e)))?;

    println!("🤖 Received response from DeepSeek API");

    let content = response
        .text()
        .ok_or_else(|| ServerFnError::new("No text in LLM response"))?;

    println!("🤖 Parsing response JSON ({} bytes)", content.len());

    // Strip markdown code blocks if present
    let json_content = if content.trim().starts_with("```") {
        println!("🤖 Detected markdown code blocks, stripping...");
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

    println!("🤖 Parsing cleaned JSON ({} bytes)", json_content.len());

    // Parse the structured response
    #[derive(serde::Deserialize)]
    struct FlashcardResponse {
        cards: Vec<GeneratedCard>,
    }

    let response: FlashcardResponse = serde_json::from_str(json_content).map_err(|e| {
        eprintln!("🤖 Failed to parse JSON response: {}", e);
        eprintln!("🤖 Response content: {}", json_content);
        ServerFnError::new(format!(
            "Failed to parse LLM response as JSON: {}. Response was: {}",
            e, json_content
        ))
    })?;

    if response.cards.is_empty() {
        eprintln!("🤖 Warning: LLM returned zero flashcards");
        return Err(ServerFnError::new("No flashcards generated"));
    }

    println!(
        "🤖 Successfully parsed {} flashcards from response",
        response.cards.len()
    );

    Ok(response.cards)
}

// ============= BACKGROUND JOB SYSTEM =============

#[server(StartGenerationJob)]
pub async fn start_generation_job(
    deck_id: i64,
    file_id: i64,
    prompt_template: Option<String>,
    segment_label: Option<String>,
    segment_ranges: Option<Vec<crate::features::projects::models::SegmentRange>>,
) -> Result<GenerationJob, ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();

    // Verify user owns the deck
    let _deck = sqlx::query!(
        r#"
        SELECT fd.id
        FROM flashcard_decks fd
        INNER JOIN study_projects sp ON fd.project_id = sp.id
        WHERE fd.id = ? AND sp.user_id = ?
        "#,
        deck_id,
        user.id
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("Deck not found or access denied"))?;

    // Verify user owns the file
    sqlx::query!(
        r#"
        SELECT pf.id
        FROM project_files pf
        INNER JOIN study_projects sp ON pf.project_id = sp.id
        WHERE pf.id = ? AND sp.user_id = ?
        "#,
        file_id,
        user.id
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("File not found or access denied"))?;

    let segment_label = segment_label
        .map(|label| label.trim().to_string())
        .filter(|label| !label.is_empty());
    let segment_ranges = segment_ranges.filter(|ranges| !ranges.is_empty());
    let segment_ranges_json = match segment_ranges {
        Some(ranges) => Some(
            serde_json::to_string(&ranges)
                .map_err(|e| ServerFnError::new(format!("Invalid segment ranges: {e}")))?,
        ),
        None => None,
    };

    // Create the job
    let result = sqlx::query(
        "INSERT INTO generation_jobs (deck_id, file_id, user_id, prompt_template, segment_label, segment_ranges, status) VALUES (?, ?, ?, ?, ?, ?, 'pending')"
    )
    .bind(deck_id)
    .bind(file_id)
    .bind(user.id)
    .bind(prompt_template)
    .bind(segment_label)
    .bind(segment_ranges_json)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let job_id = result.last_insert_rowid();

    println!(
        "✓ Created generation job {} for deck {} with file {}",
        job_id, deck_id, file_id
    );

    // Spawn background task to process the job
    let pool_clone = pool.clone();
    let app_state = expect_context::<crate::app_state::AppState>();
    let app_state_clone = app_state.clone();
    tokio::spawn(async move {
        println!("→ Starting background processing for job {}", job_id);
        if let Err(e) = process_generation_job(job_id, pool_clone, app_state_clone).await {
            eprintln!("✗ Error processing generation job {}: {}", job_id, e);
        }
    });

    // Fetch and return the created job
    let job = sqlx::query_as::<_, GenerationJob>(
        "SELECT id, deck_id, file_id, user_id, prompt_template, segment_label, segment_ranges, status, cards_generated, error_message, created_at, updated_at, completed_at FROM generation_jobs WHERE id = ?"
    )
    .bind(job_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(job)
}

#[server(GetGenerationJob)]
pub async fn get_generation_job(job_id: i64) -> Result<GenerationJob, ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();

    let job = sqlx::query_as::<_, GenerationJob>(
        "SELECT id, deck_id, file_id, user_id, prompt_template, segment_label, segment_ranges, status, cards_generated, error_message, created_at, updated_at, completed_at FROM generation_jobs WHERE id = ? AND user_id = ?"
    )
    .bind(job_id)
    .bind(user.id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("Job not found or access denied"))?;

    Ok(job)
}

#[server(ListGenerationJobsForDeck)]
pub async fn list_generation_jobs_for_deck(
    deck_id: i64,
) -> Result<Vec<GenerationJob>, ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();

    // Verify user owns the deck
    sqlx::query!(
        r#"
        SELECT fd.id
        FROM flashcard_decks fd
        INNER JOIN study_projects sp ON fd.project_id = sp.id
        WHERE fd.id = ? AND sp.user_id = ?
        "#,
        deck_id,
        user.id
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("Deck not found or access denied"))?;

    let jobs = sqlx::query_as::<_, GenerationJob>(
        "SELECT id, deck_id, file_id, user_id, prompt_template, segment_label, segment_ranges, status, cards_generated, error_message, created_at, updated_at, completed_at FROM generation_jobs WHERE deck_id = ? AND user_id = ? ORDER BY created_at DESC"
    )
    .bind(deck_id)
    .bind(user.id)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(jobs)
}

#[server(ListGenerationJobsWithFilesForDeck)]
pub async fn list_generation_jobs_with_files_for_deck(
    deck_id: i64,
) -> Result<Vec<GenerationJobWithFile>, ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();

    // Verify user owns the deck
    sqlx::query!(
        r#"
        SELECT fd.id
        FROM flashcard_decks fd
        INNER JOIN study_projects sp ON fd.project_id = sp.id
        WHERE fd.id = ? AND sp.user_id = ?
        "#,
        deck_id,
        user.id
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("Deck not found or access denied"))?;

    let rows = sqlx::query!(
        r#"
        SELECT
            gj.id as "id!: i64",
            gj.file_id as "file_id!: i64",
            pf.original_filename as "file_name!: String",
            gj.segment_label as "segment_label: String",
            gj.status as "status!: String",
            gj.cards_generated as "cards_generated!: i64",
            gj.error_message as "error_message: String",
            gj.created_at as "created_at!: String"
        FROM generation_jobs gj
        INNER JOIN project_files pf ON gj.file_id = pf.id
        WHERE gj.deck_id = ? AND gj.user_id = ?
        ORDER BY gj.created_at DESC
        "#,
        deck_id,
        user.id
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|row| GenerationJobWithFile {
            id: row.id,
            file_id: row.file_id,
            file_name: row.file_name,
            segment_label: row.segment_label,
            status: row.status,
            cards_generated: row.cards_generated,
            error_message: row.error_message,
            created_at: row.created_at,
        })
        .collect())
}

#[cfg(feature = "ssr")]
async fn process_generation_job(
    job_id: i64,
    pool: sqlx::SqlitePool,
    app_state: crate::app_state::AppState,
) -> Result<(), Box<dyn std::error::Error>> {
    use sqlx::Row;

    println!("📋 [Job {}] Starting processing", job_id);

    // Update status to processing
    sqlx::query(
        "UPDATE generation_jobs SET status = 'processing', updated_at = datetime('now') WHERE id = ?"
    )
    .bind(job_id)
    .execute(&pool)
    .await?;

    println!("📋 [Job {}] Status updated to 'processing'", job_id);

    // Fetch job details
    let job = sqlx::query(
        r#"
        SELECT gj.deck_id, gj.file_id, gj.prompt_template, gj.segment_label, gj.segment_ranges,
               fd.name as deck_name,
               pf.extracted_text, pf.original_filename, pf.s3_bucket, pf.s3_key
        FROM generation_jobs gj
        INNER JOIN flashcard_decks fd ON gj.deck_id = fd.id
        INNER JOIN project_files pf ON gj.file_id = pf.id
        WHERE gj.id = ?
        "#,
    )
    .bind(job_id)
    .fetch_one(&pool)
    .await?;

    let deck_name: String = job.try_get("deck_name")?;
    let file_name: String = job.try_get("original_filename")?;
    let deck_id: i64 = job.try_get("deck_id")?;
    let file_id: i64 = job.try_get("file_id")?;
    let prompt_template: Option<String> = job.try_get("prompt_template")?;
    let segment_label: Option<String> = job.try_get("segment_label")?;
    let segment_ranges: Option<String> = job.try_get("segment_ranges")?;

    let extracted_text = if let Some(segment_ranges) = segment_ranges {
        let ranges: Vec<crate::features::projects::models::SegmentRange> =
            serde_json::from_str(&segment_ranges)?;
        let s3_bucket: String = job.try_get("s3_bucket")?;
        let s3_key: String = job.try_get("s3_key")?;
        let (temp_path, pdf_size) = crate::features::projects::processing::download_pdf_to_temp(
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
        let extracted_text: Option<String> = job.try_get("extracted_text")?;
        extracted_text.ok_or("File has no extracted text")?
    };

    println!(
        "📋 [Job {}] Processing deck '{}' with file '{}'{} ({} chars of text)",
        job_id,
        deck_name,
        file_name,
        segment_label
            .as_ref()
            .map(|label| format!(" · Segment: {}", label))
            .unwrap_or_default(),
        extracted_text.len()
    );

    // Check word count limit
    let word_count = extracted_text.split_whitespace().count();
    let max_words = get_max_context_words();
    if word_count > max_words {
        let error_message = format!(
            "Document is too large for AI generation. Document has {} words, but the maximum is {} words. Please use a smaller document or split it into multiple files.",
            word_count, max_words
        );
        eprintln!("✗ [Job {}] {}", job_id, error_message);

        // Mark job as failed
        sqlx::query(
            "UPDATE generation_jobs SET status = 'failed', error_message = ?, updated_at = datetime('now'), completed_at = datetime('now') WHERE id = ?"
        )
        .bind(&error_message)
        .bind(job_id)
        .execute(&pool)
        .await?;

        return Err(error_message.into());
    }

    // Prepare the prompt
    let prompt_template = prompt_template.unwrap_or_else(|| DEFAULT_PROMPT_TEMPLATE.to_string());
    let prompt = prompt_template
        .replace("$DECK_TITLE$", &deck_name)
        .replace("$DOCUMENT_TEXT$", &extracted_text);

    println!(
        "📋 [Job {}] Prepared prompt ({} chars), calling LLM API...",
        job_id,
        prompt.len()
    );

    // Call LLM to generate flashcards
    match call_llm_for_flashcards(&prompt).await {
        Ok(generated_cards) => {
            println!(
                "✓ [Job {}] LLM returned {} flashcards",
                job_id,
                generated_cards.len()
            );

            // Insert all generated cards
            let mut cards_generated = 0;
            for (idx, card) in generated_cards.iter().enumerate() {
                sqlx::query!(
                    "INSERT INTO flashcards (deck_id, front, back, file_id) VALUES (?, ?, ?, ?)",
                    deck_id,
                    card.front,
                    card.back,
                    file_id
                )
                .execute(&pool)
                .await?;
                cards_generated += 1;

                if (idx + 1) % 10 == 0 {
                    println!(
                        "  [Job {}] Inserted {}/{} cards...",
                        job_id,
                        idx + 1,
                        generated_cards.len()
                    );
                }
            }

            println!(
                "✓ [Job {}] Inserted all {} flashcards into database",
                job_id, cards_generated
            );

            // Mark job as completed
            sqlx::query(
                "UPDATE generation_jobs SET status = 'completed', cards_generated = ?, updated_at = datetime('now'), completed_at = datetime('now') WHERE id = ?"
            )
            .bind(cards_generated)
            .bind(job_id)
            .execute(&pool)
            .await?;

            println!(
                "✓ [Job {}] Completed successfully - generated {} cards",
                job_id, cards_generated
            );
        }
        Err(e) => {
            eprintln!("✗ [Job {}] LLM API error: {}", job_id, e);

            // Mark job as failed
            let error_message = e.to_string();
            sqlx::query(
                "UPDATE generation_jobs SET status = 'failed', error_message = ?, updated_at = datetime('now'), completed_at = datetime('now') WHERE id = ?"
            )
            .bind(error_message)
            .bind(job_id)
            .execute(&pool)
            .await?;

            eprintln!("✗ [Job {}] Marked as failed", job_id);
        }
    }

    Ok(())
}

// ============= PROMPT MANAGEMENT =============

#[server(CreateGenerationPrompt)]
pub async fn create_generation_prompt(
    name: String,
    prompt_template: String,
    is_default: bool,
) -> Result<GenerationPrompt, ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let name = name.trim().to_string();
    let prompt_template = prompt_template.trim().to_string();

    if name.is_empty() {
        return Err(ServerFnError::new("Prompt name cannot be empty"));
    }

    if prompt_template.is_empty() {
        return Err(ServerFnError::new("Prompt template cannot be empty"));
    }

    let pool = expect_context::<SqlitePool>();

    // If this is set as default, unset all other defaults
    if is_default {
        sqlx::query!(
            "UPDATE generation_prompts SET is_default = 0 WHERE user_id = ?",
            user.id
        )
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    }

    let result = sqlx::query!(
        "INSERT INTO generation_prompts (user_id, name, prompt_template, is_default) VALUES (?, ?, ?, ?)",
        user.id,
        name,
        prompt_template,
        is_default
    )
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let prompt_id = result.last_insert_rowid();

    let prompt = sqlx::query_as!(
        GenerationPrompt,
        r#"
        SELECT id, user_id, name, prompt_template, is_default, created_at, updated_at
        FROM generation_prompts
        WHERE id = ?
        "#,
        prompt_id
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(prompt)
}

#[server(ListGenerationPrompts)]
pub async fn list_generation_prompts() -> Result<Vec<GenerationPrompt>, ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();

    let prompts = sqlx::query_as!(
        GenerationPrompt,
        r#"
        SELECT id as "id!", user_id as "user_id!", name, prompt_template, is_default, created_at, updated_at
        FROM generation_prompts
        WHERE user_id = ?
        ORDER BY is_default DESC, created_at DESC
        "#,
        user.id
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(prompts)
}

#[server(GetDefaultPrompt)]
pub async fn get_default_prompt() -> Result<Option<GenerationPrompt>, ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();

    let prompt = sqlx::query_as!(
        GenerationPrompt,
        r#"
        SELECT id as "id!", user_id as "user_id!", name, prompt_template, is_default, created_at, updated_at
        FROM generation_prompts
        WHERE user_id = ? AND is_default = 1
        "#,
        user.id
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(prompt)
}

#[server(DeleteGenerationPrompt)]
pub async fn delete_generation_prompt(prompt_id: i64) -> Result<(), ServerFnError> {
    use sqlx::SqlitePool;

    let user = require_auth().await?;
    let pool = expect_context::<SqlitePool>();

    let result = sqlx::query!(
        "DELETE FROM generation_prompts WHERE id = ? AND user_id = ?",
        prompt_id,
        user.id
    )
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err(ServerFnError::new("Prompt not found or access denied"));
    }

    Ok(())
}
