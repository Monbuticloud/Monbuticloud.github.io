# Open Todos

Sorted by priority (P) descending. See `Guide.md` for tag rules.

## [FE-006b] Offload layout and responsive CSS to Tailwind utilities

- P: 4
- B: 6
- Since: 2026-10-24
- Since commit: cf70536
- File: backend/static/\*.html, backend/static/common.css, backend/static/style.css, backend/static/contact.css, backend/static/projects.css
- Summary: Replace generic layout, spacing, typography, and breakpoint rules with Tailwind utilities, keeping bespoke pieces (rainbow, terminal, canvas grid, glass, hero split) in `@layer components`; also drop redundant `ff` hex suffixes and add the missing `sm/md/lg/xl/2xl` breakpoints (`style.css:106, 140, 205` are the only three today). Migrate one page at a time, delete the corresponding hand-rolled rules, and close FE-004 once the duplicated blocks are gone (split from FE-006, 2026-08-27). Depends on FE-006a: the Tailwind build step does not exist yet.
- Status: open
