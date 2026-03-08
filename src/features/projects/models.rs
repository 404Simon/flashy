use serde::{Deserialize, Serialize};
#[cfg(feature = "ssr")]
use sqlx::FromRow;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ssr", derive(FromRow))]
pub struct Project {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ssr", derive(FromRow))]
pub struct ProjectSummary {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
    pub file_count: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ssr", derive(FromRow))]
pub struct ProjectFile {
    pub id: i64,
    pub project_id: i64,
    pub original_filename: String,
    pub file_size: i64,
    pub processing_status: String,
    pub created_at: String,
    pub text_preview: Option<String>,
    pub word_count: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SegmentRange {
    pub start_page: i64,
    pub end_page: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PdfTocEntry {
    pub id: String,
    pub title: String,
    pub level: i64,
    pub start_page: i64,
    pub end_page: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PdfOutlineResponse {
    pub total_pages: i64,
    pub entries: Vec<PdfTocEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SegmentStats {
    pub page_count: i64,
    pub word_count: i64,
}
