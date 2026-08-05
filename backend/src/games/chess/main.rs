// A minimal chess engine inspired by Sunfish.
// No heap allocations inside search/move generation.
// Uses enums for clarity and constants for demystification.

use portable_atomic::AtomicU128;
use std::collections::HashMap;
use std::sync::{
    LazyLock,
    atomic::{AtomicBool, Ordering},
};

// -----------------------------------------------------------------------------
// Enums and constants
// -----------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]

enum Color {
    White,
    Black,
}

impl Color {
    fn sign(self) -> i8 {

        match self {
            Color::White => 1,
            Color::Black => -1,
        }
    }

    fn opposite(self) -> Self {

        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }
}

// We represent a piece as an i8 on the board:
// positive = White, negative = Black, zero = Empty.
// The absolute value gives the piece type index (1..6).
const EMPTY: i8 = 0;

const W_PAWN: i8 = 1;

const W_KNIGHT: i8 = 2;

const W_BISHOP: i8 = 3;

const W_ROOK: i8 = 4;

const W_QUEEN: i8 = 5;

const W_KING: i8 = 6;

const B_PAWN: i8 = -1;

const B_KNIGHT: i8 = -2;

const B_BISHOP: i8 = -3;

const B_ROOK: i8 = -4;

const B_QUEEN: i8 = -5;

const B_KING: i8 = -6;

// Material values (centipawns)
const PAWN_VALUE: i32 = 100;

const KNIGHT_VALUE: i32 = 320;

const BISHOP_VALUE: i32 = 330;

const ROOK_VALUE: i32 = 500;

const QUEEN_VALUE: i32 = 900;

const KING_VALUE: i32 = 20000;

fn piece_value(piece: i8) -> i32 {

    match piece.abs() {
        1 => PAWN_VALUE,
        2 => KNIGHT_VALUE,
        3 => BISHOP_VALUE,
        4 => ROOK_VALUE,
        5 => QUEEN_VALUE,
        6 => KING_VALUE,
        _ => 0,
    }
}

fn piece_to_list_idx(piece: i8) -> usize {
    (piece.unsigned_abs() as usize - 1) + if piece > 0 { 0 } else { 6 }
}

/// Remove `sq` from the piece list for `piece`.
/// Returns true if `sq` was found in the list (normal case).
/// Returns false if not found (list was corrupt — use rebuild instead).
fn remove_piece_from_list(board: &mut Board, sq: u8, piece: i8) -> bool {
    let idx = piece_to_list_idx(piece);
    let original_count = board.piece_count[idx];
    let mut removed = false;
    let mut i = 0;
    while (i as u8) < board.piece_count[idx] {
        let current_count = board.piece_count[idx];
        if board.piece_list[idx][i as usize] == sq {
            let last = current_count - 1;
            board.piece_list[idx][i as usize] = board.piece_list[idx][last as usize];
            board.piece_count[idx] = last;
            removed = true;
        } else {
            i += 1;
        }
    }
    let removed_count = (original_count as usize) - (board.piece_count[idx] as usize);
    if removed_count > 1 {
        rebuild_piece_list(board, piece);
    }
    removed
}

/// Rebuild a piece list from the board (used as fallback when remove fails).
/// After calling this, the caller should NOT call add_piece_to_list for the
/// same piece+square because the rebuild already includes the current board state.
fn rebuild_piece_list(board: &mut Board, piece: i8) {
    let idx = piece_to_list_idx(piece);
    let max = board.piece_list[idx].len() as u8;
    let mut offset = 0u8;
    for sq in 0..64u8 {
        if offset >= max {
            break;
        }
        if board.board[sq as usize] == piece {
            board.piece_list[idx][offset as usize] = sq;
            offset += 1;
        }
    }
    board.piece_count[idx] = offset;
}

/// Remove `piece` from `sq` in the list, with rebuild fallback.
/// If the piece is found and removed, returns true and the caller should
/// separately add the destination. If not found (corrupt list), rebuilds
/// the list from the board; the caller should NOT add afterward (the
/// rebuild already reflects the current board state).
fn remove_piece_or_rebuild(board: &mut Board, sq: u8, piece: i8) -> bool {
    if remove_piece_from_list(board, sq, piece) {
        true
    } else {
        rebuild_piece_list(board, piece);
        false // caller should skip subsequent add
    }
}

// Debug: validate all piece lists against the board.
// Called via validate_piece_lists!() macro — compiles out in release.
#[cfg(debug_assertions)]
fn validate_piece_lists(board: &Board, label: &str) {
    for idx in 0..12 {
        let count = board.piece_count[idx];
        let piece_type = (idx % 6) + 1;
        let expected_piece = if idx < 6 { piece_type as i8 } else { -(piece_type as i8) };
        for entry_idx in 0..count {
            let sq = board.piece_list[idx][entry_idx as usize];
            if sq >= 64 { continue; }
            let actual = board.board[sq as usize];
            if actual != expected_piece {
                debug_assert!(false, "MISMATCH {} list[{}][{}] sq={} expected={} actual={}", label, idx, entry_idx, sq, expected_piece, actual);
                return;
            }
            // Check for duplicates
            for j in 0..entry_idx {
                if board.piece_list[idx][j as usize] == sq {
                    debug_assert!(false, "DUPLICATE {} list[{}] sq={}", label, idx, sq);
                    return;
                }
            }
        }
    }
}

/// Call-site macro: evaluates format args only in debug builds.
#[cfg(debug_assertions)]
macro_rules! validate_piece_lists {
    ($board:expr, $($arg:tt)+) => {
        validate_piece_lists($board, &format!($($arg)+));
    };
}
#[cfg(not(debug_assertions))]
macro_rules! validate_piece_lists {
    ($board:expr, $($arg:tt)+) => {};
}

fn add_piece_to_list(board: &mut Board, sq: u8, piece: i8) {
    let idx = piece_to_list_idx(piece);
    let count = board.piece_count[idx];
    for entry_idx in 0..count {
        if board.piece_list[idx][entry_idx as usize] == sq {
            debug_assert!(false,
                "duplicate sq={} in list[{}] count={}", sq, idx, count);
            return;
        }
    }
    if (count as usize) < board.piece_list[idx].len() {
        board.piece_list[idx][count as usize] = sq;
        board.piece_count[idx] = count + 1;
    }
}

// Piece-square tables (from Sunfish, simplified)
// Index: square 0..63 (a1=0, h1=7, a8=56, h8=63)
const PST: [i32; 64] = [
    0, 0, 0, 0, 0, 0, 0, 0, 50, 50, 50, 50, 50, 50, 50, 50, 10, 10, 20, 30, 30, 20, 10, 10, 5, 5, 10, 25, 25, 10, 5, 5,
    0, 0, 0, 20, 20, 0, 0, 0, 5, -5, -10, 0, 0, -10, -5, 5, 5, 10, 10, -20, -20, 10, 10, 5, 0, 0, 0, 0, 0, 0, 0, 0,
];

type Square = u8; // 0..63

// Castling rights as bitmask (bits: 0=White King-side, 1=White Queen-side,
// 2=Black King-side, 3=Black Queen-side)
const CASTLING_WK: u8 = 1;

const CASTLING_WQ: u8 = 2;

const CASTLING_BK: u8 = 4;

const CASTLING_BQ: u8 = 8;

// Named squares for castling (replace magic numbers throughout).
// Square indexing: 0 = a8 … 63 = h1 (FEN top-to-bottom).
// Formula: square = rank × 8 + file, where rank 0 = 8th rank, rank 7 = 1st rank.
//   White back rank (1st):  A1=56, B1=57, C1=58, D1=59, E1=60, F1=61, G1=62, H1=63
//   Black back rank (8th):  A8=0,  B8=1,  C8=2,  D8=3,  E8=4,  F8=5,  G8=6,  H8=7
// When adding a new square constant, verify with the formula above.
const E1: u8 = 60;
const G1: u8 = 62;
const C1: u8 = 58;
const F1: u8 = 61;
const H1: u8 = 63;
const A1: u8 = 56;
const D1: u8 = 59;
const B1: u8 = 57;

const E8: u8 = 4;
const G8: u8 = 6;
const C8: u8 = 2;
const F8: u8 = 5;
const H8: u8 = 7;
const A8: u8 = 0;
const D8: u8 = 3;
const B8: u8 = 1;

// -----------------------------------------------------------------------------
// Move representation
// -----------------------------------------------------------------------------

#[derive(Clone, Copy)]

struct Move {
    from: Square,
    to: Square,
    promotion: i8, // 0 if no promotion, else promoted piece (e.g., W_QUEEN)
}

impl Move {
    fn new(from: Square, to: Square) -> Self {

        Move { from, to, promotion: 0 }
    }

    fn with_promotion(from: Square, to: Square, promo: i8) -> Self {

        Move {
            from,
            to,
            promotion: promo,
        }
    }
}

// -----------------------------------------------------------------------------
// Board state
// -----------------------------------------------------------------------------

#[derive(Clone)]
struct Board {
    board: [i8; 64],
    side_to_move: Color,
    castling_rights: u8,
    en_passant_square: i8, // -1 if none, else square index
    halfmove_clock: u16,
    fullmove_number: u16,
    wk_sq: u8, // square of white king
    bk_sq: u8, // square of black king
    hash: u64, // Zobrist hash (incremental for O(1) TT probes)
    // Piece lists: 12 fixed-size arrays (6 types × 2 colors), zero heap.
    // Max per type: pawns=8, all others ≤2 — [u8; 8] covers everything.
    piece_list: [[u8; 16]; 12], // up to 10 of a type after promotions (rooks/knights/bishops)
    piece_count: [u8; 12],
}

impl Board {
    fn new() -> Self {

        Board {
            board: [EMPTY; 64],
            side_to_move: Color::White,
            castling_rights: 0b1111,
            en_passant_square: -1,
            halfmove_clock: 0,
            fullmove_number: 1,
            wk_sq: E1,
            bk_sq: E8,
            hash: 0,
            piece_list: [[0u8; 16]; 12],
            piece_count: [0u8; 12],
        }
    }

}

// -----------------------------------------------------------------------------
// FEN parsing
// -----------------------------------------------------------------------------

fn parse_fen(fen: &str) -> Result<Board, &'static str> {

    let mut board = Board::new();

    let parts: Vec<&str> = fen.split_whitespace().collect();

    if parts.len() < 4 {

        return Err("FEN must have at least 4 fields (board, side, castling, en_passant)");
    }

    if fen.len() > 512 {

        return Err("FEN too long");
    }

    // Board placement
    // Each row from the FEN represents one rank, starting with rank 8 (top)
    let rows: Vec<&str> = parts[0].split('/').collect();

    if rows.len() != 8 {

        return Err("FEN board must have exactly 8 ranks");
    }

    let mut white_total = 0u8;

    let mut black_total = 0u8;

    let mut white_pawns = 0u8;

    let mut black_pawns = 0u8;

    let mut white_kings = 0u8;

    let mut black_kings = 0u8;

    for (rank_index, row) in rows.iter().enumerate() {

        let rank = rank_index; // 0 = rank 8, 7 = rank 1
        let mut file = 0;

        for character in row.chars() {

            if character.is_ascii_digit() {

                let count = character.to_digit(10).expect("validated digit") as usize;

                file += count;

                if file > 8 {

                    return Err("FEN rank overflows 8 files");
                }
            } else {

                if file >= 8 {

                    return Err("FEN rank overflows 8 files");
                }

                let piece = match character {
                    'P' => W_PAWN,
                    'N' => W_KNIGHT,
                    'B' => W_BISHOP,
                    'R' => W_ROOK,
                    'Q' => W_QUEEN,
                    'K' => W_KING,
                    'p' => B_PAWN,
                    'n' => B_KNIGHT,
                    'b' => B_BISHOP,
                    'r' => B_ROOK,
                    'q' => B_QUEEN,
                    'k' => B_KING,
                    _ => return Err("FEN contains an invalid piece character"),
                };

                let sq = (rank * 8 + file) as u8;

                board.board[sq as usize] = piece;

                add_piece_to_list(&mut board, sq, piece);

                match piece {
                    W_KING => board.wk_sq = sq,
                    B_KING => board.bk_sq = sq,
                    _ => {},
                }

                if piece > 0 {

                    white_total += 1;

                    match piece {
                        W_PAWN => white_pawns += 1,
                        W_KING => white_kings += 1,
                        _ => {},
                    }
                } else {

                    black_total += 1;

                    match piece {
                        B_PAWN => black_pawns += 1,
                        B_KING => black_kings += 1,
                        _ => {},
                    }
                }

                if white_total > 16 || black_total > 16 {

                    return Err("FEN has more than 16 pieces on one side");
                }

                if white_pawns > 8 || black_pawns > 8 {

                    return Err("FEN has more than 8 pawns on one side");
                }

                file += 1;
            }
        }

        if file != 8 {

            return Err("FEN rank must span exactly 8 files");
        }
    }

    if white_kings != 1 || black_kings != 1 {

        return Err("FEN must contain exactly one king per side");
    }

    // Side to move
    board.side_to_move = if parts[1] == "w" { Color::White } else { Color::Black };

    // Castling rights
    board.castling_rights = 0;

    if parts[2].contains('K') {

        board.castling_rights |= CASTLING_WK;
    }

    if parts[2].contains('Q') {

        board.castling_rights |= CASTLING_WQ;
    }

    if parts[2].contains('k') {

        board.castling_rights |= CASTLING_BK;
    }

    if parts[2].contains('q') {

        board.castling_rights |= CASTLING_BQ;
    }

    // En passant
    if parts[3] != "-" {

        let mut ep_chars = parts[3].chars();

        let file_char = ep_chars.next().ok_or("invalid en-passant square (file)")?;

        let rank_char = ep_chars.next().ok_or("invalid en-passant square (rank)")?;

        let file = match (file_char as u8).checked_sub(b'a') {
            Some(f) if f < 8 => f as usize,
            _ => return Err("invalid en-passant square (file)"),
        };

        let rank = match (rank_char as u8).checked_sub(b'0') {
            Some(r) if (1..=8).contains(&r) => (8 - r) as usize,
            _ => return Err("invalid en-passant square (rank)"),
        };

        board.en_passant_square = (rank * 8 + file) as i8;
    } else {

        board.en_passant_square = -1;
    }

    // Halfmove and fullmove
    if parts.len() > 4 {

        board.halfmove_clock = parts[4].parse().unwrap_or(0);
    }

    if parts.len() > 5 {

        board.fullmove_number = parts[5].parse().unwrap_or(1);
    }

    board.hash = ZOBRIST.hash(&board);

    Ok(board)
}

// -----------------------------------------------------------------------------
// Utility: square names
// -----------------------------------------------------------------------------

fn square_name(square: Square) -> String {

    let file = square % 8;

    let rank = 8 - square / 8;

    format!("{}{}", (b'a' + file) as char, rank)
}

// -----------------------------------------------------------------------------
// Move generation (pseudo-legal)
// -----------------------------------------------------------------------------

const MAX_MOVES: usize = 218;

const MAX_QUIESCE_PLY: u8 = 8;

struct MoveList {
    moves: [Move; MAX_MOVES],
    count: usize,
}

impl MoveList {
    fn new() -> Self {

        MoveList {
            moves: [Move {
                from: 0,
                to: 0,
                promotion: 0,
            }; MAX_MOVES],
            count: 0,
        }
    }

    fn push(&mut self, mv: Move) {

        debug_assert!(self.count < MAX_MOVES, "MoveList overflow");

        self.moves[self.count] = mv;

        self.count += 1;
    }
}

fn generate_pseudo_legal_moves(board: &Board) -> MoveList {

    let mut moves = MoveList::new();

    let current_side = board.side_to_move;

    let current_sign = current_side.sign();

    let (promotion_rank, double_push_rank) = match current_side {
        Color::White => (0, 6),
        Color::Black => (7, 1),
    };

    let pawn_direction = if current_side == Color::White { -8 } else { 8 };

    // Iterate friendly piece lists instead of all 64 squares (2–5× faster).
    let list_base = if current_side == Color::White { 0 } else { 6 };

    for type_offset in 0..6u8 {

        let list_idx = list_base + type_offset as usize;

        let count = board.piece_count[list_idx];

        for entry_idx in 0..count {

            let square = board.piece_list[list_idx][entry_idx as usize];

            let piece_type = type_offset + 1;

            // Sanity: the board must have the expected piece at this square.
            let expected_piece: i8 = if current_side == Color::White { piece_type as i8 } else { -(piece_type as i8) };
            debug_assert!(board.board[square as usize] == expected_piece,
                "piece list mismatch: list_idx={}, square={}, expected={}, actual={}",
                list_idx, square, expected_piece, board.board[square as usize]);

            match piece_type {
            1 => {

                // ── Pawn ──
                let one_forward = (square as i8 + pawn_direction) as Square;

                if one_forward < 64 && board.board[one_forward as usize] == EMPTY {

                    if square / 8 == promotion_rank {

                        moves.push(Move::with_promotion(
                            square,
                            one_forward,
                            if current_sign == 1 { W_QUEEN } else { B_QUEEN },
                        ));

                        moves.push(Move::with_promotion(
                            square,
                            one_forward,
                            if current_sign == 1 { W_KNIGHT } else { B_KNIGHT },
                        ));

                        moves.push(Move::with_promotion(
                            square,
                            one_forward,
                            if current_sign == 1 { W_ROOK } else { B_ROOK },
                        ));

                        moves.push(Move::with_promotion(
                            square,
                            one_forward,
                            if current_sign == 1 { W_BISHOP } else { B_BISHOP },
                        ));
                    } else {

                        moves.push(Move::new(square, one_forward));

                        // Double push from starting rank
                        if square / 8 == double_push_rank {

                            let two_forward = (square as i8 + 2 * pawn_direction) as Square;

                            if board.board[two_forward as usize] == EMPTY {

                                moves.push(Move::new(square, two_forward));
                            }
                        }
                    }
                }

                // Captures
                for &file_offset in &[-1, 1] {

                    if (file_offset == -1 && square.is_multiple_of(8)) || (file_offset == 1 && square % 8 == 7) {
                        continue;
                    }

                    let target_square = (square as i8 + pawn_direction + file_offset) as Square;
                    if target_square >= 64 { continue; }

                    let target = board.board[target_square as usize];

                    let is_opponent = target != EMPTY && (target > 0) != (current_sign == 1);

                    let is_en_passant = target_square as i8 == board.en_passant_square;
                    if !is_opponent && !is_en_passant { continue; }

                    if square / 8 == promotion_rank {

                        moves.push(Move::with_promotion(
                            square,
                            target_square,
                            if current_sign == 1 { W_QUEEN } else { B_QUEEN },
                        ));

                        moves.push(Move::with_promotion(
                            square,
                            target_square,
                            if current_sign == 1 { W_KNIGHT } else { B_KNIGHT },
                        ));

                        moves.push(Move::with_promotion(
                            square,
                            target_square,
                            if current_sign == 1 { W_ROOK } else { B_ROOK },
                        ));

                        moves.push(Move::with_promotion(
                            square,
                            target_square,
                            if current_sign == 1 { W_BISHOP } else { B_BISHOP },
                        ));
                    } else {

                        moves.push(Move::new(square, target_square));
                    }
                }
            },
            2 => {

                // ── Knight ──
                for &offset in &[-17, -15, -10, -6, 6, 10, 15, 17] {

                    let target_square = square as i8 + offset;
                    if !(0..64).contains(&target_square) { continue; }

                    // File-wrap check: knight moves change file by at most 2
                    let from_file = square % 8;
                    let to_file = target_square as usize % 8;
                    if (from_file as i8 - to_file as i8).abs() > 2 { continue; }

                    let target = board.board[target_square as usize];
                    if target == EMPTY || (target > 0) != (current_sign == 1) {
                        moves.push(Move::new(square, target_square as u8));
                    }
                }
            },
            3 => {

                // ── Bishop ──
                for &direction in &[-9, -7, 7, 9] {

                    // File boundary check for initial step
                    let moving_left = direction == -9 || direction == 7;

                    let moving_right = direction == -7 || direction == 9;

                    if (moving_left && square.is_multiple_of(8)) || (moving_right && square % 8 == 7) {

                        continue;
                    }

                    let mut current_square = square as i8 + direction;

                    while (0..64).contains(&current_square) {

                        let target = board.board[current_square as usize];

                        if target != EMPTY {

                            if (target > 0) != (current_sign == 1) {

                                moves.push(Move::new(square, current_square as u8));
                            }

                            break;
                        }

                        moves.push(Move::new(square, current_square as u8));

                        // File wrap check before next step
                        if (moving_left && (current_square as usize).is_multiple_of(8))
                            || (moving_right && (current_square as usize) % 8 == 7)
                        {

                            break;
                        }

                        current_square += direction;
                    }
                }
            },
            4 => {

                // ── Rook ──
                for &direction in &[-8, -1, 1, 8] {

                    // File boundary: horizontal moves can't wrap off the edge
                    if (direction == -1 && square.is_multiple_of(8)) || (direction == 1 && square % 8 == 7) {

                        continue;
                    }

                    let mut current_square = square as i8 + direction;

                    while (0..64).contains(&current_square) {

                        let target = board.board[current_square as usize];

                        if target != EMPTY {

                            if (target > 0) != (current_sign == 1) {

                                moves.push(Move::new(square, current_square as u8));
                            }

                            break;
                        }

                        moves.push(Move::new(square, current_square as u8));

                        // File wrap check before next horizontal step
                        if (direction == -1 && (current_square as usize).is_multiple_of(8))
                            || (direction == 1 && (current_square as usize) % 8 == 7)
                        {

                            break;
                        }

                        current_square += direction;
                    }
                }
            },
            5 => {

                // ── Queen (bishop + rook combined) ──
                for &direction in &[-9, -8, -7, -1, 1, 7, 8, 9] {

                    // File boundary: diagonal/horizontal moves can't wrap
                    let moving_left = direction == -9 || direction == -1 || direction == 7;

                    let moving_right = direction == -7 || direction == 1 || direction == 9;

                    if (moving_left && square.is_multiple_of(8)) || (moving_right && square % 8 == 7) {

                        continue;
                    }

                    let mut current_square = square as i8 + direction;

                    while (0..64).contains(&current_square) {

                        let target = board.board[current_square as usize];

                        if target != EMPTY {

                            if (target > 0) != (current_sign == 1) {

                                moves.push(Move::new(square, current_square as u8));
                            }

                            break;
                        }

                        moves.push(Move::new(square, current_square as u8));

                        // File wrap check before next step
                        if (moving_left && (current_square as usize).is_multiple_of(8))
                            || (moving_right && (current_square as usize) % 8 == 7)
                        {

                            break;
                        }

                        current_square += direction;
                    }
                }
            },
            6 => {

                // ── King ──
                for &offset in &[-9, -8, -7, -1, 1, 7, 8, 9] {

                    let target_square = square as i8 + offset;
                    if !(0..64).contains(&target_square) { continue; }

                    let target = board.board[target_square as usize];
                    if target == EMPTY || (target > 0) != (current_sign == 1) {
                        moves.push(Move::new(square, target_square as u8));
                    }
                }

                // Castling
                if current_side == Color::White {

                    if (board.castling_rights & CASTLING_WK) != 0
                        && board.board[F1 as usize] == EMPTY
                        && board.board[G1 as usize] == EMPTY
                        && board.board[H1 as usize] == W_ROOK
                        && board.board[E1 as usize] == W_KING
                    {

                        moves.push(Move::new(E1, G1));
                    }

                    if (board.castling_rights & CASTLING_WQ) != 0
                        && board.board[B1 as usize] == EMPTY
                        && board.board[C1 as usize] == EMPTY
                        && board.board[D1 as usize] == EMPTY
                        && board.board[A1 as usize] == W_ROOK
                        && board.board[E1 as usize] == W_KING
                    {

                        moves.push(Move::new(E1, C1));
                    }
                } else {

                    if (board.castling_rights & CASTLING_BK) != 0
                        && board.board[F8 as usize] == EMPTY
                        && board.board[G8 as usize] == EMPTY
                        && board.board[H8 as usize] == B_ROOK
                        && board.board[E8 as usize] == B_KING
                    {

                        moves.push(Move::new(E8, G8));
                    }

                    if (board.castling_rights & CASTLING_BQ) != 0
                        && board.board[B8 as usize] == EMPTY
                        && board.board[C8 as usize] == EMPTY
                        && board.board[D8 as usize] == EMPTY
                        && board.board[A8 as usize] == B_ROOK
                        && board.board[E8 as usize] == B_KING
                    {

                        moves.push(Move::new(E8, C8));
                    }
                }
            },
            _ => {},
        }
    }
}

    moves
}

// -----------------------------------------------------------------------------
// Attack detection (for legality and check)
// -----------------------------------------------------------------------------

fn is_square_attacked(board: &Board, square: Square, by_color: Color) -> bool {

    let attacker_sign = by_color.sign();

    // Pawn attacks
    let pawn_attack_offsets = if by_color == Color::White { [7, 9] } else { [-7, -9] };

    for &offset in &pawn_attack_offsets {

        let from = square as i8 + offset;

        if (0..64).contains(&from) {

            let piece = board.board[from as usize];

            if piece == (if by_color == Color::White { W_PAWN } else { B_PAWN }) {

                return true;
            }
        }
    }

    // Knight attacks
    for &offset in &[-17, -15, -10, -6, 6, 10, 15, 17] {

        let from = square as i8 + offset;

        if (0..64).contains(&from) {

            let piece = board.board[from as usize];

            if piece == (if by_color == Color::White { W_KNIGHT } else { B_KNIGHT }) {

                return true;
            }
        }
    }

    // King attacks
    for &offset in &[-9, -8, -7, -1, 1, 7, 8, 9] {

        let from = square as i8 + offset;

        if (0..64).contains(&from) {

            let piece = board.board[from as usize];

            if piece == (if by_color == Color::White { W_KING } else { B_KING }) {

                return true;
            }
        }
    }

    // Sliding pieces: bishop/queen diagonals
    for &direction in &[-9, -7, 7, 9] {

        let mut current_square = square as i8 + direction;

        while (0..64).contains(&current_square) {

            let piece = board.board[current_square as usize];

            if piece != EMPTY {

                if (piece > 0) == (attacker_sign == 1) {

                    let piece_type_abs = piece.abs();

                    if piece_type_abs == 3 || piece_type_abs == 5 {

                        // Bishop or Queen
                        return true;
                    }
                }

                break;
            }

            current_square += direction;
        }
    }

    // Sliding pieces: rook/queen straight lines
    for &direction in &[-8, -1, 1, 8] {

        let mut current_square = square as i8 + direction;

        while (0..64).contains(&current_square) {

            let piece = board.board[current_square as usize];

            if piece != EMPTY {

                if (piece > 0) == (attacker_sign == 1) {

                    let piece_type_abs = piece.abs();

                    if piece_type_abs == 4 || piece_type_abs == 5 {

                        // Rook or Queen
                        return true;
                    }
                }

                break;
            }

            current_square += direction;
        }
    }

    false
}

fn in_check(board: &Board) -> bool {

    let king_square = if board.side_to_move == Color::White {

        board.wk_sq
    } else {

        board.bk_sq
    };

    is_square_attacked(board, king_square, board.side_to_move.opposite())
}

// -----------------------------------------------------------------------------
// Make / unmake move with full state backup
// -----------------------------------------------------------------------------

struct MoveUndo {
    captured: i8,
    ep_square: i8,
    castling_rights: u8,
    halfmove_clock: u16,
    hash: u64,
}

fn make_move(board: &mut Board, mv: Move) -> MoveUndo {

    validate_piece_lists!(board, "make_move_START({},{})", mv.from, mv.to);

    let current_side = board.side_to_move;

    let opponent = current_side.opposite();

    let captured = board.board[mv.to as usize];

    let saved_ep_square = board.en_passant_square;

    let saved_castling_rights = board.castling_rights;

    let saved_halfmove_clock = board.halfmove_clock;

    let saved_hash = board.hash;

    let moving_piece = board.board[mv.from as usize];

    // Safety: reject corrupt moves (EMPTY source square)
    if moving_piece == EMPTY {

        return MoveUndo {
            captured: EMPTY,
            ep_square: -1,
            castling_rights: 0,
            halfmove_clock: 0,
            hash: board.hash,
        };
    }

    let is_king = moving_piece == (if current_side == Color::White { W_KING } else { B_KING });

    let is_rook = !is_king && moving_piece.abs() == 4;

    // ── Incremental Zobrist hash: XOR out old state ──

    // Side to move toggle (XOR removes if present, adds if absent)
    board.hash ^= ZOBRIST.keys[780];

    // Old en passant file (only if it was hashed — i.e. a capture was possible)
    if saved_ep_square != -1 && ep_capture_possible(board) {

        board.hash ^= ZOBRIST.keys[772 + (saved_ep_square as usize & 7)];
    }

    // Track whether a list rebuild is needed after the board update
    let mut list_needs_rebuild = EMPTY;

    // Moving piece leaves its from square
    board.hash ^= ZOBRIST.keys[zobrist_key(moving_piece, mv.from)];

    // Captured piece removed from to square (if any)
    if captured != EMPTY {

        board.hash ^= ZOBRIST.keys[zobrist_key(captured, mv.to)];

        // Remove captured piece from list. If the list was corrupt and
        // the piece isn't found, we'll rebuild AFTER the board update
        // (when the captured piece is no longer on the board).
        if !remove_piece_from_list(board, mv.to, captured) {
            list_needs_rebuild = captured;
        }
    }

    // ── Board changes ──

    // Move piece
    board.board[mv.to as usize] = moving_piece;

    board.board[mv.from as usize] = EMPTY;

    // If a capture remove failed (list was corrupt), rebuild the list
    // now — the board has overwritten the captured piece, so the rebuild
    // won't include it.
    if list_needs_rebuild != EMPTY {
        rebuild_piece_list(board, list_needs_rebuild);
    }

    // Piece list: move piece from source to destination
    if remove_piece_or_rebuild(board, mv.from, moving_piece) {

        add_piece_to_list(board, mv.to, moving_piece);
    }

    // Track king position
    if is_king {

        if current_side == Color::White {

            board.wk_sq = mv.to;
        } else {

            board.bk_sq = mv.to;
        }
    }

    // Promotion
    if mv.promotion != 0 {

        board.board[mv.to as usize] = mv.promotion;

        // Piece list: swap pawn for promoted piece
        // Note: remove_piece_or_rebuild rebuilds the PAWN list on failure.
        // The add below is for the PROMOTED piece (different list), so we
        // always add it — the return value only controls pawn-list logic.
        let pawn_piece = if current_side == Color::White { W_PAWN } else { B_PAWN };

        remove_piece_or_rebuild(board, mv.to, pawn_piece);

        add_piece_to_list(board, mv.to, mv.promotion);
    }

    // En passant capture
    if (mv.to as i8) == saved_ep_square && moving_piece.abs() == 1 {

        let captured_pawn_square = if current_side == Color::White {

            mv.to + 8
        } else {

            mv.to - 8
        };

        board.board[captured_pawn_square as usize] = EMPTY;

        // Piece list: remove captured pawn
        let captured_pawn = if current_side == Color::White { B_PAWN } else { W_PAWN };

        remove_piece_or_rebuild(board, captured_pawn_square, captured_pawn);
    }
    // Castling: move the rook when the king castles
    if is_king {
        match (mv.from, mv.to) {
            (E1, G1) => {
                board.board[F1 as usize] = W_ROOK;
                board.board[H1 as usize] = EMPTY;
                if remove_piece_or_rebuild(board, H1, W_ROOK) {
                    add_piece_to_list(board, F1, W_ROOK);
                }
            },
            (E1, C1) => {
                board.board[D1 as usize] = W_ROOK;
                board.board[A1 as usize] = EMPTY;
                if remove_piece_or_rebuild(board, A1, W_ROOK) {
                    add_piece_to_list(board, D1, W_ROOK);
                }
            },
            (E8, G8) => {
                board.board[F8 as usize] = B_ROOK;
                board.board[H8 as usize] = EMPTY;
                if remove_piece_or_rebuild(board, H8, B_ROOK) {
                    add_piece_to_list(board, F8, B_ROOK);
                }
            },
            (E8, C8) => {
                board.board[D8 as usize] = B_ROOK;
                board.board[A8 as usize] = EMPTY;
                if remove_piece_or_rebuild(board, A8, B_ROOK) {
                    add_piece_to_list(board, D8, B_ROOK);
                }
            },
            _ => {},
        }
    }

    // Update castling rights
    if is_king {

        if current_side == Color::White {

            board.castling_rights &= !(CASTLING_WK | CASTLING_WQ);
        } else {

            board.castling_rights &= !(CASTLING_BK | CASTLING_BQ);
        }
    }

    // Rook was captured on its starting square
    if captured == W_ROOK && mv.to == H1 {

        board.castling_rights &= !CASTLING_WK;
    }

    if captured == W_ROOK && mv.to == A1 {

        board.castling_rights &= !CASTLING_WQ;
    }

    if captured == B_ROOK && mv.to == H8 {

        board.castling_rights &= !CASTLING_BK;
    }

    if captured == B_ROOK && mv.to == A8 {

        board.castling_rights &= !CASTLING_BQ;
    }

    // Rook moved from its starting square
    if is_rook && mv.from == H1 {

        board.castling_rights &= !CASTLING_WK;
    }

    if is_rook && mv.from == A1 {

        board.castling_rights &= !CASTLING_WQ;
    }

    if is_rook && mv.from == H8 {

        board.castling_rights &= !CASTLING_BK;
    }

    if is_rook && mv.from == A8 {

        board.castling_rights &= !CASTLING_BQ;
    }

    // Update en passant
    if board.board[mv.to as usize].abs() == 1 && (mv.to as i8 - mv.from as i8).abs() == 16 {

        board.en_passant_square = (mv.from as i8 + mv.to as i8) / 2;
    } else {

        board.en_passant_square = -1;
    }

    // Halfmove clock: reset on capture or pawn move
    if captured != EMPTY || board.board[mv.to as usize].abs() == 1 {

        board.halfmove_clock = 0;
    } else {

        board.halfmove_clock += 1;
    }

    // Switch to opponent's turn
    board.side_to_move = opponent;

    if board.side_to_move == Color::White {

        board.fullmove_number += 1;
    }

    // ── Incremental Zobrist hash: XOR in new state ──

    // Final piece on to square (handles promotion)
    let final_piece = if mv.promotion != 0 { mv.promotion } else { moving_piece };

    board.hash ^= ZOBRIST.keys[zobrist_key(final_piece, mv.to)];

    // En passant capture: remove the captured pawn
    if (mv.to as i8) == saved_ep_square && moving_piece.abs() == 1 {

        let pawn_sq = if current_side == Color::White { mv.to + 8 } else { mv.to - 8 };

        let captured_pawn = if current_side == Color::White { B_PAWN } else { W_PAWN };

        board.hash ^= ZOBRIST.keys[zobrist_key(captured_pawn, pawn_sq)];
    }
    // Castling rook movement (hash update)
    if is_king {
        match (mv.from, mv.to) {
            (E1, G1) => {
                board.hash ^= ZOBRIST.keys[zobrist_key(W_ROOK, H1)];
                board.hash ^= ZOBRIST.keys[zobrist_key(W_ROOK, F1)];
            },
            (E1, C1) => {
                board.hash ^= ZOBRIST.keys[zobrist_key(W_ROOK, A1)];
                board.hash ^= ZOBRIST.keys[zobrist_key(W_ROOK, D1)];
            },
            (E8, G8) => {
                board.hash ^= ZOBRIST.keys[zobrist_key(B_ROOK, H8)];
                board.hash ^= ZOBRIST.keys[zobrist_key(B_ROOK, F8)];
            },
            (E8, C8) => {
                board.hash ^= ZOBRIST.keys[zobrist_key(B_ROOK, A8)];
                board.hash ^= ZOBRIST.keys[zobrist_key(B_ROOK, D8)];
            },
            _ => {},
        }
    }

    // Lost castling rights: XOR out the rights that were removed
    let lost_rights = saved_castling_rights ^ board.castling_rights;

    if lost_rights & CASTLING_WK != 0 {

        board.hash ^= ZOBRIST.keys[768];
    }

    if lost_rights & CASTLING_WQ != 0 {

        board.hash ^= ZOBRIST.keys[769];
    }

    if lost_rights & CASTLING_BK != 0 {

        board.hash ^= ZOBRIST.keys[770];
    }

    if lost_rights & CASTLING_BQ != 0 {

        board.hash ^= ZOBRIST.keys[771];
    }

    // New en passant square (only hashed if a capture is possible in the new position)
    if board.en_passant_square != -1 && ep_capture_possible(board) {

        board.hash ^= ZOBRIST.keys[772 + (board.en_passant_square as usize & 7)];
    }

    validate_piece_lists!(board, "make_move({},{})", mv.from, mv.to);

    MoveUndo {
        captured,
        ep_square: saved_ep_square,
        castling_rights: saved_castling_rights,
        halfmove_clock: saved_halfmove_clock,
        hash: saved_hash,
    }
}

fn unmake_move(board: &mut Board, mv: Move, undo: MoveUndo) {

    validate_piece_lists!(board, "unmake_move_START({},{})", mv.from, mv.to);

    // Restore pre-move Zobrist hash (avoids needing to reverse the incremental update)
    board.hash = undo.hash;

    let current_side = board.side_to_move; // side AFTER the move (opponent of the one that moved)
    let opponent = current_side.opposite();

    // Switch back to the side that made the move
    board.side_to_move = opponent;

    // mirror of make_move: increment happened when switching TO White,
    // so decrement when undoing from White
    if current_side == Color::White {

        board.fullmove_number -= 1;
    }

    // Restore state from before the move
    board.castling_rights = undo.castling_rights;

    board.en_passant_square = undo.ep_square;

    board.halfmove_clock = undo.halfmove_clock;

    // ── Piece list: save final piece at destination before board changes ──
    let final_piece_at_to = board.board[mv.to as usize];

    // Restore the piece to its original square
    board.board[mv.from as usize] = board.board[mv.to as usize];

    board.board[mv.to as usize] = undo.captured;

    // Piece list: move piece back from destination to source.
    // final_piece_at_to is the same piece type in both lists, so the
    // conditional add logic is correct (both sides reference the same list).
    // On rebuild failure, the list already has the piece at the board's
    // current location (mv.from), so we skip the add.
    if remove_piece_or_rebuild(board, mv.to, final_piece_at_to) {

        add_piece_to_list(board, mv.from, final_piece_at_to);
    }

    // Restore captured piece to destination list.
    // Check if already in the list to avoid duplicate from a stale entry.
    if undo.captured != EMPTY {

        let idx = piece_to_list_idx(undo.captured);
        let mut already_present = false;
        for entry_idx in 0..board.piece_count[idx] {
            if board.piece_list[idx][entry_idx as usize] == mv.to {
                already_present = true;
                break;
            }
        }
        if !already_present {

            add_piece_to_list(board, mv.to, undo.captured);
        }
    }

    // Promotion: revert the promoted piece back to a pawn
    if mv.promotion != 0 {

        board.board[mv.from as usize] = if opponent == Color::White { W_PAWN } else { B_PAWN };

        // Remove promoted piece from list (rebuild promoted list if needed)
        // then always add the pawn — these are different lists.
        remove_piece_or_rebuild(board, mv.from, mv.promotion);

        let pawn_piece = if opponent == Color::White { W_PAWN } else { B_PAWN };

        add_piece_to_list(board, mv.from, pawn_piece);
    }

    // En passant capture: restore the captured pawn
    if (mv.to as i8) == undo.ep_square && undo.captured == EMPTY {

        let captured_pawn_square = if opponent == Color::White { mv.to + 8 } else { mv.to - 8 };

        let captured_pawn = if opponent == Color::White { B_PAWN } else { W_PAWN };

        board.board[captured_pawn_square as usize] = captured_pawn;

        // Rebuild the captured pawn's list from the board to clear any
        // stale entries before adding — en passant is the most common
        // trigger for latent duplicate bugs.
        rebuild_piece_list(board, captured_pawn);
    }

    // Castling: restore the rook to its original square
    // Uses named constants (E1=60, E8=4, etc.) — no magic numbers.
    if board.board[mv.from as usize] == (if opponent == Color::White { W_KING } else { B_KING }) {
        match (mv.from, mv.to) {
            (E8, G8) => {
                board.board[F8 as usize] = EMPTY;
                board.board[H8 as usize] = B_ROOK;
                if remove_piece_or_rebuild(board, F8, B_ROOK) {
                    add_piece_to_list(board, H8, B_ROOK);
                }
            },
            (E8, C8) => {
                board.board[D8 as usize] = EMPTY;
                board.board[A8 as usize] = B_ROOK;
                if remove_piece_or_rebuild(board, D8, B_ROOK) {
                    add_piece_to_list(board, A8, B_ROOK);
                }
            },
            (E1, G1) => {
                board.board[F1 as usize] = EMPTY;
                board.board[H1 as usize] = W_ROOK;
                if remove_piece_or_rebuild(board, F1, W_ROOK) {
                    add_piece_to_list(board, H1, W_ROOK);
                }
            },
            (E1, C1) => {
                board.board[D1 as usize] = EMPTY;
                board.board[A1 as usize] = W_ROOK;
                if remove_piece_or_rebuild(board, D1, W_ROOK) {
                    add_piece_to_list(board, A1, W_ROOK);
                }
            },
            _ => {},
        }
    }
    // Restore king square if the moved piece was a king
    let restored_piece = board.board[mv.from as usize];

    match restored_piece {
        W_KING => board.wk_sq = mv.from,
        B_KING => board.bk_sq = mv.from,
        _ => {},
    }

    validate_piece_lists!(board, "unmake_move({},{})", mv.from, mv.to);
}

// -----------------------------------------------------------------------------
// Evaluation
// -----------------------------------------------------------------------------

fn evaluate_board(board: &Board) -> i32 {
    let mut score = 0i32;

    // White pieces: list indices 0..5 (W_PAWN..W_KING)
    for type_idx in 0..6 {
        let piece = (type_idx + 1) as i8;
        let count = board.piece_count[type_idx];
        let list = &board.piece_list[type_idx];
        for entry_idx in 0..count {
            let sq = list[entry_idx as usize];
            score += piece_value(piece) + PST[sq as usize];
        }
    }

    // Black pieces: list indices 6..11 (B_PAWN..B_KING)
    for type_idx in 6..12 {
        let piece = -((type_idx - 5) as i8);
        let count = board.piece_count[type_idx];
        let list = &board.piece_list[type_idx];
        for entry_idx in 0..count {
            let sq = list[entry_idx as usize];
            // Black PST is mirrored horizontally
            score -= piece_value(piece) + PST[(63 - sq) as usize];
        }
    }

    score
}

// -----------------------------------------------------------------------------
// Quiescence search (captures only — handles horizon effect)
// -----------------------------------------------------------------------------

/// Static Exchange Evaluation: skip obviously losing captures in quiescence.
/// Returns true if the capture is worth searching.
fn good_capture(board: &Board, mv: &Move) -> bool {

    // Promotions are always worth searching
    if mv.promotion != 0 {

        return true;
    }

    let victim = piece_value(board.board[mv.to as usize]);

    let attacker = piece_value(board.board[mv.from as usize]);

    // Hanging piece (undefended) — always search it
    if !is_square_attacked(board, mv.to, board.side_to_move.opposite()) {

        return true;
    }

    // Defended square: only search if we're not losing material
    attacker <= victim + 50 // allow up to ~half a pawn loss
}

fn quiesce(board: &mut Board, mut alpha: i32, beta: i32, ply: u8) -> i32 {

    // Depth limit: prevent explosion on long capture chains
    if ply >= MAX_QUIESCE_PLY {

        return evaluate_board(board);
    }

    let in_check_pos = in_check(board);

    // Static evaluation at this position (the "standing pat" option).
    // Skip stand-pat if in check — we must resolve the check.
    if !in_check_pos {

        let stand_pat = evaluate_board(board);

        if stand_pat >= beta {

            return beta;
        }

        if stand_pat > alpha {

            alpha = stand_pat;
        }
    }

    let moves = generate_pseudo_legal_moves(board);

    for i in 0..moves.count {

        let mv = moves.moves[i];

        // When NOT in check: only captures and promotions (quiet moves skipped).
        // When in check: generate all legal moves — the king might need to move
        // or a piece block, not just capture.
        if !in_check_pos && board.board[mv.to as usize] == EMPTY && mv.promotion == 0 {

            continue;
        }

        // SEE: skip obviously losing captures in quiescence
        if !in_check_pos && board.board[mv.to as usize] != EMPTY && mv.promotion == 0
            && !good_capture(board, &mv)
        {

            continue;
        }

        let undo = make_move(board, mv);

        if in_check(board) {

            unmake_move(board, mv, undo);

            continue;
        }

        let score = -quiesce(board, -beta, -alpha, ply + 1);

        unmake_move(board, mv, undo);

        if score >= beta {

            return beta;
        }

        if score > alpha {

            alpha = score;
        }
    }

    alpha
}

// -----------------------------------------------------------------------------
// Zobrist hashing (standard PolyGlot table, PolyGlot 1.4 / python-chess)
// -----------------------------------------------------------------------------

/// Compute the Zobrist key index for a piece on a square.
/// Uses the python-chess / PolyGlot convention (matches the book):
///   piece_index = (piece_type - 1) * 2 + pivot, where pivot = 0 for BLACK, 1 for WHITE
///   (python-chess `occupied_co[0]` = black, `occupied_co[1]` = white)
///   squares are a1=0 based (a1=0, h8=63) — NOT our engine's a8=0 convention.
fn zobrist_key(piece: i8, sq: Square) -> usize {
    debug_assert!(piece != EMPTY, "zobrist_key called with EMPTY piece");
    if piece == EMPTY {
        return 0;
    }
    let piece_type = piece.unsigned_abs() as usize - 1; // 0..5
    // python-chess pivot: 0 = BLACK, 1 = WHITE (opposite of our color bit)
    let pivot = if piece > 0 { 1 } else { 0 };
    // Convert our a8=0 square to python-chess a1=0 square.
    let file = (sq & 7) as usize;
    let rank = (sq >> 3) as usize; // our rank 0 = rank 8
    let py_sq = (7 - rank) * 8 + file;
    (piece_type * 2 + pivot) * 64 + py_sq
}

/// True if a pawn of the side to move can capture onto the current en-passant
/// square (python-chess `hash_ep_square` rule). Legality is irrelevant — only
/// pawn adjacency on the rank in front of the ep square matters.
fn ep_capture_possible(board: &Board) -> bool {
    let ep = board.en_passant_square;
    if ep < 0 {
        return false;
    }
    let ep_rank = (ep as usize) >> 3;
    let ep_file = (ep as usize) & 7;
    // Pawns that could capture sit one rank toward the side-to-move's own side.
    // White to move: pawns at ep_rank + 1 (our rank 0 = rank 8, so +1 = closer to rank 1).
    // Black to move: pawns at ep_rank - 1.
    let pawn_rank = if board.side_to_move == Color::White {
        ep_rank + 1
    } else {
        ep_rank - 1
    };
    if pawn_rank > 7 {
        return false;
    }
    let side_pawn = if board.side_to_move == Color::White { W_PAWN } else { B_PAWN };
    if ep_file > 0 && board.board[pawn_rank * 8 + ep_file - 1] == side_pawn {
        return true;
    }
    if ep_file < 7 && board.board[pawn_rank * 8 + ep_file + 1] == side_pawn {
        return true;
    }
    false
}

/// Standard PolyGlot Zobrist table (781 keys, python-chess `POLYGLOT_RANDOM_ARRAY`).
/// Index layout (python-chess convention):
///   pieces:  (piece_type-1)*2 + pivot for pivot=0 BLACK / 1 WHITE, times 64,
///            plus a1=0 based square → indices 0..767
///   768=WK castling, 769=WQ, 770=BK, 771=BQ
///   772..779 = en passant file
///   780 = side to move, XORed when WHITE to move
pub const POLYGLOT_KEYS: [u64; 781] = [
    0x9d39247e33776d41, 0x2af7398005aaa5c7, 0x44db015024623547, 0x9c15f73e62a76ae2,
    0x75834465489c0c89, 0x3290ac3a203001bf, 0x0fbbad1f61042279, 0xe83a908ff2fb60ca,
    0x0d7e765d58755c10, 0x1a083822ceafe02d, 0x9605d5f0e25ec3b0, 0xd021ff5cd13a2ed5,
    0x40bdf15d4a672e32, 0x011355146fd56395, 0x5db4832046f3d9e5, 0x239f8b2d7ff719cc,
    0x05d1a1ae85b49aa1, 0x679f848f6e8fc971, 0x7449bbff801fed0b, 0x7d11cdb1c3b7adf0,
    0x82c7709e781eb7cc, 0xf3218f1c9510786c, 0x331478f3af51bbe6, 0x4bb38de5e7219443,
    0xaa649c6ebcfd50fc, 0x8dbd98a352afd40b, 0x87d2074b81d79217, 0x19f3c751d3e92ae1,
    0xb4ab30f062b19abf, 0x7b0500ac42047ac4, 0xc9452ca81a09d85d, 0x24aa6c514da27500,
    0x4c9f34427501b447, 0x14a68fd73c910841, 0xa71b9b83461cbd93, 0x03488b95b0f1850f,
    0x637b2b34ff93c040, 0x09d1bc9a3dd90a94, 0x3575668334a1dd3b, 0x735e2b97a4c45a23,
    0x18727070f1bd400b, 0x1fcbacd259bf02e7, 0xd310a7c2ce9b6555, 0xbf983fe0fe5d8244,
    0x9f74d14f7454a824, 0x51ebdc4ab9ba3035, 0x5c82c505db9ab0fa, 0xfcf7fe8a3430b241,
    0x3253a729b9ba3dde, 0x8c74c368081b3075, 0xb9bc6c87167c33e7, 0x7ef48f2b83024e20,
    0x11d505d4c351bd7f, 0x6568fca92c76a243, 0x4de0b0f40f32a7b8, 0x96d693460cc37e5d,
    0x42e240cb63689f2f, 0x6d2bdcdae2919661, 0x42880b0236e4d951, 0x5f0f4a5898171bb6,
    0x39f890f579f92f88, 0x93c5b5f47356388b, 0x63dc359d8d231b78, 0xec16ca8aea98ad76,
    0x5355f900c2a82dc7, 0x07fb9f855a997142, 0x5093417aa8a7ed5e, 0x7bcbc38da25a7f3c,
    0x19fc8a768cf4b6d4, 0x637a7780decfc0d9, 0x8249a47aee0e41f7, 0x79ad695501e7d1e8,
    0x14acbaf4777d5776, 0xf145b6beccdea195, 0xdabf2ac8201752fc, 0x24c3c94df9c8d3f6,
    0xbb6e2924f03912ea, 0x0ce26c0b95c980d9, 0xa49cd132bfbf7cc4, 0xe99d662af4243939,
    0x27e6ad7891165c3f, 0x8535f040b9744ff1, 0x54b3f4fa5f40d873, 0x72b12c32127fed2b,
    0xee954d3c7b411f47, 0x9a85ac909a24eaa1, 0x70ac4cd9f04f21f5, 0xf9b89d3e99a075c2,
    0x87b3e2b2b5c907b1, 0xa366e5b8c54f48b8, 0xae4a9346cc3f7cf2, 0x1920c04d47267bbd,
    0x87bf02c6b49e2ae9, 0x092237ac237f3859, 0xff07f64ef8ed14d0, 0x8de8dca9f03cc54e,
    0x9c1633264db49c89, 0xb3f22c3d0b0b38ed, 0x390e5fb44d01144b, 0x5bfea5b4712768e9,
    0x1e1032911fa78984, 0x9a74acb964e78cb3, 0x4f80f7a035dafb04, 0x6304d09a0b3738c4,
    0x2171e64683023a08, 0x5b9b63eb9ceff80c, 0x506aacf489889342, 0x1881afc9a3a701d6,
    0x6503080440750644, 0xdfd395339cdbf4a7, 0xef927dbcf00c20f2, 0x7b32f7d1e03680ec,
    0xb9fd7620e7316243, 0x05a7e8a57db91b77, 0xb5889c6e15630a75, 0x4a750a09ce9573f7,
    0xcf464cec899a2f8a, 0xf538639ce705b824, 0x3c79a0ff5580ef7f, 0xede6c87f8477609d,
    0x799e81f05bc93f31, 0x86536b8cf3428a8c, 0x97d7374c60087b73, 0xa246637cff328532,
    0x043fcae60cc0eba0, 0x920e449535dd359e, 0x70eb093b15b290cc, 0x73a1921916591cbd,
    0x56436c9fe1a1aa8d, 0xefac4b70633b8f81, 0xbb215798d45df7af, 0x45f20042f24f1768,
    0x930f80f4e8eb7462, 0xff6712ffcfd75ea1, 0xae623fd67468aa70, 0xdd2c5bc84bc8d8fc,
    0x7eed120d54cf2dd9, 0x22fe545401165f1c, 0xc91800e98fb99929, 0x808bd68e6ac10365,
    0xdec468145b7605f6, 0x1bede3a3aef53302, 0x43539603d6c55602, 0xaa969b5c691ccb7a,
    0xa87832d392efee56, 0x65942c7b3c7e11ae, 0xded2d633cad004f6, 0x21f08570f420e565,
    0xb415938d7da94e3c, 0x91b859e59ecb6350, 0x10cff333e0ed804a, 0x28aed140be0bb7dd,
    0xc5cc1d89724fa456, 0x5648f680f11a2741, 0x2d255069f0b7dab3, 0x9bc5a38ef729abd4,
    0xef2f054308f6a2bc, 0xaf2042f5cc5c2858, 0x480412bab7f5be2a, 0xaef3af4a563dfe43,
    0x19afe59ae451497f, 0x52593803dff1e840, 0xf4f076e65f2ce6f0, 0x11379625747d5af3,
    0xbce5d2248682c115, 0x9da4243de836994f, 0x066f70b33fe09017, 0x4dc4de189b671a1c,
    0x51039ab7712457c3, 0xc07a3f80c31fb4b4, 0xb46ee9c5e64a6e7c, 0xb3819a42abe61c87,
    0x21a007933a522a20, 0x2df16f761598aa4f, 0x763c4a1371b368fd, 0xf793c46702e086a0,
    0xd7288e012aeb8d31, 0xde336a2a4bc1c44b, 0x0bf692b38d079f23, 0x2c604a7a177326b3,
    0x4850e73e03eb6064, 0xcfc447f1e53c8e1b, 0xb05ca3f564268d99, 0x9ae182c8bc9474e8,
    0xa4fc4bd4fc5558ca, 0xe755178d58fc4e76, 0x69b97db1a4c03dfe, 0xf9b5b7c4acc67c96,
    0xfc6a82d64b8655fb, 0x9c684cb6c4d24417, 0x8ec97d2917456ed0, 0x6703df9d2924e97e,
    0xc547f57e42a7444e, 0x78e37644e7cad29e, 0xfe9a44e9362f05fa, 0x08bd35cc38336615,
    0x9315e5eb3a129ace, 0x94061b871e04df75, 0xdf1d9f9d784ba010, 0x3bba57b68871b59d,
    0xd2b7adeeded1f73f, 0xf7a255d83bc373f8, 0xd7f4f2448c0ceb81, 0xd95be88cd210ffa7,
    0x336f52f8ff4728e7, 0xa74049dac312ac71, 0xa2f61bb6e437fdb5, 0x4f2a5cb07f6a35b3,
    0x87d380bda5bf7859, 0x16b9f7e06c453a21, 0x7ba2484c8a0fd54e, 0xf3a678cad9a2e38c,
    0x39b0bf7dde437ba2, 0xfcaf55c1bf8a4424, 0x18fcf680573fa594, 0x4c0563b89f495ac3,
    0x40e087931a00930d, 0x8cffa9412eb642c1, 0x68ca39053261169f, 0x7a1ee967d27579e2,
    0x9d1d60e5076f5b6f, 0x3810e399b6f65ba2, 0x32095b6d4ab5f9b1, 0x35cab62109dd038a,
    0xa90b24499fcfafb1, 0x77a225a07cc2c6bd, 0x513e5e634c70e331, 0x4361c0ca3f692f12,
    0xd941aca44b20a45b, 0x528f7c8602c5807b, 0x52ab92beb9613989, 0x9d1dfa2efc557f73,
    0x722ff175f572c348, 0x1d1260a51107fe97, 0x7a249a57ec0c9ba2, 0x04208fe9e8f7f2d6,
    0x5a110c6058b920a0, 0x0cd9a497658a5698, 0x56fd23c8f9715a4c, 0x284c847b9d887aae,
    0x04feabfbbdb619cb, 0x742e1e651c60ba83, 0x9a9632e65904ad3c, 0x881b82a13b51b9e2,
    0x506e6744cd974924, 0xb0183db56ffc6a79, 0x0ed9b915c66ed37e, 0x5e11e86d5873d484,
    0xf678647e3519ac6e, 0x1b85d488d0f20cc5, 0xdab9fe6525d89021, 0x0d151d86adb73615,
    0xa865a54edcc0f019, 0x93c42566aef98ffb, 0x99e7afeabe000731, 0x48cbff086ddf285a,
    0x7f9b6af1ebf78baf, 0x58627e1a149bba21, 0x2cd16e2abd791e33, 0xd363eff5f0977996,
    0x0ce2a38c344a6eed, 0x1a804aadb9cfa741, 0x907f30421d78c5de, 0x501f65edb3034d07,
    0x37624ae5a48fa6e9, 0x957baf61700cff4e, 0x3a6c27934e31188a, 0xd49503536abca345,
    0x088e049589c432e0, 0xf943aee7febf21b8, 0x6c3b8e3e336139d3, 0x364f6ffa464ee52e,
    0xd60f6dcedc314222, 0x56963b0dca418fc0, 0x16f50edf91e513af, 0xef1955914b609f93,
    0x565601c0364e3228, 0xecb53939887e8175, 0xbac7a9a18531294b, 0xb344c470397bba52,
    0x65d34954daf3cebd, 0xb4b81b3fa97511e2, 0xb422061193d6f6a7, 0x071582401c38434d,
    0x7a13f18bbedc4ff5, 0xbc4097b116c524d2, 0x59b97885e2f2ea28, 0x99170a5dc3115544,
    0x6f423357e7c6a9f9, 0x325928ee6e6f8794, 0xd0e4366228b03343, 0x565c31f7de89ea27,
    0x30f5611484119414, 0xd873db391292ed4f, 0x7bd94e1d8e17debc, 0xc7d9f16864a76e94,
    0x947ae053ee56e63c, 0xc8c93882f9475f5f, 0x3a9bf55ba91f81ca, 0xd9a11fbb3d9808e4,
    0x0fd22063edc29fca, 0xb3f256d8aca0b0b9, 0xb03031a8b4516e84, 0x35dd37d5871448af,
    0xe9f6082b05542e4e, 0xebfafa33d7254b59, 0x9255abb50d532280, 0xb9ab4ce57f2d34f3,
    0x693501d628297551, 0xc62c58f97dd949bf, 0xcd454f8f19c5126a, 0xbbe83f4ecc2bdecb,
    0xdc842b7e2819e230, 0xba89142e007503b8, 0xa3bc941d0a5061cb, 0xe9f6760e32cd8021,
    0x09c7e552bc76492f, 0x852f54934da55cc9, 0x8107fccf064fcf56, 0x098954d51fff6580,
    0x23b70edb1955c4bf, 0xc330de426430f69d, 0x4715ed43e8a45c0a, 0xa8d7e4dab780a08d,
    0x0572b974f03ce0bb, 0xb57d2e985e1419c7, 0xe8d9ecbe2cf3d73f, 0x2fe4b17170e59750,
    0x11317ba87905e790, 0x7fbf21ec8a1f45ec, 0x1725cabfcb045b00, 0x964e915cd5e2b207,
    0x3e2b8bcbf016d66d, 0xbe7444e39328a0ac, 0xf85b2b4fbcde44b7, 0x49353fea39ba63b1,
    0x1dd01aafcd53486a, 0x1fca8a92fd719f85, 0xfc7c95d827357afa, 0x18a6a990c8b35ebd,
    0xcccb7005c6b9c28d, 0x3bdbb92c43b17f26, 0xaa70b5b4f89695a2, 0xe94c39a54a98307f,
    0xb7a0b174cff6f36e, 0xd4dba84729af48ad, 0x2e18bc1ad9704a68, 0x2de0966daf2f8b1c,
    0xb9c11d5b1e43a07e, 0x64972d68dee33360, 0x94628d38d0c20584, 0xdbc0d2b6ab90a559,
    0xd2733c4335c6a72f, 0x7e75d99d94a70f4d, 0x6ced1983376fa72b, 0x97fcaacbf030bc24,
    0x7b77497b32503b12, 0x8547eddfb81ccb94, 0x79999cdff70902cb, 0xcffe1939438e9b24,
    0x829626e3892d95d7, 0x92fae24291f2b3f1, 0x63e22c147b9c3403, 0xc678b6d860284a1c,
    0x5873888850659ae7, 0x0981dcd296a8736d, 0x9f65789a6509a440, 0x9ff38fed72e9052f,
    0xe479ee5b9930578c, 0xe7f28ecd2d49eecd, 0x56c074a581ea17fe, 0x5544f7d774b14aef,
    0x7b3f0195fc6f290f, 0x12153635b2c0cf57, 0x7f5126dbba5e0ca7, 0x7a76956c3eafb413,
    0x3d5774a11d31ab39, 0x8a1b083821f40cb4, 0x7b4a38e32537df62, 0x950113646d1d6e03,
    0x4da8979a0041e8a9, 0x3bc36e078f7515d7, 0x5d0a12f27ad310d1, 0x7f9d1a2e1ebe1327,
    0xda3a361b1c5157b1, 0xdcdd7d20903d0c25, 0x36833336d068f707, 0xce68341f79893389,
    0xab9090168dd05f34, 0x43954b3252dc25e5, 0xb438c2b67f98e5e9, 0x10dcd78e3851a492,
    0xdbc27ab5447822bf, 0x9b3cdb65f82ca382, 0xb67b7896167b4c84, 0xbfced1b0048eac50,
    0xa9119b60369ffebd, 0x1fff7ac80904bf45, 0xac12fb171817eee7, 0xaf08da9177dda93d,
    0x1b0cab936e65c744, 0xb559eb1d04e5e932, 0xc37b45b3f8d6f2ba, 0xc3a9dc228caac9e9,
    0xf3b8b6675a6507ff, 0x9fc477de4ed681da, 0x67378d8eccef96cb, 0x6dd856d94d259236,
    0xa319ce15b0b4db31, 0x073973751f12dd5e, 0x8a8e849eb32781a5, 0xe1925c71285279f5,
    0x74c04bf1790c0efe, 0x4dda48153c94938a, 0x9d266d6a1cc0542c, 0x7440fb816508c4fe,
    0x13328503df48229f, 0xd6bf7baee43cac40, 0x4838d65f6ef6748f, 0x1e152328f3318dea,
    0x8f8419a348f296bf, 0x72c8834a5957b511, 0xd7a023a73260b45c, 0x94ebc8abcfb56dae,
    0x9fc10d0f989993e0, 0xde68a2355b93cae6, 0xa44cfe79ae538bbe, 0x9d1d84fcce371425,
    0x51d2b1ab2ddfb636, 0x2fd7e4b9e72cd38c, 0x65ca5b96b7552210, 0xdd69a0d8ab3b546d,
    0x604d51b25fbf70e2, 0x73aa8a564fb7ac9e, 0x1a8c1e992b941148, 0xaac40a2703d9bea0,
    0x764dbeae7fa4f3a6, 0x1e99b96e70a9be8b, 0x2c5e9deb57ef4743, 0x3a938fee32d29981,
    0x26e6db8ffdf5adfe, 0x469356c504ec9f9d, 0xc8763c5b08d1908c, 0x3f6c6af859d80055,
    0x7f7cc39420a3a545, 0x9bfb227ebdf4c5ce, 0x89039d79d6fc5c5c, 0x8fe88b57305e2ab6,
    0xa09e8c8c35ab96de, 0xfa7e393983325753, 0xd6b6d0ecc617c699, 0xdfea21ea9e7557e3,
    0xb67c1fa481680af8, 0xca1e3785a9e724e5, 0x1cfc8bed0d681639, 0xd18d8549d140caea,
    0x4ed0fe7e9dc91335, 0xe4dbf0634473f5d2, 0x1761f93a44d5aefe, 0x53898e4c3910da55,
    0x734de8181f6ec39a, 0x2680b122baa28d97, 0x298af231c85bafab, 0x7983eed3740847d5,
    0x66c1a2a1a60cd889, 0x9e17e49642a3e4c1, 0xedb454e7badc0805, 0x50b704cab602c329,
    0x4cc317fb9cddd023, 0x66b4835d9eafea22, 0x219b97e26ffc81bd, 0x261e4e4c0a333a9d,
    0x1fe2cca76517db90, 0xd7504dfa8816edbb, 0xb9571fa04dc089c8, 0x1ddc0325259b27de,
    0xcf3f4688801eb9aa, 0xf4f5d05c10cab243, 0x38b6525c21a42b0e, 0x36f60e2ba4fa6800,
    0xeb3593803173e0ce, 0x9c4cd6257c5a3603, 0xaf0c317d32adaa8a, 0x258e5a80c7204c4b,
    0x8b889d624d44885d, 0xf4d14597e660f855, 0xd4347f66ec8941c3, 0xe699ed85b0dfb40d,
    0x2472f6207c2d0484, 0xc2a1e7b5b459aeb5, 0xab4f6451cc1d45ec, 0x63767572ae3d6174,
    0xa59e0bd101731a28, 0x116d0016cb948f09, 0x2cf9c8ca052f6e9f, 0x0b090a7560a968e3,
    0xabeeddb2dde06ff1, 0x58efc10b06a2068d, 0xc6e57a78fbd986e0, 0x2eab8ca63ce802d7,
    0x14a195640116f336, 0x7c0828dd624ec390, 0xd74bbe77e6116ac7, 0x804456af10f5fb53,
    0xebe9ea2adf4321c7, 0x03219a39ee587a30, 0x49787fef17af9924, 0xa1e9300cd8520548,
    0x5b45e522e4b1b4ef, 0xb49c3b3995091a36, 0xd4490ad526f14431, 0x12a8f216af9418c2,
    0x001f837cc7350524, 0x1877b51e57a764d5, 0xa2853b80f17f58ee, 0x993e1de72d36d310,
    0xb3598080ce64a656, 0x252f59cf0d9f04bb, 0xd23c8e176d113600, 0x1bda0492e7e4586e,
    0x21e0bd5026c619bf, 0x3b097adaf088f94e, 0x8d14dedb30be846e, 0xf95cffa23af5f6f4,
    0x3871700761b3f743, 0xca672b91e9e4fa16, 0x64c8e531bff53b55, 0x241260ed4ad1e87d,
    0x106c09b972d2e822, 0x7fba195410e5ca30, 0x7884d9bc6cb569d8, 0x0647dfedcd894a29,
    0x63573ff03e224774, 0x4fc8e9560f91b123, 0x1db956e450275779, 0xb8d91274b9e9d4fb,
    0xa2ebee47e2fbfce1, 0xd9f1f30ccd97fb09, 0xefed53d75fd64e6b, 0x2e6d02c36017f67f,
    0xa9aa4d20db084e9b, 0xb64be8d8b25396c1, 0x70cb6af7c2d5bcf0, 0x98f076a4f7a2322e,
    0xbf84470805e69b5f, 0x94c3251f06f90cf3, 0x3e003e616a6591e9, 0xb925a6cd0421aff3,
    0x61bdd1307c66e300, 0xbf8d5108e27e0d48, 0x240ab57a8b888b20, 0xfc87614baf287e07,
    0xef02cdd06ffdb432, 0xa1082c0466df6c0a, 0x8215e577001332c8, 0xd39bb9c3a48db6cf,
    0x2738259634305c14, 0x61cf4f94c97df93d, 0x1b6baca2ae4e125b, 0x758f450c88572e0b,
    0x959f587d507a8359, 0xb063e962e045f54d, 0x60e8ed72c0dff5d1, 0x7b64978555326f9f,
    0xfd080d236da814ba, 0x8c90fd9b083f4558, 0x106f72fe81e2c590, 0x7976033a39f7d952,
    0xa4ec0132764ca04b, 0x733ea705fae4fa77, 0xb4d8f77bc3e56167, 0x9e21f4f903b33fd9,
    0x9d765e419fb69f6d, 0xd30c088ba61ea5ef, 0x5d94337fbfaf7f5b, 0x1a4e4822eb4d7a59,
    0x6ffe73e81b637fb3, 0xddf957bc36d8b9ca, 0x64d0e29eea8838b3, 0x08dd9bdfd96b9f63,
    0x087e79e5a57d1d13, 0xe328e230e3e2b3fb, 0x1c2559e30f0946be, 0x720bf5f26f4d2eaa,
    0xb0774d261cc609db, 0x443f64ec5a371195, 0x4112cf68649a260e, 0xd813f2fab7f5c5ca,
    0x660d3257380841ee, 0x59ac2c7873f910a3, 0xe846963877671a17, 0x93b633abfa3469f8,
    0xc0c0f5a60ef4cdcf, 0xcaf21ecd4377b28c, 0x57277707199b8175, 0x506c11b9d90e8b1d,
    0xd83cc2687a19255f, 0x4a29c6465a314cd1, 0xed2df21216235097, 0xb5635c95ff7296e2,
    0x22af003ab672e811, 0x52e762596bf68235, 0x9aeba33ac6ecc6b0, 0x944f6de09134dfb6,
    0x6c47bec883a7de39, 0x6ad047c430a12104, 0xa5b1cfdba0ab4067, 0x7c45d833aff07862,
    0x5092ef950a16da0b, 0x9338e69c052b8e7b, 0x455a4b4cfe30e3f5, 0x6b02e63195ad0cf8,
    0x6b17b224bad6bf27, 0xd1e0ccd25bb9c169, 0xde0c89a556b9ae70, 0x50065e535a213cf6,
    0x9c1169fa2777b874, 0x78edefd694af1eed, 0x6dc93d9526a50e68, 0xee97f453f06791ed,
    0x32ab0edb696703d3, 0x3a6853c7e70757a7, 0x31865ced6120f37d, 0x67fef95d92607890,
    0x1f2b1d1f15f6dc9c, 0xb69e38a8965c6b65, 0xaa9119ff184cccf4, 0xf43c732873f24c13,
    0xfb4a3d794a9a80d2, 0x3550c2321fd6109c, 0x371f77e76bb8417e, 0x6bfa9aae5ec05779,
    0xcd04f3ff001a4778, 0xe3273522064480ca, 0x9f91508bffcfc14a, 0x049a7f41061a9e60,
    0xfcb6be43a9f2fe9b, 0x08de8a1c7797da9b, 0x8f9887e6078735a1, 0xb5b4071dbfc73a66,
    0x230e343dfba08d33, 0x43ed7f5a0fae657d, 0x3a88a0fbbcb05c63, 0x21874b8b4d2dbc4f,
    0x1bdea12e35f6a8c9, 0x53c065c6c8e63528, 0xe34a1d250e7a8d6b, 0xd6b04d3b7651dd7e,
    0x5e90277e7cb39e2d, 0x2c046f22062dc67d, 0xb10bb459132d0a26, 0x3fa9ddfb67e2f199,
    0x0e09b88e1914f7af, 0x10e8b35af3eeab37, 0x9eedeca8e272b933, 0xd4c718bc4ae8ae5f,
    0x81536d601170fc20, 0x91b534f885818a06, 0xec8177f83f900978, 0x190e714fada5156e,
    0xb592bf39b0364963, 0x89c350c893ae7dc1, 0xac042e70f8b383f2, 0xb49b52e587a1ee60,
    0xfb152fe3ff26da89, 0x3e666e6f69ae2c15, 0x3b544ebe544c19f9, 0xe805a1e290cf2456,
    0x24b33c9d7ed25117, 0xe74733427b72f0c1, 0x0a804d18b7097475, 0x57e3306d881edb4f,
    0x4ae7d6a36eb5dbcb, 0x2d8d5432157064c8, 0xd1e649de1e7f268b, 0x8a328a1cedfe552c,
    0x07a3aec79624c7da, 0x84547ddc3e203c94, 0x990a98fd5071d263, 0x1a4ff12616eefc89,
    0xf6f7fd1431714200, 0x30c05b1ba332f41c, 0x8d2636b81555a786, 0x46c9feb55d120902,
    0xccec0a73b49c9921, 0x4e9d2827355fc492, 0x19ebb029435dcb0f, 0x4659d2b743848a2c,
    0x963ef2c96b33be31, 0x74f85198b05a2e7d, 0x5a0f544dd2b1fb18, 0x03727073c2e134b1,
    0xc7f6aa2de59aea61, 0x352787baa0d7c22f, 0x9853eab63b5e0b35, 0xabbdcdd7ed5c0860,
    0xcf05daf5ac8d77b0, 0x49cad48cebf4a71e, 0x7a4c10ec2158c4a6, 0xd9e92aa246bf719e,
    0x13ae978d09fe5557, 0x730499af921549ff, 0x4e4b705b92903ba4, 0xff577222c14f0a3a,
    0x55b6344cf97aafae, 0xb862225b055b6960, 0xcac09afbddd2cdb4, 0xdaf8e9829fe96b5f,
    0xb5fdfc5d3132c498, 0x310cb380db6f7503, 0xe87fbb46217a360e, 0x2102ae466ebb1148,
    0xf8549e1a3aa5e00d, 0x07a69afdcc42261a, 0xc4c118bfe78feaae, 0xf9f4892ed96bd438,
    0x1af3dbe25d8f45da, 0xf5b4b0b0d2deeeb4, 0x962aceefa82e1c84, 0x046e3ecaaf453ce9,
    0xf05d129681949a4c, 0x964781ce734b3c84, 0x9c2ed44081ce5fbd, 0x522e23f3925e319e,
    0x177e00f9fc32f791, 0x2bc60a63a6f3b3f2, 0x222bbfae61725606, 0x486289ddcc3d6780,
    0x7dc7785b8efdfc80, 0x8af38731c02ba980, 0x1fab64ea29a2ddf7, 0xe4d9429322cd065a,
    0x9da058c67844f20c, 0x24c0e332b70019b0, 0x233003b5a6cfe6ad, 0xd586bd01c5c217f6,
    0x5e5637885f29bc2b, 0x7eba726d8c94094b, 0x0a56a5f0bfe39272, 0xd79476a84ee20d06,
    0x9e4c1269baa4bf37, 0x17efee45b0dee640, 0x1d95b0a5fcf90bc6, 0x93cbe0b699c2585d,
    0x65fa4f227a2b6d79, 0xd5f9e858292504d5, 0xc2b5a03f71471a6f, 0x59300222b4561e00,
    0xce2f8642ca0712dc, 0x7ca9723fbb2e8988, 0x2785338347f2ba08, 0xc61bb3a141e50e8c,
    0x150f361dab9dec26, 0x9f6a419d382595f4, 0x64a53dc924fe7ac9, 0x142de49fff7a7c3d,
    0x0c335248857fa9e7, 0x0a9c32d5eae45305, 0xe6c42178c4bbb92e, 0x71f1ce2490d20b07,
    0xf1bcc3d275afe51a, 0xe728e8c83c334074, 0x96fbf83a12884624, 0x81a1549fd6573da5,
    0x5fa7867caf35e149, 0x56986e2ef3ed091b, 0x917f1dd5f8886c61, 0xd20d8c88c8ffe65f,
    0x31d71dce64b2c310, 0xf165b587df898190, 0xa57e6339dd2cf3a0, 0x1ef6e6dbb1961ec9,
    0x70cc73d90bc26e24, 0xe21a6b35df0c3ad7, 0x003a93d8b2806962, 0x1c99ded33cb890a1,
    0xcf3145de0add4289, 0xd0e4427a5514fb72, 0x77c621cc9fb3a483, 0x67a34dac4356550b,
    0xf8d626aaaf278509,
];

struct Zobrist {
    keys: [u64; 781],
}

impl Zobrist {
    fn new() -> Self {
        Zobrist { keys: POLYGLOT_KEYS }
    }

    fn hash(&self, board: &Board) -> u64 {

        let mut hash = 0u64;

        for sq in 0..64u8 {

            let piece = board.board[sq as usize];

            if piece != EMPTY {

                hash ^= self.keys[zobrist_key(piece, sq)];
            }
        }

        if board.side_to_move == Color::White {

            hash ^= self.keys[780];
        }

        const CASTLING_BITS: [(u8, usize); 4] = [
            (CASTLING_WK, 768),
            (CASTLING_WQ, 769),
            (CASTLING_BK, 770),
            (CASTLING_BQ, 771),
        ];

        for &(bit, idx) in &CASTLING_BITS {

            if board.castling_rights & bit != 0 {

                hash ^= self.keys[idx];
            }
        }

        // Conditional EP: only hashed when a pawn of the side to move can capture.
        if board.en_passant_square != -1 && ep_capture_possible(board) {

            let file = (board.en_passant_square as usize) & 7;

            hash ^= self.keys[772 + file];
        }

        hash
    }
}

static ZOBRIST: LazyLock<Zobrist> = LazyLock::new(Zobrist::new);

// -----------------------------------------------------------------------------
// Move packing for TT storage (16 bits: from:6, to:6, promo:4)
// -----------------------------------------------------------------------------

fn pack_move_data(mv: &Move) -> u16 {

    let promo_code = match mv.promotion {
        0 => 0u8,
        W_QUEEN | B_QUEEN => 1,
        W_ROOK | B_ROOK => 2,
        W_BISHOP | B_BISHOP => 3,
        W_KNIGHT | B_KNIGHT => 4,
        _ => 0,
    };

    (mv.from as u16) | ((mv.to as u16) << 6) | ((promo_code as u16) << 12)
}

fn unpack_move_data(packed: u16) -> (u8, u8, u8) {

    let from = (packed & 0x3F) as u8;

    let to = ((packed >> 6) & 0x3F) as u8;

    let promo_code = ((packed >> 12) & 0xF) as u8;

    (from, to, promo_code)
}

fn decode_promotion(code: u8, side: Color) -> i8 {

    match code {
        1 => {
            if side == Color::White {

                W_QUEEN
            } else {

                B_QUEEN
            }
        },
        2 => {
            if side == Color::White {

                W_ROOK
            } else {

                B_ROOK
            }
        },
        3 => {
            if side == Color::White {

                W_BISHOP
            } else {

                B_BISHOP
            }
        },
        4 => {
            if side == Color::White {

                W_KNIGHT
            } else {

                B_KNIGHT
            }
        },
        _ => 0,
    }
}

// -----------------------------------------------------------------------------
// Transposition table (lock‑free, 8 MB ≈ 2^19 entries, depth‑preferred eviction)
// -----------------------------------------------------------------------------

const TT_BITS: usize = 21; // 2^21 = 2 097 152 entries × 16 bytes ≈ 32 MB

const TT_MASK: usize = (1 << TT_BITS) - 1;

/// Single TT entry: key(64) ‖ data(64) packed into one atomic 128-bit slot.
///   key:   full 64‑bit Zobrist hash
///   data:  score(32) | best_move_packed(16) | depth(8) | flags(8)
///   flags: 0=EXACT, 1=LOWER (beta cutoff), 2=UPPER (fail low)
///
/// One atomic load = no torn reads, no seqlock, no race possible.
#[repr(C, align(16))]
struct TTEntry {
    packed: AtomicU128,
}

// flags
const TT_EXACT: u8 = 0;

const TT_LOWER: u8 = 1;

const TT_UPPER: u8 = 2;

fn pack_tt_data(score: i32, best_move_packed: u16, depth: u8, flags: u8) -> u64 {

    (score as u64) & 0xFFFF_FFFF | ((best_move_packed as u64) << 32) | ((depth as u64) << 48) | ((flags as u64) << 56)
}

fn unpack_tt_data(data: u64) -> (i32, u16, u8, u8) {

    let score = (data & 0xFFFF_FFFF) as u32 as i32;

    let best_move_packed = ((data >> 32) & 0xFFFF) as u16;

    let depth = ((data >> 48) & 0xFF) as u8;

    let flags = ((data >> 56) & 0xFF) as u8;

    (score, best_move_packed, depth, flags)
}

fn pack_entry(key: u64, data: u64) -> u128 {
    (key as u128) << 64 | data as u128
}

fn unpack_entry(packed: u128) -> (u64, u64) {
    ((packed >> 64) as u64, packed as u64)
}

struct TranspositionTable {
    entries: Box<[TTEntry]>,
}

impl TranspositionTable {
    fn new() -> Self {

        let count = 1 << TT_BITS;

        let mut vec = Vec::with_capacity(count);

        vec.resize_with(count, || TTEntry {
            packed: AtomicU128::new(0),
        });

        TranspositionTable {
            entries: vec.into_boxed_slice(),
        }
    }

    fn probe(&self, zobrist: u64) -> Option<(i32, u16, u8, u8)> {

        let entry = &self.entries[(zobrist as usize) & TT_MASK];

        // Single atomic 128-bit load: key + data arrive together, no race.
        let packed = entry.packed.load(Ordering::Acquire);

        let (key, data) = unpack_entry(packed);

        if key != zobrist || data == 0 {

            return None;
        }

        Some(unpack_tt_data(data))
    }

    fn store(&self, zobrist: u64, score: i32, best_move_packed: u16, depth: u8, flags: u8) {

        let entry = &self.entries[(zobrist as usize) & TT_MASK];

        let new_data = pack_tt_data(score, best_move_packed, depth, flags);

        let new_packed = pack_entry(zobrist, new_data);

        // CAS loop for depth‑preferred replacement.
        // Only one writer wins; losers retry with fresh data.
        loop {
            let old_packed = entry.packed.load(Ordering::Relaxed);

            let (_old_key, old_data) = unpack_entry(old_packed);

            // Empty slot or deeper search → overwrite
            if old_data != 0 && unpack_tt_data(old_data).2 > depth {

                return;
            }

            if entry
                .packed
                .compare_exchange_weak(
                    old_packed,
                    new_packed,
                    Ordering::Release,
                    Ordering::Relaxed,
                )
                .is_ok()
            {

                return;
            }
            // CAS failed → someone else wrote the slot, retry
        }
    }
}

// Global lock‑free TT shared across all Lazy SMP threads.
// The seqlock in probe/store handles concurrent access:
// helpers refine the same cache the leader reads, so move ordering
// improves across all threads.  Single allocation at startup, zero
// heap during search.
static TT: LazyLock<TranspositionTable> = LazyLock::new(TranspositionTable::new);

// -----------------------------------------------------------------------------
// Opening book (PolyGlot format, embedded in the binary)
// -----------------------------------------------------------------------------

/// A single PolyGlot book entry (16 bytes → key+move+weight+learn).
struct BookEntry {
    packed_move: u16,
    weight: u16,
}

/// Lightweight opening book: HashMap from Zobrist key to move list.
/// ~188KB decompressed, ~12K entries — trivial memory.
struct OpeningBook {
    map: HashMap<u64, Vec<BookEntry>>,
}

impl OpeningBook {
    fn from_bytes(data: &[u8]) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let mut map: HashMap<u64, Vec<BookEntry>> =
            HashMap::with_capacity(data.len() / 16);

        for chunk in data.chunks_exact(16) {
            // PolyGlot stores keys big-endian (and the file is sorted by key)
            let key = u64::from_be_bytes(chunk[0..8].try_into()?);
            let packed_move = u16::from_le_bytes(chunk[8..10].try_into()?);
            let weight = u16::from_le_bytes(chunk[10..12].try_into()?);
            // bytes 12–15: learn (u32, ignored)

            map.entry(key).or_default().push(BookEntry { packed_move, weight });
        }

        Ok(OpeningBook { map })
    }

    /// Probe the book. Returns the packed move with the highest weight,
    /// plus the total weight sum for that position (for weighted selection).
    fn probe(&self, zobrist: u64) -> Option<(u16, u16)> {
        let entries = self.map.get(&zobrist)?;
        let best = entries.iter().max_by_key(|e| e.weight)?;
        Some((best.packed_move, best.weight))
    }
}

/// The PolyGlot book is embedded in the binary (193KB) so the engine works
/// without any external file. Path is relative to this source file.
const BOOK_BYTES: &[u8] = include_bytes!("filtered.bin");

/// Load the opening book once at first access.
/// Uses the embedded copy by default; `OPENING_BOOK` env var can override
/// with an external path (falls back to embedded on error).
static BOOK: LazyLock<Option<OpeningBook>> = LazyLock::new(|| {
    let source = std::env::var("OPENING_BOOK")
        .map(|path| {
            let label = path.clone();
            match std::fs::read(&path) {
                Ok(data) => (data, label),
                Err(e) => {
                    crate::log_warn(crate::LogMsg::DbPing {
                        ok: false,
                        detail: format!("opening book override {label} unreadable ({e}); using embedded"),
                    });
                    (BOOK_BYTES.to_vec(), "embedded".into())
                },
            }
        })
        .unwrap_or_else(|_| (BOOK_BYTES.to_vec(), "embedded".into()));

    let (data, path) = source;
    match OpeningBook::from_bytes(&data) {
        Ok(book) => {
            crate::log_info(crate::LogMsg::ChessSearch {
                tt_entries: book.map.len(),
                depth: 0,
                fen: format!("opening book loaded ({path})"),
            });
            Some(book)
        },
        Err(e) => {
            // Non-fatal: engine works fine without a book.
            crate::log_warn(crate::LogMsg::DbPing {
                ok: false,
                detail: format!("opening book: {e}"),
            });
            None
        },
    }
});

// -----------------------------------------------------------------------------
// Pinned-piece detection (pre-filter illegal moves before make/unmake)
// -----------------------------------------------------------------------------

#[derive(Clone, Copy)]

struct Pin {
    square: u8,
    direction: i8, // step from king to pinned piece (or vice‑versa)
}

/// Maximum possible pins: at most one per direction from the king (8).
const MAX_PINS: usize = 8;

struct PinnedInfo {
    pins: [Pin; MAX_PINS],
    count: usize,
}

fn compute_pins(board: &Board) -> PinnedInfo {

    let mut info = PinnedInfo {
        pins: [Pin {
            square: 0,
            direction: 0,
        }; MAX_PINS],
        count: 0,
    };

    let king_sq = if board.side_to_move == Color::White {

        board.wk_sq
    } else {

        board.bk_sq
    };

    let friendly_sign = board.side_to_move.sign();

    for &dir in &[-9i8, -8, -7, -1, 1, 7, 8, 9] {

        let mut sq = king_sq as i8 + dir;

        let mut found_piece = None;

        while (0..64).contains(&sq) {

            let piece = board.board[sq as usize];

            if piece != EMPTY {

                let is_friendly = (piece > 0) == (friendly_sign == 1);

                if is_friendly {

                    if found_piece.is_none() {

                        found_piece = Some(sq as u8);
                    } else {

                        break; // two friendlies on same ray = no pin
                    }
                } else {
                    // Enemy piece — does it attack along this direction?
                    let piece_type = piece.abs();

                    let attacks_along = match dir {
                        -9 | -7 | 7 | 9 => piece_type == 3 || piece_type == 5, // bishop or queen
                        -8 | 1 | -1 | 8 => piece_type == 4 || piece_type == 5, // rook or queen
                        _ => false,
                    };

                    if attacks_along && let Some(pin_sq) = found_piece {
                        info.pins[info.count] = Pin {
                            square: pin_sq,
                            direction: dir,
                        };
                        info.count += 1;
                    }

                    break;
                }
            }

            sq += dir;
        }
    }

    info
}

/// Returns Some(pin_direction) if the piece at `square` is pinned.
fn pinned_dir(square: u8, info: &PinnedInfo) -> Option<i8> {
    for i in 0..info.count {
        if info.pins[i].square == square {
            return Some(info.pins[i].direction);
        }
    }
    None
}

/// A pinned piece can only move along (or against) its pin direction.
fn move_stays_on_pin(mv: &Move, pin_dir: i8) -> bool {
    let file_delta = (mv.to as i8 % 8) - (mv.from as i8 % 8);
    let rank_delta = (mv.to as i8 / 8) - (mv.from as i8 / 8);
    if file_delta == 0 && rank_delta == 0 { return true; }
    let step = match (file_delta.signum(), rank_delta.signum()) {
        (1, 1) => 9,
        (1, -1) => -7,
        (-1, 1) => 7,
        (-1, -1) => -9,
        (1, 0) => 1,
        (-1, 0) => -1,
        (0, 1) => 8,
        (0, -1) => -8,
        _ => unreachable!(),
    };
    step == pin_dir || step == -pin_dir
}

// -----------------------------------------------------------------------------
// Search (Negamax with alpha-beta + TT + killers + LMR)
// -----------------------------------------------------------------------------

const MAX_PLY: usize = 64;

const MAX_SEARCH_PLY: usize = 128; // Hard cap against stack overflow

fn search(board: &mut Board, depth: usize, mut alpha: i32, beta: i32, killers: &mut [[Move; 2]], ply: usize) -> (i32, Move) {

    validate_piece_lists!(board, "search(depth={},ply={})", depth, ply);

    // Safety guard: hard cap on recursion depth
    if ply >= MAX_SEARCH_PLY {

        return (evaluate_board(board), Move { from: 0, to: 0, promotion: 0 });
    }

    let mut best_move = Move {
        from: 0,
        to: 0,
        promotion: 0,
    };

    let orig_alpha = alpha;

    if depth == 0 {

        let score = quiesce(board, alpha, beta, 0);

        return (score, best_move);
    }

    // ── TT probe (uses incremental hash — O(1) instead of O(64)) ──
    let zobrist_key = board.hash;

    let tt_result = TT.probe(zobrist_key);

    if let Some((tt_score, tt_move_packed, tt_depth, tt_flags)) = tt_result {

        // Unpack TT best move (used on early returns instead of local zero Move)
        let tt_best = if tt_move_packed != 0 {

            let (tt_from, tt_to, promo_code) = unpack_move_data(tt_move_packed);

            let tt_promo = decode_promotion(promo_code, board.side_to_move);

            Move {
                from: tt_from,
                to: tt_to,
                promotion: tt_promo,
            }
        } else {

            best_move
        };

        if tt_depth as usize >= depth {

            match tt_flags {
                // Exact score → return immediately
                TT_EXACT => {

                    return (tt_score, tt_best);
                },
                // Lower bound (beta cutoff) → fail‑high if ≥ beta
                TT_LOWER => {
                    if tt_score >= beta {

                        return (tt_score, tt_best);
                    }
                },
                // Upper bound (no improvement) → fail‑low if ≤ alpha
                TT_UPPER if tt_score <= alpha => {

                    return (tt_score, tt_best);
                },
                _ => {},
            }
        }

        // Adjust alpha upward using stored lower bound
        if tt_flags == TT_LOWER && tt_score > alpha {

            alpha = tt_score;
        }
    }

    let mut pseudo_moves = generate_pseudo_legal_moves(board);

    if pseudo_moves.count == 0 {

        // Checkmate or stalemate
        if in_check(board) {

            // Checkmate (negative value, the deeper the worse)
            return (-20000 + board.fullmove_number as i32, best_move);
        } else {

            return (0, best_move);
        }
    }

    // ── TT best‑move ordering: promote to position 0 ──
    if let Some((_, tt_move_packed, _, _)) = tt_result && tt_move_packed != 0 {

        let (tt_from, tt_to, promo_code) = unpack_move_data(tt_move_packed);

        let tt_promo = decode_promotion(promo_code, board.side_to_move);

    for (i, mv) in pseudo_moves.moves[..pseudo_moves.count].iter().enumerate() {

            if mv.from == tt_from && mv.to == tt_to && mv.promotion == tt_promo {

                pseudo_moves.moves.swap(0, i);

                break;
            }
        }
    }

    // MVV-LVA move ordering: score captures by (victim_value - attacker_value)
    let mut move_scores = [0i16; MAX_MOVES];

    let primary_killer = killers[ply][0];

    let secondary_killer = killers[ply][1];

    for (i, &mv) in pseudo_moves.moves[..pseudo_moves.count].iter().enumerate() {

        if board.board[mv.to as usize] != EMPTY {

            let victim = piece_value(board.board[mv.to as usize]) as i16;

            let attacker = piece_value(board.board[mv.from as usize]) as i16;

            move_scores[i] = victim - attacker;
        } else if mv.promotion != 0 {

            // Promotions score between minor (330) and major (500+)
            move_scores[i] = piece_value(mv.promotion) as i16 / 10;
        } else if mv.from == primary_killer.from && mv.to == primary_killer.to && mv.promotion == primary_killer.promotion
            || mv.from == secondary_killer.from && mv.to == secondary_killer.to && mv.promotion == secondary_killer.promotion
        {

            // Killer moves: quiet moves that caused cutoffs at this ply
            move_scores[i] = 50;
        }
    }

    // Insertion sort by score descending (fast on nearly-sorted arrays)
    for i in 1..pseudo_moves.count {

        let mut j = i;

        while j > 0 && move_scores[j] > move_scores[j - 1] {

            move_scores.swap(j, j - 1);

            pseudo_moves.moves.swap(j, j - 1);

            j -= 1;
        }
    }

    // ── Null move pruning (skip a turn; if score still ≥ beta, prune) ──
    if depth >= 3 && !in_check(board) {

        let saved_ep = board.en_passant_square;

        let saved_hash = board.hash;

        // Sync hash for the null-move state (side toggle + ep clear)
        board.hash ^= ZOBRIST.keys[780];

        if saved_ep != -1 && ep_capture_possible(board) {

            board.hash ^= ZOBRIST.keys[772 + (saved_ep as usize & 7)];
        }

        board.en_passant_square = -1;

        board.side_to_move = board.side_to_move.opposite();

        let (null_score, _) = search(board, depth - 1 - 2, -beta, -beta + 1, killers, ply + 1);

        board.side_to_move = board.side_to_move.opposite();

        board.en_passant_square = saved_ep;

        board.hash = saved_hash;

        if null_score >= beta {

            return (beta, best_move);
        }
    }

    let in_check_pos = in_check(board);

    // ── Precompute pinned pieces (avoid make/unmake for illegal moves) ──
    let pin_info = if in_check_pos {
        // Pins don't matter when in check — all moves must be checked normally
        PinnedInfo {
            pins: [Pin { square: 0, direction: 0 }; MAX_PINS],
            count: 0,
        }
    } else {
        compute_pins(board)
    };

    for (i, &mv) in pseudo_moves.moves[..pseudo_moves.count].iter().enumerate() {

        let is_quiet = board.board[mv.to as usize] == EMPTY && mv.promotion == 0;

        // Quick filter: skip moves that would expose the king (via pin)
        if !in_check_pos && let Some(pin_dir) = pinned_dir(mv.from, &pin_info)
            && !move_stays_on_pin(&mv, pin_dir)
        {

            continue;
        }

        let undo = make_move(board, mv);

        // Skip illegal moves (that leave our king in check)
        if in_check(board) {

            unmake_move(board, mv, undo);

            continue;
        }

        // ── Late Move Reduction (LMR): search late quiet moves at reduced depth ──
        let search_depth = if depth >= 3 && i >= 4 && is_quiet && !in_check_pos {

            depth - 1 - if i >= 8 { 2 } else { 1 }
        } else {

            depth - 1
        };

        let (child_score, _) = search(board, search_depth, -beta, -alpha, killers, ply + 1);

        let mut score = -child_score;

        // LMR re-search: if the reduced search beat alpha, try full depth
        if search_depth != depth - 1 && score > alpha {

            let (child_score, _) = search(board, depth - 1, -beta, -alpha, killers, ply + 1);

            score = -child_score;
        }

        unmake_move(board, mv, undo);

        if score > alpha {

            alpha = score;

            best_move = mv;

            if alpha >= beta {

                // Store killer: quiet move that caused a beta cutoff
                if is_quiet && mv.from != mv.to {

                    killers[ply][1] = killers[ply][0];

                    killers[ply][0] = mv;
                }

                break;
            }
        }
    }

    // ── TT store ──
    let flags = if alpha <= orig_alpha {

        TT_UPPER
    } else if alpha >= beta {

        TT_LOWER
    } else {

        TT_EXACT
    };

    if best_move.from != best_move.to || best_move.promotion != 0 {

        TT.store(zobrist_key, alpha, pack_move_data(&best_move), depth as u8, flags);
    }

    (alpha, best_move)
}

// -----------------------------------------------------------------------------
// Public API
// -----------------------------------------------------------------------------

#[allow(dead_code)]
fn best_move_single(fen: &str, depth: usize) -> Option<String> {
    let board = parse_fen(fen).ok()?;
    let mut local_board = board;
    let mut killers = [[Move { from: 0, to: 0, promotion: 0 }; 2]; MAX_PLY];
    let mut best = Move { from: 0, to: 0, promotion: 0 };

    for current_depth in 1..=depth {
        let (_score, mv) = search(&mut local_board, current_depth, -30000, 30000, &mut killers, 0);
        let is_valid = mv.from != mv.to || mv.promotion != 0;
        if is_valid {
            best = mv;
        }
    }

    if best.from == best.to {
        return None;
    }
    Some(format!("{}{}", square_name(best.from), square_name(best.to)))
}

pub fn best_move(fen: &str, depth: usize) -> Option<String> {

    let board = match parse_fen(fen) {
        Ok(board) => board,
        Err(_) => return None,
    };

    // ── Opening book probe: return book move instantly if position is known ──
    if let Some(book) = BOOK.as_ref() {
        if let Some((packed_move, _weight)) = book.probe(board.hash) {
            let (from, to, promo_code) = unpack_move_data(packed_move);
            if from < 64 && to < 64 {
                let promo = decode_promotion(promo_code, board.side_to_move);
                // Verify the book move is legal for this position
                let legal_moves = generate_pseudo_legal_moves(&board);
                for i in 0..legal_moves.count {
                    let mv = legal_moves.moves[i];
                    if mv.from == from && mv.to == to && mv.promotion == promo {
                        let result = format!("{}{}", square_name(from), square_name(to));
                        crate::log_info(crate::LogMsg::ChessResult {
                            best_move: result.clone(),
                        });
                        return Some(result);
                    }
                }
            }
        }
    }

    crate::log_info(crate::LogMsg::ChessSearch {
        tt_entries: 1 << TT_BITS,
        depth,
        fen: fen.to_owned(),
    });

    // ── Lazy SMP: N search tasks on the rayon pool, one TT ──
    // All tasks run iterative deepening independently.
    // They share the lock‑free TT, so each task's results
    // improve move ordering for all others.
    // Using rayon::scope (not std::thread::scope) reuses the existing
    // CHESS_POOL threads — zero per‑request thread overhead,
    // and concurrent requests share the pool without oversubscription.
    let num_threads = std::thread::available_parallelism()
        .map_or(2, |n| n.get().min(4));

    let leader_finished = AtomicBool::new(false);
    let best_move_cell = std::sync::Mutex::new(None::<Move>);

    rayon::scope(|s| {
        // ── Thread 0 (leader): full ID search ──
        s.spawn(|_| {
            let mut local_board = board.clone();
            let mut killers = [[Move { from: 0, to: 0, promotion: 0 }; 2]; MAX_PLY];
            let mut best = Move { from: 0, to: 0, promotion: 0 };

            for current_depth in 1..=depth {
                // Validate before search
                validate_piece_lists!(&local_board, "leader before depth {}", current_depth);
                let (score, mv) = search(&mut local_board, current_depth, -30000, 30000, &mut killers, 0);
                // Validate after search
                validate_piece_lists!(&local_board, "leader after depth {}", current_depth);
                let is_valid = mv.from != mv.to || mv.promotion != 0;

                crate::log_debug(crate::LogMsg::ChessDepth {
                    depth: current_depth,
                    score,
                    best: if is_valid { square_name(mv.from) + &square_name(mv.to) } else { "none".into() },
                    is_valid,
                });

                if is_valid {
                    best = mv;
                }
            }

            *best_move_cell.lock().unwrap() = Some(best);
            leader_finished.store(true, Ordering::Release);
        });

        // ── Helper tasks: same search, sharing only the TT ──
        for _ in 1..num_threads {
            s.spawn(|_| {
                let mut local_board = board.clone();
                let mut killers = [[Move { from: 0, to: 0, promotion: 0 }; 2]; MAX_PLY];

                for current_depth in 1..=depth {
                    if leader_finished.load(Ordering::Acquire) {
                        break;
                    }
                    validate_piece_lists!(&local_board, "helper before depth {}", current_depth);
                    search(&mut local_board, current_depth, -30000, 30000, &mut killers, 0);
                    validate_piece_lists!(&local_board, "helper after depth {}", current_depth);
                }
            });
        }
    });

    let best_move = match best_move_cell.into_inner() {
        Ok(Some(mv)) => mv,
        _ => return None,
    };

    // No legal moves (checkmate or stalemate)
    if best_move.from == best_move.to {

        crate::log_warn(crate::LogMsg::ChessNoMove);

        return None;
    }

    let result = format!("{}{}", square_name(best_move.from), square_name(best_move.to));

    crate::log_info(crate::LogMsg::ChessResult { best_move: result.clone() });

    Some(result)
}

#[cfg(test)]
#[path = "../../tests/games/chess/main.rs"]
mod tests;
