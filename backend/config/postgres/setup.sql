-- ═══════════════════════════════════════════════════════════════════
-- setup.sql – Homepage first-run database initialization
--
-- Executed by PostgreSQL's docker-entrypoint-initdb.d on a fresh
-- data directory.  Idempotent — safe to re-run manually.
--
-- Mount to: /docker-entrypoint-initdb.d/00-setup.sql
-- ═══════════════════════════════════════════════════════════════════

BEGIN;

-- ── Extensions ───────────────────────────────────────────────────

CREATE EXTENSION IF NOT EXISTS pgcrypto WITH SCHEMA public;

-- ── Auth schema ─────────────────────────────────────────────────

CREATE SCHEMA IF NOT EXISTS auth;

-- ═══════════════════════════════════════════════════════════════════
-- Auth: FALCON + HMAC Challenge-Response + Stateless Sessions
--
-- Generic auth for the entire homepage API (chess, projects, etc.).
--
──
-- FLOW
──
--   REGISTER KEY (open — no proof required, pub key is public)
─     POST /api/auth/keys  { "key_id":"laptop", "pub_key":"<base64>" }
──
--   1. CHALLENGE  (stateless — HMAC, no DB)
--     GET /auth/challenge?key_id=laptop
--     ← { nonce(32B hex), hmac_token, expiry_unix }
--     Server: hmac_token = HMAC(secret, "chal" ‖ key_id ‖ nonce ‖ expiry)
──
--   2. AUTHORIZE  (one FALCON verify, returns stateless session)
--     POST /auth/authorize  { "key_id":"laptop", "nonce":"...",
--                              "hmac_token":"...", "falcon_sig":"<base64>" }
--     Server: verify HMAC → verify FALCON sig(nonce) against pub_key
--             → build session_token
--             ← { session_token, max_depth, expires_at }
──
--   3. USE  (no FALCON — just parse token + quick DB counter check)
--     Any route: include ?session_token=... in request
--     Server: parse key_id from token → SELECT nonce_counter
--             → recompute HMAC → reject if stale → process
──
--   LOG OUT ALL
--     POST /auth/logout-all  { "key_id":"laptop" }
--     Server: INCREMENT nonce_counter → all old session HMACs invalid
─
-- SESSION TOKEN FORMAT (hex, split by '.')
──
--   key_id_hex . session_salt_hex(32) . expiry_hex . hmac_hex(64)
──
--   hmac = HMAC-SHA256(secret, "session" ‖ key_id ‖ salt ‖ counter ‖ expiry)
--   salt = 16 random bytes (unique per issuance, even at same counter)
──
-- DB READS PER REQUEST
──
--   Challenge:  0 (pure HMAC)
--   Authorize:  1 (SELECT pub_key, nonce_counter, max_depth)
--   Use:        1 (SELECT nonce_counter)
--   Logout all: 1 (UPDATE nonce_counter)
─
-- DB WRITES PER REQUEST
──
--   Authorize:  0 (session is stateless)
-- ═══════════════════════════════════════════════════════════════════

CREATE TABLE IF NOT EXISTS auth.keys (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    key_id          TEXT        NOT NULL UNIQUE,
    label           TEXT        NOT NULL DEFAULT '',
    pub_key         BYTEA       NOT NULL,
    capabilities    SMALLINT    NOT NULL DEFAULT 15,  -- bitmask or tier: 8 = base, 15 = full
    is_active       BOOLEAN     NOT NULL DEFAULT true,
    nonce_counter   BIGINT      NOT NULL DEFAULT 0,   -- ⬆ = logout all
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE  auth.keys              IS 'FALCON public keys + per-key nonce counter for revocable stateless sessions';
COMMENT ON COLUMN auth.keys.capabilities IS 'Access tier. Currently: 15 = full access. Future: bitmask for per-feature gating.';

CREATE INDEX IF NOT EXISTS idx_auth_keys_key_id ON auth.keys (key_id);

-- ── Look up FALCON pub key + capabilities for authorize step ─────

CREATE OR REPLACE FUNCTION auth.lookup_key(p_key_id TEXT)
    RETURNS TABLE (pub_key BYTEA, capabilities SMALLINT, nonce_counter BIGINT)
    LANGUAGE sql
    STRICT
    STABLE
    PARALLEL SAFE
    AS $$
    SELECT k.pub_key, k.capabilities, k.nonce_counter
        FROM auth.keys k
        WHERE k.key_id = p_key_id
          AND k.is_active = true;
$$;

-- ── Check counter for session verification (tiny read) ───────────

CREATE OR REPLACE FUNCTION auth.get_nonce_counter(p_key_id TEXT)
    RETURNS BIGINT
    LANGUAGE sql
    STRICT
    STABLE
    PARALLEL SAFE
    AS $$
    SELECT k.nonce_counter FROM auth.keys k WHERE k.key_id = p_key_id;
$$;

-- ── Invalidate all sessions for a key ────────────────────────────

CREATE OR REPLACE FUNCTION auth.increment_nonce_counter(p_key_id TEXT)
    RETURNS BIGINT
    LANGUAGE sql
    STRICT
    VOLATILE
    AS $$
    UPDATE auth.keys
        SET nonce_counter = nonce_counter + 1
        WHERE key_id = p_key_id
        RETURNING nonce_counter;
$$;

-- ═══════════════════════════════════════════════════════════════════
-- Bootstrap: no default keys — register via API after server starts
-- ═══════════════════════════════════════════════════════════════════

COMMIT;
