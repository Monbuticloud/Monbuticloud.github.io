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

# Resolve the secret exactly as Compose will: an exported value beats .env, and
# Compose also accepts "export ", whitespace around "=", and quotes. Asking
# Compose to do the parsing keeps this in step with it rather than guessing.
secret=$(docker compose config 2>/dev/null \
  | sed -n 's/^ *AUTH_HMAC_SECRET: *//p' | head -1 | sed 's/^"//; s/"$//')
keylen=$(printf '%s' "$secret" | openssl base64 -d -A 2>/dev/null | wc -c | tr -d ' ')

# Start only if the server will accept that value: valid base64 decoding to at
# least 24 bytes. A placeholder or a bad override stops here, not at first use.
if printf '%s' "$secret" | openssl base64 -d -A >/dev/null 2>&1 && [ "$keylen" -ge 24 ]; then
  docker compose up --build     # Nginx + Rust + Postgres + PgDog
else
  echo "AUTH_HMAC_SECRET is missing, is not valid base64, or decodes to under 24 bytes — regenerate with: openssl rand -base64 32" >&2
fi
```

`AUTH_HMAC_SECRET` is required. The server asserts a decoded length of at least 24 bytes at first use, so an unset, blank, or placeholder value panics at startup. `backend/.env.example` is the reference file; its `changeme` placeholder is deliberately too short to run with.

Four properties of that block are load-bearing:

- **It never writes an empty secret.** `echo "…=$(openssl …)" > .env` exits `0` even when `openssl` is absent, leaving `AUTH_HMAC_SECRET=` in the file; a presence-only guard then preserves that broken file on every later run. Here the generator's status is checked and the file is not created on failure.
- **It validates the value the server will actually receive.** A presence check passes for `changeme`, which the server rejects on first use — and it passes for a valid `.env` even when an exported `AUTH_HMAC_SECRET` overrides it with junk. The block resolves the effective value first, then requires valid base64 decoding to 24+ bytes.
- **It lets Compose parse `.env`.** `AUTH_HMAC_SECRET = value`, `export AUTH_HMAC_SECRET=value`, and quoted values are all valid Compose syntax; a hand-rolled `grep '^AUTH_HMAC_SECRET=.'` misses them and refuses to start. `docker compose config` applies Compose's own parser and its environment-over-file precedence, so this stays correct without reimplementing either.
- **It does not regenerate an existing secret.** Session tokens are stateless HMACs keyed by this value, so a new secret signs everyone out. Re-running the block leaves an existing `.env` untouched.

`umask 022` is the common default and would create `.env` as `0644`, readable by any local account that can traverse the repository. `umask 077` in a subshell narrows it to `0600` without changing the shell you paste into.

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