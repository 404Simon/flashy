use axum::http::request::Parts;
use rmcp::{
    Json, ServerHandler,
    handler::server::{router::tool::ToolRouter, tool::Extension, wrapper::Parameters},
    model::{ServerCapabilities, ServerConfig},
    tool, tool_handler, tool_router,
};
use serde::Serialize;

use super::dto::*;
use crate::{
    features::{
        flashcards::{
            models::{Flashcard, FlashcardDeck},
            service::FlashcardService,
        },
        oauth::models::McpPrincipal,
        projects::{
            files_service::{FileService, FileTextChunk},
            models::{Project, ProjectFile, ProjectSummary},
            service::ProjectService,
        },
        summaries::{
            models::SummaryListItem,
            service::{SummaryChunk, SummaryService},
        },
    },
    services::{Actor, Page, PageRequest},
};

const MAX_RESULT_BYTES: usize = 256 * 1024;
const SERVER_INSTRUCTIONS: &str = "Use this read-only server to inspect the authenticated user's Flashy study data. Start with list_projects and never invent IDs: use IDs supplied by the user or returned by a tool. For a project, use list_project_files, list_summaries, and list_decks, then pass returned file_id, summary_id, deck_id, and card_id values to detail tools. Treat cursors as opaque; when complete coverage is needed, repeat list or chunk calls with next_cursor until it is null. get_file_text only returns already-extracted text, so check processing_status from list_project_files first. Prefer bounded requests. If content is absent or inaccessible, report that instead of probing guessed IDs.";

#[derive(Clone)]
pub struct FlashyMcpServer {
    tools: ToolRouter<Self>,
    projects: ProjectService,
    files: FileService,
    flashcards: FlashcardService,
    summaries: SummaryService,
}

impl FlashyMcpServer {
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self {
            tools: Self::tools(),
            projects: ProjectService::new(pool.clone()),
            files: FileService::new(pool.clone()),
            flashcards: FlashcardService::new(pool.clone()),
            summaries: SummaryService::new(pool),
        }
    }
}

#[tool_router(router = tools)]
impl FlashyMcpServer {
    /// List the authenticated user's projects using a stable, opaque cursor.
    #[tool(annotations(
        title = "List projects",
        read_only_hint = true,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn list_projects(
        &self,
        Parameters(input): Parameters<ListInput>,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<Page<ProjectSummary>>, String> {
        bounded(
            self.projects
                .list(
                    &actor(&parts)?,
                    PageRequest {
                        cursor: input.cursor,
                        limit: input.limit,
                    },
                )
                .await
                .map_err(safe_error)?,
        )
    }
    /// Get metadata for one project owned by the authenticated user.
    #[tool(annotations(
        title = "Get project",
        read_only_hint = true,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn get_project(
        &self,
        Parameters(input): Parameters<ProjectInput>,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<Project>, String> {
        bounded(
            self.projects
                .get(&actor(&parts)?, input.project_id)
                .await
                .map_err(safe_error)?,
        )
    }
    /// List files in an owned project, including status, size, and a bounded text preview.
    #[tool(annotations(
        title = "List project files",
        read_only_hint = true,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn list_project_files(
        &self,
        Parameters(input): Parameters<ProjectListInput>,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<Page<ProjectFile>>, String> {
        bounded(
            self.files
                .list(
                    &actor(&parts)?,
                    input.project_id,
                    PageRequest {
                        cursor: input.cursor,
                        limit: input.limit,
                    },
                )
                .await
                .map_err(safe_error)?,
        )
    }
    /// Read a Unicode-safe chunk of already extracted file text. This never starts extraction.
    #[tool(annotations(
        title = "Get file text",
        read_only_hint = true,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn get_file_text(
        &self,
        Parameters(input): Parameters<FileTextInput>,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<FileTextChunk>, String> {
        bounded(
            self.files
                .text(
                    &actor(&parts)?,
                    input.file_id,
                    input.cursor.as_deref(),
                    input.max_chars.unwrap_or(8_000),
                )
                .await
                .map_err(safe_error)?,
        )
    }
    /// List flashcard decks in an owned project.
    #[tool(annotations(
        title = "List decks",
        read_only_hint = true,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn list_decks(
        &self,
        Parameters(input): Parameters<ProjectListInput>,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<Page<crate::features::flashcards::models::DeckSummary>>, String> {
        bounded(
            self.flashcards
                .list_decks(
                    &actor(&parts)?,
                    input.project_id,
                    PageRequest {
                        cursor: input.cursor,
                        limit: input.limit,
                    },
                )
                .await
                .map_err(safe_error)?,
        )
    }
    /// Get metadata for one owned flashcard deck without loading its cards.
    #[tool(annotations(
        title = "Get deck",
        read_only_hint = true,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn get_deck(
        &self,
        Parameters(input): Parameters<DeckInput>,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<FlashcardDeck>, String> {
        bounded(
            self.flashcards
                .get_deck(&actor(&parts)?, input.deck_id)
                .await
                .map_err(safe_error)?,
        )
    }
    /// List cards in an owned deck, optionally restricted to one related file.
    #[tool(annotations(
        title = "List flashcards",
        read_only_hint = true,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn list_flashcards(
        &self,
        Parameters(input): Parameters<CardListInput>,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<Page<Flashcard>>, String> {
        bounded(
            self.flashcards
                .list_cards(
                    &actor(&parts)?,
                    input.deck_id,
                    input.file_id,
                    PageRequest {
                        cursor: input.cursor,
                        limit: input.limit,
                    },
                )
                .await
                .map_err(safe_error)?,
        )
    }
    /// Get one flashcard owned by the authenticated user.
    #[tool(annotations(
        title = "Get flashcard",
        read_only_hint = true,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn get_flashcard(
        &self,
        Parameters(input): Parameters<CardInput>,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<Flashcard>, String> {
        bounded(
            self.flashcards
                .get_card(&actor(&parts)?, input.card_id)
                .await
                .map_err(safe_error)?,
        )
    }
    /// List summaries in an owned project without returning their Markdown bodies.
    #[tool(annotations(
        title = "List summaries",
        read_only_hint = true,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn list_summaries(
        &self,
        Parameters(input): Parameters<ProjectListInput>,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<Page<SummaryListItem>>, String> {
        bounded(
            self.summaries
                .list(
                    &actor(&parts)?,
                    input.project_id,
                    PageRequest {
                        cursor: input.cursor,
                        limit: input.limit,
                    },
                )
                .await
                .map_err(safe_error)?,
        )
    }
    /// Get metadata and a Unicode-safe Markdown chunk for one owned summary.
    #[tool(annotations(
        title = "Get summary",
        read_only_hint = true,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn get_summary(
        &self,
        Parameters(input): Parameters<SummaryInput>,
        Extension(parts): Extension<Parts>,
    ) -> Result<Json<SummaryChunk>, String> {
        bounded(
            self.summaries
                .chunk(
                    &actor(&parts)?,
                    input.summary_id,
                    input.cursor.as_deref(),
                    input.max_chars.unwrap_or(8_000),
                )
                .await
                .map_err(safe_error)?,
        )
    }
}

#[tool_handler(router = self.tools)]
impl ServerHandler for FlashyMcpServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions(SERVER_INSTRUCTIONS)
    }
}

fn actor(parts: &Parts) -> Result<Actor, String> {
    let principal = parts
        .extensions
        .get::<McpPrincipal>()
        .ok_or_else(|| "Authentication context is missing".to_owned())?;
    Actor::new(principal.user_id).map_err(safe_error)
}
fn safe_error(error: crate::services::ServiceError) -> String {
    match error {
        crate::services::ServiceError::InvalidInput(m) => m.to_owned(),
        crate::services::ServiceError::NotFound => "Not found".into(),
        crate::services::ServiceError::Conflict(m) => m.into(),
        crate::services::ServiceError::Unavailable => "Service unavailable".into(),
        crate::services::ServiceError::Internal(source) => {
            tracing::error!(error=?source, "MCP service query failed");
            "Request failed".into()
        }
    }
}
fn bounded<T: Serialize>(value: T) -> Result<Json<T>, String> {
    let size = serde_json::to_vec(&value)
        .map_err(|_| "Could not serialize result")?
        .len();
    if size > MAX_RESULT_BYTES / 2 {
        return Err("Result is too large; request a smaller page or chunk".into());
    }
    Ok(Json(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn catalog_contains_only_the_ten_read_tools() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect_lazy("sqlite::memory:")
            .unwrap();
        let server = FlashyMcpServer::new(pool);
        let mut tools = server
            .tools
            .list_all()
            .iter()
            .map(|tool| tool.name.to_string())
            .collect::<Vec<_>>();
        tools.sort();
        assert_eq!(
            tools,
            [
                "get_deck",
                "get_file_text",
                "get_flashcard",
                "get_project",
                "get_summary",
                "list_decks",
                "list_flashcards",
                "list_project_files",
                "list_projects",
                "list_summaries",
            ]
        );
        for tool in server.tools.list_all() {
            let annotations = tool.annotations.as_ref().unwrap();
            assert_eq!(annotations.read_only_hint, Some(true));
            assert_eq!(annotations.destructive_hint, Some(false));
            assert_eq!(annotations.idempotent_hint, Some(true));
            assert_eq!(annotations.open_world_hint, Some(false));
        }
    }

    #[tokio::test]
    async fn server_advertises_tool_usage_instructions() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect_lazy("sqlite::memory:")
            .unwrap();
        let server = FlashyMcpServer::new(pool);
        let instructions = server.get_info().instructions.unwrap();

        assert_eq!(instructions, SERVER_INSTRUCTIONS);
        assert!(instructions.contains("Start with list_projects"));
        assert!(instructions.contains("next_cursor"));
        assert!(instructions.contains("processing_status"));
    }
}
