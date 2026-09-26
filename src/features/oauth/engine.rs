use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest, Sha256};

pub fn validate_s256_challenge(challenge: &str) -> Result<(), ()> {
    let decoded = URL_SAFE_NO_PAD.decode(challenge).map_err(|_| ())?;
    if decoded.len() != 32 || URL_SAFE_NO_PAD.encode(decoded) != challenge {
        return Err(());
    }
    Ok(())
}

pub fn verify_s256(expected_challenge: &str, verifier: &str) -> Result<(), ()> {
    if !(43..=128).contains(&verifier.len())
        || !verifier
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~'))
    {
        return Err(());
    }
    let actual = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    (actual == expected_challenge).then_some(()).ok_or(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_and_verifies_s256_pkce() {
        let verifier = "a".repeat(43);
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        assert!(validate_s256_challenge(&challenge).is_ok());
        assert!(verify_s256(&challenge, &verifier).is_ok());
        assert!(verify_s256(&challenge, &"b".repeat(43)).is_err());
        assert!(validate_s256_challenge(&format!("{challenge}a")).is_err());
        assert!(verify_s256(&challenge, "short").is_err());
    }
}
