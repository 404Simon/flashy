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
        .and_then(bearer_token);
    let Some(token) = token else {
        return unauthorized(&state);
    };
    let permit = match tokio::time::timeout(
        Duration::from_secs(2),
        concurrency_limit().clone().acquire_owned(),
    )
    .await
    {
        Ok(Ok(permit)) => permit,
        Ok(Err(_)) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
        Err(_) => return (StatusCode::TOO_MANY_REQUESTS, "MCP server is busy").into_response(),
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
    let response = tokio::time::timeout(Duration::from_secs(30), next.run(request)).await;
    drop(permit);
    let mut response = match response {
        Ok(response) => response,
        Err(_) => (StatusCode::GATEWAY_TIMEOUT, "MCP request timed out").into_response(),
    };
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn bearer_token(value: &str) -> Option<&str> {
    let (scheme, token) = value.split_once(' ')?;
    (!token.is_empty() && scheme.eq_ignore_ascii_case("Bearer")).then_some(token)
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

#[cfg(test)]
mod tests {
    use super::bearer_token;

    #[test]
    fn bearer_scheme_is_case_insensitive() {
        assert_eq!(bearer_token("Bearer token"), Some("token"));
        assert_eq!(bearer_token("bearer token"), Some("token"));
        assert_eq!(bearer_token("BEARER token"), Some("token"));
        assert_eq!(bearer_token("Basic token"), None);
        assert_eq!(bearer_token("Bearer "), None);
    }
}
