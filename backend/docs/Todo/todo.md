# Open Todos

Sorted by priority (P) descending. See `Guide.md` for tag rules.

## [CR-001] Constant-time compare panics on malformed auth token

- P: 10
- B: 2
- Since: 2026-08-22
- Since commit: f9b2955
- File: backend/src/auth/session.rs:50-52, 119-120
- Summary: `hex::decode(token).unwrap_or_default()` then `expected.ct_eq(&provided)` panics the handler thread when `provided` length ≠ 32 (invalid hex → empty, or wrong-length valid hex). `subtle` asserts equal slice lengths. Reachable unauthenticated via `/api/auth/authorize` (hmac_token) and `/api/auth/logout-all` (bearer). Per maintainer: panics the _thread_ (connection drop / request DoS), not the whole process — still a trivially reachable denial of service. Fix: check `provided.len() == expected.len()` before `ct_eq`, or compare fixed-size `[u8;32]` after `try_into`.
- Status: open

## [CR-002] Depth 13–15 not gated behind auth

- P: 7
- B: 3
- Since: 2026-08-22
- Since commit: f9b2955
- File: backend/src/api/chess.rs:70
- Summary: AGENTS.md contract #3 says depth 13–15 are FALCON-gated, but the endpoint accepts them anonymously (`(1..=15).contains(&d)`). Bounded by semaphore + 60s timeout, yet anonymous users can pin CPU slots at depth 15. Cap anonymous depth at 12 server-side until auth lands.
- Status: open

## [CR-003] Nonce replay protection is in-memory only + wrong ordering

- P: 7
- B: 4
- Since: 2026-08-22
- Since commit: f9b2955
- File: backend/src/auth/session.rs:151-160, backend/src/api/auth.rs:166
- Summary: `USED_NONCES` is a process-local `Mutex<HashMap>`. Restart clears replay protection; any multi-instance deployment breaks single-use guarantees entirely. Also `consume_nonce` runs _before_ FALCON verification (currently always 503), so every authorize attempt burns the nonce. When wiring FALCON, move consumption to after successful signature verification and document the single-instance constraint (or move to a shared store).
- Status: open

## [CR-004] Auth/session test coverage is near-zero

- P: 6
- B: 0
- Since: 2026-08-22
- Since commit: f9b2955
- File: backend/src/auth/session.rs:168-178
- Summary: Only one test (`nonce_is_single_use`). `parse_session_token` and `verify_challenge_hmac` have zero tests — which is exactly why CR-001 survived review. Add tests for malformed tokens of every part, wrong-length hex, expired challenges, and tampered HMACs.
- Status: open

## [CR-005] Docker Compose publishes DB/PgDog ports to host

- P: 5
- B: 1
- Since: 2026-08-22
- Since commit: f9b2955
- File: backend/docker-compose.yml:9-10, 37-40
- Summary: Postgres (5432) and PgDog (6432/8081/9631) published to host with `homepage/homepage` credentials. Fine for isolated dev; risky on a shared LAN. Bind to `127.0.0.1:` explicitly.
- Status: open

## [CR-006] Open key registration has no rate limit

- P: 5
- B: 2
- Since: 2026-08-22
- Since commit: f9b2955
- File: backend/src/api/auth.rs:76-125
- Summary: `/api/auth/keys` accepts unlimited registrations (open by design, refuses overwrite). No rate limiting → DB flooding. Low stakes today (authorize disabled), but add basic rate limiting before FALCON goes live.
- Status: open

## [CR-007] LOG_DEPTH undercounts request logs

- P: 3
- B: 1
- Since: 2026-08-22
- Since commit: f9b2955
- File: backend/src/main.rs:331
- Summary: `track_request` pushes directly into `LOG_BUFFER` instead of calling `log_info`, so `LOG_DEPTH` never increments for request logs. The >10k early-flush trigger undercounts. Use `log_info(LogMsg::Request{..})`.
- Status: open

## [CR-008] Engine mate scores not ply-adjusted in TT

- P: 2
- B: 3
- Since: 2026-08-22
- Since commit: f9b2955
- File: backend/src/games/chess/main.rs:2497, 2678-2693
- Summary: `search` returns `-20000 + board.fullmove_number` and TT stores raw scores with no ply adjustment. Mate distances get polluted across TT probes (stored relative to fullmove number, not distance-from-root), degrading mate-finding at high depths. Cosmetic for a portfolio engine.
- Status: open

## [CR-009] Repo hygiene: junk/untracked files

- P: 4
- B: 0
- Since: 2026-08-22
- Since commit: f9b2955
- File: backend/=, backend/v, .DS_Store (tracked), backend/docs/debugging/opening-book-zobrist.md (untracked)
- Summary: Stray files including a 128K `backend/v` (looks like shell-redirection debris) and tracked `.DS_Store`s. Delete/gitignore; keep the tree clean.
- Status: open

## [CR-010] RwLock unwrap can cascade under poisoned lock

- P: 3
- B: 1
- Since: 2026-08-22
- Since commit: f9b2955
- File: backend/src/api/html.rs:29, 51
- Summary: `FILE_CACHE.read().unwrap()` / `.write().unwrap()` — a poisoned lock (writer panicked while holding it) cascades into handler panics. Low likelihood, but use `unwrap_or_else(|p| p.into_inner())` to recover instead of panicking.
- Status: open
