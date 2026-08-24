# Auth: Signing Protocol

## Overview

Stateless, post-quantum auth for the homepage API. No passwords, no shared secrets, no email. Uses **FALCON** (NIST PQC standard) lattice signatures with **HMAC** challenge-response and **nonce counter** revocation.

```
pub_key = public (sent over wire, stored server-side)
priv_key = private (never leaves the machine)
session = stateless (HMAC-bound to key_id + nonce_counter)
```

---

## Key Generation (Client-side WASM)

When a user signs up, the browser generates a FALCON-1024 keypair in WASM:

```
Keypair = FALCON.GenerateKeypair()

File: homepage-key.priv (PEM-style, downloaded once)
────────────────────────────────────────────────────────
-----BEGIN HOMEPAGE FALCON KEY-----
key_id: laptop
encrypted: argon2id
salt: a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6
nonce: a1b2c3d4e5f6a7b8c9d0e1f2

5v8yXz... (AES-GCM encrypted private key bytes)
-----END HOMEPAGE FALCON KEY-----
────────────────────────────────────────────────────────

Unencrypted variant (not recommended):
-----BEGIN HOMEPAGE FALCON KEY-----
key_id: laptop
encrypted: none

5v8yXz... (raw private key bytes)
-----END HOMEPAGE FALCON KEY-----
```

The pub key is sent to the server, the private key is only saved to file. Never uploaded.

### Optional Encryption

| Field                 | Purpose                                               |
| --------------------- | ----------------------------------------------------- |
| `encrypted: none`     | Raw key, no passphrase                                |
| `encrypted: argon2id` | Key encrypted with Argon2id(passphrase) → AES-256-GCM |
| `salt` (16B hex)      | Argon2id salt                                         |
| `nonce` (12B hex)     | AES-GCM nonce                                         |

If encrypted, the WASM prompts for a passphrase before signing. Without it, the key is usable immediately from file.

---

## Protocol

### 1. Register

```
POST /api/auth/keys
{
    "key_id": "laptop",       // user-chosen identifier
    "pub_key": "5v8yXz..."    // FALCON-1024 public key (base64)
}
→ 201 { "key_id": "laptop" }
```

Open registration. No proof required — pub key is public by definition. The server stores it in `auth.keys`.

If a client registers a key they don't hold the private key for, they simply can't complete step 2.

### 2. Challenge

```
GET /api/auth/challenge?key_id=laptop
→ 200
{
    "nonce": "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1",
    "hmac_token": "b7c8d9...",
    "expires_at": 1712345678
}
```

Server generates 32 random bytes as the nonce. Returns it alongside:

```
hmac_token = HMAC-SHA256(
    AUTH_HMAC_SECRET,
    "challenge" || key_id || nonce || expires_at
)
```

No database write — the challenge is fully stateless. The HMAC proves the server issued this exact nonce.

### 3. Authorize

```
POST /api/auth/authorize
{
    "key_id": "laptop",
    "nonce": "a1b2c3...",
    "hmac_token": "b7c8d9...",
    "falcon_sig": "9z8y7x..."   // FALCON-1024 signature over nonce (base64)
}
→ 200
{
    "session_token": "6c6170746f70...",
    "capabilities": 15,
    "expires_at": 1712349278
}
```

Server:

```
1. Recompute HMAC("challenge" || key_id || nonce || expiry)
   → Compare to hmac_token (timing-safe)
   → Fail if mismatch (forged or expired challenge)

2. SELECT pub_key, capabilities, nonce_counter
   FROM auth.keys
   WHERE key_id = $1 AND is_active = true

3. FALCON.Verify(pub_key, nonce, falcon_sig)
   → Fail if invalid (wrong private key or tampered nonce)

4. Build session token
```

### 4. Session Token (Stateless)

Format (hex-encoded, dot-separated):

```
key_id_hex . salt_hex(32) . expiry_hex . hmac_hex(64)
│              │               │              └── HMAC-SHA256(secret, "session" || key_id || salt || nonce_counter || expiry)
│              │               └── Unix timestamp (hex)
│              └── 16 random bytes (unique per issuance)
└── key_id encoded as hex
```

Example:

```
6c6170746f70.a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6.6612a3b4.c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0
```

The session token is lightweight: the server only needs the `key_id` (plaintext) and recomputes the HMAC with the stored `nonce_counter`. No session table.

### 5. Use

```
GET /api/chess/completions?fen=...&session_token=6c6170746f70...
Authorization: Bearer <session_token>  (alternative)
```

Server:

```
1. Parse key_id_hex from session_token → decode → "laptop"
2. SELECT nonce_counter FROM auth.keys WHERE key_id = 'laptop'
3. Recompute HMAC("session" || key_id || salt || nonce_counter || expiry)
   → Compare (timing-safe)
   → Fail if mismatch (revoked, expired, or forged)
4. Check capabilities ≥ required level
5. Process request
```

### 6. Logout All

```
POST /api/auth/logout-all
{
    "key_id": "laptop"
}
→ 200 { "nonce_counter": 3 }
```

Server increments `nonce_counter` for the key. All existing session tokens for this key become invalid immediately (HMAC mismatch on next use).

```
UPDATE auth.keys SET nonce_counter = nonce_counter + 1 WHERE key_id = 'laptop';
```

---

## Cost per Request

| Step       | DB Reads           | DB Writes          | Crypto                     |
| ---------- | ------------------ | ------------------ | -------------------------- |
| Register   | 1 (INSERT)         | 1                  | None                       |
| Challenge  | 0                  | 0                  | 1× HMAC                    |
| Authorize  | 1 (SELECT key)     | 0                  | 1× HMAC + 1× FALCON verify |
| Use        | 1 (SELECT counter) | 0                  | 1× HMAC                    |
| Logout All | 0                  | 1 (UPDATE counter) | None                       |

---

## Offline Signing (.mhtml)

For users who want the signing step isolated from their browsing context, a self-contained `.mhtml` file handles it offline:

**homepage-auth.mhtml** (single file, served once):

```
┌────────────────────────────────────────────────────────────────┐
│  homepage-auth.mhtml (offline enclave)                         │
│                                                                │
│  ┌──────────────┐  ┌────────────────┐  ┌────────────────────┐ │
│  │ HTML UI      │  │ WASM           │  │ Key file           │ │
│  │ Input fields │  │ FALCON + Argon │  │ Loaded via         │ │
│  │ Output copy  │  │ AES-GCM        │  │ <input type="file">│ │
│  └──────────────┘  └────────────────┘  └────────────────────┘ │
│                                                                │
│  file:// URI — never phones home                               │
└────────────────────────────────────────────────────────────────┘
```

### Flow

```
1. Browse homepage at https://monvip.dev
2. Click "Sign in" → copy the challenge JSON from the page
3. Open homepage-auth.mhtml (local file)
4. Upload homepage-key.priv via file input
5. Enter passphrase (if encrypted) → WASM decrypts key
6. Paste challenge JSON → click "Sign"
7. WASM computes FALCON signature locally
8. Copy signature JSON from .mhtml → paste back into homepage
9. Submit → session token returned
```

### Security Properties

- **No network calls**: While running, the .mhtml is loaded from `file://`. Any attempt to `fetch()` data is visible in the dev tools and trivially auditable.
- **Manual copy/paste isolation**: The challenge goes in, the signature comes out. No automated channel for exfiltration from the page context.
- **Key never uploaded**: The private key is read into WASM memory via a local file input. Never POST'd.

These properties hold against **passive phishing pages**: there is no password field to phish, no credentials typed into the site, and signing runs isolated from the browsing context. They do **not** hold against an active proxy on first download — the .mhtml itself is served by the same site the attacker may be proxying, so the attacker can tamper with it and with any checksum published beside it (see [The One Fail-Case](#the-one-fail-case-signup-mitm)). "Verifiable source" only works if you already hold a trusted copy of the hash from somewhere other than the site — there is no such out-of-band channel today.

### Limitations

- Copy/paste across windows is friction
- WASM from `file://` may need browser flag adjustments (or serve the .mhtml once from the server, then save locally)
- Does not protect against a compromised machine (keylogger, clipboard snooper)
- Does not protect against an active proxy tampering with the .mhtml at download time — no independent distribution channel exists

---

## Schema

```sql
-- Full schema in config/postgres/setup.sql

CREATE SCHEMA IF NOT EXISTS auth;

CREATE TABLE auth.keys (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    key_id          TEXT        NOT NULL UNIQUE,
    label           TEXT        NOT NULL DEFAULT '',
    pub_key         BYTEA       NOT NULL,          -- raw FALCON-1024 public key
    capabilities    SMALLINT    NOT NULL DEFAULT 15,-- access tier / bitmask
    is_active       BOOLEAN     NOT NULL DEFAULT true,
    nonce_counter   BIGINT      NOT NULL DEFAULT 0,-- ⬆ increment = logout all
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

---

## Environment

| Variable           | Purpose                                   |
| ------------------ | ----------------------------------------- |
| `AUTH_HMAC_SECRET` | 32+ random bytes (base64). Never exposed. |

---

## Phishing Resistance

This auth system was designed to minimise phishing surface against **passive phishing pages**. Here's how each layer addresses a distinct vector — and where the limits are against an active proxy.

### No Password Field

There is no `<input type="password">`. A phisher can't build a fake login form because there is no login form. The user authenticates with a local file, not a server-side secret.

### The .mhtml Air Gap

The signing step can run in a **separate application window** that doesn't touch the network while it runs:

```
Phishing page (attacker)              .mhtml (user's local file)
─────────────────────────             ────────────────────────────
Shows fake site
User copies challenge ──────────→  Pasted into .mhtml
                                    Signs with local key
User pastes signature ←──────────  Copied from .mhtml
```

The air gap covers only the signing step itself. It does not authenticate the tool doing the signing: the .mhtml is downloaded from the same site an attacker may be proxying, so the attacker can tamper with the .mhtml — and with any checksum published beside it — exactly as they can tamper with the signup WASM:

- It's a local `file://` once saved — but nothing guarantees the file the user saved is the real one
- The published checksum lives on the same proxied site — verifying against it proves nothing under MITM
- There is no second channel (OS package manager, app store, independently pinned hash) anywhere in this flow

Against an active proxy at first download, the .mhtml changes _where_ signing happens, not _who_ delivered the signing code.

### Manual Copy/Paste

The challenge and signature cross the window boundary via **user-managed copy/paste**. There is no automated channel — no `postMessage`, no `fetch`, no `WebSocket`. This keeps the signing tool isolated from the browsing context and prevents silent scripted extraction of signatures. It does not authenticate anything: the user cannot distinguish a genuine challenge from one crafted by a proxying attacker, and if the signing code itself was tampered with at download time, manual transport makes no difference.

### Key Never Uploaded

The private key is loaded via `<input type="file">` into the .mhtml's WASM memory. It never travels over the network, not even during signup. The server only ever sees the public key.

### Optional Passphrase Encryption

If the key file is encrypted with a passphrase, even compromising the file doesn't help. The passphrase is typed into the .mhtml, never the website. A fake login page can't ask for it because there's no password field.

### Session Token is Opaque

The session token is a hex blob. A phisher can't craft a fake one without `AUTH_HMAC_SECRET`. Even if they steal one, the user can hit "logout all" and the nonce_counter invalidates every token instantly.

### Nothing to Leak

| Compromised asset          | What attacker gets                                               |
| -------------------------- | ---------------------------------------------------------------- |
| `auth.keys` table          | Public keys (public by definition)                               |
| `AUTH_HMAC_SECRET` env var | Can forge challenge tokens (still can't sign without FALCON key) |
| Session token in transit   | Temporary access (user logs out all → dead)                      |
| .mhtml file                | WASM binary (no key inside — loaded separately)                  |

---

## The One Fail-Case: Signup MITM

The weakest point in this system (and any auth system) is **first-time key generation**.

### The Attack

```
User's machine                     Attacker (MITM)                  Real Server
──────────────                     ───────────────                  ───────────
User visits https://monvip.dev
                       ── DNS/HTTP intercept ──→  Attacker proxies
                                                   request to real server,
                                                   but injects malicious WASM

User clicks "Generate Key"
Malicious WASM generates keypair
                       ── pub_key ──────────────→  Registers account ✓
                       ← 201 Created ←──────────
User sees "Account created!"
No red flags — everything works.

Malicious WASM sends priv_key ──→  Attacker stores it
User downloads key file
                      (attacker also has the same key file)

Later, attacker ── signs challenges as user ──→  Authorized ✓
```

The key difference: the user sees a **successful registration** on the real server. The pub key is legitimate, the account works, they can log in and use the site. There are zero visible red flags. But the attacker silently walked away with the private key and can authenticate as that user at any time.

The WASM served during signup is the trust root. If it's compromised, the generated keypair is compromised — the user downloads a key file the attacker also has.

### Why It's Hard to Fix

- The user has no pre-existing trust anchor on first visit (no pinned key, no local tool)
- SRI (Subresource Integrity) only checks that the bytes match the expected hash — but the user doesn't know the expected hash on first visit
- HTTPS doesn't help if the attacker controls the DNS or has a rogue CA

### Mitigations

| Mitigation                                   | What it prevents                                                            | Bypass                                                 |
| -------------------------------------------- | --------------------------------------------------------------------------- | ------------------------------------------------------ |
| **Generate key offline first**               | User generates keypair via .mhtml before visiting the site                  | Only helps if the .mhtml itself was obtained through an independent channel |
| **SRI + pinning**                            | WASM integrity is checked against a known hash                              | First visit has no pin                              |
| **Key verification handshake**               | After registration, user signs a challenge to prove key ownership           | Doesn't help — both user and attacker have the key     |
| **Logout all at end of session**             | Limits window of compromise                                                 | Attacker still has the key file                        |
| **Passphrase encryption**                    | Key file is useless without the passphrase                                  | Passphrase entered into compromised WASM = also stolen |
| **Generate key in .mhtml, register via API** | Moves signing out of the site's browsing context                            | Doesn't stop MITM — the .mhtml comes from the same proxied channel (see below) |

### Why Offline Generation Alone Doesn't Fix It

Generating the keypair in the .mhtml instead of the website moves where signing happens — but the .mhtml is downloaded from the same site an attacker may be proxying. The attacker can tamper with the .mhtml exactly like the signup WASM, and with any checksum published beside it, because there is no out-of-band channel anywhere in this loop. The trust root still lands on the network.

This failure mode is not unique to FALCON — every credential system is vulnerable to a proxying attacker at first-use/first-registration time:

| System                   | Signup attack                                                                       |
| ------------------------ | ----------------------------------------------------------------------------------- |
| Password                 | Phisher steals password at signup                                                   |
| OAuth (Google/GitHub)    | Attacker redirects to fake OAuth consent screen                                     |
| WebAuthn (hardware key)  | Attacker registers their own key if they control the page                           |
| **FALCON (this system)** | Attacker steals generated keypair                                                   |
| SSH key                  | Same problem — mitigated only because OpenSSH ships via OS package managers, an independent channel |

### What a Real Fix Requires

A genuinely independent trust anchor for the signing tool, for example:

- Second-channel distribution (OS package manager, app store, physical media)
- A pinned hash obtained from infrastructure independent of the site itself

Neither exists in the current deployment. Until one does, **signup MITM is unmitigated** — accepted residual risk, tracked as CR-011 in `docs/Todo/todo.md`.

---

## Threat Model

| Attack                  | Mitigation                                                     |
| ----------------------- | -------------------------------------------------------------- |
| Server DB dumped        | Only pub keys leaked — public by definition                    |
| Forged challenge token  | HMAC verification (attacker doesn't have `AUTH_HMAC_SECRET`)   |
| Replay attack           | Per-request nonces in challenge; session expiry; nonce_counter |
| Phished private key     | Passphrase encryption (Argon2id + AES-GCM); .mhtml isolates signing — but not against a proxying attacker at first download (see Signup MITM) |
| Quantum computer        | FALCON-1024 is NIST PQC standard — quantum-safe                 |
| Session token stolen    | Tied to nonce_counter — logout all kills it instantly          |
| Brute force HMAC secret | Server never reveals HMAC failure vs key failure (same error)  |
| WASM tampering          | Checksum verification exists, but the checksum shares the delivery channel — no independent trust anchor today (CR-011) |
