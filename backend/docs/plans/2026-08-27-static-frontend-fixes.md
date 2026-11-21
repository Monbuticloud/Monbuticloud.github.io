# Static Frontend Fixes Implementation Plan

> **For agentic workers:** Use subagent-driven development — one worktree branch per task in `.worktrees/FE-00X`, commit atomically on that branch, do NOT merge to main, do NOT push, do NOT squash.

**Goal:** Fix 10 static/frontend issues found in review (FE-001…FE-010), each on its own branch.

**Tech Stack:** Plain HTML/CSS/JS (no build tool yet), Docker Nginx serving `backend/static`

**Spec:** `backend/docs/Todo/todo.md` (FE-001…FE-010 at 2026-08-27, base 2d2acca)

## Global Constraints

- Each task owns exactly one worktree: `.worktrees/FE-00X` on branch `fix/FE-00X` (or `chore/FE-010`)
- Do NOT merge to main, do NOT push, do NOT squash — leave branch for orchestrator to handle
- Conventional commits, atomic, small
- No heap constraint here (static assets only)
- Keep `todo.md` open — orchestrator will close items after verification

---

### Task 1: FE-001 — XSS via innerHTML in terminal (P9)

**Files:** `backend/static/script.js:142-145, 259-262, 276-278`

**Fix:** `echo`/user input flows into `line.innerHTML` via `appendOutputLine`/`appendLine`/`freezePromptLine`. Escape with `textContent`/`createTextNode` or DOMPurify; `help`/`cat` handlers must escape `args`. No `innerHTML` with user data.

### Task 2: FE-002 — Auth tool ECDSA placeholder → FALCON (P7)

**Files:** `backend/static/auth.html:341-343, 351-381, 432-433`

**Fix:** Wire `falcon-wasm` (inline Uint8Array + `WebAssembly.instantiate` or wasm2js) per file's own TODO; drop ECDSA P-384 stub; fix `String.fromCharCode(...Uint8Array)` spread (stack overflow at 1793 bytes) via loop/chunking; fix single-line base64 regex to handle multi-line bodies.

### Task 3: FE-003 — CSS Nesting without build step (P6)

**Files:** `backend/static/style.css:11-16, 42-72, 192-234`, `backend/static/common.css:192-234, 307-345`

**Fix:** Add Docker image-time flattening: `Dockerfile` stage with Node 20 + `postcss-cli` + `postcss-preset-env` (or `lightningcss`) that processes `static/*.css` → `static/dist/*.css` during `docker build` and serves dist; keep source as author input; add CI stylelint nesting check. Do NOT flatten by hand.

### Task 4: FE-004 — DRY 140+ lines duplicated (P6)

**Files:** `backend/static/style.css:343-490, 492-562, 575-689`

**Fix:** Deduplicate `.skill-card-head`, `.skill-badge` + 4 variants, `.comfort-*`, legend blocks — single source with CSS custom props or `@apply`; delete duplicates; add `stylelint --max-duplicate-selectors`.

### Task 5: FE-005 — Monolithic script.js god module (P5)

**Files:** `backend/static/script.js` (479 lines)

**Fix:** Split per page — small pages stay a single file (e.g. `contact.js`), large pages become a folder (e.g. `pages/index/terminal/`, `pages/index/grid/`); e.g. `rainbow.js` / `terminal/intro.js` / `terminal/commands.js` / `terminal/input.js` / `grid/canvas.js` with explicit `export`s; `index.html` as composition root.

### Task 6: FE-006 — No Tailwind/Flowbite despite standards (P5)

**Files:** `backend/static/common.css:4-13`, `backend/static/*.html`

**Fix:** Either adopt Tailwind via `<script src="https://cdn.tailwindcss.com">` + Flowbite per `ui-styling-standards`, or document exception in `AGENTS.md`; remove redundant `ff` hex suffixes; add mobile-first `sm/md/lg/xl/2xl` breakpoints tested at 375/768/1024/1440.

### Task 7: FE-007 — Canvas grid perf + correctness (P4)

**Files:** `backend/static/script.js:399-465`

**Fix:** Gate `requestAnimationFrame` with `IntersectionObserver`/`document.hidden`, throttle `mousemove` (rAF/32 ms), cache `cr` per frame, consider `OffscreenCanvas` or CSS alternative; handle `lightnessCache` eviction.

### Task 8: FE-008 — Hero image broken without JS + double-load (P4)

**Files:** `backend/static/index.html:112-115, 397-442`

**Fix:** Add fallback `src`/`srcset`/`sizes` with `loading="lazy"`; set `img.src` only inside `full.onload`; hide `<img>` via `hidden` until ready; fix `hero-lowdata` handling.

### Task 9: FE-009 — A11y + SEO nits (P3)

**Files:** `backend/static/index.html:22, 83-101, 138-145`, `backend/static/common.css:122-128`

**Fix:** Label terminal `<input>` (`aria-label`), use `<ul><li>` for skills, make canonical/OG host-relative or env-driven, scope `* { color }` to `body`.

### Task 10: FE-010 — Repo hygiene (P2)

**Files:** `backend/static/.DS_Store` etc., `frontend/index.html`

**Fix:** `trash` the three `.DS_Store`s; update `frontend/index.html` placeholder copy to state `frontend/` is NOT the real frontend — `backend/static` is (via Nginx), link to real site, or add `frontend/AGENTS.md`.
