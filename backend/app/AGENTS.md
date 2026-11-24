# DOX

- Parent: `../AGENTS.md` (the stack) → root `AGENTS.md` (DOX rail)
- This doc owns the Rust server: routes, DB, chess engine, auth, WASM crate, Rust tooling

## Purpose

Rust HTTP server serving portfolio pages, a Lazy SMP chess engine, and FALCON post-quantum auth.

Two independent Cargo packages, not a workspace. This manifest has no `[workspace]` section and `crates/falcon-wasm/` is not a member, so `cargo check` here covers this crate only; WASM changes need their own check (see Verification). For the same reason `--workspace` selects nothing extra and is not used in these commands.

## Ownership

| Path | Owner | Scope |
|------|-------|-------|
| `src/api/` | HTTP API | `mod.rs`, `chess.rs` (chess endpoint + pool/semaphore), `auth.rs` (auth routes), `html.rs` (static serving + cache) |
| `src/main.rs` | Server entry | Axum router, `STATIC_ROUTES` table, logging (`LogMsg`/`LOG_BUFFER`), resource monitor, log flusher, panic hook |
| `src/db.rs` | DB | `connect()` opens an `AsyncPgConnection` and sets `search_path TO public, auth` |
| `src/schema.rs` | DB | Diesel `table!` macro for `auth.keys` with `schema_name = "auth"` |
| `src/auth/` | Auth | `session.rs` (HMAC tokens + challenge nonces), `falcon.rs` (FALCON-1024 verify); routes live in `src/api/auth.rs` |
| `src/games/chess/main.rs` | Chess engine | Board state, move gen, Lazy SMP search, lock-free TT |
| `src/tests/` | Tests | Mirror of `src/` — test files wired via `#[path]` |
| `build.rs` | Build | Links Homebrew `libpq` on macOS (no env var needed) |
| `crates/falcon-wasm/` | WASM | FN-DSA FIPS 206 compiled to `wasm32-unknown-unknown`; `pkg/` is a tracked build artifact |
| `scripts/` | Tooling | `bench-alloc.sh` / `.py` (allocator A/B latency harness), `bench-alloc.Dockerfile` (variant builder), `svg-split.py` / `svg-to-symbols.py` (asset prep) |
| `Cargo.toml`, `Cargo.lock`, `rustfmt.toml`, `clippy.toml`, `deny.toml`, `.config/` | Build | Workspace manifest and Rust tooling config |
| `.env.example` | Config | Template for `backend/.env` (gitignored) — the file Compose reads |

Repo-root files that apply here: `.cargo/config.toml` (platform linker flags) and `rust-toolchain.toml` (pinned channel + targets). They sit at the repo root so `cargo`/`rustup` discover them from any working directory.

## Local Contracts

1. **No client-side connection pool** — PgDog is the pooler. Open/drop as needed.
2. **Zero heap in search** — Board is `[Piece; 64]`, piece lists are `[Square; 16]` arrays (1-byte enums), stack-only. `mimalloc` global allocator.
3. **Depth 1–15** — Invalid depth returns 400. Depth 13–15 require a valid FALCON session with CAP_DEEP_SEARCH (enforced server-side).
4. **Stateless sessions** — HMAC token with `nonce_counter`. No session table. Challenge nonces are single-use (replay-protected); `logout-all` requires a valid session token.
5. **Lazy SMP** — `rayon::scope` with `CHESS_POOL` (dedicated Rayon pool, 256KB stacks).
6. **Semaphore backpressure** — `CHESS_SEMAPHORE`, `try_acquire` → 503 immediately.
7. **Auth** — FALCON pub key is public by definition. Registration is open but refuses to overwrite an existing `key_id` (409); nginx rate-limits `/api/auth/keys` to 5 req/min per IP (429). Private key never touches the server.
8. **Diesel** — Type-safe queries, `table!` macro, no raw SQL for auth.
9. **`STATIC_ROUTES` is the only page route table.** Each entry is `(URL path, CWD-relative file)`. Paths are `frontend/static/...` and `backend/app/crates/falcon-wasm/pkg/...`, which are exactly the paths the image ships them at (CWD `/`). Add a page or module and it needs an entry here.
10. **Run from the repo root.** Every `STATIC_ROUTES` path is CWD-relative and starts with `frontend/` or `backend/app/`, so `cargo run --manifest-path backend/app/Cargo.toml` from the repo root is the only working local invocation.

## Work Guidance

- FALCON verify is wired via `falcon-rs` — FN-DSA-1024 public keys only. `authorize` verifies the signature over the nonce before issuing a session; registration rejects non-1024-param keys.
- Work is tracked in GitHub Issues, not markdown. Open an issue for every new finding instead of adding a file. Every issue carries `p:N` / `b:N` labels for its priority and breaking score (`p:1`–`p:10` high-to-low urgency; `b:0`–`b:10` least-to-most invasive), plus area labels (`auth`, `chess`, `frontend`, `infra`, `ci`, `docs`, `hygiene`).
- Issue titles are `[TYPE]: short description` using conventional-commit types. Issues #1–#26 keep their legacy `[CR-###]` / `[FE-###]` titles because old commit messages name those IDs; leave them alone.
- Known residual risk: signup MITM is unmitigated (no out-of-band trust anchor for the signing tool) — see issue #3 and the corrected threat model in `docs/auth-signing.md`.
- `AUTH_HMAC_SECRET` is required (base64, at least 24 bytes). Server panics if unset.
- `build.rs` links Homebrew `libpq` on macOS — no env vars needed for local builds.
- Tests live in `src/tests/` as a mirror of `src/` (e.g. `src/tests/games/chess/main.rs`), wired via `#[path]`.
- Log tools (`log_error`, etc.) use `#[allow(dead_code)]` — keep them even if unused.
- Falcon-wasm build: `crates/falcon-wasm/build.sh` runs `wasm-bindgen` + `wasm-opt -Oz`.
- The auth tool loads the wasm-bindgen bundle from `/falcon_wasm.js` + `/falcon_wasm_bg.wasm` (`STATIC_ROUTES` → `backend/app/crates/falcon-wasm/pkg/`; `serve_static` is binary-safe). The Dockerfile copies `pkg/` into the image — rebuild it when the crate changes.
- Session token format: `key_id ‖ salt ‖ HMAC(secret, key_id ‖ salt ‖ nonce_counter ‖ expiry)`.
- Challenge-nonce replay store (`USED_NONCES`) is process-local memory — single-instance only. Do not scale to multiple replicas without a shared store (see CR-003 note in `src/auth/session.rs`).
- `.env` files are gitignored — never commit secrets. The one Compose reads is `backend/.env`, not one in this directory: Compose looks in the directory holding `docker-compose.yml`, and this manifest declares no `env_file`.
- Page HTML/CSS/JS is **not** owned here. Pages live in `frontend/static/`; see `frontend/AGENTS.md` for the CSS cascade contract. This crate only serves them.
- `src/api/html.rs` prefers `frontend/static/dist/<file>` over the source file for CSS and falls back for local runs without a build. `frontend/static/dist/` is generated and gitignored — never commit it.

## Verification

- `cargo check --all-targets` — type check of this package (no env vars needed, `build.rs` handles libpq)
- `cargo test` — unit + integration tests
- `cargo clippy --all-targets` — lint
- `cargo check` (in `crates/falcon-wasm/`) — the WASM crate is a separate package, so nothing above covers it
- `cargo nextest run` — faster parallel test runner (config at `.config/nextest.toml`)
- `cargo deny check` — dependency audit (config at `deny.toml`, cargo-deny 0.19 schema)
- `/health` returns `ok`; `/`, `/style.css`, `/static/assets/favicon.svg`, `/falcon_wasm_bg.wasm` must all return 200 from a repo-root run
- Falcon-wasm: `./crates/falcon-wasm/build.sh` produces `pkg/falcon_wasm_bg.wasm` (114KB)

## Child DOX Index

No child docs yet.

Candidates:
- `crates/falcon-wasm/AGENTS.md` — for the WASM crate build pipeline
- `src/auth/AGENTS.md` — if auth logic warrants its own contract
