# Completed Todos

Items moved here from `todo.md` on completion. Keep the original `Since` and
`Since commit` values; append `Closed: <YYYY-MM-DD>`. See `Guide.md`.

## [CR-001] Constant-time compare panics on malformed auth token

- P: 10
- B: 2
- Since: 2026-08-22
- Since commit: f9b2955
- File: backend/src/auth/session.rs:50-52, 119-120
- Summary: `hex::decode(token).unwrap_or_default()` then `expected.ct_eq(&provided)` panics the handler thread when `provided` length ≠ 32 (invalid hex → empty, or wrong-length valid hex). `subtle` asserts equal slice lengths. Reachable unauthenticated via `/api/auth/authorize` (hmac_token) and `/api/auth/logout-all` (bearer). Per maintainer: panics the _thread_ (connection drop / request DoS), not the whole process — still a trivially reachable denial of service. Fix: check `provided.len() == expected.len()` before `ct_eq`, or compare fixed-size `[u8;32]` after `try_into`.
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

