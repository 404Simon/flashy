/// Centralized application configuration with default values
#[cfg(feature = "ssr")]
use std::sync::OnceLock;

// Default values as constants that can be used on both client and server
pub const DEFAULT_MAX_UPLOAD_MB: usize = 100;
pub const DEFAULT_MAX_PDF_MB: u64 = 50;
pub const DEFAULT_MAX_CONTEXT_WORDS: usize = 12_000;

/// Application configuration loaded from environment variables (SSR only)
#[cfg(feature = "ssr")]
#[derive(Debug, Clone)]
pub struct Config {
    /// Maximum size for HTTP uploads in bytes
    pub max_upload_bytes: usize,
    /// Maximum size for a single PDF file in bytes
    pub max_pdf_bytes: u64,
    /// Maximum number of words to send to AI for flashcard generation
    pub max_context_words: usize,
}

#[cfg(feature = "ssr")]
impl Default for Config {
    fn default() -> Self {
        Self {
            max_upload_bytes: DEFAULT_MAX_UPLOAD_MB * 1024 * 1024,
            max_pdf_bytes: DEFAULT_MAX_PDF_MB * 1024 * 1024,
            max_context_words: DEFAULT_MAX_CONTEXT_WORDS,
        }
    }
}

#[cfg(feature = "ssr")]
impl Config {
    /// Load configuration from environment variables with defaults
    pub fn from_env() -> Self {
        let max_upload_mb: usize = std::env::var("MAX_UPLOAD_SIZE_MB")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_MAX_UPLOAD_MB);

        let max_pdf_mb: u64 = std::env::var("MAX_PDF_SIZE_MB")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_MAX_PDF_MB);

        let max_context_words: usize = std::env::var("MAX_CONTEXT_WORDS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_MAX_CONTEXT_WORDS);

        Self {
            max_upload_bytes: max_upload_mb * 1024 * 1024,
            max_pdf_bytes: max_pdf_mb * 1024 * 1024,
            max_context_words,
        }
    }

    /// Get the global configuration instance
    pub fn global() -> &'static Config {
        static CONFIG: OnceLock<Config> = OnceLock::new();
        CONFIG.get_or_init(Config::from_env)
    }
}
