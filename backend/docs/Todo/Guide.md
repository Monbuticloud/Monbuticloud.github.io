# Todo System Guide

A lightweight, file-based todo tracker for the Homepage monorepo. This guide
defines the rules every todo item must follow so items stay sortable,
mergeable, and auditable across time and commits.

## Files

| File          | Purpose                                                       |
| ------------- | ------------------------------------------------------------- |
| `Guide.md`    | This file — the rules.                                        |
| `todo.md`     | Open items, newest relevant first. One item per heading.      |
| `complete.md` | Items that are done. Moved here from `todo.md` on completion. |

Workflow: an item lives in `todo.md` while open. When finished (or permanently
dropped), move it to `complete.md` with the completion date appended to its
`Since` fields. Never delete an item — archive it.

## Item format

Every item is a `##` heading followed by a fixed block of tagged lines:

```markdown
## [CR-001] Short title

- P: 10
- B: 2
- Since: 2026-08-22
- Since commit: f9b2955
- File: backend/src/auth/session.rs:50-52, 119-120
- Summary: one or two sentences describing the issue and the fix.
- Status: open
```

### Tag reference

**P — Priority** (0–10)

- `0` — nice to have, cosmetic, no urgency.
- `5` — should fix, real but bounded impact.
- `10` — super duper important: security, data loss, or reachable failure.

**B — Breaking change** (0–10)

- `0` — trivial: a one-line guard, a comment, a log tweak.
- `5` — moderate: new module, API change, behavior shift needing review.
- `10` — codebase refactor: touches many files / reworks core flow.

**Since time X** — the calendar date (YYYY-MM-DD) the item was opened.
Useful for "how long has this been sitting here" triage.

**Since commit Y** — the short git commit hash (`git rev-parse --short HEAD`)
at the time the item was opened. Anchors the item to a point in history so
you can tell if it predates or postdates a given change.

### Rules

1. Every item MUST have all four metadata tags: `P`, `B`, `Since`, `Since commit`.
2. `P` and `B` are integers 0–10. No floats, no words.
3. Keep `Summary` to ≤2 sentences; link the file:line that proves the issue.
4. Sort `todo.md` by `P` descending (most important at top).
5. On completion, append `Closed: <YYYY-MM-DD>` and move the whole block to
   `complete.md`. Do not rewrite history — keep the original `Since` values.
6. If an item is split, give the children new IDs (`CR-001a`, `CR-001b`) and
   cross-reference the parent.
7. Re-classify `P`/`B` when context changes (e.g. a fix lands that lowers risk).
