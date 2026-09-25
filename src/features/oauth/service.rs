use std::{net::IpAddr, time::Duration};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use getrandom::fill;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use time::OffsetDateTime;
use url::Url;

use super::{
    config::OAuthConfig,
    engine::{encode_s256_challenge, verify_s256},
    models::{
        McpPrincipal, READ_SCOPE, RegistrationRequest, RegistrationResponse, TokenForm,
        TokenResponse,
    },
};
use crate::features::auth::models::ConnectedApplication;

const REQUEST_TTL: i64 = 600;
const CODE_TTL: i64 = 60;
const ACCESS_TTL: i64 = 900;
const REFRESH_IDLE_TTL: i64 = 7 * 86_400;
const GRANT_TTL: i64 = 30 * 86_400;

#[derive(Debug, thiserror::Error)]
pub enum OAuthError {
    #[error("invalid_request")]
    InvalidRequest,
    #[error("invalid_client")]
    InvalidClient,
    #[error("invalid_grant")]
    InvalidGrant,
    #[error("invalid_scope")]
    InvalidScope,
    #[error("invalid_target")]
    InvalidTarget,
    #[error("temporarily_unavailable")]
    Unavailable,
    #[error("server_error")]
    Internal(#[source] sqlx::Error),
}

impl From<sqlx::Error> for OAuthError {
    fn from(value: sqlx::Error) -> Self {
        Self::Internal(value)
    }
}

#[derive(Clone)]
pub struct OAuthService {
    pool: SqlitePool,
    config: OAuthConfig,
}

pub struct PendingAuthorization {
    pub client_id: String,
    pub client_name: String,
    pub redirect_uri: String,
    pub resource: String,
    pub state: Option<String>,
    pub code_challenge: String,
}

impl OAuthService {
    pub fn new(pool: SqlitePool, config: OAuthConfig) -> Self {
        Self { pool, config }
    }
    pub fn config(&self) -> &OAuthConfig {
        &self.config
    }

    pub async fn register(
        &self,
        request: RegistrationRequest,
    ) -> Result<RegistrationResponse, OAuthError> {
        let name = request.client_name.trim();
        if name.is_empty()
            || name.len() > 120
            || request.redirect_uris.is_empty()
            || request.redirect_uris.len() > 5
        {
            return Err(OAuthError::InvalidRequest);
        }
        if request
            .token_endpoint_auth_method
            .as_deref()
            .is_some_and(|m| m != "none")
            || (!request.grant_types.is_empty()
                && request
                    .grant_types
                    .iter()
                    .any(|g| g != "authorization_code" && g != "refresh_token"))
            || (!request.response_types.is_empty()
                && request.response_types.iter().any(|r| r != "code"))
        {
            return Err(OAuthError::InvalidRequest);
        }
        for uri in &request.redirect_uris {
            validate_native_redirect(uri)?;
        }
        let client_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM oauth_clients")
            .fetch_one(&self.pool)
            .await?;
        if client_count >= 10_000 {
            return Err(OAuthError::Unavailable);
        }
        let recent_registrations: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM oauth_clients WHERE registration_source = 'dcr' AND created_at > ?",
        )
        .bind(now() - 60)
        .fetch_one(&self.pool)
        .await?;
        if recent_registrations >= 60 {
            return Err(OAuthError::Unavailable);
        }
        let client_id = random_credential()?;
        let now = now();
        sqlx::query("INSERT INTO oauth_clients (client_id, registration_source, display_name, redirect_uris, created_at) VALUES (?, 'dcr', ?, ?, ?)")
            .bind(&client_id).bind(name).bind(serde_json::to_string(&request.redirect_uris).map_err(|_| OAuthError::InvalidRequest)?).bind(now).execute(&self.pool).await?;
        Ok(RegistrationResponse {
            client_id,
            client_name: name.to_owned(),
            redirect_uris: request.redirect_uris,
            token_endpoint_auth_method: "none",
            grant_types: ["authorization_code", "refresh_token"],
            response_types: ["code"],
        })
    }

    pub async fn resolve_client(&self, client_id: &str) -> Result<(), OAuthError> {
        if sqlx::query_as::<_, (String, Option<i64>)>(
            "SELECT registration_source, metadata_expires_at FROM oauth_clients WHERE client_id = ?",
        )
            .bind(client_id)
            .fetch_optional(&self.pool)
            .await?
            .is_some_and(|(source, expiry)| source != "cimd" || expiry.is_some_and(|value| value > now()))
        {
            return Ok(());
        }
        self.resolve_cimd(client_id).await
    }

    async fn resolve_cimd(&self, client_id: &str) -> Result<(), OAuthError> {
        #[derive(Deserialize)]
        struct Metadata {
            client_id: String,
            client_name: String,
            redirect_uris: Vec<String>,
            token_endpoint_auth_method: Option<String>,
        }
        let url = Url::parse(client_id).map_err(|_| OAuthError::InvalidClient)?;
        let host = url.host_str().ok_or(OAuthError::InvalidClient)?;
        if url.scheme() != "https"
            || !self.config.trusted_metadata_hosts.contains(host)
            || url.fragment().is_some()
            || url.username() != ""
            || url.password().is_some()
        {
            return Err(OAuthError::InvalidClient);
        }
        let port = url
            .port_or_known_default()
            .ok_or(OAuthError::InvalidClient)?;
        let addresses: Vec<_> = tokio::net::lookup_host((host, port))
            .await
            .map_err(|_| OAuthError::Unavailable)?
            .filter(|a| is_public(a.ip()))
            .collect();
        if addresses.is_empty() {
            return Err(OAuthError::InvalidClient);
        }
        let mut builder = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(5));
        for address in addresses {
            builder = builder.resolve(host, address);
        }
        let response = builder
            .build()
            .map_err(|_| OAuthError::Unavailable)?
            .get(url)
            .send()
            .await
            .map_err(|_| OAuthError::Unavailable)?;
        if !response.status().is_success()
            || response.content_length().is_some_and(|n| n > 32 * 1024)
        {
            return Err(OAuthError::InvalidClient);
        }
        let cache_ttl = response
            .headers()
            .get(reqwest::header::CACHE_CONTROL)
            .and_then(|value| value.to_str().ok())
            .and_then(cache_max_age)
            .unwrap_or(300)
            .min(3_600);
        let bytes = response
            .bytes()
            .await
            .map_err(|_| OAuthError::Unavailable)?;
        if bytes.len() > 32 * 1024 {
            return Err(OAuthError::InvalidClient);
        }
        let metadata: Metadata =
            serde_json::from_slice(&bytes).map_err(|_| OAuthError::InvalidClient)?;
        if metadata.client_id != client_id
            || metadata.client_name.trim().is_empty()
            || metadata.client_name.len() > 120
            || metadata.redirect_uris.is_empty()
            || metadata.redirect_uris.len() > 10
            || metadata
                .token_endpoint_auth_method
                .as_deref()
                .is_some_and(|m| m != "none")
        {
            return Err(OAuthError::InvalidClient);
        }
        for uri in &metadata.redirect_uris {
            validate_native_redirect(uri)?;
        }
        let timestamp = now();
        sqlx::query(r#"INSERT INTO oauth_clients (client_id, registration_source, display_name, redirect_uris, created_at, metadata_expires_at)
            VALUES (?, 'cimd', ?, ?, ?, ?)
            ON CONFLICT(client_id) DO UPDATE SET display_name = excluded.display_name,
                redirect_uris = excluded.redirect_uris, metadata_expires_at = excluded.metadata_expires_at"#)
            .bind(client_id).bind(metadata.client_name).bind(serde_json::to_string(&metadata.redirect_uris).map_err(|_| OAuthError::InvalidClient)?).bind(timestamp).bind(timestamp + cache_ttl).execute(&self.pool).await?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_request(
        &self,
        browser_binding: &str,
        client_id: &str,
        redirect_uri: &str,
        resource: &str,
        scope: &str,
        state: Option<&str>,
        challenge: &str,
    ) -> Result<String, OAuthError> {
        if resource != self.config.resource {
            return Err(OAuthError::InvalidTarget);
        }
        if scope != READ_SCOPE {
            return Err(OAuthError::InvalidScope);
        }
        if challenge.len() < 43
            || challenge.len() > 128
            || !challenge
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~'))
        {
            return Err(OAuthError::InvalidRequest);
        }
        let encoded_challenge =
            encode_s256_challenge(challenge).map_err(|_| OAuthError::InvalidRequest)?;
        let redirects: String =
            sqlx::query_scalar("SELECT redirect_uris FROM oauth_clients WHERE client_id = ?")
                .bind(client_id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or(OAuthError::InvalidClient)?;
        let redirects: Vec<String> =
            serde_json::from_str(&redirects).map_err(|_| OAuthError::InvalidClient)?;
        if !redirects
            .iter()
            .any(|registered| redirect_matches(registered, redirect_uri))
        {
            return Err(OAuthError::InvalidRequest);
        }
        let request_id = random_credential()?;
        let timestamp = now();
        sqlx::query(r#"INSERT INTO oauth_authorization_requests
            (request_id_hash, browser_binding_hash, client_id, redirect_uri, resource, scopes, state, code_challenge, created_at, expires_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#)
            .bind(hash(&request_id)).bind(hash(browser_binding)).bind(client_id).bind(redirect_uri).bind(resource).bind(scope).bind(state).bind(encoded_challenge).bind(timestamp).bind(timestamp + REQUEST_TTL).execute(&self.pool).await?;
        sqlx::query("UPDATE oauth_clients SET last_used_at = ? WHERE client_id = ?")
            .bind(timestamp)
            .bind(client_id)
            .execute(&self.pool)
            .await?;
        Ok(request_id)
    }

    pub async fn pending(
        &self,
        request_id: &str,
        browser_binding: &str,
    ) -> Result<PendingAuthorization, OAuthError> {
        let row = sqlx::query(r#"SELECT r.client_id, c.display_name, r.redirect_uri, r.resource, r.state, r.code_challenge
            FROM oauth_authorization_requests r JOIN oauth_clients c ON c.client_id = r.client_id
            WHERE r.request_id_hash = ? AND r.browser_binding_hash = ? AND r.expires_at > ? AND r.consumed_at IS NULL"#)
            .bind(hash(request_id)).bind(hash(browser_binding)).bind(now()).fetch_optional(&self.pool).await?.ok_or(OAuthError::InvalidRequest)?;
        Ok(PendingAuthorization {
            client_id: row.get(0),
            client_name: row.get(1),
            redirect_uri: row.get(2),
            resource: row.get(3),
            state: row.get(4),
            code_challenge: row.get(5),
        })
    }

    pub async fn decide(
        &self,
        request_id: &str,
        browser_binding: &str,
        user_id: i64,
        allow: bool,
    ) -> Result<(PendingAuthorization, Option<String>), OAuthError> {
        let pending = self.pending(request_id, browser_binding).await?;
        let mut tx = self.pool.begin().await?;
        let timestamp = now();
        let consumed = sqlx::query("UPDATE oauth_authorization_requests SET consumed_at = ? WHERE request_id_hash = ? AND browser_binding_hash = ? AND consumed_at IS NULL AND expires_at > ?")
            .bind(timestamp).bind(hash(request_id)).bind(hash(browser_binding)).bind(timestamp).execute(&mut *tx).await?;
        if consumed.rows_affected() != 1 {
            return Err(OAuthError::InvalidRequest);
        }
        if !allow {
            tx.commit().await?;
            return Ok((pending, None));
        }
        let user_exists = sqlx::query_scalar::<_, i64>("SELECT 1 FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?
            .is_some();
        if !user_exists {
            return Err(OAuthError::InvalidGrant);
        }
        let grant_id = random_credential()?;
        let code = random_credential()?;
        sqlx::query("INSERT INTO oauth_grants (id, user_id, client_id, resource, scopes, created_at, absolute_expires_at, last_refresh_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(&grant_id).bind(user_id).bind(&pending.client_id).bind(&pending.resource).bind(READ_SCOPE).bind(timestamp).bind(timestamp + GRANT_TTL).bind(timestamp).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO oauth_authorization_codes (code_hash, grant_id, redirect_uri, code_challenge, resource, expires_at) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(hash(&code)).bind(grant_id).bind(&pending.redirect_uri).bind(&pending.code_challenge).bind(&pending.resource).bind(timestamp + CODE_TTL).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok((pending, Some(code)))
    }

    pub async fn token(&self, form: TokenForm) -> Result<TokenResponse, OAuthError> {
        if form.scope.as_deref().is_some_and(|s| s != READ_SCOPE) {
            return Err(OAuthError::InvalidScope);
        }
        match form.grant_type.as_str() {
            "authorization_code" => self.exchange_code(&form).await,
            "refresh_token" => self.refresh(&form).await,
            _ => Err(OAuthError::InvalidRequest),
        }
    }

    async fn exchange_code(&self, form: &TokenForm) -> Result<TokenResponse, OAuthError> {
        let code = form.code.as_deref().ok_or(OAuthError::InvalidRequest)?;
        let redirect = form
            .redirect_uri
            .as_deref()
            .ok_or(OAuthError::InvalidRequest)?;
        let verifier = form
            .code_verifier
            .as_deref()
            .ok_or(OAuthError::InvalidRequest)?;
        if verifier.len() < 43 || verifier.len() > 128 {
            return Err(OAuthError::InvalidGrant);
        }
        let mut tx = self.pool.begin().await?;
        let timestamp = now();
        let row = sqlx::query(
            r#"SELECT c.grant_id, c.redirect_uri, c.code_challenge, c.resource, g.client_id,
                c.consumed_at, c.expires_at, g.revoked_at
            FROM oauth_authorization_codes c JOIN oauth_grants g ON g.id = c.grant_id
            WHERE c.code_hash = ?"#,
        )
        .bind(hash(code))
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(OAuthError::InvalidGrant)?;
        let grant_id: String = row.get(0);
        let expected: String = row.get(2);
        let resource: String = row.get(3);
        let client: String = row.get(4);
        if row.get::<Option<i64>, _>(5).is_some() {
            sqlx::query(
                "UPDATE oauth_grants SET revoked_at = COALESCE(revoked_at, ?) WHERE id = ?",
            )
            .bind(timestamp)
            .bind(&grant_id)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            return Err(OAuthError::InvalidGrant);
        }
        if row.get::<i64, _>(6) <= timestamp
            || row.get::<Option<i64>, _>(7).is_some()
            || client != form.client_id
            || row.get::<String, _>(1) != redirect
            || form.resource.as_deref() != Some(resource.as_str())
            || verify_s256(expected, verifier).is_err()
        {
            return Err(OAuthError::InvalidGrant);
        }
        let result = sqlx::query("UPDATE oauth_authorization_codes SET consumed_at = ? WHERE code_hash = ? AND consumed_at IS NULL").bind(timestamp).bind(hash(code)).execute(&mut *tx).await?;
        if result.rows_affected() != 1 {
            return Err(OAuthError::InvalidGrant);
        }
        let response = issue_tokens(&mut tx, &grant_id, timestamp).await?;
        tx.commit().await?;
        Ok(response)
    }

    async fn refresh(&self, form: &TokenForm) -> Result<TokenResponse, OAuthError> {
        let token = form
            .refresh_token
            .as_deref()
            .ok_or(OAuthError::InvalidRequest)?;
        let digest = hash(token);
        let mut tx = self.pool.begin().await?;
        let timestamp = now();
        let row = sqlx::query(r#"SELECT rt.grant_id, rt.expires_at, rt.consumed_at, g.client_id, g.resource, g.absolute_expires_at, g.last_refresh_at, g.revoked_at
            FROM oauth_refresh_tokens rt JOIN oauth_grants g ON g.id = rt.grant_id WHERE rt.token_hash = ?"#).bind(&digest).fetch_optional(&mut *tx).await?.ok_or(OAuthError::InvalidGrant)?;
        let grant_id: String = row.get(0);
        if row.get::<Option<i64>, _>(2).is_some() {
            sqlx::query(
                "UPDATE oauth_grants SET revoked_at = COALESCE(revoked_at, ?) WHERE id = ?",
            )
            .bind(timestamp)
            .bind(&grant_id)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            return Err(OAuthError::InvalidGrant);
        }
        let resource: String = row.get(4);
        if row.get::<String, _>(3) != form.client_id
            || form.resource.as_deref().is_some_and(|r| r != resource)
            || row.get::<i64, _>(1) <= timestamp
            || row.get::<i64, _>(5) <= timestamp
            || row.get::<i64, _>(6) + REFRESH_IDLE_TTL <= timestamp
            || row.get::<Option<i64>, _>(7).is_some()
        {
            return Err(OAuthError::InvalidGrant);
        }
        let next_refresh = random_credential()?;
        let next_digest = hash(&next_refresh);
        let consumed = sqlx::query("UPDATE oauth_refresh_tokens SET consumed_at = ?, successor_hash = ? WHERE token_hash = ? AND consumed_at IS NULL").bind(timestamp).bind(&next_digest).bind(&digest).execute(&mut *tx).await?;
        if consumed.rows_affected() != 1 {
            sqlx::query(
                "UPDATE oauth_grants SET revoked_at = COALESCE(revoked_at, ?) WHERE id = ?",
            )
            .bind(timestamp)
            .bind(&grant_id)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            return Err(OAuthError::InvalidGrant);
        }
        let access = random_credential()?;
        sqlx::query(
            "INSERT INTO oauth_access_tokens (token_hash, grant_id, expires_at) VALUES (?, ?, ?)",
        )
        .bind(hash(&access))
        .bind(&grant_id)
        .bind(timestamp + ACCESS_TTL)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO oauth_refresh_tokens (token_hash, grant_id, expires_at) VALUES (?, ?, ?)",
        )
        .bind(&next_digest)
        .bind(&grant_id)
        .bind(timestamp + REFRESH_IDLE_TTL)
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE oauth_grants SET last_refresh_at = ? WHERE id = ?")
            .bind(timestamp)
            .bind(&grant_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(TokenResponse {
            access_token: access,
            token_type: "Bearer",
            expires_in: ACCESS_TTL,
            refresh_token: next_refresh,
            scope: READ_SCOPE,
        })
    }

    pub async fn authenticate(&self, token: &str) -> Result<McpPrincipal, OAuthError> {
        let row = sqlx::query(r#"SELECT g.user_id, g.client_id, g.id, g.resource, g.scopes
            FROM oauth_access_tokens at JOIN oauth_grants g ON g.id = at.grant_id JOIN users u ON u.id = g.user_id
            WHERE at.token_hash = ? AND at.expires_at > ? AND g.revoked_at IS NULL AND g.absolute_expires_at > ?"#)
            .bind(hash(token)).bind(now()).bind(now()).fetch_optional(&self.pool).await?.ok_or(OAuthError::InvalidGrant)?;
        if row.get::<String, _>(3) != self.config.resource {
            return Err(OAuthError::InvalidGrant);
        }
        let scopes = row
            .get::<String, _>(4)
            .split_ascii_whitespace()
            .map(str::to_owned)
            .collect();
        Ok(McpPrincipal {
            user_id: row.get(0),
            client_id: row.get(1),
            grant_id: row.get(2),
            scopes,
        })
    }

    pub async fn revoke(&self, token: &str, client_id: &str) -> Result<(), OAuthError> {
        let timestamp = now();
        sqlx::query(r#"UPDATE oauth_grants SET revoked_at = COALESCE(revoked_at, ?) WHERE client_id = ? AND id IN (
            SELECT grant_id FROM oauth_access_tokens WHERE token_hash = ? UNION SELECT grant_id FROM oauth_refresh_tokens WHERE token_hash = ?)"#)
            .bind(timestamp).bind(client_id).bind(hash(token)).bind(hash(token)).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn connected_apps(
        &self,
        user_id: i64,
    ) -> Result<Vec<ConnectedApplication>, OAuthError> {
        let rows = sqlx::query(r#"SELECT g.client_id, c.display_name, MIN(g.created_at) FROM oauth_grants g JOIN oauth_clients c ON c.client_id = g.client_id
            WHERE g.user_id = ? AND g.revoked_at IS NULL AND g.absolute_expires_at > ? GROUP BY g.client_id, c.display_name ORDER BY MIN(g.created_at) DESC"#).bind(user_id).bind(now()).fetch_all(&self.pool).await?;
        Ok(rows
            .into_iter()
            .map(|r| ConnectedApplication {
                client_id: r.get(0),
                display_name: r.get(1),
                authorized_at: r.get(2),
            })
            .collect())
    }

    pub async fn revoke_client(&self, user_id: i64, client_id: &str) -> Result<(), OAuthError> {
        sqlx::query("UPDATE oauth_grants SET revoked_at = COALESCE(revoked_at, ?) WHERE user_id = ? AND client_id = ?").bind(now()).bind(user_id).bind(client_id).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn revoke_user(&self, user_id: i64) -> Result<(), OAuthError> {
        sqlx::query(
            "UPDATE oauth_grants SET revoked_at = COALESCE(revoked_at, ?) WHERE user_id = ?",
        )
        .bind(now())
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn cleanup(&self) -> Result<(), OAuthError> {
        let timestamp = now();
        sqlx::query("DELETE FROM oauth_authorization_requests WHERE expires_at < ?")
            .bind(timestamp - 86_400)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM oauth_access_tokens WHERE expires_at < ?")
            .bind(timestamp)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM oauth_clients WHERE registration_source = 'dcr' AND last_used_at IS NULL AND created_at < ? AND NOT EXISTS (SELECT 1 FROM oauth_grants g WHERE g.client_id = oauth_clients.client_id)").bind(timestamp - 86_400).execute(&self.pool).await?;
        Ok(())
    }
}

async fn issue_tokens(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    grant_id: &str,
    timestamp: i64,
) -> Result<TokenResponse, OAuthError> {
    let access = random_credential()?;
    let refresh = random_credential()?;
    sqlx::query(
        "INSERT INTO oauth_access_tokens (token_hash, grant_id, expires_at) VALUES (?, ?, ?)",
    )
    .bind(hash(&access))
    .bind(grant_id)
    .bind(timestamp + ACCESS_TTL)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "INSERT INTO oauth_refresh_tokens (token_hash, grant_id, expires_at) VALUES (?, ?, ?)",
    )
    .bind(hash(&refresh))
    .bind(grant_id)
    .bind(timestamp + REFRESH_IDLE_TTL)
    .execute(&mut **tx)
    .await?;
    Ok(TokenResponse {
        access_token: access,
        token_type: "Bearer",
        expires_in: ACCESS_TTL,
        refresh_token: refresh,
        scope: READ_SCOPE,
    })
}

fn random_credential() -> Result<String, OAuthError> {
    let mut bytes = [0u8; 32];
    fill(&mut bytes).map_err(|_| OAuthError::Unavailable)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}
fn hash(value: &str) -> Vec<u8> {
    Sha256::digest(value.as_bytes()).to_vec()
}
fn now() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp()
}

fn validate_native_redirect(raw: &str) -> Result<(), OAuthError> {
    let url = Url::parse(raw).map_err(|_| OAuthError::InvalidRequest)?;
    if url.fragment().is_some() || url.username() != "" || url.password().is_some() {
        return Err(OAuthError::InvalidRequest);
    }
    let host = url.host_str().ok_or(OAuthError::InvalidRequest)?;
    let loopback = is_loopback_host(host);
    if url.scheme() != "http" || !loopback {
        return Err(OAuthError::InvalidRequest);
    }
    Ok(())
}

fn redirect_matches(registered: &str, actual: &str) -> bool {
    if registered == actual {
        return true;
    }
    let (Ok(a), Ok(b)) = (Url::parse(registered), Url::parse(actual)) else {
        return false;
    };
    let loopback = a.host_str().is_some_and(is_loopback_host);
    loopback
        && a.scheme() == b.scheme()
        && a.host_str() == b.host_str()
        && a.path() == b.path()
        && a.query() == b.query()
}

fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !(ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_unspecified()
                || ip.is_broadcast()
                || ip.is_documentation()
                || ip.is_multicast()
                || (a == 100 && (64..=127).contains(&b))
                || (a == 192 && b == 0 && c == 0)
                || (a == 198 && matches!(b, 18 | 19))
                || a >= 240)
        }
        IpAddr::V6(ip) => {
            !(ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_unique_local()
                || ip.is_unicast_link_local()
                || ip.is_multicast()
                || (ip.segments()[0] == 0x2001 && ip.segments()[1] == 0x0db8)
                || ip
                    .to_ipv4_mapped()
                    .is_some_and(|v4| !is_public(IpAddr::V4(v4))))
        }
    }
}

fn is_loopback_host(host: &str) -> bool {
    host == "localhost"
        || host
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
}

fn cache_max_age(value: &str) -> Option<i64> {
    value.split(',').map(str::trim).find_map(|directive| {
        directive
            .strip_prefix("max-age=")
            .and_then(|seconds| seconds.parse::<i64>().ok())
            .filter(|seconds| *seconds >= 0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn loopback_redirect_allows_only_port_variation() {
        let registered = "http://127.0.0.1:1234/oauth/callback?source=codex";
        assert!(redirect_matches(
            registered,
            "http://127.0.0.1:49152/oauth/callback?source=codex"
        ));
        assert!(!redirect_matches(
            registered,
            "http://localhost:49152/oauth/callback?source=codex"
        ));
        assert!(!redirect_matches(
            registered,
            "http://127.0.0.1:49152/other?source=codex"
        ));
        assert!(!redirect_matches(
            registered,
            "http://127.0.0.1:49152/oauth/callback?source=other"
        ));
    }

    #[test]
    fn dcr_redirects_must_be_native_loopback_urls() {
        assert!(validate_native_redirect("http://[::1]:9876/callback").is_ok());
        assert!(validate_native_redirect("http://localhost/callback").is_ok());
        assert!(validate_native_redirect("https://example.com/callback").is_err());
        assert!(validate_native_redirect("http://127.0.0.1/callback#fragment").is_err());
    }

    #[test]
    fn cimd_network_filter_rejects_non_public_addresses() {
        assert!(!is_public("127.0.0.1".parse().unwrap()));
        assert!(!is_public("10.0.0.1".parse().unwrap()));
        assert!(!is_public("::1".parse().unwrap()));
        assert!(!is_public("::ffff:127.0.0.1".parse().unwrap()));
        assert!(!is_public("100.64.0.1".parse().unwrap()));
        assert!(!is_public("2001:db8::1".parse().unwrap()));
        assert!(is_public("1.1.1.1".parse().unwrap()));
        assert_eq!(cache_max_age("public, max-age=900"), Some(900));
        assert_eq!(cache_max_age("no-store"), None);
    }

    #[tokio::test]
    async fn authorization_refresh_replay_and_restart_are_persistent() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO users (id, username, password_hash) VALUES (1, 'alice', 'unused')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let config = OAuthConfig {
            enabled: true,
            issuer: Url::parse("http://127.0.0.1:3000").unwrap(),
            resource: "http://127.0.0.1:3000/mcp".into(),
            trusted_metadata_hosts: HashSet::new(),
        };
        let service = OAuthService::new(pool.clone(), config.clone());
        let registration = service
            .register(RegistrationRequest {
                client_name: "Test CLI".into(),
                redirect_uris: vec!["http://127.0.0.1:1234/callback".into()],
                token_endpoint_auth_method: Some("none".into()),
                grant_types: vec!["authorization_code".into(), "refresh_token".into()],
                response_types: vec!["code".into()],
            })
            .await
            .unwrap();
        let verifier = "a".repeat(43);
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let request_id = service
            .create_request(
                "browser",
                &registration.client_id,
                "http://127.0.0.1:54321/callback",
                &config.resource,
                READ_SCOPE,
                Some("state"),
                &challenge,
            )
            .await
            .unwrap();
        let (_, code) = service
            .decide(&request_id, "browser", 1, true)
            .await
            .unwrap();
        let restarted = OAuthService::new(pool, config);
        let first = restarted
            .token(TokenForm {
                grant_type: "authorization_code".into(),
                client_id: registration.client_id.clone(),
                code,
                redirect_uri: Some("http://127.0.0.1:54321/callback".into()),
                code_verifier: Some(verifier),
                refresh_token: None,
                resource: Some("http://127.0.0.1:3000/mcp".into()),
                scope: None,
            })
            .await
            .unwrap();
        assert_eq!(
            restarted
                .authenticate(&first.access_token)
                .await
                .unwrap()
                .user_id,
            1
        );
        let refreshed = restarted
            .token(TokenForm {
                grant_type: "refresh_token".into(),
                client_id: registration.client_id.clone(),
                code: None,
                redirect_uri: None,
                code_verifier: None,
                refresh_token: Some(first.refresh_token.clone()),
                resource: None,
                scope: None,
            })
            .await
            .unwrap();
        assert!(matches!(
            restarted
                .token(TokenForm {
                    grant_type: "refresh_token".into(),
                    client_id: registration.client_id,
                    code: None,
                    redirect_uri: None,
                    code_verifier: None,
                    refresh_token: Some(first.refresh_token),
                    resource: None,
                    scope: None,
                })
                .await,
            Err(OAuthError::InvalidGrant)
        ));
        assert!(matches!(
            restarted.authenticate(&refreshed.access_token).await,
            Err(OAuthError::InvalidGrant)
        ));
    }
}
