// A minimal chess engine inspired by Sunfish.
// No heap allocations inside search/move generation.
// Uses enums for clarity and constants for demystification.

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

#[derive(Clone, Copy, PartialEq, Eq)]

enum PieceType {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
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

// Convert a piece i8 to a (Color, PieceType) pair, if non-empty.
fn piece_info(piece: i8) -> Option<(Color, PieceType)> {

    if piece == EMPTY {

        return None;
    }

    let color = if piece > 0 { Color::White } else { Color::Black };

    let piece_type = match piece.abs() {
        1 => PieceType::Pawn,
        2 => PieceType::Knight,
        3 => PieceType::Bishop,
        4 => PieceType::Rook,
        5 => PieceType::Queen,
        6 => PieceType::King,
        _ => return None,
    };

    Some((color, piece_type))
}

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

struct Board {
    board: [i8; 64],
    side_to_move: Color,
    castling_rights: u8,
    en_passant_square: i8, // -1 if none, else square index
    halfmove_clock: u16,
    fullmove_number: u16,
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
        }
    }

    fn color_of(&self, square: Square) -> Option<Color> {

        let piece = self.board[square as usize];

        if piece == EMPTY {

            None
        } else if piece > 0 {

            Some(Color::White)
        } else {

            Some(Color::Black)
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

            if character.is_digit(10) {

                let count = character.to_digit(10).unwrap() as usize;

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

                board.board[rank * 8 + file] = piece;

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

        let file = (parts[3].chars().nth(0).unwrap() as u8 - b'a') as usize;

        let rank = (8 - (parts[3].chars().nth(1).unwrap() as u8 - b'0')) as usize;

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

    Ok(board)
}

// -----------------------------------------------------------------------------
// Utility: square names
// -----------------------------------------------------------------------------

fn square_name(square: Square) -> String {

    let file = (square % 8) as u8;

    let rank = 8 - (square / 8) as u8;

    format!("{}{}", (b'a' + file) as char, rank)
}

// -----------------------------------------------------------------------------
// Move generation (pseudo-legal)
// -----------------------------------------------------------------------------

const MAX_MOVES: usize = 218;

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

        if self.count < MAX_MOVES {

            self.moves[self.count] = mv;

            self.count += 1;
        }
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

    for square in 0..64u8 {

        let piece = board.board[square as usize];

        // Skip empty squares and opponent pieces
        if piece == EMPTY || (piece > 0) != (current_sign == 1) {

            continue;
        }

        let piece_type = piece.abs();

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

                    // File boundary: can't capture off the edge
                    if (file_offset == -1 && square % 8 == 0) || (file_offset == 1 && square % 8 == 7) {

                        continue;
                    }

                    let target_square = (square as i8 + pawn_direction + file_offset) as Square;

                    if target_square < 64 {

                        let target = board.board[target_square as usize];

                        let is_opponent = target != EMPTY && (target > 0) != (current_sign == 1);

                        let is_en_passant = target_square as i8 == board.en_passant_square;

                        if is_opponent || is_en_passant {

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
                    }
                }
            },
            2 => {

                // ── Knight ──
                for &offset in &[-17, -15, -10, -6, 6, 10, 15, 17] {

                    let target_square = square as i8 + offset;

                    if target_square >= 0 && target_square < 64 {

                        // File-wrap check: knight moves change file by at most 2
                        let from_file = square % 8;

                        let to_file = target_square as usize % 8;

                        if (from_file as i8 - to_file as i8).abs() > 2 {

                            continue;
                        }

                        let target = board.board[target_square as usize];

                        if target == EMPTY || (target > 0) != (current_sign == 1) {

                            moves.push(Move::new(square, target_square as u8));
                        }
                    }
                }
            },
            3 => {

                // ── Bishop ──
                for &direction in &[-9, -7, 7, 9] {

                    // File boundary check for initial step
                    let moving_left = direction == -9 || direction == 7;

                    let moving_right = direction == -7 || direction == 9;

                    if (moving_left && square % 8 == 0) || (moving_right && square % 8 == 7) {

                        continue;
                    }

                    let mut current_square = square as i8 + direction;

                    while current_square >= 0 && current_square < 64 {

                        let target = board.board[current_square as usize];

                        if target != EMPTY {

                            if (target > 0) != (current_sign == 1) {

                                moves.push(Move::new(square, current_square as u8));
                            }

                            break;
                        }

                        moves.push(Move::new(square, current_square as u8));

                        // File wrap check before next step
                        if (moving_left && (current_square as usize) % 8 == 0)
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
                    if (direction == -1 && square % 8 == 0) || (direction == 1 && square % 8 == 7) {

                        continue;
                    }

                    let mut current_square = square as i8 + direction;

                    while current_square >= 0 && current_square < 64 {

                        let target = board.board[current_square as usize];

                        if target != EMPTY {

                            if (target > 0) != (current_sign == 1) {

                                moves.push(Move::new(square, current_square as u8));
                            }

                            break;
                        }

                        moves.push(Move::new(square, current_square as u8));

                        // File wrap check before next horizontal step
                        if (direction == -1 && (current_square as usize) % 8 == 0)
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

                    if (moving_left && square % 8 == 0) || (moving_right && square % 8 == 7) {

                        continue;
                    }

                    let mut current_square = square as i8 + direction;

                    while current_square >= 0 && current_square < 64 {

                        let target = board.board[current_square as usize];

                        if target != EMPTY {

                            if (target > 0) != (current_sign == 1) {

                                moves.push(Move::new(square, current_square as u8));
                            }

                            break;
                        }

                        moves.push(Move::new(square, current_square as u8));

                        // File wrap check before next step
                        if (moving_left && (current_square as usize) % 8 == 0)
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

                    if target_square >= 0 && target_square < 64 {

                        let target = board.board[target_square as usize];

                        if target == EMPTY || (target > 0) != (current_sign == 1) {

                            moves.push(Move::new(square, target_square as u8));
                        }
                    }
                }

                // Castling
                if current_side == Color::White {

                    if (board.castling_rights & CASTLING_WK) != 0 {

                        if board.board[5] == EMPTY
                            && board.board[6] == EMPTY
                            && board.board[7] == W_ROOK
                            && board.board[4] == W_KING
                        {

                            moves.push(Move::new(4, 6));
                        }
                    }

                    if (board.castling_rights & CASTLING_WQ) != 0 {

                        if board.board[1] == EMPTY
                            && board.board[2] == EMPTY
                            && board.board[3] == EMPTY
                            && board.board[0] == W_ROOK
                            && board.board[4] == W_KING
                        {

                            moves.push(Move::new(4, 2));
                        }
                    }
                } else {

                    if (board.castling_rights & CASTLING_BK) != 0 {

                        if board.board[61] == EMPTY
                            && board.board[62] == EMPTY
                            && board.board[63] == B_ROOK
                            && board.board[60] == B_KING
                        {

                            moves.push(Move::new(60, 62));
                        }
                    }

                    if (board.castling_rights & CASTLING_BQ) != 0 {

                        if board.board[57] == EMPTY
                            && board.board[58] == EMPTY
                            && board.board[59] == EMPTY
                            && board.board[56] == B_ROOK
                            && board.board[60] == B_KING
                        {

                            moves.push(Move::new(60, 58));
                        }
                    }
                }
            },
            _ => {},
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

        if from >= 0 && from < 64 {

            let piece = board.board[from as usize];

            if piece == (if by_color == Color::White { W_PAWN } else { B_PAWN }) {

                return true;
            }
        }
    }

    // Knight attacks
    for &offset in &[-17, -15, -10, -6, 6, 10, 15, 17] {

        let from = square as i8 + offset;

        if from >= 0 && from < 64 {

            let piece = board.board[from as usize];

            if piece == (if by_color == Color::White { W_KNIGHT } else { B_KNIGHT }) {

                return true;
            }
        }
    }

    // King attacks
    for &offset in &[-9, -8, -7, -1, 1, 7, 8, 9] {

        let from = square as i8 + offset;

        if from >= 0 && from < 64 {

            let piece = board.board[from as usize];

            if piece == (if by_color == Color::White { W_KING } else { B_KING }) {

                return true;
            }
        }
    }

    // Sliding pieces: bishop/queen diagonals
    for &direction in &[-9, -7, 7, 9] {

        let mut current_square = square as i8 + direction;

        while current_square >= 0 && current_square < 64 {

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

        while current_square >= 0 && current_square < 64 {

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

    // Find the king of the side to move
    let king_piece = if board.side_to_move == Color::White {

        W_KING
    } else {

        B_KING
    };

    let mut king_square = 0;

    for square in 0..64u8 {

        if board.board[square as usize] == king_piece {

            king_square = square;

            break;
        }
    }

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
}

fn make_move(board: &mut Board, mv: Move) -> MoveUndo {

    let current_side = board.side_to_move;

    let opponent = current_side.opposite();

    let captured = board.board[mv.to as usize];

    let saved_ep_square = board.en_passant_square;

    let saved_castling_rights = board.castling_rights;

    let saved_halfmove_clock = board.halfmove_clock;

    let moving_piece = board.board[mv.from as usize];

    let is_king = moving_piece == (if current_side == Color::White { W_KING } else { B_KING });

    let is_rook = !is_king && moving_piece.abs() == 4;

    // Move piece
    board.board[mv.to as usize] = moving_piece;

    board.board[mv.from as usize] = EMPTY;

    // Promotion
    if mv.promotion != 0 {

        board.board[mv.to as usize] = mv.promotion;
    }

    // En passant capture
    if (mv.to as i8) == board.en_passant_square
        && board.board[mv.to as usize] == (if current_side == Color::White { W_PAWN } else { B_PAWN })
    {

        let captured_pawn_square = if current_side == Color::White {

            mv.to + 8
        } else {

            mv.to - 8
        };

        board.board[captured_pawn_square as usize] = EMPTY;
    }

    // Castling: move the rook when the king castles
    if is_king {

        if mv.from == 4 && mv.to == 6 {

            board.board[5] = W_ROOK;

            board.board[7] = EMPTY;
        } else if mv.from == 4 && mv.to == 2 {

            board.board[3] = W_ROOK;

            board.board[0] = EMPTY;
        } else if mv.from == 60 && mv.to == 62 {

            board.board[61] = B_ROOK;

            board.board[63] = EMPTY;
        } else if mv.from == 60 && mv.to == 58 {

            board.board[59] = B_ROOK;

            board.board[56] = EMPTY;
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
    if board.board[mv.to as usize] == W_ROOK && mv.to == 7 {

        board.castling_rights &= !CASTLING_WK;
    }

    if board.board[mv.to as usize] == W_ROOK && mv.to == 0 {

        board.castling_rights &= !CASTLING_WQ;
    }

    if board.board[mv.to as usize] == B_ROOK && mv.to == 63 {

        board.castling_rights &= !CASTLING_BK;
    }

    if board.board[mv.to as usize] == B_ROOK && mv.to == 56 {

        board.castling_rights &= !CASTLING_BQ;
    }

    // Rook moved from its starting square
    if is_rook && mv.from == 7 {

        board.castling_rights &= !CASTLING_WK;
    }

    if is_rook && mv.from == 0 {

        board.castling_rights &= !CASTLING_WQ;
    }

    if is_rook && mv.from == 63 {

        board.castling_rights &= !CASTLING_BK;
    }

    if is_rook && mv.from == 56 {

        board.castling_rights &= !CASTLING_BQ;
    }

    // Update en passant
    if board.board[mv.to as usize].abs() == 1 && (mv.to as i8 - mv.from as i8).abs() == 16 {

        board.en_passant_square = ((mv.from as i8 + mv.to as i8) / 2) as i8;
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

    MoveUndo {
        captured,
        ep_square: saved_ep_square,
        castling_rights: saved_castling_rights,
        halfmove_clock: saved_halfmove_clock,
    }
}

fn unmake_move(board: &mut Board, mv: Move, undo: MoveUndo) {

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

    // Restore the piece to its original square
    board.board[mv.from as usize] = board.board[mv.to as usize];

    board.board[mv.to as usize] = undo.captured;

    // Promotion: revert the promoted piece back to a pawn
    if mv.promotion != 0 {

        board.board[mv.from as usize] = if opponent == Color::White { W_PAWN } else { B_PAWN };
    }

    // En passant capture: restore the captured pawn
    if (mv.to as i8) == undo.ep_square && undo.captured == EMPTY {

        let captured_pawn_square = if opponent == Color::White { mv.to + 8 } else { mv.to - 8 };

        board.board[captured_pawn_square as usize] = if opponent == Color::White { B_PAWN } else { W_PAWN };
    }

    // Castling: restore the rook to its original square
    if board.board[mv.from as usize] == (if opponent == Color::White { W_KING } else { B_KING }) {

        if mv.from == 4 && mv.to == 6 {

            board.board[7] = W_ROOK;

            board.board[5] = EMPTY;
        } else if mv.from == 4 && mv.to == 2 {

            board.board[0] = W_ROOK;

            board.board[3] = EMPTY;
        } else if mv.from == 60 && mv.to == 62 {

            board.board[63] = B_ROOK;

            board.board[61] = EMPTY;
        } else if mv.from == 60 && mv.to == 58 {

            board.board[56] = B_ROOK;

            board.board[59] = EMPTY;
        }
    }
}

// -----------------------------------------------------------------------------
// Evaluation (SIMD via wide::i32x4 — 128-bit registers, 4 squares per op)
// -----------------------------------------------------------------------------

use wide::i32x4;

fn evaluate_board(board: &Board) -> i32 {

    let mut score = i32x4::ZERO;

    // Process 4 squares per SIMD lane
    for chunk in (0..64u8).step_by(4) {

        let mut vals = [0i32; 4];

        for (j, square) in (chunk..chunk + 4).enumerate() {

            let piece = board.board[square as usize];

            if piece != EMPTY {

                let material = piece_value(piece);

                let pst = if piece > 0 {

                    PST[square as usize]
                } else {

                    PST[(63 - square) as usize]
                };

                let total = material + pst;

                vals[j] = if piece > 0 { total } else { -total };
            }
        }

        score += i32x4::new(vals);
    }

    // Horizontal sum of 4 SIMD lanes
    score.reduce_add()
}

// -----------------------------------------------------------------------------
// Quiescence search (captures only — handles horizon effect)
// -----------------------------------------------------------------------------

fn quiesce(board: &mut Board, mut alpha: i32, beta: i32) -> i32 {

    // Static evaluation at this position (the "standing pat" option)
    let stand_pat = evaluate_board(board);

    if stand_pat >= beta {

        return beta;
    }

    if stand_pat > alpha {

        alpha = stand_pat;
    }

    let moves = generate_pseudo_legal_moves(board);

    for i in 0..moves.count {

        let mv = moves.moves[i];

        // Only consider captures and promotions — skip quiet moves
        if board.board[mv.to as usize] == EMPTY && mv.promotion == 0 {

            continue;
        }

        let undo = make_move(board, mv);

        if in_check(board) {

            unmake_move(board, mv, undo);

            continue;
        }

        let score = -quiesce(board, -beta, -alpha);

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
// Search (Negamax with alpha-beta)
// -----------------------------------------------------------------------------

fn search(board: &mut Board, depth: usize, mut alpha: i32, beta: i32) -> (i32, Move) {

    let mut best_move = Move {
        from: 0,
        to: 0,
        promotion: 0,
    };

    if depth == 0 {

        let score = quiesce(board, alpha, beta);

        return (score, best_move);
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

    // MVV-LVA move ordering: score captures by (victim_value - attacker_value)
    let mut move_scores = [0i16; MAX_MOVES];

    for i in 0..pseudo_moves.count {

        let mv = pseudo_moves.moves[i];

        if board.board[mv.to as usize] != EMPTY {

            let victim = piece_value(board.board[mv.to as usize]) as i16;

            let attacker = piece_value(board.board[mv.from as usize]) as i16;

            move_scores[i] = victim - attacker;
        } else if mv.promotion != 0 {

            // Promotions score between minor (330) and major (500+)
            move_scores[i] = piece_value(mv.promotion) as i16 / 10;
        }
    }

    // Insertion sort by score descending (fast on nearly-sorted arrays)
    for i in 1..pseudo_moves.count {

        let mut j = i;

        while j > 0 && move_scores[j] > move_scores[j - 1] {

            let higher_score = move_scores[j];

            move_scores[j] = move_scores[j - 1];

            move_scores[j - 1] = higher_score;

            let higher_move = pseudo_moves.moves[j];

            pseudo_moves.moves[j] = pseudo_moves.moves[j - 1];

            pseudo_moves.moves[j - 1] = higher_move;

            j -= 1;
        }
    }

    for i in 0..pseudo_moves.count {

        let mv = pseudo_moves.moves[i];

        let undo = make_move(board, mv);

        // Skip illegal moves (that leave our king in check)
        if in_check(board) {

            unmake_move(board, mv, undo);

            continue;
        }

        let (child_score, _) = search(board, depth - 1, -beta, -alpha);

        let score = -child_score;

        unmake_move(board, mv, undo);

        if score > alpha {

            alpha = score;

            best_move = mv;

            if alpha >= beta {

                break;
            }
        }
    }

    (alpha, best_move)
}

// -----------------------------------------------------------------------------
// Public API
// -----------------------------------------------------------------------------

pub fn best_move(fen: &str, depth: usize) -> Option<String> {

    let mut board = match parse_fen(fen) {
        Ok(board) => board,
        Err(_) => return None,
    };

    let (_, best_move) = search(&mut board, depth, -30000, 30000);

    // No legal moves (checkmate or stalemate — returned with from == to)
    if best_move.from == best_move.to {

        return None;
    }

    Some(format!("{}{}", square_name(best_move.from), square_name(best_move.to)))
}
