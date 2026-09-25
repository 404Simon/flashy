use axum::{
    Form, Json, Router,
    extract::{DefaultBodyLimit, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use tower_sessions::Session;
use url::Url;

use super::{
    models::{AuthorizationQuery, ConsentForm, RegistrationRequest, RevokeForm, TokenForm},
    service::OAuthError,
};
use crate::{app_state::AppState, features::auth::models::UserSession};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/.well-known/oauth-authorization-server",
            get(authorization_metadata),
        )
        .route(
            "/.well-known/oauth-protected-resource",
            get(resource_metadata),
        )
        .route(
            "/.well-known/oauth-protected-resource/mcp",
            get(resource_metadata),
        )
        .route("/oauth/register", post(register))
        .route("/oauth/authorize", get(authorize))
        .route("/oauth/consent/{request_id}", get(consent_page))
        .route("/oauth/consent", post(consent))
        .route("/oauth/token", post(token))
        .route("/oauth/revoke", post(revoke))
        .layer(DefaultBodyLimit::max(64 * 1024))
}

async fn authorization_metadata(State(state): State<AppState>) -> Json<serde_json::Value> {
    let c = state.oauth.config();
    Json(json!({
        "issuer": c.issuer.as_str(), "authorization_endpoint": c.endpoint("oauth/authorize"),
        "token_endpoint": c.endpoint("oauth/token"), "revocation_endpoint": c.endpoint("oauth/revoke"),
        "registration_endpoint": c.endpoint("oauth/register"),
        "response_types_supported": ["code"], "grant_types_supported": ["authorization_code", "refresh_token"],
        "token_endpoint_auth_methods_supported": ["none"], "code_challenge_methods_supported": ["S256"],
        "scopes_supported": ["flashy:read"], "client_id_metadata_document_supported": true
    }))
}

async fn resource_metadata(State(state): State<AppState>) -> Json<serde_json::Value> {
    let c = state.oauth.config();
    Json(
        json!({ "resource": c.resource, "authorization_servers": [c.issuer.as_str()], "scopes_supported": ["flashy:read"], "bearer_methods_supported": ["header"] }),
    )
}

async fn register(
    State(state): State<AppState>,
    Json(request): Json<RegistrationRequest>,
) -> Response {
    match state.oauth.register(request).await {
        Ok(client) => (StatusCode::CREATED, Json(client)).into_response(),
        Err(e) => oauth_error(e).into_response(),
    }
}

async fn authorize(
    State(state): State<AppState>,
    session: Session,
    Query(q): Query<AuthorizationQuery>,
) -> Response {
    if q.response_type != "code" || q.code_challenge_method != "S256" {
        return local_error("Unsupported authorization request");
    }
    if let Err(error) = state.oauth.resolve_client(&q.client_id).await {
        return local_oauth_error(error);
    }
    let binding = match browser_binding(&session).await {
        Ok(value) => value,
        Err(_) => return local_error("Could not create browser session"),
    };
    let request_id = match state
        .oauth
        .create_request(
            &binding,
            &q.client_id,
            &q.redirect_uri,
            &q.resource,
            &q.scope,
            q.state.as_deref(),
            &q.code_challenge,
        )
        .await
    {
        Ok(id) => id,
        Err(error) => return local_oauth_error(error),
    };
    let logged_in = session
        .get::<UserSession>("user")
        .await
        .ok()
        .flatten()
        .is_some();
    let target = if logged_in {
        format!("/oauth/consent/{request_id}")
    } else {
        format!("/login?oauth_request={request_id}")
    };
    Redirect::to(&target).into_response()
}

async fn consent_page(
    State(state): State<AppState>,
    session: Session,
    axum::extract::Path(request_id): axum::extract::Path<String>,
) -> Response {
    let Some(user) = session.get::<UserSession>("user").await.ok().flatten() else {
        return Redirect::to(&format!("/login?oauth_request={request_id}")).into_response();
    };
    let exists = sqlx::query_scalar::<_, i64>("SELECT 1 FROM users WHERE id = ?")
        .bind(user.id)
        .fetch_optional(&state.db_pool)
        .await
        .ok()
        .flatten()
        .is_some();
    if !exists {
        return local_error("Your account is no longer available");
    }
    let binding = match browser_binding(&session).await {
        Ok(value) => value,
        Err(_) => return local_error("Invalid browser session"),
    };
    let pending = match state.oauth.pending(&request_id, &binding).await {
        Ok(value) => value,
        Err(_) => return local_error("This authorization request is invalid or expired"),
    };
    let csrf = consent_token(&request_id, &binding);
    let callback_host = Url::parse(&pending.redirect_uri)
        .ok()
        .and_then(|u| u.host_str().map(str::to_owned))
        .unwrap_or_else(|| "unknown".into());
    // The Allow/Deny POST redirects to the client's loopback callback, which is
    // always a different origin (different host and/or ephemeral port) than this
    // page. Browsers enforce `form-action` across the redirect chain of a form
    // submission, so the callback origin must be listed or the handoff is blocked.
    let form_action_extra = form_action_origin(&pending.redirect_uri);
    let body = format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Authorize Flashy</title>
        <style>
        :root{{color-scheme:dark;font-family:Manrope,ui-sans-serif,system-ui,sans-serif}}
        *{{box-sizing:border-box}} body{{min-height:100vh;margin:0;padding:2rem 1rem;display:grid;place-items:center;background:#020617;color:#e2e8f0}}
        main{{width:min(100%,42rem);padding:2rem;border:1px solid #1e293b;border-radius:1.25rem;background:rgba(15,23,42,.88);box-shadow:0 24px 70px rgba(0,0,0,.35)}}
        .eyebrow{{margin:0 0 .75rem;color:#94a3b8;font-size:.75rem;font-weight:700;letter-spacing:.22em;text-transform:uppercase}}
        h1{{margin:0 0 1rem;color:#fff;font-size:clamp(1.75rem,5vw,2.25rem);line-height:1.15}} p{{color:#cbd5e1;line-height:1.65}}
        .scope{{margin:1.5rem 0;padding:1rem 1.125rem;border:1px solid #334155;border-radius:.875rem;background:#020617}}
        dl{{display:grid;grid-template-columns:auto 1fr;gap:.5rem 1rem;margin:1.5rem 0;font-size:.875rem}} dt{{color:#94a3b8}} dd{{min-width:0;margin:0;overflow-wrap:anywhere}} code{{color:#e2e8f0}}
        form{{display:flex;flex-wrap:wrap;gap:.75rem;margin-top:2rem}} button{{min-width:7rem;padding:.72rem 1.25rem;border-radius:999px;border:1px solid #475569;font:inherit;font-size:.875rem;font-weight:700;cursor:pointer}}
        .allow{{border-color:#fff;background:#fff;color:#0f172a}} .allow:hover{{background:#e2e8f0}} .deny{{background:transparent;color:#e2e8f0}} .deny:hover{{border-color:#94a3b8;background:#1e293b}}
        button:focus-visible{{outline:3px solid #38bdf8;outline-offset:3px}}
        @media(max-width:30rem){{main{{padding:1.5rem}} form{{display:grid}} button{{width:100%}} dl{{grid-template-columns:1fr;gap:.25rem}} dd{{margin-bottom:.5rem}}}}
        </style></head><body><main><p class="eyebrow">Flashy authorization</p><h1>Allow read access?</h1>
        <p><strong>{}</strong> wants to access Flashy as <strong>{}</strong>.</p>
        <p class="scope">This grants read-only access to your projects, filenames, document contents, decks, flashcards, and summaries. It cannot change study data or access another account.</p>
        <dl><dt>Client identifier</dt><dd><code>{}</code></dd><dt>Callback host</dt><dd><code>{}</code></dd></dl>
        <form method="post" action="/oauth/consent"><input type="hidden" name="request_id" value="{}"><input type="hidden" name="csrf_token" value="{}">
        <button class="allow" name="decision" value="allow" type="submit">Allow</button><button class="deny" name="decision" value="deny" type="submit">Deny</button></form></main></body></html>"#,
        escape(&pending.client_name),
        escape(&user.username),
        escape(&pending.client_id),
        escape(&callback_host),
        escape(&request_id),
        escape(&csrf)
    );
    secured_html_with_form_action(body, form_action_extra)
}

async fn consent(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<ConsentForm>,
) -> Response {
    let Some(user) = session.get::<UserSession>("user").await.ok().flatten() else {
        return local_error("Login required");
    };
    let binding = match browser_binding(&session).await {
        Ok(value) => value,
        Err(_) => return local_error("Invalid browser session"),
    };
    if consent_token(&form.request_id, &binding) != form.csrf_token {
        return local_error("Invalid consent submission");
    }
    let allow = form.decision == "allow";
    let (pending, code) = match state
        .oauth
        .decide(&form.request_id, &binding, user.id, allow)
        .await
    {
        Ok(value) => value,
        Err(error) => return local_oauth_error(error),
    };
    let mut redirect = match Url::parse(&pending.redirect_uri) {
        Ok(url) => url,
        Err(_) => return local_error("Invalid callback"),
    };
    let issuer = state.oauth.config().issuer.to_string();
    {
        let mut pairs = redirect.query_pairs_mut();
        if let Some(code) = code {
            pairs.append_pair("code", &code);
        } else {
            pairs.append_pair("error", "access_denied");
        }
        if let Some(oauth_state) = pending.state {
            pairs.append_pair("state", &oauth_state);
        }
        pairs.append_pair("iss", &issuer);
    }
    Redirect::to(redirect.as_str()).into_response()
}

async fn token(State(state): State<AppState>, Form(form): Form<TokenForm>) -> Response {
    match state.oauth.token(form).await {
        Ok(tokens) => (no_store_headers(), Json(tokens)).into_response(),
        Err(error) => oauth_error(error).into_response(),
    }
}

async fn revoke(State(state): State<AppState>, Form(form): Form<RevokeForm>) -> Response {
    match state.oauth.revoke(&form.token, &form.client_id).await {
        Ok(()) => StatusCode::OK.into_response(),
        Err(OAuthError::Internal(_)) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        Err(_) => StatusCode::OK.into_response(),
    }
}

fn oauth_error(error: OAuthError) -> (StatusCode, HeaderMap, Json<serde_json::Value>) {
    tracing::warn!(error = ?error, "OAuth request rejected");
    let (status, code) = match error {
        OAuthError::InvalidClient => (StatusCode::UNAUTHORIZED, "invalid_client"),
        OAuthError::InvalidGrant => (StatusCode::BAD_REQUEST, "invalid_grant"),
        OAuthError::InvalidScope => (StatusCode::BAD_REQUEST, "invalid_scope"),
        OAuthError::InvalidTarget => (StatusCode::BAD_REQUEST, "invalid_target"),
        OAuthError::Unavailable => (StatusCode::SERVICE_UNAVAILABLE, "temporarily_unavailable"),
        OAuthError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "server_error"),
        OAuthError::InvalidRequest => (StatusCode::BAD_REQUEST, "invalid_request"),
    };
    (status, no_store_headers(), Json(json!({"error":code})))
}

fn local_oauth_error(error: OAuthError) -> Response {
    local_error(match error {
        OAuthError::InvalidClient => "Unknown client",
        OAuthError::InvalidScope => "Unsupported scope",
        OAuthError::InvalidTarget => "Invalid resource",
        OAuthError::Unavailable => "Authorization service temporarily unavailable",
        _ => "Invalid authorization request",
    })
}
fn local_error(message: &str) -> Response {
    secured_html(format!(
        "<!doctype html><title>Authorization error</title><h1>Authorization error</h1><p>{}</p>",
        escape(message)
    ))
}
fn secured_html(body: String) -> Response {
    secured_html_with_form_action(body, None)
}
fn secured_html_with_form_action(body: String, form_action_extra: Option<String>) -> Response {
    let mut response = Html(body).into_response();
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    let policy = match form_action_extra {
        Some(origin) => format!(
            "default-src 'none'; style-src 'unsafe-inline'; form-action 'self' {origin}; frame-ancestors 'none'"
        ),
        None => "default-src 'none'; style-src 'unsafe-inline'; form-action 'self'; frame-ancestors 'none'"
            .to_owned(),
    };
    headers.insert(
        "content-security-policy",
        HeaderValue::from_str(&policy).expect("static CSP characters"),
    );
    response
}
/// Origin (`scheme://host[:port]`) of the OAuth client's redirect target, for
/// the consent page's `form-action` allowlist. Returns `None` unless the target
/// is an absolute HTTP(S) URL, so non-navigable schemes can never be injected.
fn form_action_origin(redirect_uri: &str) -> Option<String> {
    let url = Url::parse(redirect_uri).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let host = url.host_str()?;
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_owned()
    };
    Some(match url.port() {
        Some(port) => format!("{}://{host}:{port}", url.scheme()),
        None => format!("{}://{host}", url.scheme()),
    })
}
fn no_store_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    headers
}
async fn browser_binding(session: &Session) -> Result<String, ()> {
    if let Some(value) = session.get("oauth_browser_binding").await.map_err(|_| ())? {
        return Ok(value);
    }
    let value = random_value()?;
    session
        .insert("oauth_browser_binding", &value)
        .await
        .map_err(|_| ())?;
    Ok(value)
}
fn random_value() -> Result<String, ()> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| ())?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}
fn consent_token(request_id: &str, browser_binding: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(b"flashy-oauth-consent-v1\0");
    digest.update(browser_binding.as_bytes());
    digest.update(b"\0");
    digest.update(request_id.as_bytes());
    URL_SAFE_NO_PAD.encode(digest.finalize())
}
fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[allow(dead_code)]
fn _assert_json<T: Serialize>() {}

#[cfg(test)]
mod tests {
    use super::{consent_token, form_action_origin};

    #[test]
    fn consent_token_is_bound_to_request_and_browser_session() {
        let token = consent_token("request-a", "browser-a");

        assert_eq!(token, consent_token("request-a", "browser-a"));
        assert_ne!(token, consent_token("request-b", "browser-a"));
        assert_ne!(token, consent_token("request-a", "browser-b"));
    }

    #[test]
    fn form_action_origin_allows_loopback_callbacks_with_ephemeral_ports() {
        assert_eq!(
            form_action_origin("http://localhost:51234/callback?code=x").as_deref(),
            Some("http://localhost:51234")
        );
        assert_eq!(
            form_action_origin("http://127.0.0.1:3000/callback").as_deref(),
            Some("http://127.0.0.1:3000")
        );
        assert_eq!(
            form_action_origin("http://[::1]:8080/cb").as_deref(),
            Some("http://[::1]:8080")
        );
    }

    #[test]
    fn form_action_origin_rejects_non_navigable_targets() {
        assert_eq!(form_action_origin("not a url"), None);
        assert_eq!(form_action_origin("javascript:alert(1)"), None);
        assert_eq!(form_action_origin("data:text/html,hi"), None);
        assert_eq!(form_action_origin("ftp://example.com/cb"), None);
        assert_eq!(form_action_origin("/oauth/consent"), None);
    }
}
