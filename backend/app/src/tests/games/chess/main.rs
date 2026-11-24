use super::*;

fn board_fen(fen: &str) -> Board {
    parse_fen(fen).expect("valid FEN")
}

#[allow(dead_code)]
fn count_moves(board: &Board) -> usize {
    let moves = generate_pseudo_legal_moves(board);
    moves.count
}

/// Parse a UCI move ("e2e4", "e7e8q", ...) into engine Move (a8=0 squares).
fn uci_move(board: &Board, uci: &str) -> Move {
    let bytes = uci.as_bytes();
    let file = |c: u8| (c - b'a') as i8;
    let rank = |c: u8| (8 - (c - b'0')) as i8;
    let from = Square::from_index((rank(bytes[1]) * 8 + file(bytes[0])) as u8);
    let to = Square::from_index((rank(bytes[3]) * 8 + file(bytes[2])) as u8);
    let promotion = if bytes.len() > 4 {
        match bytes[4] {
            b'q' => if board.side_to_move == Color::White { Piece::WQueen } else { Piece::BQueen },
            b'r' => if board.side_to_move == Color::White { Piece::WRook } else { Piece::BRook },
            b'b' => if board.side_to_move == Color::White { Piece::WBishop } else { Piece::BBishop },
            b'n' => if board.side_to_move == Color::White { Piece::WKnight } else { Piece::BKnight },
            _ => Piece::Empty,
        }
    } else {
        Piece::Empty
    };
    Move { from, to, promotion }
}

#[test]
fn start_position_piece_lists() {
    let b = board_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
    // White piece counts
    assert_eq!(b.piece_count[0], 8, "white pawns");
    assert_eq!(b.piece_count[1], 2, "white knights");
    assert_eq!(b.piece_count[2], 2, "white bishops");
    assert_eq!(b.piece_count[3], 2, "white rooks");
    assert_eq!(b.piece_count[4], 1, "white queen");
    assert_eq!(b.piece_count[5], 1, "white king");
    // Black piece counts
    assert_eq!(b.piece_count[6], 8, "black pawns");
    assert_eq!(b.piece_count[7], 2, "black knights");
    assert_eq!(b.piece_count[8], 2, "black bishops");
    assert_eq!(b.piece_count[9], 2, "black rooks");
    assert_eq!(b.piece_count[10], 1, "black queen");
    assert_eq!(b.piece_count[11], 1, "black king");
    // No white piece in black lists
    for i in 0..6 {
        for j in 0..b.piece_count[i] {
            let sq = b.piece_list[i][j as usize];
            assert!(b.board[sq.index()].is_white(), "white list {} sq {} has {} (not white)", i, sq, b.board[sq.index()]);
        }
    }
    for i in 6..12 {
        for j in 0..b.piece_count[i] {
            let sq = b.piece_list[i][j as usize];
            assert!(b.board[sq.index()].is_black(), "black list {} sq {} has {} (not black)", i, sq, b.board[sq.index()]);
        }
    }
}

#[test]
fn start_position_white_moves() {
    let b = board_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
    let moves = generate_pseudo_legal_moves(&b);
    assert!(moves.count >= 20, "expected ≥20 legal moves, got {}", moves.count);
    // None should be from a black piece square
    for i in 0..moves.count {
        let mv = moves.moves[i];
        let piece = b.board[mv.from.index()];
        assert!(piece.is_white(), "move {}: from {} ({}) not white piece (found {})", i, square_name(mv.from), mv.from, piece);
    }
}

#[test]
fn depth_1_best_move() {
    let b = board_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
    // Search must return a legal white move
    let mut board = b.clone();
    let mut killers = [[Move::null(); 2]; MAX_PLY];
    let (_score, best) = search(&mut board, 1, -30000, 30000, &mut killers, 0);
    let piece = b.board[best.from.index()];
    assert!(piece.is_white(), "best move from {} ({}) not white (found {})", square_name(best.from), best.from, piece);
}

#[test]
fn best_move_white_start() {
    let result = best_move("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", 1);
    assert!(result.is_some(), "expected a move, got None");
    let mv = result.unwrap();
    // Parse the move to validate
    let from_file = mv.as_bytes()[0] - b'a';
    let from_rank = mv.as_bytes()[1] - b'1';
    let from_sq = ((7 - from_rank) * 8 + from_file) as usize;
    // Must be a white piece
    let b = board_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
    assert!(b.board[from_sq].is_white(), "best move {} from sq {} has {} (not white)", mv, from_sq, b.board[from_sq]);
}

#[test]
fn best_move_multi_depth() {
    for depth in 1..=3 {
        let result = best_move("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", depth);
        assert!(result.is_some(), "depth {}: expected a move", depth);
    }
    // Depth 3 with Lazy SMP pass
    let result = best_move("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", 3);
    assert!(result.is_some(), "depth 3: expected a move");
}

#[test]
fn best_move_deep_search() {
    // Test that deeper searches still return legal moves
    let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR b KQkq - 0 1";
    for depth in 1..=6 {
        let b = board_fen(fen);
        let mut local_board = b.clone();
        let mut killers = [[Move::null(); 2]; MAX_PLY];
        for d in 1..=depth {
            let (_score, mv) = search(&mut local_board, d, -30000, 30000, &mut killers, 0);
            let is_valid = mv.from != mv.to || mv.promotion != Piece::Empty;
            if is_valid {
                let piece = b.board[mv.from.index()];
                assert!(piece.is_black(),
                    "depth {}: move {} from sq {} has {} (not black piece)",
                    d, square_name(mv.from)+&square_name(mv.to), mv.from, piece);
            }
        }
        validate_lists(&local_board);
        for sq in 0..64u8 {
            assert_eq!(local_board.board[sq as usize], b.board[sq as usize],
                "board[{}] changed after depth {} search", sq, depth);
        }
    }
}

fn validate_lists(b: &Board) {
    for idx in 0..12 {
        let count = b.piece_count[idx];
        for i in 0..count {
            let sq = b.piece_list[idx][i as usize];
            let expected = Piece::from_i8(if idx < 6 { (idx + 1) as i8 } else { -((idx - 5) as i8) });
            assert_eq!(b.board[sq.index()], expected,
                "list[{}][{}] = sq {} has {} on board, expected {}",
                idx, i, sq, b.board[sq.index()], expected);
        }
    }
}

#[test]
fn best_move_single_threaded() {
    for depth in 1..=2 {
        let result = best_move_single("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", depth);
        assert!(result.is_some(), "depth {}: expected a move", depth);
        let mv = result.unwrap();
        let from_file = (mv.as_bytes()[0] - b'a') as i8;
        let from_rank = (mv.as_bytes()[1] - b'1') as i8;
        let from_sq = ((7 - from_rank) * 8 + from_file) as usize;
        let b = board_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        assert!(b.board[from_sq].is_white(),
            "depth {}: best move {} from sq {} has {} (not white)",
             depth, mv, from_sq, b.board[from_sq]);
    }
}

#[test]
fn search_restores_board() {
    let board = board_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
    let hash_before = board.hash;
    validate_lists(&board);

    let mut b = board.clone();
    let mut killers = [[Move::null(); 2]; MAX_PLY];
    let (_score, _mv) = search(&mut b, 1, -30000, 30000, &mut killers, 0);

    // After search depth 1, board should be restored
    assert_eq!(b.hash, hash_before, "hash changed after depth 1 search");
    validate_lists(&b);
    for sq in 0..64u8 {
        assert_eq!(b.board[sq as usize], board.board[sq as usize],
            "board[{}] changed after depth 1 search", sq);
    }

    let (_score2, _mv2) = search(&mut b, 1, -30000, 30000, &mut killers, 0);
    assert_eq!(b.hash, hash_before, "hash changed after 2nd depth 1 search");
    validate_lists(&b);
    for sq in 0..64u8 {
        assert_eq!(b.board[sq as usize], board.board[sq as usize],
            "board[{}] changed after 2nd depth 1 search", sq);
    }

}

#[test]
fn search_depth3_restores_board() {
    let board = board_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
    let hash_before = board.hash;
    validate_lists(&board);

    let mut b = board.clone();
    let mut killers = [[Move::null(); 2]; MAX_PLY];
    let (_score, _mv) = search(&mut b, 3, -30000, 30000, &mut killers, 0);

    assert_eq!(b.hash, hash_before, "hash changed after depth 3 search");
    validate_lists(&b);
    for sq in 0..64u8 {
        assert_eq!(b.board[sq as usize], board.board[sq as usize],
            "board[{}] changed after depth 3 search", sq);
    }
}

#[test]
fn iterative_deepening_restores() {
    let board = board_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
    let hash_before = board.hash;
    let mut b = board.clone();
    let mut killers = [[Move::null(); 2]; MAX_PLY];

    for d in 1..=3 {
        let (_score, _mv) = search(&mut b, d, -30000, 30000, &mut killers, 0);
        validate_lists(&b);
        assert_eq!(b.hash, hash_before, "hash changed after depth {}", d);
    }
}

#[test]
fn incremental_hash_matches_full_hash() {
    // The book probe uses board.hash (incremental). It MUST equal a full
    // Zobrist::hash() recompute after every make/unmake, or book hits break.
    // This exercises the conditional-EP incremental logic in make_move.
    let fens = [
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        "rnbqkbnr/ppp1pppp/8/3p4/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2", // 1.e4 d5, ep d6 (no capture)
        "rnbqkb1r/pppp1ppp/5n2/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R b KQkq - 3 3", // Italian
        "r1bqkbnr/pppp1ppp/2n5/1B2p3/4P3/5N2/PPPP1PPP/RNBQK2R b KQkq - 3 3", // Ruy
    ];
    for fen in fens {
        let board = board_fen(fen);
        assert_eq!(board.hash, ZOBRIST.hash(&board),
            "parse_fen hash mismatch for {fen}");
    }

    // Play a few real moves and check incremental == full at every ply.
    let mut b = board_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
    let moves_uci = ["e2e4", "c7c5", "g1f3", "b8c6", "f1b5", "a7a6", "b5c6", "d7c6"];
    for m in moves_uci {
        let mv = uci_move(&b, m);
        let undo = make_move(&mut b, mv);
        assert_eq!(b.hash, ZOBRIST.hash(&b),
            "incremental hash diverged after {m}");
        let _ = unmake_move(&mut b, mv, undo);
        assert_eq!(b.hash, ZOBRIST.hash(&b),
            "incremental hash diverged after unmake {m}");
        // redo for next ply
        let undo = make_move(&mut b, mv);
        let _ = undo;
    }
    // And an en-passant capture line (incremental ep path)
    let mut b = board_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
    let ep_moves = ["e2e4", "d7d5", "e4e5", "f7f5", "e5f6"];
    for m in ep_moves {
        let mv = uci_move(&b, m);
        let undo = make_move(&mut b, mv);
        assert_eq!(b.hash, ZOBRIST.hash(&b),
            "incremental hash diverged after ep line {m}");
        let _ = unmake_move(&mut b, mv, undo);
        assert_eq!(b.hash, ZOBRIST.hash(&b),
            "incremental hash diverged after unmake ep line {m}");
        let undo = make_move(&mut b, mv);
        let _ = undo;
    }
}

#[test]
fn book_diagnostic() {
    // Probe the opening book to see which positions are present.
    // Expected hashes below were verified with python-chess zobrist_hash AND
    // confirmed present in filtered.bin (the same table + convention).
    // The book is embedded (BOOK_BYTES) and BOOK is always loaded.

    // (name, fen, expected_python_chess_hash, in_book)
    let cases: &[(&str, &str, u64, bool)] = &[
        ("start",   "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", 0x463b96181691fc9c, true),
        ("1.e4 e5", "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2", 0x0844931a6ef4b9a0, true),
        ("Sicilian","rnbqkbnr/pp1ppppp/8/2p5/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2", 0x644d4afe02564aeb, true),
        ("1.d4 d5", "rnbqkbnr/ppp1pppp/8/3p4/3P4/8/PPP1PPPP/RNBQKBNR w KQkq - 0 2", 0x06649ba69b8c9ff8, true),
        ("1.d4 Nf6 2.c4", "rnbqkb1r/pppppppp/5n2/8/2PP4/8/PP2PPPP/RNBQKBNR b KQkq - 0 2", 0x54c31263e9ad3b4f, true),
        ("Italian", "r1bqkbnr/pppp1ppp/2n5/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R b KQkq - 3 3", 0x1b7e13263fc0ab38, true),
        ("Ruy Lopez", "r1bqkbnr/pppp1ppp/2n5/1B2p3/4P3/5N2/PPPP1PPP/RNBQK2R b KQkq - 3 3", 0x4b1376a17217ee1d, true),
        ("QGD",     "rnbqkbnr/ppp2ppp/4p3/3p4/2PP4/8/PP2PPPP/RNBQKBNR w KQkq - 0 3", 0xfc30f6b3c0a1a4ad, true),
        ("KID",     "rnbqkb1r/pppppp1p/5np1/8/2PP4/8/PP2PPPP/RNBQKBNR w KQkq - 0 3", 0xbd7741389222a904, true),
        ("Scandi",  "rnbqkbnr/ppp1pppp/8/3p4/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2", 0x0756b94461c50fb0, true),
        ("1.Nf3",   "rnbqkbnr/pppppppp/8/8/8/5N2/PPPPPPPP/RNBQKB1R b KQkq - 1 1", 0x9d5f7aee7e779da1, true),
        ("1.c4",    "rnbqkbnr/pppppppp/8/8/2P5/8/PP1PPPPP/RNBQKBNR b KQkq - 0 1", 0xca18093c559e579b, true),
        ("Petrov",  "rnbqkb1r/pppp1ppp/5n2/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 2 3", 0x88ce4b19abe8990b, true),
        ("French",  "rnbqkbnr/pppp1ppp/4p3/8/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2", 0xf44b6961e533d1c4, true),
        ("Caro-Kann","rnbqkbnr/pp1ppppp/2p5/8/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2", 0x104676bf8ad1922d, true),
        ("1.e4 c5 2.Nf3", "rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 1 2", 0xbf29a6086ab02bd6, true),
    ];

    let data = crate::games::chess::main::BOOK_BYTES;
    let mut book_map: std::collections::HashMap<u64, Vec<u16>> = std::collections::HashMap::new();
    for chunk in data.chunks_exact(16) {
        let key = u64::from_be_bytes(chunk[0..8].try_into().unwrap());
        let mv = u16::from_le_bytes(chunk[8..10].try_into().unwrap());
        book_map.entry(key).or_default().push(mv);
    }

    println!("\n=== Opening book diagnostic ===");
    println!("Book entries: {}", data.len() / 16);
    println!("Book positions: {}", book_map.len());

    let mut all_pass = true;
    println!("\n{:<16} {:<18} {:<18} {:<8} {:<8}", "position", "engine_hash", "expected", "hash_ok", "in_book");
    for (name, fen, expected, in_book) in cases {
        let b = board_fen(fen);
        let hash_ok = b.hash == *expected;
        let hit = book_map.contains_key(&b.hash);
        // The engine must match python-chess AND the book (if in_book=true).
        let pass = hash_ok && (hit == *in_book);
        if !pass { all_pass = false; }
        println!("{:<16} 0x{:016x} 0x{:016x} {:<8} {:<8}{}",
            name, b.hash, expected,
            if hash_ok { "✓" } else { "✗" },
            if hit { "✓" } else { "✗" },
            if pass { "" } else { "  <-- FAIL" });
    }

    if all_pass {
        println!("\nALL {} positions match python-chess AND the book ✓", cases.len());
    } else {
        println!("\nSOME POSITIONS FAIL (see above)");
    }

    assert!(all_pass, "diagnostic complete");
}

#[test]
fn parse_fen_rejects_malformed_input() {
    // Every one of these used to panic (index out of bounds) or silently
    // accept garbage. They must all return Err — never panic, never succeed.
    let bad = [
        "",
        "8/8/8/8/8/8/8/8",                       // < 4 fields
        "8/8/8/8/8/8/8/8/8 w - - 0 1",           // 9 ranks (used to panic)
        "8/8/8/8/8/8/8/99 w - - 0 1",            // rank overflows (used to panic)
        "8/8/8/8/8/8/8/88 w - - 0 1",            // rank overflows via digits
        "8/8/8/8/8/8/8/8P w - - 0 1",            // piece after full rank
        "8/8/8/8/8/8/8/7 w - - 0 1",             // rank underfills
        "7x/8/8/8/8/8/8/8 w - - 0 1",            // invalid piece character
        "8/8/8/8/8/8/8/8 w KQkq e9 0 1",         // ep rank out of range
        "8/8/8/8/8/8/8/8 w KQkq a0 0 1",         // ep rank underflow (used to panic in debug)
        "K6K/8/8/8/8/8/8/k7 w - - 0 1",          // two white kings
        "8/8/8/8/8/8/8/8 w - - 0 1",             // no kings
        "pppppppp/8/8/8/8/8/8/ppp w - - 0 1",    // 11 pawns on one side
    ];
    for fen in bad {
        assert!(parse_fen(fen).is_err(), "expected Err for: {fen}");
    }

    // Over-length FEN (the length cap rejects it before any parsing)
    let long = format!("K7/8/8/8/8/8/8/8 w - - 0 {}", "1".repeat(600));
    assert!(parse_fen(&long).is_err(), "expected Err for over-length FEN");

    // Valid minimal position still parses
    assert!(parse_fen("k7/8/8/8/8/8/8/K7 w - - 0 1").is_ok());
}
