# DOX

- Parent: root `AGENTS.md` — DOX rail, repo-wide rules, top-level index
- This doc owns the stack: build entry points, database, pooler, reverse proxy
- The Rust server has its own contract: see `app/AGENTS.md`

## Purpose

Everything needed to build and run the stack around the Rust app.

`backend/` is the deployable unit, not just server code. It holds the Docker entry points (`Dockerfile`, `docker-compose.yml`, `docker-bake.hcl`), the Postgres and PgDog configuration those containers mount, and the reverse proxy that fronts the app. The Rust source lives one level down in `app/`.

## Ownership

| Path | Owner | Scope |
|------|-------|-------|
| `Dockerfile` | Build | Multi-stage: css-builder (reads `../frontend/`), Rust build, UPX, scratch runtime |
| `docker-compose.yml` | Build | Nginx + Rust + Postgres + PgDog orchestration; both build contexts are `..` (repo root) |
| `docker-bake.hcl` | Build | `default` and `push` bake targets for the app image |
| `postgres/` | DB | `pg_hba.conf`, `postgresql.conf`, `setup.sql` (mounted read-only into the Postgres container) |
| `pgdog/` | DB | `pgdog.toml`, `users.toml` — the connection pooler config |
| `nginx/` | Infra | `default.conf` (TLS, rate limit, static asset aliases, proxy) + `Dockerfile.local` |
| `app/` | Rust server | See `app/AGENTS.md` |
| `data/` | Runtime | Bind mounts (`data/postgres`). Gitignored, never committed |

## Local Contracts

1. **Build contexts are the repo root.** `context: ..` for both the `app` and `nginx` services. The css-builder stage reads `frontend/`; the Rust stage reads `backend/app/`. Neither is reachable from a `backend/` context.
2. **`.dockerignore` is at the repo root.** It applies to the context directory only, and the context is `..`.
3. **The runtime image is `scratch` with CWD `/`.** Every path the app resolves at runtime is compiled in as a CWD-relative string, so the image must ship files at the exact paths `STATIC_ROUTES` names: `/frontend/static/...` and `/backend/app/crates/falcon-wasm/pkg/...`.
4. **The binary is `/server`.** Not `/backend` — that name is now a directory holding the WASM pkg path.
5. **CSS generation happens only here.** The `css-builder` stage mirrors the `frontend/` layout in `/build`, so `frontend/package.json`'s relative script paths work unchanged, then hands `/build/static/dist` to the Rust stage. It fails the build on empty output or leftover `&` nesting operators.
6. **`app` service has no static volume.** Assets are baked into the image. Rebuild to change a page.
7. **Postgres image is DHI** (`dhi.io/postgres:18-alpine3.23`) and needs `docker login dhi.io` (free). `PGDATA` is at `/var/lib/postgresql/18/data`, so the bind mount is the parent dir.
8. **PgDog is the pooler.** `DATABASE_URL` points at `pgdog:6432`, never at Postgres directly.
9. **nginx owns two URL spaces**: `/static/assets/` and `/auth-tool.mhtml` are served from the filesystem (`/frontend/static/...`); everything else proxies to `app:8000`.
10. **`nginx/certs/` is gitignored.** The nginx image build expects local self-signed certs; a fresh clone must generate them first.

## Work Guidance

- Compose commands run from `backend/`: `docker compose up --build`, `docker compose config`.
- Compose reads `.env` from this directory. `backend/.env` is the file; `backend/app/.env` is silently ignored because there is no `env_file` entry. Exported variables still take precedence over the file.
- Bake runs from `backend/`: `docker buildx bake default`. The targets resolve `context = ".."` and `dockerfile = "backend/Dockerfile"` relative to the HCL file.
- Changing a runtime path means changing three places together: the `STATIC_ROUTES` entry in `app/src/main.rs`, the `COPY` destination in `Dockerfile`, and any `alias` in `nginx/default.conf`.
- Adding a new service config: put it in its own directory beside the service that mounts it, then update the compose volume.
- `scripts/bench-alloc.sh` builds its A/B variants from the repo root now: `APP="$ROOT/backend/app"`, compose at `$ROOT/backend/docker-compose.yml`, context `$ROOT`.
- `.env` is gitignored and lives here, beside `docker-compose.yml`. `AUTH_HMAC_SECRET` is required: the server asserts a decoded length of at least 24 bytes at first use, so a blank value panics rather than producing forgeable tokens. Generate one with `openssl rand -base64 32`.

## Verification

- `docker compose config` (in `backend/`) — validates compose syntax and that both build contexts and dockerfile paths resolve
- `docker compose up --build` (in `backend/`) — full stack build; the css-builder stage fails loudly on empty or unflattened CSS
- `docker buildx bake --print default` (in `backend/`) — resolves the bake targets without building
- Runtime smoke, over TLS. The `:80` listener answers every path with `301`, so an HTTP check cannot tell a served asset from a missing one and will never show `200`:

  ```bash
  for p in / /style.css /static/assets/favicon.svg /falcon_wasm_bg.wasm; do
    curl -ks -o /dev/null -w "%{http_code}  $p\n" "https://localhost$p"
  done
  ```

  All four must be `200`. The redirect is a separate check: `curl -sI http://localhost/ | head -1` (expect `301`).
- TLS needs local certs. `nginx/certs/` is gitignored, so a fresh clone has no `selfsigned.crt` / `selfsigned.key` and the nginx image build fails until they exist. Generate them first.

## Child DOX Index

| Path | Scope |
|------|-------|
| `app/AGENTS.md` | Rust server: routes, DB, chess engine, auth, FALCON WASM crate, Rust tooling |

Stays owned by this doc: build entry points, Postgres, PgDog, reverse proxy, runtime image layout.
