# Open Todos

Sorted by priority (P) descending. See `Guide.md` for tag rules.

## [FE-011] No frontend verification in CI

- P: 5
- B: 3
- Since: 2026-10-24
- Since commit: cf70536
- File: .github/workflows/css-lint.yml, .github/workflows/static.yml:39-40, backend/static/\*.html, backend/static/\*.js
- Summary: The CSS lint half landed in 68095f9 — `css-lint.yml` runs stylelint on `static/**/*.css`. Still missing: an HTML lint (HTMLHint) and a Playwright smoke test covering index, terminal and auth, which is why FE-001...FE-010 accumulated unnoticed. `static.yml` still only uploads `./frontend` to Pages, so none of `static/` is verified.
- Status: open

## [FE-006b] Offload layout and responsive CSS to Tailwind utilities

- P: 4
- B: 6
- Since: 2026-10-24
- Since commit: cf70536
- File: backend/static/\*.html, backend/static/common.css, backend/static/style.css, backend/static/contact.css, backend/static/projects.css
- Summary: Replace generic layout, spacing, typography, and breakpoint rules with Tailwind utilities, keeping bespoke pieces (rainbow, terminal, canvas grid, glass, hero split) in `@layer components`; also drop redundant `ff` hex suffixes and add the missing `sm/md/lg/xl/2xl` breakpoints (`style.css:106, 140, 205` are the only three today). Migrate one page at a time, delete the corresponding hand-rolled rules, and close FE-004 once the duplicated blocks are gone (split from FE-006, 2026-08-27). Depends on FE-006a: the Tailwind build step does not exist yet.
- Status: open
