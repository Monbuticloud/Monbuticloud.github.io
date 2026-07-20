# DOX

- DOX is a highly performant AGENTS.md hierarchy installed here
- Agents must follow DOX instructions across any edits

## Core Contract

- AGENTS.md files are binding work contracts for their subtrees
- Work products, source materials, instructions, records, assets, and durable docs must stay understandable from the nearest applicable AGENTS.md plus every parent AGENTS.md above it

## Read Before Editing

1. Read the root AGENTS.md
2. Identify every file or folder you expect to touch
3. Walk from the repository root to each target path
4. Read every AGENTS.md found along each route
5. If a parent AGENTS.md lists a child AGENTS.md whose scope contains the path, read that child and continue from there
6. Use the nearest AGENTS.md as the local contract and parent docs for repo-wide rules
7. If docs conflict, the closer doc controls local work details, but no child doc may weaken DOX

Do not rely on memory. Re-read the applicable DOX chain in the current session before editing.

## Update After Editing

Every meaningful change requires a DOX pass before the task is done.

Update the closest owning AGENTS.md when a change affects:

- purpose, scope, ownership, or responsibilities
- durable structure, contracts, workflows, or operating rules
- required inputs, outputs, permissions, constraints, side effects, or artifacts
- user preferences about behavior, communication, process, organization, or quality
- AGENTS.md creation, deletion, move, rename, or index contents

Update parent docs when parent-level structure, ownership, workflow, or child index changes. Update child docs when parent changes alter local rules. Remove stale or contradictory text immediately. Small edits that do not change behavior or contracts may leave docs unchanged, but the DOX pass still must happen.

## Hierarchy

- Root AGENTS.md is the DOX rail: project-wide instructions, global preferences, durable workflow rules, and the top-level Child DOX Index
- Child AGENTS.md files own domain-specific instructions and their own Child DOX Index
- Each parent explains what its direct children cover and what stays owned by the parent
- The closer a doc is to the work, the more specific and practical it must be

## Child Doc Shape

- Create a child AGENTS.md when a folder becomes a durable boundary with its own purpose, rules, responsibilities, workflow, materials, or quality standards
- Work Guidance must reflect the current standards of the project or user instructions; if there are no specific standards or instructions yet, leave it empty
- Verification must reflect an existing check; if no verification framework exists yet, leave it empty and update it when one exists

Default section order:
- Purpose
- Ownership
- Local Contracts
- Work Guidance
- Verification
- Child DOX Index

## Style

- Keep docs concise, current, and operational
- Document stable contracts, not diary entries
- Put broad rules in parent docs and concrete details in child docs
- Prefer direct bullets with explicit names
- Do not duplicate rules across many files unless each scope needs a local version
- Delete stale notes instead of explaining history
- Trim obvious statements, repeated rules, misplaced detail, and warnings for risks that no longer exist

## Closeout

1. Re-check changed paths against the DOX chain
2. Update nearest owning docs and any affected parents or children
3. Refresh every affected Child DOX Index
4. Remove stale or contradictory text
5. Run existing verification when relevant
6. Report any docs intentionally left unchanged and why

## User Preferences

- Monbuticloud's personal monorepo: portfolio site + chess engine backend
- Frontend deploys to GitHub Pages from `./frontend`
- Backend runs via docker-compose locally (Nginx + Rust + Postgres + PgDog)
- Lazy SMP chess engine uses `rayon::scope`, lock-free transposition table
- Auth: FALCON (FN-DSA FIPS 206) post-quantum signatures + HMAC challenge-response
- No heap allocations inside chess search/move generation
- `mimalloc` global allocator
- Strict `#[allow(dead_code)]` on log tools — keep them even if unused

## Purpose

Monorepo for Monbuticloud's personal portfolio site and projects.
- **backend/** — Rust HTTP server (Axum) serving portfolio pages, a Lazy SMP chess engine, and FALCON-based auth. Stack: Rust, Axum, Diesel (async), PostgreSQL, PgDog, Nginx, Docker.
- **frontend/** — Stub for client-side app. Deployed to GitHub Pages via CI. Not yet built.

## Ownership

| Path | Owner | Scope |
|------|-------|-------|
| `backend/` | Rust server | HTTP routes, chess engine, DB, auth, static assets, Docker infra |
| `frontend/` | (future) | Client-side UI — not yet implemented |
| `.github/workflows/` | CI/CD | GitHub Actions: pages deployment from `./frontend` |

## Local Contracts

1. **Backend**: Run with `docker compose up --build` from `backend/`. Depends on Docker Desktop, `wasm-bindgen` + `wasm-opt` for falcon-wasm.
2. **Frontend**: Static HTML stub. Once built, will deploy from `frontend/` to GitHub Pages.
3. **Auth**: FALCON pub key is public. Private key generated client-side, never uploaded. HMAC stateless sessions with nonce_counter revocation.
4. **Chess**: Depth 1–15 (13–15 gated behind FALCON auth, not wired yet). Lazy SMP via `rayon::scope`. Semaphore backpressure → 503.

## Work Guidance

- Keep session tokens stateless (HMAC, no session table)
- FALCON verify is currently stubbed — wire `fn_dsa::verify()` when ready
- `.env` files are gitignored; `.env.example` is the template
- PgDog is the pooler — no client-side connection pool
- Cargo workspace: root `backend/Cargo.toml` + child `backend/crates/falcon-wasm/`
- Rust toolchain set in `backend/rust-toolchain.toml`

## Verification

- `cargo check` — type checks the backend
- `cargo test` — runs backend tests
- `cargo clippy` — lints
- `cargo deny check` — dependency audit (config at `backend/.config/deny.toml`)
- `cargo nextest run` — faster test runner (config at `backend/.config/nextest.toml`)
- `wasm-opt` + `wasm-bindgen` for falcon-wasm builds (`crates/falcon-wasm/build.sh`)

## Child DOX Index

| Path | Scope |
|------|-------|
| `backend/AGENTS.md` | Rust server: routes, DB, chess engine, auth, Docker |

Children not yet created:
- `frontend/AGENTS.md` — once frontend implementation begins
- `.github/workflows/AGENTS.md` — if CI complexity warrants it
