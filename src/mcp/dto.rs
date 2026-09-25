use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListInput {
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProjectInput {
    pub project_id: i64,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProjectListInput {
    pub project_id: i64,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FileTextInput {
    pub file_id: i64,
    pub cursor: Option<String>,
    pub max_chars: Option<u32>,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeckInput {
    pub deck_id: i64,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CardListInput {
    pub deck_id: i64,
    pub file_id: Option<i64>,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CardInput {
    pub card_id: i64,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SummaryInput {
    pub summary_id: i64,
    pub cursor: Option<String>,
    pub max_chars: Option<u32>,
}
