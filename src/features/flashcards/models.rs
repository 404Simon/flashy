use serde::{Deserialize, Serialize};
#[cfg(feature = "ssr")]
use sqlx::FromRow;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ssr", derive(FromRow))]
pub struct FlashcardDeck {
    pub id: i64,
    pub project_id: i64,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ssr", derive(FromRow))]
pub struct DeckSummary {
    pub id: i64,
    pub project_id: i64,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
    pub card_count: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ssr", derive(FromRow))]
pub struct Flashcard {
    pub id: i64,
    pub deck_id: i64,
    pub front: String,
    pub back: String,
    pub document_reference: Option<String>,
    pub file_id: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateFlashcardRequest {
    pub front: String,
    pub back: String,
    pub document_reference: Option<String>,
    pub file_id: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ssr", derive(FromRow))]
pub struct GenerationPrompt {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub prompt_template: String,
    pub is_default: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ssr", derive(FromRow))]
pub struct GenerationJob {
    pub id: i64,
    pub deck_id: i64,
    pub file_id: i64,
    pub user_id: i64,
    pub prompt_template: Option<String>,
    pub segment_label: Option<String>,
    pub segment_ranges: Option<String>,
    pub status: String, // pending, processing, completed, failed
    pub cards_generated: i64,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileCardGroup {
    pub file_id: i64,
    pub file_name: String,
    pub card_count: i64,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenerationJobWithFile {
    pub id: i64,
    pub file_id: i64,
    pub file_name: String,
    pub segment_label: Option<String>,
    pub status: String,
    pub cards_generated: i64,
    pub error_message: Option<String>,
    pub created_at: String,
}
