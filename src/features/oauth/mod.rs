#![cfg(feature = "ssr")]

mod client;
pub mod config;
mod engine;
pub mod handlers;
pub mod models;
mod rate_limit;
pub mod service;

pub use config::OAuthConfig;
pub use handlers::router;
pub use service::OAuthService;
