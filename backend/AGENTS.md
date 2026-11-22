# DOX

- DOX is a highly performant AGENTS.md hierarchy installed here
- Agent must follow DOX instructions across any edits

## Purpose

Rust HTTP server serving portfolio pages, a Lazy SMP chess engine, and FALCON post-quantum auth.

## Ownership

| Path | Owner | Scope |
|------|-------|-------|
| `src/api/` | HTTP API | `mod.rs`, `chess.rs` (chess endpoint + pool/semaphore), `auth.rs` (auth routes), `html.rs` (static serving + cache) |
| `src/main.rs` | Server entry | Axum router, logging (`LogMsg`/`LOG_BUFFER`), resource monitor, log flusher, panic hook |
| `src/db.rs` | DB | `connect()` opens an `AsyncPgConnection` and sets `search_path TO public, auth` |
| `src/schema.rs` | DB | Diesel `table!` macro for `auth.keys` with `schema_name = "auth"` |
| `src/auth/` | Auth | `session.rs` (HMAC tokens + challenge nonces), `falcon.rs` (FALCON-1024 verify); routes live in `src/api/auth.rs` |
| `src/games/chess/main.rs` | Chess engine | Board state, move gen, Lazy SMP search, lock-free TT |
| `src/tests/` | Tests | Mirror of `src/` — test files wired via `#[path]` |
| `build.rs` | Build | Links Homebrew `libpq` on macOS (no env var needed) |
| `crates/falcon-wasm/` | WASM | FN-DSA FIPS 206 compiled to `wasm32-unknown-unknown` |
| `config/` | Infra | `pgdog/` + `postgres/` config files |
| `docker-compose.yml` | Infra | Nginx + Rust + Postgres + PgDog orchestration |
| `Dockerfile` | Infra | Multi-stage Rust build + UPX + scratch image |
| `package.json`, `package-lock.json`, `postcss.config.cjs`, `tailwind.input.css`, `.stylelintrc.json` | CSS tooling | Tailwind v4 utilities (FE-006a) + PostCSS nesting flattening (FE-003) + stylelint |
| `nginx/` | Infra | Nginx reverse proxy config |
| `static/` | Frontend | HTML/CSS/JS, portfolio pages, auth tool |
| `docs/` | Docs | Auth protocol spec (`auth-signing.md`) |
| `docs/specs/` | Docs | Feature design specs (SDD design docs) |
| `docs/plans/` | Docs | Implementation plans (SDD task briefs) |
| `docs/Todo/` | Docs | Todo tracking system (Guide.md rules; todo.md open, complete.md done) |
| `scripts/` | Tooling | `bench-alloc.sh` (orchestrator, `--compare` builds mimalloc + jemalloc variants), `bench-alloc.py` (HTTP latency sampling), `bench-alloc.Dockerfile` (A/B variant builder); `svg-split.py` / `svg-to-symbols.py` (SVG asset prep) |

## Local Contracts

1. **No client-side connection pool** — PgDog is the pooler. Open/drop as needed.
2. **Zero heap in search** — Board is `[Piece; 64]`, piece lists are `[Square; 16]` arrays (1-byte enums), stack-only. `mimalloc` global allocator.
3. **Depth 1–15** — Invalid depth returns 400. Depth 13–15 require a valid FALCON session with CAP_DEEP_SEARCH (enforced server-side).
4. **Stateless sessions** — HMAC token with `nonce_counter`. No session table. Challenge nonces are single-use (replay-protected); `logout-all` requires a valid session token.
5. **Lazy SMP** — `rayon::scope` with `CHESS_POOL` (dedicated Rayon pool, 256KB stacks).
6. **Semaphore backpressure** — `CHESS_SEMAPHORE`, `try_acquire` → 503 immediately.
7. **Auth** — FALCON pub key is public by definition. Registration is open but refuses to overwrite an existing `key_id` (409); nginx rate-limits `/api/auth/keys` to 5 req/min per IP (429). Private key never touches the server.
8. **Diesel** — Type-safe queries, `table!` macro, no raw SQL for auth.

## Work Guidance

- FALCON verify is wired via `falcon-rs` — FN-DSA-1024 public keys only. `authorize` verifies the signature over the nonce before issuing a session; registration rejects non-1024-param keys.
- Known residual risk: signup MITM is unmitigated (no out-of-band trust anchor for the signing tool) — see CR-011 in `docs/Todo/todo.md` and the corrected threat model in `docs/auth-signing.md`.
- `AUTH_HMAC_SECRET` is required (base64, at least 24 bytes). Server panics if unset.
- `build.rs` links Homebrew `libpq` on macOS — no env vars needed for local builds.
- Tests live in `src/tests/` as a mirror of `src/` (e.g. `src/tests/games/chess/main.rs`), wired via `#[path]`.
- Log tools (`log_error`, etc.) use `#[allow(dead_code)]` — keep them even if unused.
- Falcon-wasm build: `crates/falcon-wasm/build.sh` runs `wasm-bindgen` + `wasm-opt -Oz`.
- Auth tool loads the wasm-bindgen bundle from `/falcon_wasm.js` + `/falcon_wasm_bg.wasm` (`STATIC_ROUTES` → `crates/falcon-wasm/pkg/`; `serve_static` is binary-safe). The Dockerfile copies `crates/falcon-wasm/pkg/` into the image — rebuild it when the crate changes.
- Session token format: `key_id ‖ salt ‖ HMAC(secret, key_id ‖ salt ‖ nonce_counter ‖ expiry)`.
- Challenge-nonce replay store (`USED_NONCES`) is process-local memory — single-instance only. Do not scale to multiple replicas without a shared store (see CR-003 note in `src/auth/session.rs`).
- `.env` is gitignored — never commit secrets.
- CSS generation happens only in the Docker `css-builder` stage. PostCSS flattens native nesting from `static/*.css` into `static/dist/*.css`, and the Tailwind v4 CLI compiles `tailwind.input.css` into `static/dist/tailwind.css`. The Tailwind input sits at the backend root, not in `static/`, so the `static/*.css` glob stays author-CSS only. Pages link bare names (`style.css`, `tailwind.css`); `src/api/html.rs` prefers `dist` over the source file and falls back for local `cargo run`. `static/dist/` is generated and gitignored — never commit it.
- CSS and page checks are split across two workflows, both read-only and both pinned by commit SHA. `.github/workflows/css-lint.yml` runs `lint:css` and nothing else — CSS generation and its output checks stay in the Docker build so CI does not duplicate them. `.github/workflows/frontend-verify.yml` runs `lint:html` (HTMLHint) plus the Playwright smoke suite in Chromium over all four pages.
- **Cascade contract for CSS (FE-006b).** Layer order is declared once, in `tailwind.input.css`: `@layer theme, base, components, utilities`. Unlayered CSS beats every layer regardless of specificity, so an unlayered `* { margin: 0 }` silently defeats `pt-24` — that is exactly what happened, and why `common.css`'s reset and body/theme globals now live in `@layer base`. Everything else in `common.css` and all of `style.css`, `contact.css` and `projects.css` stay unlayered or in `@layer components` so existing component rules keep winning over utilities. New page styles: utilities in the markup, bespoke surfaces in `@layer components`.
- Two Tailwind v4 traps when migrating CSS. Named type-scale utilities pair a line-height (`text-base` turns `16px/normal` into `16px/24px`), so use arbitrary sizes like `text-[1em]` to set font-size alone. And `border-<color>` sets all four sides — a left-only border needs `border-l-<color>`.
- Pages link bare CSS names (`style.css`, `tailwind.css`) and `src/api/html.rs` serves the `dist/` copy when present. The e2e harness in `e2e/serve.mjs` mirrors that preference, so a page works there exactly as it does in production.

## Verification

- `cargo check` — type check (no env vars needed, `build.rs` handles libpq)
- `cargo test` — unit + integration tests
- `cargo clippy` — lint
- `cargo nextest run` — faster parallel test runner
- `cargo deny check` — dependency audit
- `npm ci && npm run build:css` (in `backend/`) — build `static/dist/` (Tailwind utilities + flattened nesting); `build:tailwind` and `build:flatten` run the halves separately
- `npm run lint:css` (in `backend/`) — stylelint correctness + nesting depth
- `npm run lint:html` (in `backend/`) — HTMLHint over `static/*.html`, config in `.htmlhintrc`
- `npm run test:e2e` (in `backend/`) — Playwright smoke: every page loads without an uncaught exception, the terminal answers `help`, `echo` keeps markup inert, and the auth modules boot; first run needs `npx playwright install chromium`
- Falcon-wasm: `./crates/falcon-wasm/build.sh` produces `pkg/falcon_wasm_bg.wasm` (114KB)

## Child DOX Index

- `docs/Todo/AGENTS.md` — file-based todo tracking system (Guide.md, todo.md, complete.md)

Candidates:
- `crates/falcon-wasm/AGENTS.md` — for the WASM crate build pipeline
- `src/auth/AGENTS.md` — if auth logic warrants its own contract
