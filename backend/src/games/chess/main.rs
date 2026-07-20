// A minimal chess engine inspired by Sunfish.
// No heap allocations inside search/move generation.
// Uses enums for clarity and constants for demystification.

use std::sync::{
    LazyLock,
    atomic::{AtomicBool, AtomicU64, Ordering},
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
                debug_assert!(false, "MISMATCH {} list[{}][{}] sq={} expected={} actual={}", label, idx, i, sq, expected_piece, actual);
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

    // Board placement
    // Each row from the FEN represents one rank, starting with rank 8 (top)
    let rows: Vec<&str> = parts[0].split('/').collect();

    for (rank_index, row) in rows.iter().enumerate() {

        let rank = rank_index; // 0 = rank 8, 7 = rank 1
        let mut file = 0;

        for character in row.chars() {

            if character.is_ascii_digit() {

                let count = character.to_digit(10).expect("validated digit") as usize;

                for _ in 0..count {

                    board.board[rank * 8 + file] = EMPTY;

                    file += 1;
                }
            } else {

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
                    _ => EMPTY,
                };

                let sq = (rank * 8 + file) as u8;

                board.board[sq as usize] = piece;

                add_piece_to_list(&mut board, sq, piece);

                match piece {
                    W_KING => board.wk_sq = sq,
                    B_KING => board.bk_sq = sq,
                    _ => {},
                }

                file += 1;
            }
        }
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

        let file = (file_char as u8 - b'a') as usize;

        let rank = (8 - (rank_char as u8 - b'0')) as usize;

        if file >= 8 || rank >= 8 {

            return Err("invalid en-passant square (out of bounds)");
        }

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
    board.hash ^= ZOBRIST.keys[768];

    // Old en passant file
    if saved_ep_square != -1 {

        board.hash ^= ZOBRIST.keys[773 + (saved_ep_square as usize & 7)];
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

        board.hash ^= ZOBRIST.keys[769];
    }

    if lost_rights & CASTLING_WQ != 0 {

        board.hash ^= ZOBRIST.keys[770];
    }

    if lost_rights & CASTLING_BK != 0 {

        board.hash ^= ZOBRIST.keys[771];
    }

    if lost_rights & CASTLING_BQ != 0 {

        board.hash ^= ZOBRIST.keys[772];
    }

    // New en passant square
    if board.en_passant_square != -1 {

        board.hash ^= ZOBRIST.keys[773 + (board.en_passant_square as usize & 7)];
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
// Zobrist hashing (for transposition table)
// -----------------------------------------------------------------------------

struct XorShift64(u64);

impl XorShift64 {
    fn next(&mut self) -> u64 {

        self.0 ^= self.0 << 13;

        self.0 ^= self.0 >> 7;

        self.0 ^= self.0 << 17;

        self.0
    }
}

/// Compute the Zobrist key index for a piece on a square.
fn zobrist_key(piece: i8, sq: Square) -> usize {
    debug_assert!(piece != EMPTY, "zobrist_key called with EMPTY piece");
    if piece == EMPTY {
        return 0;
    }
    let piece_type = piece.unsigned_abs() as usize - 1; // 0..5
    let color = if piece > 0 { 0 } else { 1 }; // 0=White, 1=Black
    (piece_type * 2 + color) * 64 + sq as usize
}

struct Zobrist {
    keys: [u64; 781],
}

impl Zobrist {
    fn new() -> Self {

        let mut rng = XorShift64(12_345_678_901_234_567);

        let mut keys = [0u64; 781];

        for key in keys.iter_mut() {

            *key = rng.next();
        }

        Zobrist { keys }
    }

    fn hash(&self, board: &Board) -> u64 {

        let mut hash = 0u64;

        for sq in 0..64u8 {

            let piece = board.board[sq as usize];

            if piece != EMPTY {

                let piece_type = piece.unsigned_abs() as usize - 1; // 0..5

                let color = if piece > 0 { 0usize } else { 1usize };

                hash ^= self.keys[(piece_type * 2 + color) * 64 + sq as usize];
            }
        }

        if board.side_to_move == Color::Black {

            hash ^= self.keys[768];
        }

        const CASTLING_BITS: [(u8, usize); 4] = [
            (CASTLING_WK, 769),
            (CASTLING_WQ, 770),
            (CASTLING_BK, 771),
            (CASTLING_BQ, 772),
        ];

        for &(bit, idx) in &CASTLING_BITS {

            if board.castling_rights & bit != 0 {

                hash ^= self.keys[idx];
            }
        }

        if board.en_passant_square != -1 {

            let file = (board.en_passant_square as usize) & 7;

            hash ^= self.keys[773 + file];
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

/// Single TT entry packed into two atomics:
///   key:   full 64‑bit Zobrist hash
///   data:  score(32) | best_move_packed(16) | depth(8) | flags(8)
#[repr(C, align(8))]
struct TTEntry {
    key: AtomicU64,
    data: AtomicU64,
}

// flags
const TT_EXACT: u8 = 0;

const TT_LOWER: u8 = 1; // beta cutoff → score is a lower bound

const TT_UPPER: u8 = 2; // no move improved alpha → score is an upper bound

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

struct TranspositionTable {
    entries: Box<[TTEntry]>,
}

impl TranspositionTable {
    fn new() -> Self {

        let count = 1 << TT_BITS;

        let mut vec = Vec::with_capacity(count);

        vec.resize_with(count, || TTEntry {
            key: AtomicU64::new(0),
            data: AtomicU64::new(0),
        });

        TranspositionTable {
            entries: vec.into_boxed_slice(),
        }
    }

    fn probe(&self, zobrist: u64) -> Option<(i32, u16, u8, u8)> {

        let entry = &self.entries[(zobrist as usize) & TT_MASK];

        // Double-key-read seqlock:
        //   store: data (Relaxed), then key (Release)
        //   probe: key (Acquire), data (Relaxed), key (Acquire)
        //
        // If two threads write to the same slot concurrently (Lazy SMP),
        // thread A's data + thread B's key could produce a false match.
        // Double-checking the key catches this: if the key changed between
        // the two reads, the data may be from a different store.
        loop {
            let key1 = entry.key.load(Ordering::Acquire);

            let data = entry.data.load(Ordering::Relaxed);

            let key2 = entry.key.load(Ordering::Acquire);

            if key1 != key2 {

                // Concurrent write in progress — retry
                continue;
            }

            if key1 != zobrist {

                return None;
            }

            return Some(unpack_tt_data(data));
        }
    }

    fn store(&self, zobrist: u64, score: i32, best_move_packed: u16, depth: u8, flags: u8) {

        let entry = &self.entries[(zobrist as usize) & TT_MASK];

        let new_data = pack_tt_data(score, best_move_packed, depth, flags);

        let old_data = entry.data.load(Ordering::Relaxed);

        // Depth‑preferred replacement: deeper searches overwrite shallower ones
        if old_data != 0 && unpack_tt_data(old_data).2 > depth {

            return;
        }

        entry.data.store(new_data, Ordering::Relaxed);

        entry.key.store(zobrist, Ordering::Release);
    }
}

// Thread-local TT: each Lazy SMP thread has its own independent cache.
// No shared state = no race conditions in probe/store.
// The leader thread's TT provides move ordering for iterative deepening;
// helper threads get fresh TTs and independently refine their own searches.
thread_local! {
    static TT: TranspositionTable = TranspositionTable::new();
}

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

    let tt_result = TT.with(|tt| tt.probe(zobrist_key));

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
        board.hash ^= ZOBRIST.keys[768];

        if saved_ep != -1 {

            board.hash ^= ZOBRIST.keys[773 + (saved_ep as usize & 7)];
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

        TT.with(|tt| tt.store(zobrist_key, alpha, pack_move_data(&best_move), depth as u8, flags));
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
