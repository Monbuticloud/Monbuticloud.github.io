# Open Todos

Sorted by priority (P) descending. See `Guide.md` for tag rules.

## [FE-006c] auth.html is a debug tool; /auth should be login/signup

- P: 6
- B: 7
- Since: 2026-11-22
- Since commit: 15503b8
- File: backend/static/auth.html:7-191, backend/src/main.rs:188-189, backend/static/auth/\*.js
- Summary: `static/auth.html` is a 266-line keygen/debug tool with a 185-line inline `<style>` that redefines `--lime-cream-500` (#b2d430), `--wine-plum-500`, `--berry-blush-500` and others with different values than `common.css`, so the same token names mean two different palettes. It is routed at `/auth-tool` and `/auth-tool.html`, while `/auth` is unrouted: the keygen should become an offline page (private keys never leave the device, so it must work without the backend), and `/auth` should serve a real login/signup page. Split from FE-006b, which left auth unmigrated rather than styling a page that is about to be restructured.
- Status: open
