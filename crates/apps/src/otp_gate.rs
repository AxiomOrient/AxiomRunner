use totp_rs::{Algorithm, Secret, TOTP};

pub struct OtpGate {
    totp: TOTP,
}

impl OtpGate {
    /// Create from a base32-encoded secret string (e.g. from AXIOM_OTP_SECRET env var).
    pub fn new(secret_base32: &str) -> Result<Self, String> {
        let secret = Secret::Encoded(secret_base32.to_string())
            .to_bytes()
            .map_err(|e| format!("invalid OTP secret: {e}"))?;
        let totp = TOTP::new(Algorithm::SHA1, 6, 1, 30, secret)
            .map_err(|e| format!("TOTP init failed: {e}"))?;
        Ok(Self { totp })
    }

    /// Load from AXIOM_OTP_SECRET env var. Returns None if the env var is not set (OTP disabled).
    pub fn load_from_env() -> Option<Result<Self, String>> {
        std::env::var("AXIOM_OTP_SECRET")
            .ok()
            .map(|s| Self::new(&s))
    }

    /// Verify a 6-digit code string. Returns true if valid.
    pub fn verify(&self, code: &str) -> bool {
        self.totp.check_current(code).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 256-bit (32-byte) base32 secret — the minimum required by totp-rs v5 is 128 bits.
    const TEST_SECRET: &str = "JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP";

    #[test]
    fn rejects_obviously_wrong_code() {
        let gate = OtpGate::new(TEST_SECRET).expect("valid secret");
        assert!(!gate.verify("000000"));
        assert!(!gate.verify("not-a-code"));
    }

    #[test]
    fn new_valid_secret_constructs_gate() {
        // Verifies that a well-formed base32 secret produces a valid OtpGate.
        let result = OtpGate::new(TEST_SECRET);
        assert!(result.is_ok());
    }

    #[test]
    fn new_invalid_secret_returns_err() {
        // Verifies that a malformed secret yields an error, covering the same
        // branch that load_from_env exposes when AXIOM_OTP_SECRET is bad.
        let result = OtpGate::new("!!!invalid!!!");
        assert!(result.is_err());
        if let Err(msg) = result {
            assert!(
                msg.contains("invalid OTP secret"),
                "unexpected message: {msg}"
            );
        }
    }
}
