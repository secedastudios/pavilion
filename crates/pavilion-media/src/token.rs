use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// A signed, time-limited token for segment access.
///
/// Fields are generic — the caller decides what `subject`, `resource`,
/// and `scope` mean:
/// - Pavilion: subject=person_id, resource=film_id, scope=platform_id
/// - Public streaming: subject="public", resource=video_id, scope="default"
/// - SlateHub: subject=user_id, resource=reel_id, scope=org_id
#[derive(Debug, Clone)]
pub struct SegmentToken {
    pub subject: String,
    pub resource: String,
    pub scope: String,
    pub segment_path: String,
    pub expires_at: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum TokenError {
    #[error("Invalid token format")]
    InvalidFormat,
    #[error("Invalid signature")]
    InvalidSignature,
    #[error("Token expired")]
    Expired,
}

impl SegmentToken {
    /// Create a new token with an explicit TTL.
    pub fn new(
        subject: &str,
        resource: &str,
        scope: &str,
        segment_path: &str,
        ttl_secs: u64,
    ) -> Self {
        let expires_at = now_secs() + ttl_secs;
        Self {
            subject: subject.to_string(),
            resource: resource.to_string(),
            scope: scope.to_string(),
            segment_path: segment_path.to_string(),
            expires_at,
        }
    }

    /// Sign the token and return a URL-safe base64 string.
    pub fn sign(&self, secret: &str) -> String {
        let payload = self.to_payload();
        let sig = compute_signature(&payload, secret);
        let raw = format!("{payload}.{sig}");
        URL_SAFE_NO_PAD.encode(raw.as_bytes())
    }

    /// Parse and verify a signed token string.
    pub fn verify(token_str: &str, secret: &str) -> Result<Self, TokenError> {
        let decoded = URL_SAFE_NO_PAD
            .decode(token_str.as_bytes())
            .map_err(|_| TokenError::InvalidFormat)?;
        let raw = String::from_utf8(decoded).map_err(|_| TokenError::InvalidFormat)?;

        let (payload, sig) = raw.rsplit_once('.').ok_or(TokenError::InvalidFormat)?;

        let expected_sig = compute_signature(payload, secret);
        if !constant_time_eq(sig.as_bytes(), expected_sig.as_bytes()) {
            return Err(TokenError::InvalidSignature);
        }

        let parts: Vec<&str> = payload.split('|').collect();
        if parts.len() != 5 {
            return Err(TokenError::InvalidFormat);
        }

        let expires_at: u64 = parts[4].parse().map_err(|_| TokenError::InvalidFormat)?;
        if now_secs() > expires_at {
            return Err(TokenError::Expired);
        }

        Ok(Self {
            subject: parts[0].to_string(),
            resource: parts[1].to_string(),
            scope: parts[2].to_string(),
            segment_path: parts[3].to_string(),
            expires_at,
        })
    }

    pub fn matches_subject(&self, subject: &str) -> bool {
        self.subject == subject
    }

    fn to_payload(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}",
            self.subject, self.resource, self.scope, self.segment_path, self.expires_at
        )
    }
}

/// Generate a signed segment URL.
pub fn sign_segment_url(
    subject: &str,
    resource: &str,
    scope: &str,
    segment_path: &str,
    secret: &str,
    ttl_secs: u64,
    prefix: &str,
) -> String {
    let token = SegmentToken::new(subject, resource, scope, segment_path, ttl_secs);
    let signed = token.sign(secret);
    format!("{prefix}{signed}")
}

fn compute_signature(payload: &str, secret: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(payload.as_bytes());
    let result = mac.finalize();
    URL_SAFE_NO_PAD.encode(result.into_bytes())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time went backwards")
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_and_verify() {
        let token = SegmentToken::new("user1", "video1", "site1", "360p/seg_001.m4s", 300);
        let signed = token.sign("test-secret");
        let verified = SegmentToken::verify(&signed, "test-secret").unwrap();
        assert_eq!(verified.subject, "user1");
        assert_eq!(verified.resource, "video1");
        assert_eq!(verified.scope, "site1");
        assert_eq!(verified.segment_path, "360p/seg_001.m4s");
    }

    #[test]
    fn wrong_secret_fails() {
        let token = SegmentToken::new("u", "v", "s", "seg.m4s", 300);
        let signed = token.sign("secret-a");
        assert!(SegmentToken::verify(&signed, "secret-b").is_err());
    }

    #[test]
    fn tampered_token_fails() {
        let token = SegmentToken::new("u", "v", "s", "seg.m4s", 300);
        let signed = token.sign("test-secret");
        assert!(SegmentToken::verify(&format!("{signed}x"), "test-secret").is_err());
    }

    #[test]
    fn subject_match() {
        let token = SegmentToken::new("user1", "v", "s", "seg.m4s", 300);
        assert!(token.matches_subject("user1"));
        assert!(!token.matches_subject("user2"));
    }
}
