#![cfg(feature = "ssr")]

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub const DEFAULT_PAGE_SIZE: u32 = 25;
pub const MAX_PAGE_SIZE: u32 = 100;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Actor {
    user_id: i64,
}

impl Actor {
    pub fn new(user_id: i64) -> Result<Self, ServiceError> {
        (user_id > 0)
            .then_some(Self { user_id })
            .ok_or(ServiceError::InvalidInput("invalid user"))
    }

    pub fn user_id(self) -> i64 {
        self.user_id
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error("invalid input: {0}")]
    InvalidInput(&'static str),
    #[error("not found")]
    NotFound,
    #[error("conflict: {0}")]
    Conflict(&'static str),
    #[error("service unavailable")]
    Unavailable,
    #[error("internal error")]
    Internal(#[source] sqlx::Error),
}

impl From<sqlx::Error> for ServiceError {
    fn from(error: sqlx::Error) -> Self {
        Self::Internal(error)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct PageRequest {
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}

impl PageRequest {
    pub fn limit(&self) -> Result<u32, ServiceError> {
        let limit = self.limit.unwrap_or(DEFAULT_PAGE_SIZE);
        if limit == 0 || limit > MAX_PAGE_SIZE {
            return Err(ServiceError::InvalidInput(
                "limit must be between 1 and 100",
            ));
        }
        Ok(limit)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
}

pub(crate) fn encode_cursor<T: Serialize>(cursor: &T) -> Result<String, ServiceError> {
    let bytes = serde_json::to_vec(cursor).map_err(|_| ServiceError::InvalidInput("cursor"))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

pub(crate) fn decode_cursor<T: DeserializeOwned>(
    cursor: Option<&str>,
) -> Result<Option<T>, ServiceError> {
    let Some(cursor) = cursor else {
        return Ok(None);
    };
    if cursor.len() > 512 {
        return Err(ServiceError::InvalidInput("cursor"));
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(cursor)
        .map_err(|_| ServiceError::InvalidInput("cursor"))?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| ServiceError::InvalidInput("cursor"))
}

pub fn to_server_error(error: ServiceError) -> leptos::prelude::ServerFnError {
    tracing::warn!(error = ?error, "service request failed");
    let message = match error {
        ServiceError::InvalidInput(message) => message,
        ServiceError::NotFound => "Not found",
        ServiceError::Conflict(message) => message,
        ServiceError::Unavailable => "Service unavailable",
        ServiceError::Internal(_) => "Request failed",
    };
    leptos::prelude::ServerFnError::new(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Deserialize, PartialEq, Serialize)]
    struct TestCursor {
        collection: String,
        id: i64,
    }

    #[test]
    fn cursor_round_trip_is_opaque_and_bounded() {
        let expected = TestCursor {
            collection: "projects".into(),
            id: 42,
        };
        let encoded = encode_cursor(&expected).unwrap();
        assert!(!encoded.contains("projects"));
        assert_eq!(decode_cursor(Some(&encoded)).unwrap(), Some(expected));
        assert!(decode_cursor::<TestCursor>(Some(&"a".repeat(513))).is_err());
    }

    #[test]
    fn page_size_policy_is_explicit() {
        assert_eq!(
            PageRequest {
                cursor: None,
                limit: None
            }
            .limit()
            .unwrap(),
            25
        );
        assert!(
            PageRequest {
                cursor: None,
                limit: Some(0)
            }
            .limit()
            .is_err()
        );
        assert!(
            PageRequest {
                cursor: None,
                limit: Some(101)
            }
            .limit()
            .is_err()
        );
    }
}

#[cfg(test)]
mod isolation_tests {
    use super::*;
    use crate::features::{
        flashcards::service::FlashcardService,
        projects::{files_service::FileService, service::ProjectService},
        summaries::service::SummaryService,
    };
    use sqlx::sqlite::SqlitePoolOptions;

    async fn fixture() -> sqlx::SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        for statement in [
            "CREATE TABLE users(id INTEGER PRIMARY KEY)",
            "CREATE TABLE study_projects(id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL, name TEXT NOT NULL, description TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL)",
            "CREATE TABLE project_files(id INTEGER PRIMARY KEY, project_id INTEGER NOT NULL, original_filename TEXT NOT NULL, file_size INTEGER NOT NULL, processing_status TEXT NOT NULL, extracted_text TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL)",
            "CREATE TABLE flashcard_decks(id INTEGER PRIMARY KEY, project_id INTEGER NOT NULL, name TEXT NOT NULL, description TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL)",
            "CREATE TABLE flashcards(id INTEGER PRIMARY KEY, deck_id INTEGER NOT NULL, front TEXT NOT NULL, back TEXT NOT NULL, document_reference TEXT, file_id INTEGER, created_at TEXT NOT NULL, updated_at TEXT NOT NULL)",
            "CREATE TABLE summaries(id INTEGER PRIMARY KEY, project_id INTEGER NOT NULL, title TEXT NOT NULL, description TEXT, content_markdown TEXT NOT NULL, file_id INTEGER, segment_label TEXT, status TEXT NOT NULL, error_message TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL)",
            "INSERT INTO users VALUES (1), (2)",
            "INSERT INTO study_projects VALUES (10,1,'Mine',NULL,'2026','2026'), (20,2,'Theirs',NULL,'2026','2026')",
            "INSERT INTO project_files VALUES (11,10,'mine.pdf',10,'completed','αβγ','2026','r1'), (21,20,'secret.pdf',10,'completed','secret','2026','r1')",
            "INSERT INTO flashcard_decks VALUES (12,10,'Mine',NULL,'2026','2026'), (22,20,'Theirs',NULL,'2026','2026')",
            "INSERT INTO flashcards VALUES (13,12,'front','back',NULL,21,'2026','2026'), (23,22,'secret','secret',NULL,21,'2026','2026')",
            "INSERT INTO summaries VALUES (14,10,'Mine',NULL,'content',21,NULL,'completed',NULL,'2026','r1'), (24,20,'Theirs',NULL,'secret',21,NULL,'completed',NULL,'2026','r1')",
        ] {
            sqlx::query(statement).execute(&pool).await.unwrap();
        }
        pool
    }

    #[tokio::test]
    async fn every_read_service_hides_foreign_entities_and_associations() {
        let pool = fixture().await;
        let actor = Actor::new(1).unwrap();
        let projects = ProjectService::new(pool.clone());
        assert_eq!(projects.get(&actor, 10).await.unwrap().name, "Mine");
        assert!(matches!(
            projects.get(&actor, 20).await,
            Err(ServiceError::NotFound)
        ));

        let files = FileService::new(pool.clone());
        assert_eq!(files.full_text(&actor, 11).await.unwrap(), "αβγ");
        assert!(matches!(
            files.full_text(&actor, 21).await,
            Err(ServiceError::NotFound)
        ));

        let cards = FlashcardService::new(pool.clone());
        assert!(cards.get_card(&actor, 13).await.unwrap().file_id.is_none());
        assert!(matches!(
            cards.get_card(&actor, 23).await,
            Err(ServiceError::NotFound)
        ));
        assert!(matches!(
            cards.get_deck(&actor, 22).await,
            Err(ServiceError::NotFound)
        ));

        let summaries = SummaryService::new(pool);
        assert!(summaries.get(&actor, 14).await.unwrap().file_id.is_none());
        assert!(matches!(
            summaries.get(&actor, 24).await,
            Err(ServiceError::NotFound)
        ));
    }
}
