use log::info;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::board::{Board, Move};
use crate::score::score;

const POS_INF: i32 = 9_999_999;
const NEG_INF: i32 = -POS_INF;
const MATE: i32 = 1_000_000;

#[derive(Debug)]
pub struct Search {
    board: Board,
    start_time: Instant,
    time_limit: Duration,
    stop: Arc<AtomicBool>,
}

impl Search {
    pub fn new(board: Board, time_limit: Duration, stop: Arc<AtomicBool>) -> Search {
        Search {
            board,
            start_time: Instant::now(),
            time_limit,
            stop,
        }
    }

    pub fn search(&mut self) -> Move {
        self.start_time = Instant::now();

        info!("starting search: {:?}", self);

        let moves: Vec<Move> = self
            .board
            .gen_moves()
            .into_iter()
            .filter(|m| self.board.is_legal(m))
            .collect();
        let mut best = NEG_INF;
        let mut best_move = Move::default();

        if moves.is_empty() {
            return best_move;
        }

        let mut canceled = false;
        for depth in 0..255 {
            if self.should_stop() {
                info!("search canceled at depth: {}", depth);
                break;
            }
            let mut depth_best = NEG_INF;
            let mut depth_best_move = Move::default();

            info!("searching depth: {}", depth);
            for mv in moves.as_slice() {
                self.board.make_move(mv);
                let res = self.nega_max(NEG_INF, POS_INF, depth, 0);
                info!(
                    "move: {}, score: {:?}, depth: {}, depth_best: {}, depth_best_move: {}",
                    mv, res, depth, depth_best, depth_best_move
                );
                self.board.unmake_move(mv);
                match res {
                    Some(score) => {
                        if -score > depth_best {
                            // info!("new best move: {}, score: {}, depth: {}", mv, -score, depth);
                            depth_best = -score;
                            depth_best_move = *mv;
                        }
                    }
                    None => {
                        // search was canceled
                        canceled = true;
                        break;
                    }
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

    fn nega_max(&mut self, alpha: i32, beta: i32, depth: u8, ply_from_root: u8) -> Option<i32> {
        if depth == 0 {
            if self.board.is_check() {
                return Some(-MATE + ply_from_root as i32);
            }
            return Some(score(&self.board));
        }
        let moves: Vec<Move> = self
            .board
            .gen_moves()
            .into_iter()
            .filter(|m| self.board.is_legal(m))
            .collect();
        if moves.is_empty() {
            if self.board.is_check() {
                return Some(-MATE + ply_from_root as i32);
            } else {
                return Some(0);
            }
        }
        let mut new_alpha = alpha;

        for mv in moves {
            if self.must_stop() {
                return None;
            }

            self.board.make_move(&mv);
            let res = self.nega_max(-beta, -new_alpha, depth - 1, ply_from_root + 1);
            self.board.unmake_move(&mv);

            match res {
                Some(score) => {
                    if -score >= beta {
                        info!("beta cutoff at depth: {}", depth);
                        return Some(beta); // fail hard beta-cutoff
                    }
                    if -score > new_alpha {
                        new_alpha = -score; // alpha acts like max in MiniMax
                    }
                }
                None => {
                    info!("search canceled at depth: {}", depth);
                    return None;
                }
            }
        }
        info!("returning alpha: {}, depth: {}", new_alpha, depth);
        Some(new_alpha)
    }

    fn time_remaining(&self) -> Duration {
        match self.time_limit.checked_sub(self.start_time.elapsed()) {
            Some(remaining) => remaining,
            None => Duration::from_secs(0),
        }
    }

    pub fn stop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    fn should_stop(&self) -> bool {
        self.stop.load(std::sync::atomic::Ordering::Relaxed)
            || (self.time_remaining() < self.time_limit / 3)
    }

    fn must_stop(&self) -> bool {
        self.stop.load(std::sync::atomic::Ordering::Relaxed)
            || (self.time_remaining() == Duration::from_secs(0))
    }
}
