use log::info;

use crate::board::{Board, Move};
use crate::score::score;

pub fn search(board: &mut Board, max_depth: u8) -> Move {
    let moves: Vec<Move> = board
        .gen_moves()
        .into_iter()
        .filter(|m| board.is_legal(m))
        .collect();
    let mut best = -9999;
    let mut best_move = Move::default();
    for depth in 0..=max_depth {
        info!("searching depth: {}", depth);
        for mv in moves.as_slice() {
            board.make_move(mv);
            let score = -recursive_search(board, depth);
            board.unmake_move(mv);
            if score > best {
                info!("new best move: {}, score: {}", mv, score);
                best = score;
                best_move = *mv;
            }
        }
    }
    info!("best move: {}, score: {}", best_move, best);
    best_move
}

fn recursive_search(board: &mut Board, depth: u8) -> i32 {
    if depth == 0 {
        return score(board);
    }
    let moves = board.gen_moves();
    let mut best = -9999;
    for mv in moves {
        if !board.is_legal(&mv) {
            continue;
        }
        board.make_move(&mv);
        let score = -recursive_search(board, depth - 1);
        board.unmake_move(&mv);
        if score > best {
            best = score;
        }
    }
    best
}
