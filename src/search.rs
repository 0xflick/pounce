use log::info;
use std::time::{Duration, Instant};

use crate::board::{Board, Move};
use crate::score::score;

const POS_INF: i32 = 9_999_999;
const NEG_INF: i32 = -POS_INF;
const MATE: i32 = 1_000_000;

pub fn search(board: &mut Board, time_limit: Duration) -> Move {
    let moves: Vec<Move> = board
        .gen_moves()
        .into_iter()
        .filter(|m| board.is_legal(m))
        .collect();
    let mut best = NEG_INF;
    let mut best_move = Move::default();

    if moves.is_empty() {
        return best_move;
    }

    let start_time = Instant::now();
    let mut canceled = false;
    for depth in 0..255 {
        let mut depth_best = NEG_INF;
        let mut depth_best_move = Move::default();

        info!("searching depth: {}", depth);
        for mv in moves.as_slice() {
            if start_time.elapsed() >= time_limit {
                info!("timeout reached at depth: {}", depth);
                break;
            }
            board.make_move(mv);
            let score = -nega_max(NEG_INF, POS_INF, board, depth, 0, time_limit);
            board.unmake_move(mv);
            if score > depth_best {
                info!("new best move: {}, score: {}, depth: {}", mv, score, depth);
                depth_best = score;
                depth_best_move = *mv;
            }
            if start_time.elapsed() >= time_limit {
                info!(
                    "timeout reached during depth: {}. returning {}",
                    depth, best_move
                );
                canceled = true;
                break;
            }
        }
        if canceled {
            break;
        }

        info!("depth: {}, best: {}", depth, depth_best);
        if depth_best_move != Move::default() {
            best = depth_best;
            best_move = depth_best_move;
        }
    }
    info!("best move: {}, score: {}", best_move, best);
    best_move
}

fn nega_max(
    alpha: i32,
    beta: i32,
    board: &mut Board,
    depth: u8,
    ply_from_root: u8,
    time_limit: Duration,
) -> i32 {
    if depth == 0 {
        return score(board);
    }
    let moves: Vec<Move> = board
        .gen_moves()
        .into_iter()
        .filter(|m| board.is_legal(m))
        .collect();
    if moves.is_empty() {
        if board.is_check() {
            return -MATE + ply_from_root as i32;
        } else {
            return 0;
        }
    }
    let mut new_alpha = alpha;

    for mv in moves {
        if Instant::now().elapsed() >= time_limit {
            return 0;
        }
        board.make_move(&mv);
        let score = -nega_max(
            -beta,
            -new_alpha,
            board,
            depth - 1,
            ply_from_root + 1,
            time_limit,
        );
        board.unmake_move(&mv);
        if score >= beta {
            return beta; // fail hard beta-cutoff
        }
        if score > alpha {
            new_alpha = score; // alpha acts like max in MiniMax
        }
    }
    // info!("returning alpha: {}, depth: {}", new_alpha, depth);
    new_alpha
}
