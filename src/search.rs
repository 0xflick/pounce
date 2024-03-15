use log::info;
use std::time::{Duration, Instant};

use crate::board::{Board, Move};
use crate::score::score;

pub fn search(board: &mut Board, time_limit: Duration) -> Move {
    let moves: Vec<Move> = board
        .gen_moves()
        .into_iter()
        .filter(|m| board.is_legal(m))
        .collect();
    let mut best = -9999;
    let mut best_move = Move::default();

    let start_time = Instant::now();
    for depth in 0.. {
        let mut depth_best = -9999;
        let mut depth_best_move = Move::default();

        info!("searching depth: {}", depth);
        for mv in moves.as_slice() {
            if start_time.elapsed() >= time_limit {
                info!("timeout reached at depth: {}", depth);
                break;
            }
            board.make_move(mv);
            let score = -recursive_search(board, depth, time_limit);
            board.unmake_move(mv);
            if score > depth_best {
                info!("new for depth best move: {}, score: {}", mv, score);
                depth_best = score;
                depth_best_move = *mv;
            }
            if start_time.elapsed() >= time_limit {
                info!("timeout reached during depth: {}", depth);
                return best_move;
            }
        }
        if depth_best > best {
            best = depth_best;
            best_move = depth_best_move;
        }
    }
    info!("best move: {}, score: {}", best_move, best);
    best_move
}

fn recursive_search(board: &mut Board, depth: u8, time_limit: Duration) -> i32 {
    if depth == 0 {
        return score(board);
    }
    let moves = board.gen_moves();
    let mut best = -9999;
    for mv in moves {
        if Instant::now().elapsed() >= time_limit {
            return best;
        }
        if !board.is_legal(&mv) {
            continue;
        }
        board.make_move(&mv);
        let score = -recursive_search(board, depth - 1, time_limit);
        board.unmake_move(&mv);
        if score > best {
            best = score;
        }
    }
    best
}
