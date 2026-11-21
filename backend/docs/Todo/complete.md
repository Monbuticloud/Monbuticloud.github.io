# Completed Todos

Items moved here from `todo.md` on completion. Keep the original `Since` and
`Since commit` values; append `Closed: <YYYY-MM-DD>`. See `Guide.md`.

## [CR-001] Constant-time compare panics on malformed auth token

- P: 10
- B: 2
- Since: 2026-08-22
- Since commit: f9b2955
- File: backend/src/auth/session.rs:50-52, 119-120
- Summary: `hex::decode(token).unwrap_or_default()` then `expected.ct_eq(&provided)` panics the handler thread when `provided` length ≠ 32 (invalid hex → empty, or wrong-length valid hex). `subtle` asserts equal slice lengths. Reachable unauthenticated via `/api/auth/authorize` (hmac*token) and `/api/auth/logout-all` (bearer). Per maintainer: panics the \_thread* (connection drop / request DoS), not the whole process — still a trivially reachable denial of service. Fix: check `provided.len() == expected.len()` before `ct_eq`, or compare fixed-size `[u8;32]` after `try_into`.
- Status: closed
- Closed: 2026-08-24
- Outcome: premise invalid. subtle 2.6.1 (`lib.rs:289-296`) short-circuits `ct_eq` on length mismatch — returns "not equal", never panics. No fix needed; regression tests pinning the behavior added in ecd8036.

## [CR-002] Depth 13–15 not gated behind auth

- P: 7
- B: 3
- Since: 2026-08-22
- Since commit: f9b2955
- File: backend/src/api/chess.rs:70
- Summary: AGENTS.md contract #3 says depth 13–15 are FALCON-gated, but the endpoint accepts them anonymously (`(1..=15).contains(&d)`). Bounded by semaphore + 60s timeout, yet anonymous users can pin CPU slots at depth 15. Cap anonymous depth at 12 server-side until auth lands.
- Status: closed
- Closed: 2026-08-24
- Outcome: resolved by feature/falcon-authorize (merged 0b35e03) — depth 13–15 now require a Bearer session with CAP_DEEP_SEARCH; anonymous deep requests get 401.

## [CR-011] Signup MITM unmitigated — no out-of-band trust anchor

- P: 5
- B: 0
- Since: 2026-08-24
- Since commit: b3cf80f
- File: backend/docs/auth-signing.md
- Summary: First-use key registration trusts whatever the site serves (WASM or .mhtml), so a proxying attacker can steal generated keypairs. Universal to all credential systems; real fix needs an out-of-band distribution channel for the signing tool. Doc corrected to state this honestly.
- Status: closed
- Closed: 2026-08-24
- Outcome: accepted risk. No code fix exists without an out-of-band trust anchor; threat model documented honestly in auth-signing.md. Reopen only if a second-channel distribution path for the signing tool becomes feasible.

## [CR-003] Nonce replay protection is in-memory only + wrong ordering

- P: 7
- B: 4
- Since: 2026-08-22
- Since commit: f9b2955
- File: backend/src/auth/session.rs:151-160, backend/src/api/auth.rs:166
- Summary: `USED_NONCES` is a process-local `Mutex<HashMap>`. Restart clears replay protection; any multi-instance deployment breaks single-use guarantees entirely. Also `consume_nonce` runs _before_ FALCON verification (currently always 503), so every authorize attempt burns the nonce. When wiring FALCON, move consumption to after successful signature verification and document the single-instance constraint (or move to a shared store).
- Status: closed
- Closed: 2026-08-24
- Outcome: Remaining scope closed: single-instance constraint documented in src/auth/session.rs and backend/AGENTS.md (3bd0590). Nonce consumption now happens after successful FALCON verification (0b35e03). Shared store deferred until multi-instance deployment is real.

## [CR-004] Auth/session test coverage is near-zero

- P: 6
- B: 0
- Since: 2026-08-22
- Since commit: f9b2955
- File: backend/src/auth/session.rs:168-178
- Summary: Only one test (`nonce_is_single_use`). `parse_session_token` and `verify_challenge_hmac` have zero tests — which is exactly why CR-001 survived review. Add tests for malformed tokens of every part, wrong-length hex, expired challenges, and tampered HMACs.
- Status: closed
- Closed: 2026-08-24
- Outcome: Resolved by test/auth-session-coverage (8107b42): 12 new tests covering challenge HMAC roundtrip/expiry/tamper/binding and session token parse (part counts, invalid hex per part, wrong-length salt/expiry, non-UTF-8 key_id, wrong counter, expiry).

## [CR-005] Docker Compose publishes DB/PgDog ports to host

- P: 5
- B: 1
- Since: 2026-08-22
- Since commit: f9b2955
- File: backend/docker-compose.yml:9-10, 37-40
- Summary: Postgres (5432) and PgDog (6432/8081/9631) published to host with `homepage/homepage` credentials. Fine for isolated dev; risky on a shared LAN. Bind to `127.0.0.1:` explicitly.
- Status: closed
- Closed: 2026-08-24
- Outcome: Resolved by fix/compose-loopback-bind (8eb8643): Postgres 5432 and PgDog 6432/8081/9631 now bind to 127.0.0.1 only.

## [CR-006] Open key registration has no rate limit

- P: 5
- B: 2
- Since: 2026-08-22
- Since commit: f9b2955
- File: backend/src/api/auth.rs:76-125
- Summary: `/api/auth/keys` accepts unlimited registrations (open by design, refuses overwrite). No rate limiting → DB flooding. Low stakes today (authorize disabled), but add basic rate limiting before FALCON goes live.
- Status: closed
- Closed: 2026-08-24
- Outcome: Resolved by feat/nginx-rate-limit (18cc9f3): limit_req zone at 5r/m per IP, burst 3, 429 status, on /api/auth/keys in nginx/default.conf.

## [CR-007] LOG_DEPTH undercounts request logs

- P: 3
- B: 1
- Since: 2026-08-22
- Since commit: f9b2955
- File: backend/src/main.rs:331
- Summary: `track_request` pushes directly into `LOG_BUFFER` instead of calling `log_info`, so `LOG_DEPTH` never increments for request logs. The >10k early-flush trigger undercounts. Use `log_info(LogMsg::Request{..})`.
- Status: closed
- Closed: 2026-08-24
- Outcome: Resolved by fix/log-depth-counts-requests (f722396): track_request routes through log_info so LOG_DEPTH increments for request logs.

## [CR-008] Engine mate scores not ply-adjusted in TT

- P: 2
- B: 3
- Since: 2026-08-22
- Since commit: f9b2955
- File: backend/src/games/chess/main.rs:2497, 2678-2693
- Summary: `search` returns `-20000 + board.fullmove_number` and TT stores raw scores with no ply adjustment. Mate distances get polluted across TT probes (stored relative to fullmove number, not distance-from-root), degrading mate-finding at high depths. Cosmetic for a portfolio engine.
- Status: closed
- Closed: 2026-08-24
- Outcome: Resolved by fix/tt-mate-ply-adjustment (913d77a): mate scores encoded as distance-from-root; TT store/probe re-anchor via MATE_BOUND.

## [CR-009] Repo hygiene: junk/untracked files

- P: 4
- B: 0
- Since: 2026-08-22
- Since commit: f9b2955
- File: backend/=, backend/v, .DS_Store (tracked), backend/docs/debugging/opening-book-zobrist.md (untracked)
- Summary: Stray files including a 128K `backend/v` (looks like shell-redirection debris) and tracked `.DS_Store`s. Delete/gitignore; keep the tree clean.
- Status: closed
- Closed: 2026-08-24
- Outcome: Resolved by chore/repo-hygiene (2be4f02): backend/= deleted, .DS_Store untracked + gitignored, opening-book-zobrist.md committed under docs/debugging/. Note: backend/v no longer existed on disk; stale deny.toml deletion left unstaged by the earlier config move also completed here.

## [CR-010] RwLock unwrap can cascade under poisoned lock

- P: 3
- B: 1
- Since: 2026-08-22
- Since commit: f9b2955
- File: backend/src/api/html.rs:29, 51
- Summary: `FILE_CACHE.read().unwrap()` / `.write().unwrap()` — a poisoned lock (writer panicked while holding it) cascades into handler panics. Low likelihood, but use `unwrap_or_else(|p| p.into_inner())` to recover instead of panicking.
- Status: closed
- Closed: 2026-08-24
- Outcome: Resolved by fix/file-cache-poisoned-lock (df88e30): FILE_CACHE read/write use unwrap_or_else(|p| p.into_inner()) to recover from poisoning instead of cascading handler panics.

## [CR-012] Migrate Postgres image to dhi.io/postgres:18-alpine3.23

- P: 4
- B: 2
- Since: 2026-08-26
- Since commit: 9484b58
- File: backend/docker-compose.yml:4
- Summary: Replace `postgres:16-alpine` with `dhi.io/postgres:18-alpine3.23` (Docker Hardened Images). Major-version jump 16→18 requires pg_upgrade or dump/restore of `./data/postgres`; verify PgDog compatibility and that the hardened image honors the same config/init mounts (`config/postgres/*`, healthcheck).
- Status: closed
- Closed: 2026-08-26
- Outcome: Migrated and verified live (7544093 + fix/dhi-migration-findings). Gotchas: DHI entrypoint takes flags-only command; custom command skips POSTGRES_DB creation (created manually); setup.sql had NEVER run on any version due to invalid box-drawing comment chars — fixed. Full stack verified on 18.6: chess via PgDog OK, auth challenge OK. Old datadir archived at data/postgres-v16.bak.

## [FE-001] XSS via innerHTML in terminal

- P: 9
- B: 3
- Since: 2026-08-27
- Since commit: 2d2acca
- File: backend/static/script.js:142-145, 259-262, 276-278
- Summary: User input from `echo`/`help`/`cat` flows through `appendOutputLine`/`appendLine`/`freezePromptLine` into `line.innerHTML` without sanitization — typing `<img onerror=alert(1)>` executes. Fix: use `textContent`/`createTextNode` or sanitize with DOMPurify; escape `args` before interpolation.
- Status: closed
- Closed: 2026-11-21
- Outcome: Fixed in e297901 — terminal output is written with `textContent` only; no `innerHTML` remains in the terminal code (verified by grep). The `script.js` path no longer exists because FE-005 split it into `static/pages/index/terminal/`. An earlier attempt on `fix/FE-001-xss-terminal` (ebbfcbe) was never merged and is superseded.

## [FE-002] Auth tool still ECDSA placeholder, not FALCON

- P: 7
- B: 2
- Since: 2026-08-27
- Since commit: 2d2acca
- File: backend/static/auth.html:341-343, 351-381, 432-433
- Summary: `generateKeypair`/`signNonce` use Web Crypto ECDSA P-384, but the backend rejects non-1024 FALCON keys (`auth.rs:88-97` requires 1793 bytes) — the UI cannot register. Fix: wire `falcon-wasm` via inline `Uint8Array` + `WebAssembly.instantiate` (or wasm2js) per the file's own TODO, and drop the ECDSA path.
- Status: closed
- Closed: 2026-08-27
- Outcome: Fixed in 0b848e4 — the ECDSA path was dropped and the tool loads the wasm-bindgen bundle from `/falcon_wasm.js`. `auth.html` was later restructured by FE-012; the FALCON-1024-only contract now lives in `static/auth/falcon.js` and `static/auth/keygen.js`, with no Web Crypto usage (verified by grep).

## [FE-003] CSS Nesting without build step — add image-time flattening

- P: 6
- B: 3
- Since: 2026-08-27
- Since commit: 2d2acca
- File: backend/static/style.css:11-16, 42-72, 192-234, backend/static/common.css:192-234, 307-345
- Summary: Raw CSS Nesting Level 1 (`#name-section { #name-span {} }`, `.image-split { > div:nth-child(1) { img {} } }`, `.glass { &::before }`) is served as plain CSS and drops entirely in Firefox <120/Safari <16.5. Fix at **image build time**: add a Docker stage (Node 20 + `postcss-cli` + `postcss-preset-env` or `lightningcss`) that flattens `static/*.css` → `static/dist/*.css` during `docker build` and serves the dist output; add CI stylelint with a nesting check and keep the source files as authors' input.
- Status: closed
- Closed: 2026-08-27
- Outcome: Fixed in bffcbe4 (image-time PostCSS flattening) plus 68095f9 (CI stylelint + flatten verification). `postcss.config.cjs`, the `build:css`/`lint:css` scripts, the `css-builder` Docker stage and `.github/workflows/css-lint.yml` are all on main; `static/dist/` remains gitignored and the source files stay nested for authors.

## [FE-004] Massive DRY violation — 140+ lines duplicated

- P: 6
- B: 2
- Since: 2026-08-27
- Since commit: 2d2acca
- File: backend/static/style.css:343-490, 492-562, 575-689
- Summary: `.skill-card-head`, `.skill-badge` + 4 variants with hover shadows, `.comfort-row/.comfort-bar`, and legend/comfort blocks are duplicated verbatim inside `.skill-card` scope and again globally. Fix: extract to a single block with CSS custom props or Tailwind `@apply`; delete duplicates; add `stylelint --max-duplicate-selectors`.
- Status: closed
- Closed: 2026-11-21
- Outcome: Fixed in 70294d1. `style.css` went from 708 to 548 lines; `.skill-card-head`, `.skill-badge` and `.comfort-row` each now have a single definition, and a comment at `style.css:363` warns against re-declaring them inside the card scope. The Tailwind `@apply` variant of the fix was not needed and is not what landed.

## [FE-005] Monolithic script.js (479 lines, god module)

- P: 5
- B: 4
- Since: 2026-08-27
- Since commit: 2d2acca
- File: backend/static/script.js
- Summary: Rainbow, terminal intro, command map, interactive terminal, and grid canvas live in one file — violates <50 lines/function and testability. Fix: split per page — small pages stay a single file (e.g. `contact.js`), large pages become a folder (e.g. `pages/index/terminal/`, `pages/index/grid/`); e.g. `rainbow.js` / `terminal/intro.js` / `terminal/commands.js` / `terminal/input.js` / `grid/canvas.js` with explicit `export`s; keep `index.html` as composition root.
- Status: closed
- Closed: 2026-11-21
- Outcome: Fixed in 2b6322c along the exact lines proposed — `static/pages/index/rainbow.js` plus `terminal/` and `grid/` folders, with routes registered in `STATIC_ROUTES` (`src/main.rs`). `script.js` is gone.

## [FE-007] Canvas grid performance + correctness

- P: 4
- B: 2
- Since: 2026-08-27
- Since commit: 2d2acca
- File: backend/static/script.js:399-465
- Summary: `drawFrame` via `requestAnimationFrame` runs forever offscreen and recomputes `getBoundingClientRect()` + `Math.sqrt` for 1200 cells per frame; `lightnessCache` holds `Float64Array(1200)` × 69 phases (~660 KB) with no eviction; `window.mousemove` has no throttle. Fix: gate rAF with `IntersectionObserver`/`document.hidden`, throttle mousemove (rAF or 32 ms), cache `cr` per frame, consider `OffscreenCanvas` or a CSS alternative.
- Status: closed
- Closed: 2026-11-21
- Outcome: Fixed in 6fd5457. Verified in `static/pages/index/grid/canvas.js`: `MOUSE_THROTTLE_MS = 32`, rAF gated behind `IntersectionObserver` and `visibilitychange`.

## [FE-008] Hero image broken without JS + double-load

- P: 4
- B: 1
- Since: 2026-08-27
- Since commit: 2d2acca
- File: backend/static/index.html:112-115, 397-442
- Summary: `<img id="hero-img">` has no `src` — broken when JS is disabled; `loadHd()` sets `img.src = hd` then immediately creates `new Image()` and overwrites it on `onload`, causing a double download. Fix: add fallback `src`/`srcset`/`sizes` with `loading="lazy"`, set `img.src` only inside `full.onload`, and hide the `<img>` via `hidden` until ready.
- Status: closed
- Closed: 2026-11-21
- Outcome: Fixed in 4d1a5b4. Verified: `index.html:107-111` now carries `alt`, a `src` and a `srcset` fallback, so the hero renders without JS.

## [FE-009] A11y + SEO nits

- P: 3
- B: 1
- Since: 2026-08-27
- Since commit: 2d2acca
- File: backend/static/index.html:22, 83-101, 138-145, backend/static/common.css:122-128
- Summary: Terminal `<input>` has only a placeholder (no `<label>`/`aria-label`); skills use `<div class="skill-card">` instead of `<ul><li>`; canonical/OG URLs hard-code `https://monbuticloud.github.io/` while dev serves from `localhost`/`monvip.dev`; `* { color: var(--body-text) }` overrides SVG `fill: currentColor` intent. Fix: label the input, use list semantics, make canonical/OG host-relative or env-driven, and scope the `*` color to `body`.
- Status: closed
- Closed: 2026-11-21
- Outcome: Fixed in bb2d3cf. Verified: `aria-label="Terminal input"` (`index.html:148`), list semantics via `<ul class="skill-grid">` + `<li class="skill-card">`, and `rel="canonical" href="/"` replacing the hardcoded host.

## [FE-010] Repo hygiene — .DS_Store reappeared + Pages placeholder

- P: 2
- B: 0
- Since: 2026-08-27
- Since commit: 2d2acca
- File: backend/static/.DS_Store, backend/static/assets/.DS_Store, backend/static/assets/images/.DS_Store, frontend/index.html
- Summary: Glob found three `.DS_Store` files (untracked, gitignored since `2be4f02` but still on disk). `frontend/` is NOT the real frontend — `backend/static` is (served via Nginx; `frontend/` is just the GitHub Pages placeholder per `AGENTS.md`). Fix: `trash` the three `.DS_Store`s; update `frontend/index.html` copy to make the placeholder role explicit (link to the real site) or add `frontend/AGENTS.md` documenting the split.
- Status: closed
- Closed: 2026-08-27
- Outcome: Fixed in 1a7d93d, taking the copy branch of the fix: `frontend/index.html` now states it is not the real frontend, points at `backend/static/`, and links the live site, so no `frontend/AGENTS.md` is needed while the folder stays a placeholder. All three `static/` `.DS_Store` files are gone; four unrelated `.DS_Store` files elsewhere in the tree were removed later (see 2026-11-21 hygiene commit).

## [FE-013] Rainbow animations run forever offscreen

- P: 3
- B: 2
- Since: 2026-10-24
- Since commit: cf70536
- File: backend/static/script.js:27, 52-55
- Summary: Two `setInterval(..., 75)` loops rewrite 30 `backgroundColor` values and `backgroundPosition` continuously, even when the tab is hidden or the section is offscreen. Gate both on `IntersectionObserver` + `visibilitychange`, or replace with a CSS animation.
- Status: closed
- Closed: 2026-11-21
- Outcome: Fixed in f90af7c. Verified in `static/pages/index/rainbow.js`: both loops are gated on `IntersectionObserver` + `visibilitychange` rather than being replaced with a CSS animation.

## [FE-012] auth.html embeds a 336-line inline script

- P: 5
- B: 4
- Since: 2026-10-24
- Since commit: cf70536
- File: backend/static/auth.html:264-599
- Summary: Keygen, signing, base64 helpers, and UI wiring are inlined in a 336-line `<script>` with no module boundary or shared code, making the FALCON rework harder to test. Extract to `auth/*.js` modules loaded with `type="module"`; keep `auth.html` as markup + composition root.
- Status: closed
- Closed: 2026-11-21
- Outcome: Fixed exactly as proposed — `auth.html` drops the inline `<script>` for `<script type="module" src="./auth/main.js">` and stays the markup composition root. Nine modules under `static/auth/` (571 LOC): `main.js`, `state.js`, `dom.js`, `api.js`, `base64.js`, `falcon.js`, `keygen.js`, `session.js`, `api-panel.js`, all registered in `STATIC_ROUTES` and copied by the Dockerfile. Verified: every relative import resolves and all 18 ids queried by `dom.js` exist in `auth.html`.
