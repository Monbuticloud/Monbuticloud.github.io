# FALCON Authorize + Chess Depth Gating Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire FALCON-1024 signature verification into `/api/auth/authorize`, issue stateless session tokens, and gate chess depth 13–15 behind authenticated sessions.

**Architecture:** Same-crate interop (`falcon-rs` client + server), stateless HMAC sessions (existing `session.rs`), capability bitmask check in the chess handler. Nonce consumption moves after successful verification (CR-003).

**Tech Stack:** Rust / Axum / Diesel async / `falcon-rs` 0.2.x (pure Rust, native-safe, MSRV 1.84)

**Spec:** `backend/docs/specs/2026-08-23-falcon-authorize-design.md`

## Global Constraints

- FN-DSA-1024 only: public keys exactly **1793 bytes**; reject everything else.
- Domain separation must be `DomainSeparation::None` (matches WASM client signing).
- Import path: `use falcon::prelude::*;` — exports `FnDsaKeyPair`, `FnDsaSignature`, `DomainSeparation`.
- Verify API: `FnDsaSignature::verify(sig: &[u8], pubkey: &[u8], message: &[u8], domain: &DomainSeparation) -> Result<(), FalconError>` (associated function).
- Unknown key_id and bad signature return **identical** 401 bodies (no enumeration); details logged server-side via `log_error(LogMsg::Auth{..})`.
- No heap-allocation rule applies ONLY to chess search — auth paths may allocate freely.
- Tests live inline (`#[cfg(test)] mod tests`) for auth modules (matches `session.rs` convention); commit atomically per task, conventional format.
- Handler-level DB flows have no test harness today — covered by manual smoke (Task 6), not fake tests.

---

### Task 1: `falcon-rs` dependency + `falcon_verify()`

**Files:**

- Modify: `backend/Cargo.toml` (dependencies section)
- Create: `backend/src/auth/falcon.rs`
- Modify: `backend/src/auth/mod.rs`

**Interfaces:**

- Produces: `crate::auth::falcon::falcon_verify(pub_key: &[u8], message: &[u8], sig: &[u8]) -> bool` and `pub const FALCON_PUB_KEY_LEN: usize = 1793` (used by Tasks 2–3).

- [ ] **Step 1: Add dependency**

In `backend/Cargo.toml` under `[dependencies]`:

```toml
falcon-rs = { version = "0.2", default-features = false, features = ["std"] }
```

- [ ] **Step 2: Write failing test + module skeleton**

Create `backend/src/auth/falcon.rs`:

```rust
// ── FALCON (FN-DSA FIPS 206) signature verification ──
//
// Server-side counterpart to crates/falcon-wasm. Signatures are produced
// client-side with DomainSeparation::None over the raw challenge nonce;
// verification here must match exactly.

use falcon::prelude::*;

/// FN-DSA-1024 public key size in bytes.
pub const FALCON_PUB_KEY_LEN: usize = 1793;

/// Verify a FALCON-1024 signature over `message`.
/// Returns false for any malformed input — never panics.
pub fn falcon_verify(pub_key: &[u8], message: &[u8], sig: &[u8]) -> bool {
    if pub_key.len() != FALCON_PUB_KEY_LEN {
        return false; // FN-DSA-1024 keys only
    }
    FnDsaSignature::verify(sig, pub_key, message, &DomainSeparation::None).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Signing a challenge requires a private key; tests generate one.
    /// Slow (~seconds) but runs once per test binary.
    fn make_keypair() -> (Vec<u8>, Vec<u8>) {
        let kp = FnDsaKeyPair::generate(10).expect("FN-DSA-1024 keypair");
        (kp.public_key().to_vec(), kp.private_key().to_vec())
    }

    #[test]
    fn valid_signature_verifies() {
        let (pk, sk) = make_keypair();
        let kp = FnDsaKeyPair::from_private_key(&sk).unwrap();
        let msg = [0xABu8; 32];
        let sig = kp.sign(&msg, &DomainSeparation::None).unwrap();
        assert!(falcon_verify(&pk, &msg, &sig.to_bytes()));
    }

    #[test]
    fn wrong_message_fails() {
        let (pk, sk) = make_keypair();
        let kp = FnDsaKeyPair::from_private_key(&sk).unwrap();
        let sig = kp.sign(&[1u8; 32], &DomainSeparation::None).unwrap();
        assert!(!falcon_verify(&pk, &[2u8; 32], &sig.to_bytes()));
    }

    #[test]
    fn tampered_signature_fails() {
        let (pk, sk) = make_keypair();
        let kp = FnDsaKeyPair::from_private_key(&sk).unwrap();
        let msg = [3u8; 32];
        let mut sig = kp.sign(&msg, &DomainSeparation::None).unwrap().to_bytes();
        sig[0] ^= 0xFF;
        assert!(!falcon_verify(&pk, &msg, &sig));
    }

    #[test]
    fn wrong_pubkey_length_rejected_before_crypto() {
        // 897 = FN-DSA-512 size; must fail the length guard, not panic.
        assert!(!falcon_verify(&[0u8; 897], &[0u8; 32], &[0u8; 64]));
        assert!(!falcon_verify(&[], &[0u8; 32], &[0u8; 64]));
    }
}
```

Wire into `backend/src/auth/mod.rs` (add one line):

```rust
pub mod falcon;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p backend auth::falcon -- --nocapture` (from `backend/`)
Expected: 4 pass. If `falcon-rs` fails to compile natively, STOP and report (spec §1 fallback: `fn-dsa` crate direct).

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock src/auth/falcon.rs src/auth/mod.rs
git commit -m "feat(auth): add falcon-rs dep and FALCON-1024 verify function"
```

### Task 2: Registration tightening (1793-byte keys)

**Files:**

- Modify: `backend/src/api/auth.rs:81-88` (`register_key` pub_key decode block)

**Interfaces:**

- Consumes: `crate::auth::falcon::FALCON_PUB_KEY_LEN` (Task 1).

- [ ] **Step 1: Replace the size check**

Change:

```rust
    let pub_key = match base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &body.pub_key,
    ) {
        Ok(b) if b.len() <= 4096 => b,
        Ok(_) => return err(StatusCode::BAD_REQUEST, "pub_key too large (max 4096 bytes)"),
        Err(_) => return err(StatusCode::BAD_REQUEST, "pub_key must be valid base64"),
    };
```

To:

```rust
    let pub_key = match base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &body.pub_key,
    ) {
        // FN-DSA-1024 only — any other size could register but never verify.
        Ok(b) if b.len() == crate::auth::falcon::FALCON_PUB_KEY_LEN => b,
        Ok(_) => {
            return err(
                StatusCode::BAD_REQUEST,
                "pub_key must be a FALCON-1024 public key (1793 bytes)",
            )
        },
        Err(_) => return err(StatusCode::BAD_REQUEST, "pub_key must be valid base64"),
    };
```

- [ ] **Step 2: Type-check**

Run: `cargo check` (from `backend/`) — Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add src/api/auth.rs
git commit -m "feat(auth): restrict registration to FALCON-1024 public keys"
```

### Task 3: Wire `authorize` (lookup → verify → consume → issue)

**Files:**

- Modify: `backend/src/api/auth.rs` (`authorize`, remove dead-code attrs on `AuthorizeReq.falcon_sig` and `KeyRow`)

**Interfaces:**

- Consumes: `falcon_verify` (Task 1); existing `session::{verify_challenge_hmac, consume_nonce, build_session_token, session_expiry}`; existing `KeyRow`.

- [ ] **Step 1: Rewrite `authorize`**

Replace the entire stub body (from `// 4. Session issuance is DISABLED...` through the closing 503 tuple) so the function reads:

```rust
/// POST /api/auth/authorize — verify FALCON sig, issue session token
pub async fn authorize(Json(body): Json<AuthorizeReq>) -> impl IntoResponse {
    // 1. Parse nonce
    let nonce: [u8; 32] = match hex::decode(&body.nonce) {
        Ok(b) if b.len() == 32 => {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&b);
            arr
        },
        _ => return err(StatusCode::BAD_REQUEST, "nonce must be 64 hex chars"),
    };

    // 2. Verify challenge HMAC (proves server issued this nonce, not expired)
    if !session::verify_challenge_hmac(&body.key_id, &nonce, body.expires_at, &body.hmac_token) {
        return err(StatusCode::UNAUTHORIZED, "invalid or expired challenge");
    }

    // 3. Look up the active key
    let mut conn = match crate::db::connect().await {
        Ok(c) => c,
        Err(e) => {
            crate::log_error(crate::LogMsg::Auth {
                detail: format!("authorize: DB connect failed: {e}"),
            });
            return err(StatusCode::INTERNAL_SERVER_ERROR, "internal error");
        },
    };

    let key = match tbl::table
        .filter(tbl::key_id.eq(&body.key_id))
        .filter(tbl::is_active.eq(true))
        .select(KeyRow::as_select())
        .get_result::<KeyRow>(&mut conn)
        .await
    {
        Ok(k) => k,
        // Same generic body as a bad signature — no key enumeration.
        Err(diesel::result::Error::NotFound) => {
            return err(StatusCode::UNAUTHORIZED, "authentication failed");
        },
        Err(e) => {
            crate::log_error(crate::LogMsg::Auth {
                detail: format!("authorize: DB select failed: {e}"),
            });
            return err(StatusCode::INTERNAL_SERVER_ERROR, "internal error");
        },
    };

    // 4. Decode + verify the FALCON signature over the raw nonce
    let sig = match base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &body.falcon_sig,
    ) {
        Ok(s) => s,
        Err(_) => return err(StatusCode::UNAUTHORIZED, "authentication failed"),
    };

    if !crate::auth::falcon::falcon_verify(&key.pub_key, &nonce, &sig) {
        crate::log_error(crate::LogMsg::Auth {
            detail: format!("authorize: bad FALCON signature for key_id={}", body.key_id),
        });
        return err(StatusCode::UNAUTHORIZED, "authentication failed");
    }

    // 5. Consume the nonce AFTER successful verification (CR-003): failed
    //    attempts must not burn challenges; successes stay single-use.
    if !session::consume_nonce(&nonce, body.expires_at) {
        return err(StatusCode::UNAUTHORIZED, "nonce already used");
    }

    // 6. Issue the stateless session token
    let expiry = session::session_expiry(3600);
    let token = session::build_session_token(&body.key_id, key.nonce_counter, expiry);

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "session_token": token,
            "capabilities": key.capabilities,
            "expires_at": expiry,
        })),
    )
}
```

Also delete these two attributes (no longer dead):

- `#[allow(dead_code)]` above `pub falcon_sig` in `AuthorizeReq`
- `#[allow(dead_code)]` above `struct KeyRow` (keep the comment removed too)

- [ ] **Step 2: Type-check + lint**

Run: `cargo check && cargo clippy` (from `backend/`) — Expected: clean, no new warnings.

- [ ] **Step 3: Commit**

```bash
git add src/api/auth.rs
git commit -m "feat(auth): wire FALCON verification and session issuance into authorize"
```

### Task 4: Chess depth gating (Bearer session + capability bit)

**Files:**

- Modify: `backend/src/auth/session.rs` (add `peek_key_id`)
- Modify: `backend/src/auth/mod.rs` (add `CAP_DEEP_SEARCH`)
- Modify: `backend/src/api/mod.rs` (receive `bearer_token` from `api/auth.rs`)
- Modify: `backend/src/api/auth.rs` (remove local `bearer_token`, import shared)
- Modify: `backend/src/api/chess.rs` (gate depth ≥ 13)

**Interfaces:**

- Consumes: `session::parse_session_token(token, nonce_counter) -> Result<SessionData, &'static str>` (exists).
- Produces: `session::peek_key_id(token: &str) -> Option<String>`; `crate::auth::CAP_DEEP_SEARCH: i16`; `crate::api::bearer_token(&HeaderMap) -> Option<&str>`.

- [ ] **Step 1: Add `peek_key_id` to `session.rs`** (after `parse_session_token`)

```rust
/// Extract the plaintext key_id from a session token WITHOUT verifying it.
/// Needed because session HMAC binds to the DB-stored nonce_counter, which
/// can only be looked up after parsing key_id. The result is UNTRUSTED —
/// callers must still run `parse_session_token` against the fetched counter.
pub fn peek_key_id(token: &str) -> Option<String> {
    let key_id_hex = token.split('.').next()?;
    let bytes = hex::decode(key_id_hex).ok()?;
    String::from_utf8(bytes).ok()
}
```

Plus an inline test in `session.rs`'s existing `mod tests`:

```rust
    #[test]
    fn peek_key_id_roundtrip_and_garbage() {
        let token = build_session_token("laptop", 0, session_expiry(60));
        assert_eq!(peek_key_id(&token).as_deref(), Some("laptop"));
        assert_eq!(peek_key_id("not-a-token"), None);
        assert_eq!(peek_key_id("zz.zz.zz.zz"), None); // invalid hex
    }
```

Run: `cargo test auth::session` — Expected: pass (2 tests total in module).

- [ ] **Step 2: Capability constant in `auth/mod.rs`**

```rust
/// Capability bits stored in `auth.keys.capabilities` (SMALLINT).
/// Bit assignment is first-come; the schema default of 15 sets all low bits.
pub const CAP_DEEP_SEARCH: i16 = 0b0000_0001;

pub mod falcon;
pub mod session;
```

- [ ] **Step 3: Move `bearer_token` to `api/mod.rs`**

Cut the `fn bearer_token` from `api/auth.rs`; paste into `api/mod.rs` as:

```rust
use axum::http::{HeaderMap, header};

/// Extract a "Bearer <token>" from the Authorization header.
pub(crate) fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let auth = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    auth.strip_prefix("Bearer ").map(str::trim).filter(|t| !t.is_empty())
}
```

In `api/auth.rs` replace the definition with `use super::bearer_token;` (or call `super::bearer_token(...)` at the one call site in `logout_all`). `cargo check` must stay clean.

- [ ] **Step 4: Gate chess depth ≥ 13**

In `api/chess.rs`: add `HeaderMap` extractor and the gate between depth parsing and semaphore acquisition:

```rust
use axum::{
    body::Body,
    extract::Query,
    http::{HeaderMap, Response, StatusCode, header},
};
```

Handler signature becomes:

```rust
pub async fn get_chess_completion(
    headers: HeaderMap,
    Query(q): Query<ChessQuery>,
) -> Response<Body> {
```

Replace `// TODO: gate depth 13–15 behind auth token` with:

```rust
    // ── Auth gate: depth 13–15 requires a session with CAP_DEEP_SEARCH ──
    if depth >= 13 {
        let token = match crate::api::bearer_token(&headers) {
            Some(t) => t,
            None => {
                return json_body(
                    StatusCode::UNAUTHORIZED,
                    r#"{"error":"depth 13–15 requires a session token"}"#.into(),
                );
            },
        };

        // Session HMAC binds to the DB-stored nonce_counter, so resolve the
        // (untrusted) key_id first, fetch the counter, then verify fully.
        let key_id = match crate::auth::session::peek_key_id(token) {
            Some(k) => k,
            None => {
                return json_body(
                    StatusCode::UNAUTHORIZED,
                    r#"{"error":"invalid or expired session token"}"#.into(),
                );
            },
        };

        let mut conn = match crate::db::connect().await {
            Ok(c) => c,
            Err(e) => {
                crate::log_error(crate::LogMsg::Auth {
                    detail: format!("chess: DB connect failed: {e}"),
                });
                return json_body(StatusCode::INTERNAL_SERVER_ERROR, r#"{"error":"internal error"}"#.into());
            },
        };

        let row = crate::schema::auth::keys::table
            .filter(crate::schema::auth::keys::key_id.eq(key_id))
            .filter(crate::schema::auth::keys::is_active.eq(true))
            .select((crate::schema::auth::keys::nonce_counter, crate::schema::auth::keys::capabilities))
            .get_result::<(i64, i16)>(&mut conn)
            .await;

        let (nonce_counter, capabilities) = match row {
            Ok(r) => r,
            Err(diesel::result::Error::NotFound) => {
                return json_body(
                    StatusCode::UNAUTHORIZED,
                    r#"{"error":"invalid or expired session token"}"#.into(),
                );
            },
            Err(e) => {
                crate::log_error(crate::LogMsg::Auth {
                    detail: format!("chess: DB select failed: {e}"),
                });
                return json_body(StatusCode::INTERNAL_SERVER_ERROR, r#"{"error":"internal error"}"#.into());
            },
        };

        if crate::auth::session::parse_session_token(token, nonce_counter).is_err() {
            return json_body(
                StatusCode::UNAUTHORIZED,
                r#"{"error":"invalid or expired session token"}"#.into(),
            );
        }

        if capabilities & crate::auth::CAP_DEEP_SEARCH == 0 {
            return json_body(
                StatusCode::FORBIDDEN,
                r#"{"error":"insufficient capabilities"}"#.into(),
            );
        }
    }
```

Add imports at top of chess.rs: `use diesel_async::RunQueryDsl;`

Update the doc comment on `ChessQuery`/handler: "`depth`: 1–12 anonymous; 13–15 requires Bearer session with CAP_DEEP_SEARCH."

- [ ] **Step 5: Full check**

Run: `cargo check && cargo clippy && cargo test` (from `backend/`) — Expected: all clean/passing.

- [ ] **Step 6: Commit**

```bash
git add src/auth/session.rs src/auth/mod.rs src/api/mod.rs src/api/auth.rs src/api/chess.rs
git commit -m "feat(chess): gate depth 13-15 behind FALCON sessions with CAP_DEEP_SEARCH"
```

### Task 5: Doc correction + residual-risk todo + DOX pass

**Files:**

- Modify: `backend/docs/auth-signing.md` (Offline Signing §Security Properties, Phishing Resistance §The .mhtml Air Gap + §Manual Copy/Paste framing, The One Fail-Case §The Real Fix)
- Modify: `backend/docs/Todo/todo.md` (new CR-011)
- Modify: `backend/AGENTS.md` + root `AGENTS.md` (DOX pass)

- [ ] **Step 1: Correct `auth-signing.md`**

Replace the "Real Fix: Generate Offline, Register via API" framing and the .mhtml "Security Properties"/"Phishing Resistance" claims with honest text. Core points to land (edit surrounding prose as needed):

- The .mhtml is downloaded from the same site an attacker may be proxying — the attacker can tamper with it and any checksum published beside it. There is no out-of-band channel anywhere in the loop.
- Signup MITM is therefore **unmitigated** in the current deployment — and this is not unique to FALCON: every credential system (passwords, OAuth, WebAuthn, SSH-key upload) is vulnerable to a proxying attacker at first-use/first-registration time. The .mhtml changes _where_ signing happens, not _who_ delivered the signing code.
- Residual value: no password field to phish, credentials never typed into the site, signing isolated from the browsing context. These hold against passive phishing pages, NOT against an active proxy on first download.
- A real fix requires a genuinely independent trust anchor (second-channel distribution, pinned hash from independent infrastructure) — currently absent.

- [ ] **Step 2: Open CR-011 in `todo.md`** (top of file, keep P-descending order)

```markdown
## [CR-011] Signup MITM unmitigated — no out-of-band trust anchor

- P: 5
- B: 0
- Since: <today YYYY-MM-DD>
- Since commit: <git rev-parse --short HEAD>
- File: backend/docs/auth-signing.md
- Summary: First-use key registration trusts whatever the site serves (WASM or .mhtml), so a proxying attacker can steal generated keypairs. Universal to all credential systems; real fix needs an out-of-band distribution channel for the signing tool. Doc corrected to state this honestly.
- Status: open
```

- [ ] **Step 3: DOX pass**

- Root `AGENTS.md`: update "Auth: FALCON..." contract line and Work Guidance bullet "FALCON verify is currently stubbed..." → verify wired via `falcon-rs`, FN-DSA-1024 only.
- `backend/AGENTS.md`: same updates; add `docs/specs/` + `docs/plans/` to ownership table; note depth 13–15 now enforced server-side (contract #3 "not wired yet" clause removed); add `crates/falcon-wasm` note unchanged.
- Remove stale text anywhere it claims authorize returns 503.

- [ ] **Step 4: Commit**

```bash
git add ../docs/auth-signing.md docs/Todo/todo.md AGENTS.md ../AGENTS.md
git commit -m "docs(backend): correct signup-MITM threat model, record CR-011, refresh DOX"
```

### Task 6: Verification + manual smoke

- [ ] **Step 1:** `cargo check && cargo clippy && cargo nextest run && cargo deny check` (from `backend/`) — all green.
- [ ] **Step 2:** Manual smoke with `docker compose up --build`:
  1. Register 1793-byte pub key → 201; wrong-size key → 400.
  2. Challenge → sign nonce with WASM client (FN-DSA-1024) → authorize → 200 with session_token.
  3. Replay same authorize → 401 "nonce already used".
  4. Tampered signature → 401 "authentication failed"; unknown key_id → identical body.
  5. Depth 14 with `Authorization: Bearer <token>` → search runs; without header → 401.
  6. Depth 12 anonymous → still works.
  7. logout-all, then reuse old token at depth 14 → 401.
- [ ] **Step 3:** Report results; archive nothing yet (CR-002/CR-003 close only after smoke passes).
