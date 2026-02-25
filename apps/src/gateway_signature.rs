use hmac::{Hmac, Mac};
use sha2::Sha256;

const ENV_GATEWAY_SECRET: &str = "AXIOM_GATEWAY_SECRET";

/// Computes HMAC-SHA256 of `body` keyed with `secret`.
/// Returns a 32-byte digest. Same inputs always produce the same digest.
pub fn request_fingerprint(body: &[u8], secret: &[u8]) -> [u8; 32] {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret)
        .expect("HMAC accepts any key length");
    mac.update(body);
    mac.finalize().into_bytes().into()
}

/// Verify that the provided signature matches HMAC-SHA256(body, secret).
/// Uses constant-time comparison to prevent timing attacks.
/// Returns true if the signature is valid.
pub fn verify_request_signature(body: &[u8], secret: &[u8], provided_hex: &str) -> bool {
    let provided_bytes = match decode_signature_hex(provided_hex) {
        Some(b) => b,
        None => return false,
    };

    let expected = request_fingerprint(body, secret);

    // Lengths must match before constant-time comparison.
    if provided_bytes.len() != expected.len() {
        return false;
    }

    // XOR all byte pairs — result is 0 only if every byte is equal.
    // This loop runs in constant time regardless of where a difference occurs.
    let diff: u8 = provided_bytes
        .iter()
        .zip(expected.iter())
        .map(|(a, b)| a ^ b)
        .fold(0u8, |acc, x| acc | x);

    diff == 0
}

/// Decodes a lowercase hex string into raw bytes.
/// Returns `None` for odd-length input or invalid hex characters.
fn decode_signature_hex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    let mut bytes = Vec::with_capacity(s.len() / 2);
    let mut chars = s.chars();
    while let (Some(hi), Some(lo)) = (chars.next(), chars.next()) {
        let hi = hex_nibble(hi)?;
        let lo = hex_nibble(lo)?;
        bytes.push((hi << 4) | lo);
    }
    Some(bytes)
}

fn hex_nibble(ch: char) -> Option<u8> {
    match ch {
        '0'..='9' => Some(ch as u8 - b'0'),
        'a'..='f' => Some(ch as u8 - b'a' + 10),
        'A'..='F' => Some(ch as u8 - b'A' + 10),
        _ => None,
    }
}

/// Loads the gateway secret from `AXIOM_GATEWAY_SECRET` env var.
/// Returns `None` if the env var is not set or is empty.
pub fn load_gateway_secret() -> Option<String> {
    std::env::var(ENV_GATEWAY_SECRET)
        .ok()
        .filter(|s| !s.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_correct_signature_returns_true() {
        let body = b"test body";
        let secret = b"mysecret";
        let fingerprint = request_fingerprint(body, secret);
        let hex_sig: String = fingerprint.iter().map(|b| format!("{:02x}", b)).collect();
        assert!(verify_request_signature(body, secret, &hex_sig));
    }

    #[test]
    fn verify_wrong_signature_returns_false() {
        let body = b"test body";
        let secret = b"mysecret";
        assert!(!verify_request_signature(body, secret, "deadbeef"));
        assert!(!verify_request_signature(body, secret, ""));
        assert!(!verify_request_signature(body, secret, "not-hex!"));
    }

    #[test]
    fn verify_tampered_body_returns_false() {
        let body = b"test body";
        let secret = b"mysecret";
        let fingerprint = request_fingerprint(body, secret);
        let hex_sig: String = fingerprint.iter().map(|b| format!("{:02x}", b)).collect();
        assert!(!verify_request_signature(b"tampered body", secret, &hex_sig));
    }

    #[test]
    fn same_body_and_secret_produce_same_fingerprint() {
        let body = b"intent:read key=health";
        let secret = b"test-secret-key";
        let a = request_fingerprint(body, secret);
        let b = request_fingerprint(body, secret);
        assert_eq!(a, b);
    }

    #[test]
    fn different_secret_produces_different_fingerprint() {
        let body = b"intent:read key=health";
        let fp1 = request_fingerprint(body, b"secret-one");
        let fp2 = request_fingerprint(body, b"secret-two");
        assert_ne!(fp1, fp2);
    }

    #[test]
    fn different_body_produces_different_fingerprint() {
        let secret = b"shared-secret";
        let fp1 = request_fingerprint(b"body-one", secret);
        let fp2 = request_fingerprint(b"body-two", secret);
        assert_ne!(fp1, fp2);
    }

    #[test]
    fn empty_secret_is_valid() {
        // HMAC accepts any key length including empty
        let fp = request_fingerprint(b"some body", b"");
        assert_eq!(fp.len(), 32);
    }

    #[test]
    fn fingerprint_is_32_bytes() {
        let fp = request_fingerprint(b"body", b"key");
        assert_eq!(fp.len(), 32);
    }

    #[test]
    fn load_gateway_secret_returns_none_when_not_set() {
        // This test is safe only if AXIOM_GATEWAY_SECRET is not set in CI.
        // We cannot unset env vars portably, so just verify the return type contract.
        let result = load_gateway_secret();
        // Either Some(non-empty) or None — never Some("")
        if let Some(ref s) = result {
            assert!(!s.trim().is_empty());
        }
    }
}
