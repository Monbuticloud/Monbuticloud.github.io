# DOX — docs/Todo

Child of `backend/AGENTS.md`. Owns the file-based todo tracking system.

## Purpose

Track open work items and review findings for the Homepage monorepo in a
human-readable, git-friendly way. No external tooling — just three markdown
files and a rules doc.

## Ownership

| Path          | Owner | Scope                                              |
| ------------- | ----- | -------------------------------------------------- |
| `Guide.md`    | Docs  | The rules every item must follow (P/B/Since tags). |
| `todo.md`     | Docs  | Open items, sorted by `P` descending.              |
| `complete.md` | Docs  | Finished/dropped items, archived from `todo.md`.   |

## Local Contracts

1. **Three files only** — `Guide.md` (rules), `todo.md` (open), `complete.md` (done). Never add a fourth; if a new category is needed, extend `Guide.md`.
2. **Every item has P, B, Since, Since commit** — see `Guide.md`. Missing tags = malformed item.
3. **Archive, don't delete** — move completed items to `complete.md` with a `Closed:` date; keep original `Since` values for history.
4. **Sort `todo.md` by P descending** so the most important work is always at the top.

## Work Guidance

- Open a todo when a review, bug, or task is identified. Close it (move to
  `complete.md`) only when the underlying code change is merged and verified.
- Re-classify `P`/`B` when context changes (e.g. a fix lands that lowers risk).
- `Since commit` is the `git rev-parse --short HEAD` at open time — re-anchor
  if an item is migrated across repos.

## Verification

- No automated check. Sanity: every `##` block in `todo.md`/`complete.md` has
  exactly the four required tags; `todo.md` is sorted by `P` descending.

## Child DOX Index

No children.
