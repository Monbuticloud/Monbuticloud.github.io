# Open Todos

Sorted by priority (P) descending. See `Guide.md` for tag rules.

## [FE-006a] Tailwind v4 build step in Docker

- P: 5
- B: 3
- Since: 2026-10-24
- Since commit: cf70536
- File: backend/Dockerfile:37-54, backend/static/tailwind.input.css, backend/static/\*.html, backend/AGENTS.md
- Summary: Add `@tailwindcss/cli` to the existing builder-stage Node block and emit `static/tailwind.css` scanned from `static/*.html`, removing the CDN/Flowbite scripts and documenting the build in `backend/AGENTS.md`. Use v4 CSS-first config (`@source`, `@theme` for the palette), skip preflight so the hand-rolled CSS is not reset, pin the CLI version, and share the stage with FE-003 (split from FE-006, 2026-08-27). No Tailwind ships today: the original FE-006 attempt on `fix/FE-006` (b361f72) was never merged to main, so this starts from scratch.
- Status: open

## [FE-011] No frontend verification in CI

- P: 5
- B: 3
- Since: 2026-10-24
- Since commit: cf70536
- File: .github/workflows/css-lint.yml, .github/workflows/static.yml:39-40, backend/static/\*.html, backend/static/\*.js
- Summary: The CSS half landed in 68095f9 — `css-lint.yml` runs stylelint + flatten verification on `static/**/*.css`. Still missing: an HTML lint (HTMLHint) and a Playwright smoke test covering index, terminal and auth, which is why FE-001...FE-010 accumulated unnoticed. `static.yml` still only uploads `./frontend` to Pages, so none of `static/` is verified.
- Status: open

## [FE-012] auth.html embeds a 336-line inline script

- P: 5
- B: 4
- Since: 2026-10-24
- Since commit: cf70536
- File: backend/static/auth.html:264-599
- Summary: Keygen, signing, base64 helpers, and UI wiring are inlined in a 336-line `<script>` with no module boundary or shared code, making the FALCON rework harder to test. Extract to `auth/*.js` modules loaded with `type="module"`; keep `auth.html` as markup + composition root.
- Status: open

## [FE-006b] Offload layout and responsive CSS to Tailwind utilities

- P: 4
- B: 6
- Since: 2026-10-24
- Since commit: cf70536
- File: backend/static/\*.html, backend/static/common.css, backend/static/style.css, backend/static/contact.css, backend/static/projects.css
- Summary: Replace generic layout, spacing, typography, and breakpoint rules with Tailwind utilities, keeping bespoke pieces (rainbow, terminal, canvas grid, glass, hero split) in `@layer components`; also drop redundant `ff` hex suffixes and add the missing `sm/md/lg/xl/2xl` breakpoints (`style.css:106, 140, 205` are the only three today). Migrate one page at a time, delete the corresponding hand-rolled rules, and close FE-004 once the duplicated blocks are gone (split from FE-006, 2026-08-27). Depends on FE-006a: the Tailwind build step does not exist yet.
- Status: open
