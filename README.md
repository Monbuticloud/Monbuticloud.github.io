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

# Create .env once, owner-only (0600), and never with an empty secret.
if [ ! -f .env ]; then
  secret=$(openssl rand -base64 32) || secret=
  if [ -n "$secret" ]; then
    ( umask 077; printf 'AUTH_HMAC_SECRET=%s\n' "$secret" > .env )
  else
    echo "openssl missing or returned nothing — .env not written" >&2
  fi
  unset secret
fi

# Start only if a usable secret exists, in the file or the environment.
if grep -qE '^AUTH_HMAC_SECRET=.+' .env 2>/dev/null || [ -n "${AUTH_HMAC_SECRET:-}" ]; then
  docker compose up --build     # Nginx + Rust + Postgres + PgDog
else
  echo "no usable AUTH_HMAC_SECRET — fix .env or export one, then re-run" >&2
fi
```

`AUTH_HMAC_SECRET` is required. The server asserts a decoded length of at least 24 bytes at first use, so an unset, blank, or placeholder value panics at startup. `backend/.env.example` is the reference file; its `changeme` placeholder is deliberately too short to run with.

Three properties of that block are load-bearing:

- **It never writes an empty secret.** `echo "…=$(openssl …)" > .env` exits `0` even when `openssl` is absent, leaving `AUTH_HMAC_SECRET=` in the file; the presence guard then preserves that broken file on every later run. Here the generator's status is checked and the file is not created on failure.
- **It does not regenerate an existing secret.** Session tokens are stateless HMACs keyed by this value, so a new secret signs everyone out. Re-running the block leaves an existing `.env` untouched.
- **It uses owner-only permissions.** `umask 022` is the common default and would create `.env` as `0644`, readable by any local account that can traverse the repository. `umask 077` in a subshell narrows it to `0600` without changing the shell you paste into.

To rotate the secret, unset any exported `AUTH_HMAC_SECRET`, delete `.env`, and re-run the block — then expect every session to be signed out. Unsetting matters: an exported value takes precedence over the file, so deleting `.env` alone leaves the effective secret unchanged and old tokens still valid.

Compose reads `.env` from `backend/`, the directory holding `docker-compose.yml`.

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