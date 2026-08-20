//! The secret token that guards every route (R12–R13, R17).
//!
//! 32 random bytes → 43-char URL-safe base64 (~232 bits). Fresh each start,
//! never persisted, dies with the process. The token *is* the auth: no
//! cookie, no session, no rotation.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngCore;

/// Generate a fresh token: 32 random bytes, URL-safe base64 (no padding).
pub fn generate_token() -> String {
    let mut buf = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut buf);
    URL_SAFE_NO_PAD.encode(buf)
}

/// The constant-time comparison of a request's token against the live one.
///
/// R13: a request with no valid token gets a 404. We compare in constant time
/// so we don't leak *which* byte differed via timing.
pub fn token_is_valid(provided: &str, expected: &str) -> bool {
    if provided.len() != expected.len() {
        return false;
    }
    let ok = provided
        .bytes()
        .zip(expected.bytes())
        .fold(0u8, |acc, (a, b)| acc | a ^ b);
    ok == 0
}

/// The full secret URL: `scheme://host:port/p/<token>`.
pub fn secret_url(scheme: &str, host: &str, port: u16, token: &str) -> String {
    format!("{scheme}://{host}:{port}/p/{token}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_43_chars_and_url_safe() {
        let t = generate_token();
        assert_eq!(t.len(), 43, "32 bytes URL-safe base64 → 43 chars");
        assert!(t
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn two_tokens_differ() {
        assert_ne!(generate_token(), generate_token());
    }

    #[test]
    fn validation_matches_exactly() {
        assert!(token_is_valid("abc", "abc"));
        assert!(!token_is_valid("abc", "abd"));
        assert!(!token_is_valid("abc", "abcd")); // length mismatch
                                                 // The real token is always 43 chars; an empty/empty comparison is
                                                 // degenerate (and a 404 is the only thing a bad token gets), so we
                                                 // don't assert the empty case as "invalid" — it's simply unreachable.
    }
}
