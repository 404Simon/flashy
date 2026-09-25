use std::collections::HashSet;
use url::Url;

#[derive(Clone, Debug)]
pub struct OAuthConfig {
    pub enabled: bool,
    pub issuer: Url,
    pub resource: String,
    pub trusted_metadata_hosts: HashSet<String>,
}

impl OAuthConfig {
    pub fn from_env() -> Result<Self, String> {
        let enabled = std::env::var("MCP_ENABLED")
            .is_ok_and(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes"));
        let raw = std::env::var("FLASHY_PUBLIC_ORIGIN")
            .unwrap_or_else(|_| "http://127.0.0.1:3000".into());
        let mut issuer =
            Url::parse(&raw).map_err(|_| "FLASHY_PUBLIC_ORIGIN must be an absolute URL")?;
        if issuer.cannot_be_a_base()
            || issuer.host_str().is_none()
            || issuer.query().is_some()
            || issuer.fragment().is_some()
        {
            return Err(
                "FLASHY_PUBLIC_ORIGIN must be an origin without a path, query, or fragment".into(),
            );
        }
        if issuer.path() != "/" {
            return Err("FLASHY_PUBLIC_ORIGIN must not contain a path".into());
        }
        let loopback = issuer.host_str().is_some_and(|h| {
            h == "localhost"
                || h.trim_start_matches('[')
                    .trim_end_matches(']')
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback())
        });
        if enabled && issuer.scheme() != "https" && !loopback {
            return Err(
                "MCP requires an HTTPS public origin (HTTP is allowed only on loopback)".into(),
            );
        }
        let secure_cookie = std::env::var("SESSION_SECURE").is_ok_and(|value| {
            !matches!(value.to_ascii_lowercase().as_str(), "0" | "false" | "no")
        });
        if enabled && !loopback && !secure_cookie {
            return Err("MCP on a public origin requires SESSION_SECURE=true".into());
        }
        issuer.set_path("");
        let resource = issuer.join("mcp").expect("valid resource URL").to_string();
        let mut trusted_metadata_hosts = HashSet::from(["chatgpt.com".to_owned()]);
        if let Ok(hosts) = std::env::var("MCP_CIMD_TRUSTED_HOSTS") {
            trusted_metadata_hosts.extend(
                hosts
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned),
            );
        }
        Ok(Self {
            enabled,
            issuer,
            resource,
            trusted_metadata_hosts,
        })
    }

    pub fn endpoint(&self, path: &str) -> String {
        self.issuer
            .join(path.trim_start_matches('/'))
            .expect("static endpoint")
            .to_string()
    }
}
