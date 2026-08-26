# Open Todos

Sorted by priority (P) descending. See `Guide.md` for tag rules.

## [CR-012] Migrate Postgres image to dhi.io/postgres:18-alpine3.23

- P: 4
- B: 2
- Since: 2026-08-26
- Since commit: 9484b58
- File: backend/docker-compose.yml:4
- Summary: Replace `postgres:16-alpine` with `dhi.io/postgres:18-alpine3.23` (Docker Hardened Images). Major-version jump 16→18 requires pg_upgrade or dump/restore of `./data/postgres`; verify PgDog compatibility and that the hardened image honors the same config/init mounts (`config/postgres/*`, healthcheck).
- Status: open
