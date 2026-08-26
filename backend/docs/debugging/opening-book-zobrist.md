# Debugging: Opening book Zobrist hash mismatch

**Status:** ✅ SOLVED — book hits verified in engine, book embedded in binary
**Date:** 2026-08-01
**Owner:** monvip / agent

## ✅ RESOLUTION (2026-08-01)

**Root cause was DOUBLE inversion + unconditional EP.** python-chess `zobrist_hash`
(matches the book for every position) uses:

1. `occupied_co[0]` = BLACK pieces, `occupied_co[1]` = WHITE
   (because `chess.WHITE = True = 1`, `chess.BLACK = False = 0`).
2. `piece_index = (piece_type - 1) * 2 + pivot` where pivot = occupied_co index
   → **BLACK = even (0,2,4,...), WHITE = odd (1,3,5,...)** — INVERTED vs our engine.
3. **Squares are a1=0 based** (a1=0, h8=63) — INVERTED vs our engine (a8=0).
4. Castling: 768=WK, 769=WQ, 770=BK, 771=BQ (already correct).
5. EP: **772+file, ONLY if a pawn of the side-to-move can actually capture** —
   ours was unconditional (WRONG).
6. Side to move: 780 XORed when WHITE to move (already correct).

**Start matched earlier by coincidence:** the start position is invariant under
color-swap + square-mirror, so the double-inverted convention produces the same
XOR hash `0x463B96181691FC9C`. Asymmetric positions exposed the inversion.

**Rust fix (in `zobrist_key` + `hash` + `make_move` + null-move):**
- `zobrist_key(piece, sq)`: pivot = 1 if White else 0 (Black=even index),
  square converted to a1-based: `py_sq = (7 - rank) * 8 + file`.
- `hash()` now calls `zobrist_key` for pieces (single source of truth).
- New `ep_capture_possible(&Board)` helper (pawn adjacency on rank in front of
  ep square, per side to move) gates EP hashing in `hash`, `make_move` (old+new),
  and null-move.

**Book now embedded:** `const BOOK_BYTES: &[u8] = include_bytes!("filtered.bin");`
(193KB — no env var needed, works in debug/tests/Docker). `OPENING_BOOK` env var
is an optional override that falls back to embedded.

**Verification:**
- `cargo test book_diagnostic` — 16 positions: engine hash == python-chess AND in book ✓
- `cargo test incremental_hash_matches_full_hash` — incremental == full recompute
  after every make/unmake (incl. ep capture line) ✓
- All 12 tests pass with NO env var. `cargo build` embeds book (confirmed in binary).

## Verified evidence (python-chess zobrist_hash, all ✓ in book)

- start, after 1.e4, after 1.Nf3, after 1.d4, 1.e4 e5, Sicilian, Italian, Petrov,
  Ruy Lopez, QGD, KID, Scandi, 1.c4, French, Caro-Kann, 1.e4 c5 2.Nf3
- The book's moves decode as LEGAL moves for the side to move (a1-based names):
  after 1.e4 c5 (W): g1f3, b1c3, c2c3, g1e2, d2d3, b2b3, b1a3, f2f4 — all legal ✓
  after 1.e4 c5 2.Nf3 (B): d7d6, b8c6, e7e6, g7g6, a7a6, g8f6 — all legal ✓
- start entries: d2d4, e2e4, g1f3, c2c4, g2g3, b2b3, f2f4 — WHITE's moves ✓
- Start position entries = 7 moves (d2d4 wt=12936, e2e4 wt=10439, g1f3 3546, c2c4 3222, g2g3 151, b2b3 38, f2f4 15)

## Old false leads (do NOT re-try)

- Turn-flipped book (stores opponent replies): WRONG, book is standard.
- a1=0 vs a8=0 as the ONLY issue: WRONG — both color AND square inverted.
- LCG-generated Zobrist tables (any variant): WRONG — the table is hardcoded
  constants (PolyGlot 1.4 / python-chess `POLYGLOT_RANDOM_ARRAY`).
- `rust-openzl` (0.1.0) and `ozlrip` (0.1.0) both FAIL to decode `filtered.zl`
  ("Format version unsupported" / "Malformed") — raw `.bin` is the way.

## Known Good (from this debugging session)

- Book is valid PolyGlot: 193072 bytes, 12067 entries, 9428 unique keys, sorted BE.
- MD5 of `backend/src/games/chess/filtered.bin` == `./Backend/filtered.bin`
  (`161deeb387f06d50aa0d2be56d65fad1`).
- docker build on Alpine failed for `rust-openzl-sys` bindgen (dlopen on musl) — now moot
  (decompression deps removed entirely).

## Files

- `backend/src/games/chess/filtered.bin` — the PolyGlot book (BE keys, sorted, embedded)
- `backend/src/games/chess/main.rs` — `zobrist_key`, `ep_capture_possible`, `Zobrist`,
  `OpeningBook` (embedded `BOOK_BYTES`), book probe in `best_move`
- `backend/src/tests/games/chess/main.rs` — `book_diagnostic` (16-position table),
  `incremental_hash_matches_full_hash`
- `backend/my_compressor.zlc` — OpenZL compressor config (book was zl-compressed)
- `./Backend/filtered.bin` — original decompressed book (same MD5 as current copy)
