use crate::{
    app_state::AppState,
    features::oauth::models::{McpPrincipal, READ_SCOPE},
};
use axum::{
    extract::{Request, State},
    http::{HeaderValue, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};

pub async fn require_bearer(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let token = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    let Some(token) = token else {
        return unauthorized(&state);
    };
    let principal: McpPrincipal = match state.oauth.authenticate(token).await {
        Ok(value) => value,
        Err(_) => return unauthorized(&state),
    };
    if !principal.scopes.iter().any(|scope| scope == READ_SCOPE) {
        return insufficient_scope(&state);
    }
    tracing::debug!(user_id=principal.user_id, client_id=%principal.client_id, grant_id=%principal.grant_id, "authenticated MCP request");
    request.extensions_mut().insert(principal);
    let permit = match concurrency_limit().clone().acquire_owned().await {
        Ok(permit) => permit,
        Err(_) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };
    let response = tokio::time::timeout(Duration::from_secs(30), next.run(request)).await;
    drop(permit);
    match response {
        Ok(response) => response,
        Err(_) => (StatusCode::GATEWAY_TIMEOUT, "MCP request timed out").into_response(),
    }
}

fn concurrency_limit() -> &'static Arc<tokio::sync::Semaphore> {
    static LIMIT: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
    LIMIT.get_or_init(|| Arc::new(tokio::sync::Semaphore::new(32)))
}

fn insufficient_scope(state: &AppState) -> Response {
    let metadata = state
        .oauth
        .config()
        .endpoint(".well-known/oauth-protected-resource/mcp");
    let mut response = (StatusCode::FORBIDDEN, "Insufficient OAuth scope").into_response();
    let value = format!(
        "Bearer error=\"insufficient_scope\", scope=\"{READ_SCOPE}\", resource_metadata=\"{metadata}\""
    );
    if let Ok(value) = HeaderValue::from_str(&value) {
        response
            .headers_mut()
            .insert(header::WWW_AUTHENTICATE, value);
    }
    response
}

fn unauthorized(state: &AppState) -> Response {
    let metadata = state
        .oauth
        .config()
        .endpoint(".well-known/oauth-protected-resource/mcp");
    let mut response = (StatusCode::UNAUTHORIZED, "Bearer token required").into_response();
    let value = format!("Bearer resource_metadata=\"{metadata}\"");
    if let Ok(value) = HeaderValue::from_str(&value) {
        response
            .headers_mut()
            .insert(header::WWW_AUTHENTICATE, value);
    }
    response
}
