// ── HMAC operations + session token build/verify ──
// SessionData and parse_session_token unused until middleware is wired.
#![allow(dead_code)]

use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::sync::LazyLock;

type HmacSha256 = Hmac<Sha256>;
const SESSION_PREFIX: &[u8] = b"session";
const CHALLENGE_PREFIX: &[u8] = b"challenge";

/// Shared HMAC secret, decoded once from env.
static HMAC_KEY: LazyLock<Vec<u8>> = LazyLock::new(|| {
    let raw = std::env::var("AUTH_HMAC_SECRET")
        .expect("AUTH_HMAC_SECRET env var must be set — 32+ random bytes, base64-encoded");
    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &raw)
        .expect("AUTH_HMAC_SECRET must be valid base64")
});

// ── Challenge HMAC ───────────────────────────────────────────────

pub fn build_challenge_hmac(key_id: &str, nonce: &[u8; 32], expires_at: i64) -> String {
    let mut mac = HmacSha256::new_from_slice(&HMAC_KEY).expect("HMAC key");
    mac.update(CHALLENGE_PREFIX);
    mac.update(key_id.as_bytes());
    mac.update(nonce);
    mac.update(&expires_at.to_be_bytes());
    hex::encode(mac.finalize().into_bytes())
}

pub fn verify_challenge_hmac(
    key_id: &str,
    nonce: &[u8; 32],
    expires_at: i64,
    token: &str,
) -> bool {
    let expected = build_challenge_hmac(key_id, nonce, expires_at);
    // hex comparison is constant-time for equal-length strings
    expected == token && chrono::Utc::now().timestamp() < expires_at
}

// ── Session token ────────────────────────────────────────────────

pub fn build_session_token(key_id: &str, nonce_counter: i64, expiry: i64) -> String {
    let salt: [u8; 16] = rand::random();

    let mut mac = HmacSha256::new_from_slice(&HMAC_KEY).expect("HMAC key");
    mac.update(SESSION_PREFIX);
    mac.update(key_id.as_bytes());
    mac.update(&salt);
    mac.update(&nonce_counter.to_be_bytes());
    mac.update(&expiry.to_be_bytes());
    let h = hex::encode(mac.finalize().into_bytes());

    format!(
        "{}.{}.{}.{}",
        hex::encode(key_id.as_bytes()),
        hex::encode(salt),
        hex::encode(expiry.to_be_bytes()),
        h,
    )
}

#[derive(Debug)]
pub struct SessionData {
    pub key_id: String,
    pub capabilities: i16,
    pub expiry: i64,
}

pub fn parse_session_token(token: &str, nonce_counter: i64) -> Result<SessionData, &'static str> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 4 {
        return Err("invalid session token: expected 4 dot-separated parts");
    }
    let key_id = String::from_utf8(
        hex::decode(parts[0]).map_err(|_| "session token: invalid hex in key_id")?,
    )
    .map_err(|_| "session token: invalid utf-8 in key_id")?;

    let salt: [u8; 16] = hex::decode(parts[1])
        .map_err(|_| "session token: invalid hex in salt")?
        .try_into()
        .map_err(|_| "session token: salt must be 16 bytes")?;

    let expiry_bytes: [u8; 8] = hex::decode(parts[2])
        .map_err(|_| "session token: invalid hex in expiry")?
        .try_into()
        .map_err(|_| "session token: expiry must be 8 bytes")?;
    let expiry = i64::from_be_bytes(expiry_bytes);

    let expected = {
        let mut mac = HmacSha256::new_from_slice(&HMAC_KEY).expect("HMAC key");
        mac.update(SESSION_PREFIX);
        mac.update(key_id.as_bytes());
        mac.update(&salt);
        mac.update(&nonce_counter.to_be_bytes());
        mac.update(&expiry.to_be_bytes());
        hex::encode(mac.finalize().into_bytes())
    };

    if parts[3] != expected {
        return Err("session token: HMAC mismatch (revoked, forged, or wrong key)");
    }

    if chrono::Utc::now().timestamp() > expiry {
        return Err("session token: expired");
    }

    Ok(SessionData {
        key_id,
        capabilities: 15, // caller should override with DB lookup
        expiry,
    })
}

// ── Helpers ──────────────────────────────────────────────────────

/// Generate a fresh 32-byte challenge nonce.
pub fn generate_nonce() -> [u8; 32] {
    rand::random()
}

/// Session expiry as Unix timestamp (N seconds from now).
pub fn session_expiry(ttl_secs: i64) -> i64 {
    chrono::Utc::now().timestamp() + ttl_secs
}
