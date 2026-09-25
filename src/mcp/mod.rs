#![cfg(feature = "ssr")]

mod auth;
mod dto;
mod server;

use crate::app_state::AppState;
use axum::{Router, middleware};
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use server::FlashyMcpServer;

pub fn router(state: &AppState) -> Router<AppState> {
    let host = state
        .oauth
        .config()
        .issuer
        .host_str()
        .unwrap_or("localhost")
        .to_owned();
    let config = StreamableHttpServerConfig::default()
        .with_allowed_hosts([host, "localhost".into(), "127.0.0.1".into(), "::1".into()])
        .enforce_origin_validation()
        .with_max_request_body_bytes(64 * 1024)
        .with_legacy_session_mode(false);
    let pool = state.db_pool.clone();
    let service: StreamableHttpService<FlashyMcpServer, LocalSessionManager> =
        StreamableHttpService::new(
            move || Ok(FlashyMcpServer::new(pool.clone())),
            Default::default(),
            config,
        );
    Router::new()
        .route_service("/mcp", service)
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_bearer,
        ))
}
