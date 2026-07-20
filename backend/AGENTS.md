# DOX

- DOX is a highly performant AGENTS.md hierarchy installed here
- Agent must follow DOX instructions across any edits

## Purpose

Rust HTTP server serving portfolio pages, a Lazy SMP chess engine, and FALCON post-quantum auth.

## Ownership

| Path | Owner | Scope |
|------|-------|-------|
| `src/main.rs` | Server entry | Axum router, semaphore, resource monitor, log flusher, Rayon pool, auth routes |
| `src/db.rs` | DB | `connect()` opens an `AsyncPgConnection` and sets `search_path TO public, auth` |
| `src/schema.rs` | DB | Diesel `table!` macro for `auth.keys` with `schema_name = "auth"` |
| `src/auth/` | Auth | `mod.rs`, `handlers.rs` (routes), `session.rs` (HMAC tokens) |
| `src/games/chess/main.rs` | Chess engine | Board state, move gen, Lazy SMP search, lock-free TT |
| `src/tests/` | Tests | Mirror of `src/` — test files wired via `#[path]` |
| `build.rs` | Build | Links Homebrew `libpq` on macOS (no env var needed) |
| `crates/falcon-wasm/` | WASM | FN-DSA FIPS 206 compiled to `wasm32-unknown-unknown` |
| `config/` | Infra | `pgdog/` + `postgres/` config files |
| `docker-compose.yml` | Infra | Nginx + Rust + Postgres + PgDog orchestration |
| `Dockerfile` | Infra | Multi-stage Rust build + UPX + scratch image |
| `nginx/` | Infra | Nginx reverse proxy config |
| `static/` | Frontend | HTML/CSS/JS, portfolio pages, auth tool |
| `docs/` | Docs | Auth protocol spec |

## Local Contracts

1. **No client-side connection pool** — PgDog is the pooler. Open/drop as needed.
2. **Zero heap in search** — Piece lists are `[u8; 16]` arrays, stack-only. `mimalloc` global allocator.
3. **Depth 1–15** — Invalid depth returns 400. Depth 13+ gated behind FALCON auth (not wired yet).
4. **Stateless sessions** — HMAC token with `nonce_counter`. No session table.
5. **Lazy SMP** — `rayon::scope` with `CHESS_POOL` (dedicated Rayon pool, 256KB stacks).
6. **Semaphore backpressure** — `CHESS_SEMAPHORE`, `try_acquire` → 503 immediately.
7. **Auth** — FALCON pub key is public by definition. Signup is open. Private key never touches the server.
8. **Diesel** — Type-safe queries, `table!` macro, no raw SQL for auth.

## Work Guidance

- FALCON verify on server is **stubbed** (accepts any signature). Wire `fn_dsa::verify()` when ready.
- `AUTH_HMAC_SECRET` is required (base64, at least 24 bytes). Server panics if unset.
- `build.rs` links Homebrew `libpq` on macOS — no env vars needed for local builds.
- Tests live in `src/tests/` as a mirror of `src/` (e.g. `src/tests/games/chess/main.rs`), wired via `#[path]`.
- Log tools (`log_error`, etc.) use `#[allow(dead_code)]` — keep them even if unused.
- Falcon-wasm build: `crates/falcon-wasm/build.sh` runs `wasm-bindgen` + `wasm-opt -Oz`.
- Session token format: `key_id ‖ salt ‖ HMAC(secret, key_id ‖ salt ‖ nonce_counter ‖ expiry)`.
- `.env` is gitignored — never commit secrets.

## Verification

- `cargo check` — type check (no env vars needed, `build.rs` handles libpq)
- `cargo test` — unit + integration tests
- `cargo clippy` — lint
- `cargo nextest run` — faster parallel test runner
- `cargo deny check` — dependency audit
- Falcon-wasm: `./crates/falcon-wasm/build.sh` produces `pkg/falcon_wasm_bg.wasm` (114KB)

## Child DOX Index

No children yet. Candidates:
- `crates/falcon-wasm/AGENTS.md` — for the WASM crate build pipeline
- `src/auth/AGENTS.md` — if auth logic warrants its own contract
