# FALCON Authorize + Chess Depth Gating — Design

- Date: 2026-08-23
- Status: Approved (pending implementation)
- Scope: `backend/src/api/auth.rs`, `backend/src/api/chess.rs`, `backend/src/auth/session.rs`, `backend/Cargo.toml`, `backend/docs/auth-signing.md`
- Related todos: CR-002 (depth gating), CR-003 (nonce ordering)

## Problem

`POST /api/auth/authorize` refuses all session issuance (503) because FALCON
signature verification is not wired (`src/api/auth.rs:170-197`). Consequently:

1. No client can obtain a session token.
2. Chess depth 13–15 cannot be gated (CR-002) — accepted anonymously today.
3. Challenge nonces are consumed *before* verification (CR-003), so every
   authorize attempt burns the nonce even though nothing is issued.

## Goals

- Wire the full authorize flow: challenge HMAC → key lookup → FALCON verify →
  issue stateless session token.
- Gate chess depth 13–15 behind a valid session with `CAP_DEEP_SEARCH`.
- Fix nonce ordering while wiring (consume after successful verification).
- Tighten registration to FN-DSA-1024 keys only.
- Correct the threat-model overstatement in `docs/auth-signing.md`.

## Non-goals

- Rate limiting on `/api/auth/keys` (CR-006, separate).
- Session middleware/extractor abstraction (one consumer; extract later).
- Fixing CR-001 (constant-time compare panic) — tracked separately; must land
  before or with this work going live.
- Client-side changes (.mhtml tool, WASM rebuild).

## Decisions (agreed)

| Decision | Choice |
|---|---|
| Library | `falcon-rs = "0.2"` (same crate as falcon-wasm client) |
| Key size | **FN-DSA-1024 only** — pub key exactly 1793 bytes |
| Domain separation | `DomainSeparation::None` (must match client signing) |
| Capability model | Bitmask const `CAP_DEEP_SEARCH`; check `capabilities & CAP_DEEP_SEARCH != 0`; default `15` passes |
| Session transport (chess) | `Authorization: Bearer` header only |
| Anonymous deep request | `401 Unauthorized` with explicit message |
| Nonce consumption | Moved to **after** successful FALCON verification |
| Error uniformity | Unknown key_id and bad signature return identical generic 401 bodies |

## Design

### 1. Dependency

Add to `backend/Cargo.toml`:

```toml
falcon-rs = { version = "0.2", default-features = false, features = ["std"] }
```

Verify at implementation start that falcon-rs 0.2 compiles natively (macOS +
the Docker Linux build) — it has so far only been built for wasm32. If native
compilation fails, stop and re-plan (fallback: `fn-dsa` crate direct).

### 2. Verification function

New pure function `src/auth/falcon.rs` (new module under `src/auth/`, wired
in `src/auth/mod.rs`):

```rust
/// Verify a FALCON-1024 signature over `message`.
/// Returns false for any malformed input — never panics.
pub fn falcon_verify(pub_key: &[u8], message: &[u8], sig: &[u8]) -> bool {
    if pub_key.len() != 1793 {
        return false; // FN-DSA-1024 public keys only
    }
    FnDsaSignature::verify(sig, pub_key, message, &DomainSeparation::None).is_ok()
}
```

No heap-allocation constraint applies here (auth path, not search path).

### 3. Registration tightening (`register_key`)

Replace the ≤4096-byte acceptance with an exact-length check:
`pub_key.len() == 1793`, else `400 "pub_key must be a FALCON-1024 public key
(1793 bytes)"`. Existing short rows simply fail verification later.

### 4. Authorize flow (`authorize`)

Replaces the stub at `src/api/auth.rs:170-197`, per `docs/auth-signing.md` §3:

1. Parse nonce (unchanged).
2. Verify challenge HMAC incl. expiry (unchanged) → 401 on failure.
3. DB connect; select `pub_key, capabilities, nonce_counter` for the active
   key via existing `KeyRow`. `NotFound` → generic 401 (same body as bad
   signature — no enumeration). DB error → 500 + server log.
4. Decode `falcon_sig` from base64 → 401 on malformed base64.
5. `falcon_verify(&key.pub_key, &nonce, &sig)` → generic 401 on failure,
   detail logged server-side.
6. **Now** consume the nonce (CR-003 fix): replay → 401 "nonce already used".
7. Issue: `build_session_token(key_id, nonce_counter, session_expiry(3600))`
   → 200 `{ session_token, capabilities, expires_at }`.

Rationale: verification before consumption means failed attempts do not burn
challenges; consumption after verification preserves single-use semantics.

Remove `#[allow(dead_code)]` from `AuthorizeReq.falcon_sig` and `KeyRow`.

### 5. Chess gating (`get_chess_completion`)

Depth range stays `1..=15`.

- `depth <= 12`: proceed anonymously (unchanged).
- `depth >= 13`:
  1. Bearer token from `Authorization` header (move the `bearer_token`
     helper from `api/auth.rs` to `api/mod.rs`). Missing → 401
     `"depth 13–15 requires a session token"`.
  2. DB: select `nonce_counter, capabilities` for the active key.
     `NotFound` → 401 generic.
  3. `parse_session_token(token, nonce_counter)` → Err → 401.
  4. `capabilities & CAP_DEEP_SEARCH == 0` → **403** `"insufficient
     capabilities"` (authenticated but not allowed — distinct from 401).
  5. Proceed to semaphore/search unchanged.

`CAP_DEEP_SEARCH` lives in `src/auth/mod.rs`:
`pub const CAP_DEEP_SEARCH: i16 = 0b0000_0001;` — bit assignment is
first-come; the schema default of 15 sets all four low bits.

### 6. Error responses

| Condition | Status | Body |
|---|---|---|
| Bad/expired challenge HMAC | 401 | `invalid or expired challenge` |
| Unknown/inactive key_id | 401 | same generic body as bad signature |
| Malformed/bad signature | 401 | `authentication failed` |
| Replay (nonce reuse) | 401 | `nonce already used` |
| Deep depth, no bearer token | 401 | `depth 13–15 requires a session token` |
| Valid session, no capability | 403 | `insufficient capabilities` |

All failure details logged server-side via `log_error(LogMsg::Auth{..})`.

### 7. Documentation correction (`docs/auth-signing.md`)

The doc claims the .mhtml air gap is "The Real Fix" for signup MITM. That is
wrong for this deployment: the .mhtml is downloaded from the same site an
attacker may be proxying, so the attacker can tamper with it and any checksum
published beside it. There is no out-of-band channel anywhere in the loop.

Rewrite the affected sections to state honestly:

- Signup MITM is **unmitigated** in the current deployment — and this is not
  unique to FALCON: every credential system (passwords, OAuth, WebAuthn,
  SSH-key upload) is vulnerable to a proxying attacker at first-use /
  first-registration time. The .mhtml changes *where* signing happens, not
  *who* delivered the signing code.
- The .mhtml still has value against passive phishing pages (no password
  field to fake, no credential typed into the site), but it does NOT protect
  against an active proxy on first download.
- A real fix requires a genuinely out-of-band trust anchor (e.g. .mhtml
  obtained over a second channel, or a pinned hash from independent
  infrastructure) — currently absent.

Also open a todo item (Guide.md format) recording the residual risk so it is
not lost when the doc edit lands.

### 8. Testing

Unit/integration tests in `src/tests/` (mirror structure):

- `falcon_verify`: valid signature round-trip using a generated FN-DSA-1024
  keypair (sign in test, verify), wrong message, tampered sig, 512-size pub
  key rejected by length guard.
- `authorize`: happy path issues a token that `parse_session_token` accepts;
  bad sig → 401; unknown key → identical 401 body; expired challenge → 401;
  replayed nonce after success → 401; failed verify does NOT burn the nonce
  (retry with correct sig succeeds).
- `register_key`: non-1793-byte pub_key → 400.
- Chess gating: depth 13 anonymous → 401; depth 12 anonymous → OK; valid
  session + capability → proceeds; session without capability bit → 403;
  revoked counter (post logout-all) → 401.

DB-backed tests use the existing test DB conventions in `src/tests/`.

## Verification plan

1. `cargo check` / `cargo clippy` clean.
2. `cargo nextest run` — new tests green, no regressions.
3. Manual smoke via docker compose: register → challenge → sign with WASM
   client → authorize → use session at depth 14 → logout-all → reuse fails.
