# Open Todos

Sorted by priority (P) descending. See `Guide.md` for tag rules.

## [FE-001] XSS via innerHTML in terminal

- P: 9
- B: 3
- Since: 2026-08-27
- Since commit: 2d2acca
- File: backend/static/script.js:142-145, 259-262, 276-278
- Summary: User input from `echo`/`help`/`cat` flows through `appendOutputLine`/`appendLine`/`freezePromptLine` into `line.innerHTML` without sanitization — typing `<img onerror=alert(1)>` executes. Fix: use `textContent`/`createTextNode` or sanitize with DOMPurify; escape `args` before interpolation.
- Status: open

## [FE-002] Auth tool still ECDSA placeholder, not FALCON

- P: 7
- B: 2
- Since: 2026-08-27
- Since commit: 2d2acca
- File: backend/static/auth.html:341-343, 351-381, 432-433
- Summary: `generateKeypair`/`signNonce` use Web Crypto ECDSA P-384, but the backend rejects non-1024 FALCON keys (`auth.rs:88-97` requires 1793 bytes) — the UI cannot register. Fix: wire `falcon-wasm` via inline `Uint8Array` + `WebAssembly.instantiate` (or wasm2js) per the file's own TODO, and drop the ECDSA path.
- Status: open

## [FE-003] CSS Nesting without build step — add image-time flattening

- P: 6
- B: 3
- Since: 2026-08-27
- Since commit: 2d2acca
- File: backend/static/style.css:11-16, 42-72, 192-234, backend/static/common.css:192-234, 307-345
- Summary: Raw CSS Nesting Level 1 (`#name-section { #name-span {} }`, `.image-split { > div:nth-child(1) { img {} } }`, `.glass { &::before }`) is served as plain CSS and drops entirely in Firefox <120/Safari <16.5. Fix at **image build time**: add a Docker stage (Node 20 + `postcss-cli` + `postcss-preset-env` or `lightningcss`) that flattens `static/*.css` → `static/dist/*.css` during `docker build` and serves the dist output; add CI stylelint with a nesting check and keep the source files as authors' input.
- Status: open

## [FE-004] Massive DRY violation — 140+ lines duplicated

- P: 6
- B: 2
- Since: 2026-08-27
- Since commit: 2d2acca
- File: backend/static/style.css:343-490, 492-562, 575-689
- Summary: `.skill-card-head`, `.skill-badge` + 4 variants with hover shadows, `.comfort-row/.comfort-bar`, and legend/comfort blocks are duplicated verbatim inside `.skill-card` scope and again globally. Fix: extract to a single block with CSS custom props or Tailwind `@apply`; delete duplicates; add `stylelint --max-duplicate-selectors`.
- Status: open

## [FE-005] Monolithic script.js (479 lines, god module)

- P: 5
- B: 4
- Since: 2026-08-27
- Since commit: 2d2acca
- File: backend/static/script.js
- Summary: Rainbow, terminal intro, command map, interactive terminal, and grid canvas live in one file — violates <50 lines/function and testability. Fix: split per page — small pages stay a single file (e.g. `contact.js`), large pages become a folder (e.g. `pages/index/terminal/`, `pages/index/grid/`); e.g. `rainbow.js` / `terminal/intro.js` / `terminal/commands.js` / `terminal/input.js` / `grid/canvas.js` with explicit `export`s; keep `index.html` as composition root.
- Status: open

## [FE-006a] Tailwind v4 build step in Docker

- P: 5
- B: 3
- Since: 2026-10-24
- Since commit: cf70536
- File: backend/Dockerfile:37-54, backend/static/tailwind.input.css, backend/static/\*.html, backend/AGENTS.md
- Summary: Add `@tailwindcss/cli` to the existing builder-stage Node block and emit `static/tailwind.css` scanned from `static/*.html`, removing the CDN/Flowbite scripts and documenting the build in `backend/AGENTS.md`. Use v4 CSS-first config (`@source`, `@theme` for the palette), skip preflight so the hand-rolled CSS is not reset, pin the CLI version, and share the stage with FE-003 (split from FE-006, 2026-08-27).
- Status: open

## [FE-011] No frontend verification in CI

- P: 5
- B: 3
- Since: 2026-10-24
- Since commit: cf70536
- File: .github/workflows/static.yml:39-40, backend/static/\*.html, backend/static/\*.js
- Summary: CI only uploads `./frontend` to Pages — there is no HTML/CSS lint, accessibility check, or Playwright smoke test, which is why FE-001…FE-010 accumulated unnoticed. Add a CI job (stylelint/HTMLHint + Playwright smoke for index/auth/terminal) alongside the existing Pages deploy.
- Status: open

## [FE-012] auth.html embeds a 336-line inline script

- P: 5
- B: 4
- Since: 2026-10-24
- Since commit: cf70536
- File: backend/static/auth.html:264-599
- Summary: Keygen, signing, base64 helpers, and UI wiring are inlined in a 336-line `<script>` with no module boundary or shared code, making the FALCON rework harder to test. Extract to `auth/*.js` modules loaded with `type="module"`; keep `auth.html` as markup + composition root.
- Status: open

## [FE-007] Canvas grid performance + correctness

- P: 4
- B: 2
- Since: 2026-08-27
- Since commit: 2d2acca
- File: backend/static/script.js:399-465
- Summary: `drawFrame` via `requestAnimationFrame` runs forever offscreen and recomputes `getBoundingClientRect()` + `Math.sqrt` for 1200 cells per frame; `lightnessCache` holds `Float64Array(1200)` × 69 phases (~660 KB) with no eviction; `window.mousemove` has no throttle. Fix: gate rAF with `IntersectionObserver`/`document.hidden`, throttle mousemove (rAF or 32 ms), cache `cr` per frame, consider `OffscreenCanvas` or a CSS alternative.
- Status: open

## [FE-008] Hero image broken without JS + double-load

- P: 4
- B: 1
- Since: 2026-08-27
- Since commit: 2d2acca
- File: backend/static/index.html:112-115, 397-442
- Summary: `<img id="hero-img">` has no `src` — broken when JS is disabled; `loadHd()` sets `img.src = hd` then immediately creates `new Image()` and overwrites it on `onload`, causing a double download. Fix: add fallback `src`/`srcset`/`sizes` with `loading="lazy"`, set `img.src` only inside `full.onload`, and hide the `<img>` via `hidden` until ready.
- Status: open

## [FE-006b] Offload layout and responsive CSS to Tailwind utilities

- P: 4
- B: 6
- Since: 2026-10-24
- Since commit: cf70536
- File: backend/static/\*.html, backend/static/common.css, backend/static/style.css, backend/static/contact.css, backend/static/projects.css
- Summary: Replace generic layout, spacing, typography, and breakpoint rules with Tailwind utilities, keeping bespoke pieces (rainbow, terminal, canvas grid, glass, hero split) in `@layer components`; also drop redundant `ff` hex suffixes and add the missing `sm/md/lg/xl/2xl` breakpoints (`style.css:106, 140, 205` are the only three today). Migrate one page at a time, delete the corresponding hand-rolled rules, and close FE-004 once the duplicated blocks are gone (split from FE-006, 2026-08-27).
- Status: open

## [FE-009] A11y + SEO nits

- P: 3
- B: 1
- Since: 2026-08-27
- Since commit: 2d2acca
- File: backend/static/index.html:22, 83-101, 138-145, backend/static/common.css:122-128
- Summary: Terminal `<input>` has only a placeholder (no `<label>`/`aria-label`); skills use `<div class="skill-card">` instead of `<ul><li>`; canonical/OG URLs hard-code `https://monbuticloud.github.io/` while dev serves from `localhost`/`monvip.dev`; `* { color: var(--body-text) }` overrides SVG `fill: currentColor` intent. Fix: label the input, use list semantics, make canonical/OG host-relative or env-driven, and scope the `*` color to `body`.
- Status: open

## [FE-013] Rainbow animations run forever offscreen

- P: 3
- B: 2
- Since: 2026-10-24
- Since commit: cf70536
- File: backend/static/script.js:27, 52-55
- Summary: Two `setInterval(..., 75)` loops rewrite 30 `backgroundColor` values and `backgroundPosition` continuously, even when the tab is hidden or the section is offscreen. Gate both on `IntersectionObserver` + `visibilitychange`, or replace with a CSS animation.
- Status: open

## [FE-010] Repo hygiene — .DS_Store reappeared + Pages placeholder

- P: 2
- B: 0
- Since: 2026-08-27
- Since commit: 2d2acca
- File: backend/static/.DS_Store, backend/static/assets/.DS_Store, backend/static/assets/images/.DS_Store, frontend/index.html
- Summary: Glob found three `.DS_Store` files (untracked, gitignored since `2be4f02` but still on disk). `frontend/` is NOT the real frontend — `backend/static` is (served via Nginx; `frontend/` is just the GitHub Pages placeholder per `AGENTS.md`). Fix: `trash` the three `.DS_Store`s; update `frontend/index.html` copy to make the placeholder role explicit (link to the real site) or add `frontend/AGENTS.md` documenting the split.
- Status: open
