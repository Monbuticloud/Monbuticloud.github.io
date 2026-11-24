# Monbuticloud's HUB

Monorepo for my portfolio site and the Rust backend that serves it.

## Layout

| Tree | What it is |
|------|------------|
| `backend/` | The stack: Rust app (`app/`), Postgres + PgDog config, Nginx, `Dockerfile`, `docker-compose.yml` |
| `frontend/` | The real site — pages, CSS, ES modules, assets — plus the Node toolchain that builds and tests them |
| `github-pages/` | The GitHub Pages placeholder. Not the real site |
| `docs/` | Auth protocol spec, design specs, implementation plans, debugging runbooks |

## Run it

```bash
cd backend
echo "AUTH_HMAC_SECRET=$(openssl rand -base64 32)" > .env
docker compose up --build       # Nginx + Rust + Postgres + PgDog
```

`AUTH_HMAC_SECRET` is required. The server asserts a decoded length of at least 24 bytes at first use, so an unset, blank, or placeholder value panics at startup. `backend/.env.example` is the reference file; its `changeme` placeholder is deliberately too short to run with.

Compose reads `.env` from `backend/`, the directory holding `docker-compose.yml`. An exported `AUTH_HMAC_SECRET` takes precedence over the file.

Two prerequisites beyond Docker Desktop:

- `docker login dhi.io` (free) pulls the hardened Postgres image.
- `backend/nginx/certs/selfsigned.crt` and `selfsigned.key` are gitignored, so generate local self-signed certs before the first build or the nginx image fails.

## Run the server without Docker

```bash
# from the repository root — every static path is CWD-relative
AUTH_HMAC_SECRET="$(head -c 48 /dev/urandom | base64)" \
  cargo run --manifest-path backend/app/Cargo.toml
```

## Checks

Each block runs from the repository root.

```bash
# Rust — type check, tests, lint
(cd backend/app && cargo check --all-targets && cargo test && cargo clippy --all-targets)

# WASM crate — a separate package, not covered by the app's checks
(cd backend/app/crates/falcon-wasm && cargo check)

# Pages — install, lint, browser smoke test
(cd frontend && npm ci && npm run lint && npm run test:e2e)

# Stack wiring — resolves both build contexts without building
(cd backend && docker compose config)
```