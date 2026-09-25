use std::borrow::Cow;

use oxide_auth::{code_grant::extensions::Pkce, primitives::grant::Value};

/// Keeps OAuth-engine types at the authorization boundary. Persisted records
/// contain only this opaque private extension value, never an oxide-auth type.
pub fn encode_s256_challenge(challenge: &str) -> Result<String, ()> {
    Pkce::required()
        .challenge(Some(Cow::Borrowed("S256")), Some(Cow::Borrowed(challenge)))?
        .ok_or(())?
        .into_private_value()
        .map_err(|_| ())?
        .ok_or(())
}

pub fn verify_s256(encoded_challenge: String, verifier: &str) -> Result<(), ()> {
    Pkce::required().verify(
        Some(Value::private(Some(encoded_challenge))),
        Some(Cow::Borrowed(verifier)),
    )
}
