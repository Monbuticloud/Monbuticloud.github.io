use super::*;

fn board_fen(fen: &str) -> Board {
    parse_fen(fen).expect("valid FEN")
}

#[allow(dead_code)]
fn count_moves(board: &Board) -> usize {
    let moves = generate_pseudo_legal_moves(board);
    moves.count
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
            assert!(b.board[sq as usize] > 0, "white list {} sq {} has {} (not white)", i, sq, b.board[sq as usize]);
        }
    }
    for i in 6..12 {
        for j in 0..b.piece_count[i] {
            let sq = b.piece_list[i][j as usize];
            assert!(b.board[sq as usize] < 0, "black list {} sq {} has {} (not black)", i, sq, b.board[sq as usize]);
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
        let piece = b.board[mv.from as usize];
        assert!(piece > 0, "move {}: from {} ({}) not white piece (found {})", i, square_name(mv.from), mv.from, piece);
    }
}

#[test]
fn depth_1_best_move() {
    let b = board_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
    // Search must return a legal white move
    let mut board = b.clone();
    let mut killers = [[Move { from: 0, to: 0, promotion: 0 }; 2]; MAX_PLY];
    let (_score, best) = search(&mut board, 1, -30000, 30000, &mut killers, 0);
    let piece = b.board[best.from as usize];
    assert!(piece > 0, "best move from {} ({}) not white (found {})", square_name(best.from), best.from, piece);
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
    assert!(b.board[from_sq] > 0, "best move {} from sq {} has {} (not white)", mv, from_sq, b.board[from_sq]);
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
        let mut killers = [[Move { from: 0, to: 0, promotion: 0 }; 2]; MAX_PLY];
        for d in 1..=depth {
            let (_score, mv) = search(&mut local_board, d, -30000, 30000, &mut killers, 0);
            let is_valid = mv.from != mv.to || mv.promotion != 0;
            if is_valid {
                let piece = b.board[mv.from as usize];
                assert!(piece < 0,
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
            let expected = if idx < 6 { (idx + 1) as i8 } else { -((idx - 5) as i8) };
            assert_eq!(b.board[sq as usize], expected,
                "list[{}][{}] = sq {} has {} on board, expected {}",
                idx, i, sq, b.board[sq as usize], expected);
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
        assert!(b.board[from_sq] > 0,
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
    let mut killers = [[Move { from: 0, to: 0, promotion: 0 }; 2]; MAX_PLY];
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
    let mut killers = [[Move { from: 0, to: 0, promotion: 0 }; 2]; MAX_PLY];
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
    let mut killers = [[Move { from: 0, to: 0, promotion: 0 }; 2]; MAX_PLY];

    for d in 1..=3 {
        let (_score, _mv) = search(&mut b, d, -30000, 30000, &mut killers, 0);
        validate_lists(&b);
        assert_eq!(b.hash, hash_before, "hash changed after depth {}", d);
    }
}
