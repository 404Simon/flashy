use serde::{Deserialize, Serialize};

pub const READ_SCOPE: &str = "flashy:read";

#[derive(Deserialize)]
pub struct AuthorizationQuery {
    pub response_type: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: String,
    pub state: Option<String>,
    pub code_challenge: String,
    pub code_challenge_method: String,
    pub resource: String,
}

#[derive(Debug, Deserialize)]
pub struct RegistrationRequest {
    pub client_name: String,
    pub redirect_uris: Vec<String>,
    #[serde(default)]
    pub token_endpoint_auth_method: Option<String>,
    #[serde(default)]
    pub grant_types: Vec<String>,
    #[serde(default)]
    pub response_types: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct RegistrationResponse {
    pub client_id: String,
    pub client_name: String,
    pub redirect_uris: Vec<String>,
    pub token_endpoint_auth_method: &'static str,
    pub grant_types: [&'static str; 2],
    pub response_types: [&'static str; 1],
}

#[derive(Deserialize)]
pub struct ConsentForm {
    pub request_id: String,
    pub csrf_token: String,
    pub decision: String,
}

#[derive(Deserialize)]
pub struct TokenForm {
    pub grant_type: String,
    pub client_id: String,
    pub code: Option<String>,
    pub redirect_uri: Option<String>,
    pub code_verifier: Option<String>,
    pub refresh_token: Option<String>,
    pub resource: Option<String>,
    pub scope: Option<String>,
}

#[derive(Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: &'static str,
    pub expires_in: i64,
    pub refresh_token: String,
    pub scope: &'static str,
}

#[derive(Deserialize)]
pub struct RevokeForm {
    pub token: String,
    pub client_id: String,
}

#[derive(Clone, Debug)]
pub struct McpPrincipal {
    pub user_id: i64,
    pub client_id: String,
    pub grant_id: String,
    pub scopes: Vec<String>,
}
