# DOX

- Parent: root `AGENTS.md` — DOX rail, repo-wide rules, top-level index
- This doc owns the real site and the toolchain that builds, lints, and tests it

## Purpose

The portfolio site: three server-rendered pages plus an offline auth tool, their CSS and ES modules, and the Node toolchain that compiles, lints, and smoke-tests them.

`frontend/static/` is the web root the Rust server serves. `github-pages/` is a separate placeholder and is **not** this site — do not confuse them.

## Ownership

| Path | Owner | Scope |
|------|-------|-------|
| `static/*.html` | Pages | `index.html`, `contact.html`, `projects.html`, `auth.html` (auth tool, served as `/auth-tool.html` and `/auth-tool`) |
| `static/*.css` | Styles | Author CSS — `style.css`, `common.css`, `contact.css`, `projects.css`. Nested, unflattened input |
| `static/common.js` | Scripts | Shared bootstrapping |
| `static/pages/index/` | Scripts | Index composition root: `index.js`, `rainbow.js`, `terminal/*`, `grid/canvas.js` (ES modules) |
| `static/auth/` | Scripts | Auth tool ES modules — `main.js` (composition root), `api.js`, `falcon.js`, `keygen.js`, `session.js`, `state.js`, `dom.js`, `base64.js`, `api-panel.js` |
| `static/assets/` | Assets | `favicon.svg`, `icons.svg`, `Chess-Pieces.svg`, `images/` |
| `static/robots.txt`, `static/sitemap.xml` | Assets | Crawler files |
| `static/dist/` | Generated | PostCSS-flattened + Tailwind output. Gitignored, never committed |
| `tailwind.input.css` | Styles | Tailwind v4 entry point. Lives at the frontend root, **not** in `static/`, so the `static/*.css` glob stays author-CSS only |
| `postcss.config.cjs` | Build | `postcss-preset-env` with `stage: false` — flattens nesting only, no other polyfill |
| `package.json`, `package-lock.json` | Build | Scripts + pinned devDependencies. Never `npm install`; always `npm ci` from the lockfile |
| `.stylelintrc.json` | Lint | stylelint correctness + `max-nesting-depth: 4` |
| `.htmlhintrc` | Lint | HTMLHint config |
| `playwright.config.js` | Test | Smoke-test config; `webServer` boots `e2e/serve.mjs` |
| `e2e/serve.mjs` | Test | Dependency-free static server mirroring the Rust server's dist preference |
| `e2e/smoke.spec.js` | Test | Page smoke suite — renders, boots, terminal answers, `echo` stays inert |

## Local Contracts

1. **`static/dist/` is generated and gitignored.** Generated in the Docker `css-builder` stage (see `../backend/AGENTS.md`), and locally by `npm run build:css`. Never commit it.
2. **Pages link bare CSS names** (`style.css`, `tailwind.css`, `common.css`). The Rust server prefers `static/dist/<file>` and falls back to the source file for a run without a build. Keep links bare; do not point them at `dist/`.
3. **Cascade contract.** Layer order is declared once, in `tailwind.input.css`: `@layer theme, base, components, utilities`. Unlayered CSS beats every layer regardless of specificity, so an unlayered `* { margin: 0 }` silently defeats `pt-24` — that is exactly what happened, and why `common.css`'s reset and body/theme globals live in `@layer base`. Everything else in `common.css` and all of `style.css`, `contact.css`, `projects.css` stay unlayered or in `@layer components` so existing component rules keep winning over utilities. New page styles: utilities in the markup, bespoke surfaces in `@layer components`.
4. **Two Tailwind v4 traps.** Named type-scale utilities pair a line-height (`text-base` turns `16px/normal` into `16px/24px`), so use arbitrary sizes like `text-[1em]` to set font-size alone. And `border-<color>` sets all four sides — a left-only border needs `border-l-<color>`.
5. **ES modules are served individually**, not bundled. Each one needs a `STATIC_ROUTES` entry in `../backend/app/src/main.rs`; a new module without a route 404s in production while working in the e2e harness.
6. **Tailwind preflight is excluded**, so the hand-rolled CSS is not reset. Do not enable it.
7. **The e2e harness mirrors production, not the filesystem.** `e2e/serve.mjs` reproduces the dist-preference logic deliberately; a page that passes here should pass in the image.

## Work Guidance

- `npm run build:css` runs `build:tailwind` then `build:flatten`. `build:tailwind` writes `static/dist/tailwind.css`; `build:flatten` runs PostCSS over `static/*.css` into `static/dist/`.
- Author CSS stays nested — it is the input, not the output. Only `dist/` is flat.
- `@source` in `tailwind.input.css` scans `static/*.html`, so utility classes only survive if they literally appear in the markup.
- The auth tool's private key is generated client-side and never uploaded. FALCON signing runs in WASM; see `static/auth/falcon.js` for the bundle-loading contract.
- Playwright's first run needs `npx playwright install chromium`.
- Work is tracked in GitHub Issues, not markdown. Open an issue for every new finding.

## Verification

- `npm ci` — install from the committed lockfile
- `npm run build:css` — rebuild `static/dist/`
- `npm run lint:css` — stylelint correctness + nesting depth
- `npm run lint:html` — HTMLHint over `static/*.html`
- `npm run lint` — both linters
- `npm run test:e2e` — Playwright smoke: every page loads without an uncaught exception, the terminal answers `help`, `echo` keeps markup inert, the auth modules boot
- CI mirrors these in `../.github/workflows/css-lint.yml` and `../.github/workflows/frontend-verify.yml`

## Child DOX Index

No child docs yet.

Candidates:
- `static/auth/AGENTS.md` — the FALCON auth tool has its own protocol contract
