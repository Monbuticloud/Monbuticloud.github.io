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
docker compose up --build        # Nginx + Rust + Postgres + PgDog
```

Needs Docker Desktop and `docker login dhi.io` (free) for the hardened Postgres image.

## Run the server without Docker

```bash
# from the repository root — every static path is CWD-relative
AUTH_HMAC_SECRET="$(head -c 48 /dev/urandom | base64)" \
  cargo run --manifest-path backend/app/Cargo.toml
```

## Checks

```bash
cd backend/app && cargo check && cargo test && cargo clippy   # Rust
cd frontend   && npm ci && npm run lint && npm run test:e2e   # pages
cd backend    && docker compose config                        # stack wiring
```
