// ── HMAC operations + session token build/verify ──

use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use subtle::ConstantTimeEq;

type HmacSha256 = Hmac<Sha256>;
const SESSION_PREFIX: &[u8] = b"session";
const CHALLENGE_PREFIX: &[u8] = b"challenge";

/// Shared HMAC secret, decoded once from env.
static HMAC_KEY: LazyLock<Vec<u8>> = LazyLock::new(|| {
    let raw = std::env::var("AUTH_HMAC_SECRET")
        .expect("AUTH_HMAC_SECRET env var must be set — base64 of 24+ random bytes");
    let key = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &raw)
        .expect("AUTH_HMAC_SECRET must be valid base64");
    // The var can be *present but empty* (e.g. compose passing "${AUTH_HMAC_SECRET}"
    // with nothing in .env), which used to silently yield an empty key and fully
    // forgeable tokens. Fail fast at startup instead.
    assert!(
        key.len() >= 24,
        "AUTH_HMAC_SECRET must decode to at least 24 bytes (got {}); regenerate with: openssl rand -base64 32",
        key.len(),
    );
    key
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
    // Constant-time comparison over decoded bytes. `==` on Strings
    // short-circuits on the first differing byte — a timing side channel.
    let expected = hex::decode(expected).unwrap_or_default();
    let provided = hex::decode(token).unwrap_or_default();
    bool::from(expected.ct_eq(&provided)) && chrono::Utc::now().timestamp() < expires_at
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
    let hmac_hex = hex::encode(mac.finalize().into_bytes());

    format!(
        "{}.{}.{}.{}",
        hex::encode(key_id.as_bytes()),
        hex::encode(salt),
        hex::encode(expiry.to_be_bytes()),
        hmac_hex,
    )
}

#[derive(Debug)]
pub struct SessionData {
    pub key_id: String,
    #[allow(dead_code)] // expiry is carried by the token itself; read by future middleware
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

    // Constant-time comparison over decoded bytes (see verify_challenge_hmac).
    let expected = hex::decode(expected).unwrap_or_default();
    let provided = hex::decode(parts[3]).unwrap_or_default();

    if !bool::from(provided.ct_eq(&expected)) {
        return Err("session token: HMAC mismatch (revoked, forged, or wrong key)");
    }

    if chrono::Utc::now().timestamp() > expiry {
        return Err("session token: expired");
    }

    Ok(SessionData {
        key_id,
        expiry,
    })
}

/// Extract the plaintext key_id from a session token WITHOUT verifying it.
/// Needed because session HMAC binds to the DB-stored nonce_counter, which
/// can only be looked up after parsing key_id. The result is UNTRUSTED —
/// callers must still run `parse_session_token` against the fetched counter.
pub fn peek_key_id(token: &str) -> Option<String> {
    let key_id_hex = token.split('.').next()?;
    let bytes = hex::decode(key_id_hex).ok()?;
    String::from_utf8(bytes).ok()
}

// ── Helpers ──────────────────────────────────────────────────────

/// Generate a fresh 32-byte challenge nonce.
pub fn generate_nonce() -> [u8; 32] {
    rand::random()
}

/// One-time challenge nonces: nonce -> expires_at.
/// Prevents replaying an authorize request within the challenge window.
///
/// SINGLE-INSTANCE CONSTRAINT (CR-003): this store is process-local memory.
/// A restart clears replay protection (mitigated by the 60s challenge
/// expiry), and any multi-instance deployment would break single-use
/// guarantees entirely — each instance tracks its own nonces. Do not scale
/// to multiple backend replicas without moving this to a shared store
/// (e.g. Postgres or Redis). Consumption happens AFTER successful FALCON
/// verification, so failed attempts do not burn challenges.
static USED_NONCES: LazyLock<Mutex<HashMap<[u8; 32], i64>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Mark `nonce` as used, expiring at `expires_at`.
/// Returns false if the nonce was already consumed (replay) — the caller
/// must reject the request. Expired entries are purged opportunistically.
pub fn consume_nonce(nonce: &[u8; 32], expires_at: i64) -> bool {
    let mut used = USED_NONCES.lock().unwrap_or_else(|p| p.into_inner());
    let now = chrono::Utc::now().timestamp();
    used.retain(|_, exp| *exp > now);
    if used.contains_key(nonce) {
        return false;
    }
    used.insert(*nonce, expires_at);
    true
}

/// Session expiry as Unix timestamp (N seconds from now).
pub fn session_expiry(ttl_secs: i64) -> i64 {
    chrono::Utc::now().timestamp() + ttl_secs
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Initialize HMAC_KEY for tests. Each nextest test runs in its own
    /// process; under plain `cargo test` the Once keeps this idempotent.
    fn init_hmac() {
        use std::sync::Once;
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            unsafe {
                std::env::set_var(
                    "AUTH_HMAC_SECRET",
                    base64::Engine::encode(
                        &base64::engine::general_purpose::STANDARD,
                        [0x5Au8; 32],
                    ),
                );
            }
        });
    }

    #[test]
    fn nonce_is_single_use() {
        let nonce = [7u8; 32];
        let expiry = chrono::Utc::now().timestamp() + 60;
        assert!(consume_nonce(&nonce, expiry), "first use must succeed");
        assert!(!consume_nonce(&nonce, expiry), "replay must be rejected");
    }

    // ── CR-001 regressions: malformed tokens must reject, never panic ──

    #[test]
    fn wrong_length_hmac_token_rejected_not_panic() {
        init_hmac();
        let nonce = [1u8; 32];
        let expiry = chrono::Utc::now().timestamp() + 60;
        // Valid hex, but 31 bytes instead of 32 — used to panic in ct_eq.
        let token = "ab".repeat(31);
        assert!(!verify_challenge_hmac("k", &nonce, expiry, &token));
    }

    #[test]
    fn invalid_hex_hmac_token_rejected_not_panic() {
        init_hmac();
        let nonce = [2u8; 32];
        let expiry = chrono::Utc::now().timestamp() + 60;
        assert!(!verify_challenge_hmac("k", &nonce, expiry, "zz"));
        assert!(!verify_challenge_hmac("k", &nonce, expiry, ""));
    }

    #[test]
    fn session_token_wrong_length_hmac_part_rejected_not_panic() {
        init_hmac();
        let key_id = "laptop";
        let counter = 0i64;
        let expiry = chrono::Utc::now().timestamp() + 60;
        let token = build_session_token(key_id, counter, expiry);
        let mut parts: Vec<String> = token.split('.').map(String::from).collect();
        let last = parts.pop().expect("4 parts");
        // Truncate the HMAC part to a wrong length (valid hex).
        parts.push(last[..last.len() - 2].to_string());
        let truncated = parts.join(".");
        assert!(parse_session_token(&truncated, counter).is_err());
    }

    #[test]
    fn peek_key_id_roundtrip_and_garbage() {
        // Hand-built token: avoids build_session_token, whose HMAC_KEY
        // lazy-static would panic without AUTH_HMAC_SECRET in test runs.
        let token = format!(
            "{}.{}.{}.{}",
            hex::encode("laptop"),
            hex::encode([0u8; 16]),
            hex::encode(0i64.to_be_bytes()),
            "ab".repeat(32),
        );
        assert_eq!(peek_key_id(&token).as_deref(), Some("laptop"));
        assert_eq!(peek_key_id("not-a-token"), None);
        assert_eq!(peek_key_id("zz.zz.zz.zz"), None); // invalid hex
    }
}
