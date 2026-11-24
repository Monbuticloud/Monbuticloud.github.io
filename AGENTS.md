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
- Four top-level trees, one purpose each: `backend/` (the stack), `frontend/` (the real site), `github-pages/` (the placeholder deployed to Pages), `docs/` (durable documentation)
- `frontend/static/` is what the Rust server serves; `github-pages/` is a stub, never the real site
- Backend runs via docker-compose locally (Nginx + Rust + Postgres + PgDog)
- Lazy SMP chess engine uses `rayon::scope`, lock-free transposition table
- Auth: FALCON (FN-DSA FIPS 206) post-quantum signatures + HMAC challenge-response
- No heap allocations inside chess search/move generation
- `mimalloc` global allocator
- Strict `#[allow(dead_code)]` on log tools — keep them even if unused
- Commit atomically **as you go** — small, focused commits the moment a unit of work is done; don't batch unrelated changes or wait until the end. More, smaller commits is usually better. Conventional format: `type(scope): description`.

## Purpose

Monorepo for Monbuticloud's personal portfolio site and projects.

| Tree | What it is |
|------|------------|
| `backend/` | The stack: Rust app under `app/`, Postgres config, the reverse proxy, and the `Dockerfile` / `docker-compose.yml` / `docker-bake.hcl` entry points |
| `frontend/` | The real site — server-rendered pages, CSS, ES modules, assets — plus the Node toolchain that builds, lints, and smoke-tests them |
| `github-pages/` | The GitHub Pages placeholder. Deployed by CI; **not** the real site |
| `docs/` | Auth protocol spec, design specs, implementation plans, debugging runbooks |

## Ownership

| Path | Owner | Scope |
|------|-------|-------|
| `backend/` | Stack | Compose/bake/build entry points, Postgres + PgDog config, reverse proxy |
| `backend/app/` | Rust server | HTTP routes, chess engine, DB, auth, FALCON WASM crate, Rust-level tooling |
| `frontend/` | Frontend | Pages, CSS, ES modules, assets, Tailwind/PostCSS/stylelint/HTMLHint/Playwright |
| `github-pages/` | CI/CD | The Pages placeholder and its deploy workflow |
| `docs/` | Docs | Auth protocol spec, design specs, implementation plans, debugging runbooks |
| `.github/workflows/` | CI/CD | CSS lint, frontend verification (HTMLHint + Playwright), Pages deploy |

## Local Contracts

1. **Build contexts are the repository root.** `backend/docker-compose.yml` sets `context: ..` for both the app and nginx images, because the CSS stage reads `frontend/` and the Rust stage reads `backend/app/`. A context of `backend/` cannot reach `frontend/`.
2. **`.dockerignore` lives at the repo root**, not in `backend/`. It only applies to a context in its own directory.
3. **Static paths are CWD-relative and identical in dev and prod.** `STATIC_ROUTES` in `backend/app/src/main.rs` resolves `frontend/static/...`; the image ships those files at `/frontend/static/` (CWD is `/`). Run the server from the repo root for local dev.
4. **The image binary is `/server`, not `/backend`** — `/backend/app/crates/falcon-wasm/pkg/` is a real directory in the image, so a file at `/backend` would collide.
5. **Backend**: `docker compose up --build` from `backend/`. Depends on Docker Desktop, `docker login dhi.io` (free, for the hardened Postgres image), `wasm-bindgen` + `wasm-opt` for falcon-wasm.
6. **Frontend**: `npm ci` in `frontend/`. `frontend/static/dist/` is generated and gitignored.
7. **Pages**: `github-pages/index.html` is the only tracked Pages file. CI uploads that directory, nothing else.
8. **`backend/.env` is the only env file Compose reads.** It reads `.env` from the Compose project directory; it does not read `backend/app/.env`, and the compose file has no `env_file` entry. `AUTH_HMAC_SECRET` is required and the server panics without it.
9. **Auth**: FALCON (FN-DSA-1024) signatures verified server-side via `falcon-rs`; registration restricted to 1024-param pub keys. Private key generated client-side, never uploaded. HMAC stateless sessions with nonce_counter revocation.
10. **Chess**: Depth 1–15; depth 13–15 require a Bearer session with CAP_DEEP_SEARCH (enforced server-side). Lazy SMP via `rayon::scope`. Semaphore backpressure → 503.

## Work Guidance

- Work is tracked in GitHub Issues, not markdown. Open an issue for every new finding instead of adding a file. Every issue carries `p:N` / `b:N` labels for its priority and breaking score (`p:1`–`p:10` high-to-low urgency; `b:0`–`b:10` least-to-most invasive), plus area labels (`auth`, `chess`, `frontend`, `infra`, `ci`, `docs`, `hygiene`).
- Issue titles are `[TYPE]: short description` using conventional-commit types. Issues #1–#26 keep their legacy `[CR-###]` / `[FE-###]` titles because old commit messages name those IDs; leave them alone.
- Keep session tokens stateless (HMAC, no session table)
- FALCON verify is wired via `falcon-rs` — FN-DSA-1024 public keys only
- `.env` files are gitignored; `backend/.env.example` is the template. Copy it to `backend/.env` — Compose reads `.env` from the directory holding `docker-compose.yml`, so a file anywhere else is silently ignored
- PgDog is the pooler — no client-side connection pool
- When adding a file, put it in the tree that matches its purpose, not the tree that happens to be nearest. A page asset goes in `frontend/static/`; a Rust module in `backend/app/src/`; an infra config beside the service that reads it; a durable doc in `docs/`.
- `docs/plans/` and `docs/specs/` are dated implementation records pinned to the commit that shipped them. Their internal paths predate the `backend/app/` move; do not rewrite them, and do not treat them as current instructions.

## Verification

- `cargo check --all-targets` — type checks the Rust app (run in `backend/app/`)
- `cargo test` — runs Rust tests
- `cargo clippy` — lints
- `cargo deny check` — dependency audit (config at `backend/app/deny.toml`, cargo-deny 0.19 schema)
- `cargo nextest run` — faster test runner (config at `backend/app/.config/nextest.toml`)
- `npm run build:css` (in `frontend/`) — builds `frontend/static/dist/`
- `npm run lint` (in `frontend/`) — stylelint + HTMLHint
- `npm run test:e2e` (in `frontend/`) — Playwright page smoke suite
- `docker compose config` (in `backend/`) — validates the compose file and its build contexts
- `cargo check` in `backend/app/crates/falcon-wasm/` — a separate package, not a member of the app crate, so the app's checks do not cover it
- `wasm-opt` + `wasm-bindgen` for falcon-wasm builds (`backend/app/crates/falcon-wasm/build.sh`)

## Child DOX Index

| Path | Scope |
|------|-------|
| `backend/AGENTS.md` | The stack: compose/bake/build entry points, Postgres, PgDog, reverse proxy |
| `backend/app/AGENTS.md` | Rust server: routes, DB, chess engine, auth, FALCON WASM crate |
| `frontend/AGENTS.md` | Pages, CSS cascade contract, Tailwind/PostCSS/stylelint/HTMLHint/Playwright |

`github-pages/` has no child doc: it is one placeholder file with no contracts of its own. `docs/` has no child doc: it is a documentation tree, and the docs themselves are the material.
