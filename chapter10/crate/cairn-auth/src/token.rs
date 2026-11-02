//! Random tokens, and the PKCE check between the desktop app and
//! `cairn-auth`.
//!
//! When the app opens, it makes a random verifier and sends only its
//! SHA-256 hash (the challenge) through the browser. To trade the one-time
//! code for a credential, it must show the verifier itself. A browser that
//! lands on `cairn-auth` without the app never had the verifier, so it
//! can't get, or reset, anyone's password.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use sha2::{Digest, Sha256};

/// 32 random bytes, URL-safe base64: 43 characters.
pub fn random_token() -> String {
    URL_SAFE_NO_PAD.encode(rand::random::<[u8; 32]>())
}

/// The challenge for a verifier, as PKCE's S256 method computes it.
pub fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// Constant-time comparison of a verifier against a stored challenge.
pub fn verifies(verifier: &str, expected_challenge: &str) -> bool {
    let got = challenge(verifier);
    got.len() == expected_challenge.len()
        && got
            .bytes()
            .zip(expected_challenge.bytes())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_rfc_7636_appendix_b() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let expected = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
        assert_eq!(challenge(verifier), expected);
        assert!(verifies(verifier, expected));
        assert!(!verifies("wrong", expected));
    }

    #[test]
    fn tokens_are_43_characters_and_differ() {
        let (a, b) = (random_token(), random_token());
        assert_eq!(a.len(), 43);
        assert_ne!(a, b);
    }
}
