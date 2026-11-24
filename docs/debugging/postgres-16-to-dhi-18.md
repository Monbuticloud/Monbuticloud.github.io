# Postgres 16 (stock) → 18 (DHI) Migration

Migrating `postgres:16-alpine` → `dhi.io/postgres:18-alpine3.23` (CR-012).

## What changed in docker-compose.yml

| Item | Before | After |
|---|---|---|
| Image | `postgres:16-alpine` | `dhi.io/postgres:18-alpine3.23` |
| Data mount | `./data/postgres:/var/lib/postgresql/data` | `./data/postgres:/var/lib/postgresql` |
| PGDATA (in container) | `/var/lib/postgresql/data` | `/var/lib/postgresql/18/data` |

DHI images use a **versioned PGDATA path**; mounting the parent keeps future
major upgrades to a rename instead of a mount-path change.

## Prerequisites

- `docker login dhi.io` — free Docker account (DHI Community is Apache-2.0,
  but the registry requires auth even for pulls).
- Major jump 16→18 means the old v16 datadir **cannot** be started by the new
  image — export/import is required.

## Steps

```bash
cd backend/

# 1. Export from the OLD stack (must run before switching images)
docker compose up -d postgres
docker compose exec postgres pg_dumpall -U homepage > data/pre-dhi-dump.sql
docker compose down

# 2. Archive the old datadir (don't delete until the new stack verifies)
mv data/postgres data/postgres-v16.bak && mkdir data/postgres

# 3. Start the NEW stack — initdb runs fresh, setup.sql creates schema
docker compose up -d postgres

# 4. Import
docker compose exec -T postgres psql -U homepage -d homepage < data/pre-dhi-dump.sql

# 5. Verify: auth.keys survived, chess endpoint works against PgDog
docker compose exec postgres psql -U homepage -d homepage -c "SELECT key_id, capabilities FROM auth.keys;"
docker compose up -d && curl 'http://localhost/api/games/chess/completions?fen=START&depth=3'

# 6. Only after verification:
rm -rf data/postgres-v16.bak data/pre-dhi-dump.sql   # contains no secrets, but is bulky
```

## Verification checklist

- [ ] Container healthy (`pg_isready` healthcheck passes)
- [ ] `auth` schema + `auth.keys` rows present after import
- [ ] Full auth round-trip works (register → challenge → authorize)
- [ ] Chess search works through PgDog

## Gotchas found during the actual migration (2026-08-26)

- **`command:` must be flags-only.** The DHI entrypoint invokes the postgres
  binary itself; `command: postgres -c ...` fails with
  `postgres: invalid argument: "postgres"`. Use
  `command: ["-c", "config_file=/etc/postgresql/postgresql.conf"]`.
- **Custom `command:` skips POSTGRES_DB creation.** initdb ran and created
  the `homepage` role, but the `homepage` database was not created. Fixed
  manually: `CREATE DATABASE homepage OWNER homepage;`
- **setup.sql had never run successfully on ANY version** — it used
  box-drawing characters (`──`, `┬`) as comment markers, which are invalid
  SQL. This is why the old v16 cluster was empty (no `auth` schema). Fixed
  in the same commit as this migration.
- DHI runtime variants have **no shell/apk** — debug via `docker debug`, not
  `docker compose exec ... sh`.
- Stop signal is **SIGINT** (fast shutdown) vs stock SIGTERM — compose
  `restart: unless-stopped` handles it; just don't be surprised by faster
  stops.
- **DHI default `pg_hba.conf` locks to localhost only.** PgDog connections
  from the Docker network failed with
  `FATAL: no pg_hba.conf entry for host "fd3b:..."`. Fixed by adding a
  custom `config/postgres/pg_hba.conf` (trust locally, scram-sha-256 for
  `0.0.0.0/0` + `::0/0`) and wiring `hba_file` in `postgresql.conf`.
- `POSTGRES_HOST_AUTH_METHOD=trust` would be **rejected** by the entrypoint
  (scram-sha-256/md5 only) — we don't use it.

## Actual result

- PostgreSQL 18.6 (dhi.io/postgres:18-alpine3.23), healthy, healthcheck green
- `auth.keys` + helper functions created from fixed setup.sql
  (after fixing `command:` + `POSTGRES_DB` + `pg_hba.conf` + box-drawing).
- Full stack **and FALCON smoke verified** after `docker compose up -d --build`:
  register 201, challenge 200, authorize 200 → session, depth 14 with token
  200, depth 14 anon 401, depth 12 anon 200, replay 401, logout-all 200,
  old token after logout 401.
- Old datadir archived at `data/postgres-v16.bak` + dump at
  `data/pre-dhi-dump.sql` (3.5 KB, role only — cluster was empty) — delete
  after a soak period.
