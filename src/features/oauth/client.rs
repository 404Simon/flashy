use std::{net::IpAddr, time::Duration};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;
use sqlx::SqlitePool;
use time::OffsetDateTime;
use url::Url;

use super::{
    config::OAuthConfig,
    models::{RegistrationRequest, RegistrationResponse},
    service::OAuthError,
};

pub(super) async fn register(
    pool: &SqlitePool,
    request: RegistrationRequest,
) -> Result<RegistrationResponse, OAuthError> {
    let name = request.client_name.trim();
    if name.is_empty()
        || name.len() > 120
        || request.redirect_uris.is_empty()
        || request.redirect_uris.len() > 5
        || request
            .token_endpoint_auth_method
            .as_deref()
            .is_some_and(|method| method != "none")
        || (!request.grant_types.is_empty()
            && request
                .grant_types
                .iter()
                .any(|grant| grant != "authorization_code" && grant != "refresh_token"))
        || (!request.response_types.is_empty()
            && request
                .response_types
                .iter()
                .any(|response| response != "code"))
    {
        return Err(OAuthError::InvalidRequest);
    }
    for uri in &request.redirect_uris {
        if uri.len() > 2_048 {
            return Err(OAuthError::InvalidRequest);
        }
        validate_native_redirect(uri)?;
    }
    let client_id = random_client_id()?;
    let timestamp = now();
    sqlx::query(
        "INSERT INTO oauth_clients (client_id, registration_source, display_name, redirect_uris, created_at) VALUES (?, 'dcr', ?, ?, ?)",
    )
    .bind(&client_id)
    .bind(name)
    .bind(
        serde_json::to_string(&request.redirect_uris)
            .map_err(|_| OAuthError::InvalidRequest)?,
    )
    .bind(timestamp)
    .execute(pool)
    .await?;
    Ok(RegistrationResponse {
        client_id,
        client_name: name.to_owned(),
        redirect_uris: request.redirect_uris,
        token_endpoint_auth_method: "none",
        grant_types: ["authorization_code", "refresh_token"],
        response_types: ["code"],
    })
}

pub(super) async fn resolve(
    pool: &SqlitePool,
    config: &OAuthConfig,
    client_id: &str,
) -> Result<(), OAuthError> {
    if client_id.is_empty() || client_id.len() > 2_048 {
        return Err(OAuthError::InvalidClient);
    }
    if sqlx::query_as::<_, (String, Option<i64>)>(
        "SELECT registration_source, metadata_expires_at FROM oauth_clients WHERE client_id = ?",
    )
    .bind(client_id)
    .fetch_optional(pool)
    .await?
    .is_some_and(|(source, expiry)| source != "cimd" || expiry.is_some_and(|value| value > now()))
    {
        return Ok(());
    }
    resolve_cimd(pool, config, client_id).await
}

async fn resolve_cimd(
    pool: &SqlitePool,
    config: &OAuthConfig,
    client_id: &str,
) -> Result<(), OAuthError> {
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
        || !config.trusted_metadata_hosts.contains(host)
        || url.fragment().is_some()
        || !url.username().is_empty()
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
        .filter(|address| is_public(address.ip()))
        .collect();
    if addresses.is_empty() {
        return Err(OAuthError::InvalidClient);
    }
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(5))
        .resolve_to_addrs(host, &addresses)
        .build()
        .map_err(|_| OAuthError::Unavailable)?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|_| OAuthError::Unavailable)?;
    if !response.status().is_success()
        || response
            .content_length()
            .is_some_and(|size| size > 32 * 1024)
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
            .is_some_and(|method| method != "none")
    {
        return Err(OAuthError::InvalidClient);
    }
    for uri in &metadata.redirect_uris {
        if uri.len() > 2_048 {
            return Err(OAuthError::InvalidClient);
        }
        validate_native_redirect(uri).map_err(|_| OAuthError::InvalidClient)?;
    }
    let timestamp = now();
    sqlx::query(
        r#"INSERT INTO oauth_clients (client_id, registration_source, display_name, redirect_uris, created_at, metadata_expires_at)
           VALUES (?, 'cimd', ?, ?, ?, ?)
           ON CONFLICT(client_id) DO UPDATE SET display_name = excluded.display_name,
               redirect_uris = excluded.redirect_uris, metadata_expires_at = excluded.metadata_expires_at"#,
    )
    .bind(client_id)
    .bind(metadata.client_name)
    .bind(
        serde_json::to_string(&metadata.redirect_uris)
            .map_err(|_| OAuthError::InvalidClient)?,
    )
    .bind(timestamp)
    .bind(timestamp + cache_ttl)
    .execute(pool)
    .await?;
    Ok(())
}

pub(super) fn redirect_matches(registered: &str, actual: &str) -> bool {
    if registered == actual {
        return true;
    }
    let (Ok(registered), Ok(actual)) = (Url::parse(registered), Url::parse(actual)) else {
        return false;
    };
    registered.host_str().is_some_and(is_loopback_host)
        && registered.scheme() == actual.scheme()
        && registered.host_str() == actual.host_str()
        && registered.path() == actual.path()
        && registered.query() == actual.query()
        && actual.fragment().is_none()
        && actual.username().is_empty()
        && actual.password().is_none()
}

fn validate_native_redirect(raw: &str) -> Result<(), OAuthError> {
    let url = Url::parse(raw).map_err(|_| OAuthError::InvalidRequest)?;
    if url.fragment().is_some() || !url.username().is_empty() || url.password().is_some() {
        return Err(OAuthError::InvalidRequest);
    }
    let host = url.host_str().ok_or(OAuthError::InvalidRequest)?;
    if url.scheme() != "http" || !is_loopback_host(host) {
        return Err(OAuthError::InvalidRequest);
    }
    Ok(())
}

fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !(a == 0
                || ip.is_private()
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
            let value = u128::from(ip);
            // Globally routed unicast space is currently 2000::/3. Keeping the
            // allow-list narrow avoids SSRF bypasses through mapped, translated,
            // site-local, documentation, discard-only, or future-use ranges.
            if !in_v6_prefix(value, 0x2000_u128 << 112, 3)
                || in_v6_prefix(value, 0x2001_u128 << 112, 23)
                || in_v6_prefix(value, 0x2001_0db8_u128 << 96, 32)
                || in_v6_prefix(value, 0x3fff_u128 << 112, 20)
            {
                return false;
            }

            // 6to4 embeds the destination IPv4 address. Reject it when that
            // embedded address would not itself pass the public-address policy.
            if in_v6_prefix(value, 0x2002_u128 << 112, 16) {
                let embedded = std::net::Ipv4Addr::from((value >> 80) as u32);
                return is_public(IpAddr::V4(embedded));
            }
            true
        }
    }
}

fn in_v6_prefix(address: u128, network: u128, prefix_len: u32) -> bool {
    let mask = u128::MAX << (128 - prefix_len);
    address & mask == network & mask
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

fn random_client_id() -> Result<String, OAuthError> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| OAuthError::Unavailable)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn now() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp()
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(!is_public("0.1.2.3".parse().unwrap()));
        assert!(!is_public("127.0.0.1".parse().unwrap()));
        assert!(!is_public("10.0.0.1".parse().unwrap()));
        assert!(!is_public("::1".parse().unwrap()));
        assert!(!is_public("::ffff:127.0.0.1".parse().unwrap()));
        assert!(!is_public("100.64.0.1".parse().unwrap()));
        assert!(!is_public("fec0::1".parse().unwrap()));
        assert!(!is_public("2001::1".parse().unwrap()));
        assert!(!is_public("2001:db8::1".parse().unwrap()));
        assert!(!is_public("2002:7f00:0001::1".parse().unwrap()));
        assert!(is_public("1.1.1.1".parse().unwrap()));
        assert!(is_public("2606:4700:4700::1111".parse().unwrap()));
        assert!(is_public("2002:0101:0101::1".parse().unwrap()));
        assert_eq!(cache_max_age("public, max-age=900"), Some(900));
        assert_eq!(cache_max_age("no-store"), None);
    }
}
