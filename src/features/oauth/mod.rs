#![cfg(feature = "ssr")]

pub mod config;
mod engine;
pub mod handlers;
pub mod models;
pub mod service;

pub use config::OAuthConfig;
pub use handlers::router;
pub use service::OAuthService;
