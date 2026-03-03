use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub max_context_words: usize,
    pub max_pdf_mb: u64,
    pub max_upload_mb: usize,
}

#[server(GetAppConfig)]
pub async fn get_app_config() -> Result<AppConfig, ServerFnError> {
    use crate::config::Config;

    let config = Config::global();
    Ok(AppConfig {
        max_context_words: config.max_context_words,
        max_pdf_mb: config.max_pdf_bytes / 1024 / 1024,
        max_upload_mb: config.max_upload_bytes / 1024 / 1024,
    })
}
