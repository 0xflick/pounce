use log::info;
use std::slice::Iter;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::board::{Board, Move, MoveList};
use crate::score::{score, MATE};
use crate::table::{Entry, ScoreType, Table};

const POS_INF: i32 = 9_999_999;
const NEG_INF: i32 = -POS_INF;

pub struct Search {
    board: Board,
    start_time: Instant,
    time_limit: Duration,
    table: Arc<Mutex<Table>>,
    stop: Arc<AtomicBool>,
    nodes: u64,
}

impl Search {
    pub fn new(
        board: Board,
        time_limit: Duration,
        stop: Arc<AtomicBool>,
        table: Arc<Mutex<Table>>,
    ) -> Search {
        Search {
            board,
            start_time: Instant::now(),
            time_limit,
            stop,
            table,
            nodes: 0,
        }
    }

    pub fn search(&mut self) -> Move {
        self.start_time = Instant::now();

        let moves: MoveList = self.board.gen_moves();
        let mut best = NEG_INF;

        if moves.is_empty() {
            return Move::default();
        }
        let mut best_move: Option<Move> = None;
        let mut depth_best_move: Option<Move> = None;
        let mut depth_best;

        let mut canceled = false;
        for depth in 0..200 {
            if self.should_stop() {
                info!("search canceled at depth: {}", depth);
                break;
            }

            depth_best_move = None;
            depth_best = NEG_INF;

            info!("searching depth: {}", depth);

            for mv in MoveOrderer::new(&moves, best_move.as_ref()) {
                if !self.board.is_legal(mv) {
                    continue;
                }
                self.board.make_move(mv);
                let res = self.nega_max(NEG_INF, POS_INF, depth, 0);
                self.board.unmake_move(mv);
                match res {
                    Some(score) => {
                        if -score > depth_best {
                            // info!("new best move: {}, score: {}, depth: {}", mv, -score, depth);
                            depth_best = -score;
                            depth_best_move = Some(*mv);
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
            if depth_best_move.is_some() {
                println!(
                    "info score cp {} depth {} nodes {}",
                    depth_best, depth, self.nodes
                );
                best = depth_best;
                best_move = depth_best_move;
            }
        }
        println!("info score cp {} nodes {}", best, self.nodes);
        best_move.unwrap_or(depth_best_move.unwrap_or(moves[0]))
    }

    fn nega_max(&mut self, mut alpha: i32, beta: i32, depth: u8, ply_from_root: u8) -> Option<i32> {
        if depth == 0 {
            self.nodes += 1;
            return self.quiesce(alpha, beta);
        }

        let mut best_move: Option<Move> = None;

        if let Some(hit) = self.table.lock().unwrap().probe(self.board.z_hash) {
            if hit.z_key == self.board.z_hash {
                if hit.depth >= depth {
                    match hit.score_type {
                        ScoreType::Exact => {
                            return Some(hit.score);
                        }
                        ScoreType::Alpha => {
                            if hit.score <= alpha {
                                return Some(alpha);
                            }
                        }
                        ScoreType::Beta => {
                            if hit.score >= beta {
                                return Some(beta);
                            }
                        }
                    }
                }

                best_move = hit.best_move;
            }
        }

        let moves: MoveList = self.board.gen_moves();
        if moves.is_empty() {
            if self.board.is_check() {
                return Some(-MATE + ply_from_root as i32);
            }
            return Some(0);
        }
        let mut score_type = ScoreType::Alpha;
        let mut search_best = best_move.to_owned();
        for mv in MoveOrderer::new(&moves, best_move.as_ref()) {
            if !self.board.is_legal(mv) {
                continue;
            }
            if self.nodes % (1 << 16) == 0 {
                let table = self.table.lock().unwrap();
                println!(
                    "info nodes {} nps {} hashfull {} string hashhits {}",
                    self.nodes,
                    (self.nodes as f64 / self.start_time.elapsed().as_secs_f64()) as usize,
                    table.per_mille_full(),
                    table.per_mille_hits()
                );
            }

            if self.nodes % (1 << 14) == 0 && self.must_stop() {
                return None;
            }

            self.board.make_move(mv);
            let res = self.nega_max(-beta, -alpha, depth - 1, ply_from_root + 1);
            self.board.unmake_move(mv);

            match res {
                Some(score) => {
                    if -score >= beta {
                        self.table.lock().unwrap().save(Entry {
                            z_key: self.board.z_hash,
                            best_move: Some(*mv),
                            depth,
                            score: -score,
                            score_type: ScoreType::Beta,
                        });
                        return Some(beta); // fail hard beta-cutoff
                    }
                    if -score > alpha {
                        score_type = ScoreType::Exact;
                        search_best = Some(*mv);
                        alpha = -score; // alpha acts like max in MiniMax
                    }
                }
                None => {
                    return None;
                }
            }
        }
        self.table.lock().unwrap().save(Entry {
            z_key: self.board.z_hash,
            best_move: search_best,
            depth,
            score: alpha,
            score_type,
        });
        Some(alpha)
    }

    fn quiesce(&mut self, mut alpha: i32, beta: i32) -> Option<i32> {
        if self.must_stop() {
            return None;
        }
        let stand_pat = score(&self.board);
        if stand_pat >= beta {
            return Some(beta);
        }
        if stand_pat > alpha {
            alpha = stand_pat;
        }
        let moves: Vec<Move> = self
            .board
            .gen_moves()
            .into_iter()
            .filter(|m| m.capture.is_some())
            .collect();
        for mv in moves {
            if !self.board.is_legal(&mv) {
                continue;
            }
            self.board.make_move(&mv);
            let res = self.quiesce(-beta, -alpha);
            self.board.unmake_move(&mv);
            match res {
                Some(score) => {
                    if -score >= beta {
                        return Some(beta);
                    }
                    if -score > alpha {
                        alpha = -score;
                    }
                }
                None => {
                    return None;
                }
            }
        }
        Some(alpha)
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

enum MoveOrderStage {
    HashMove,
    Rest,
}

struct MoveOrderer<'a> {
    moves: Iter<'a, Move>,
    hash_move: Option<&'a Move>,
    stage: MoveOrderStage,
}

impl<'a> MoveOrderer<'a> {
    fn new(moves: &'a MoveList, hash_move: Option<&'a Move>) -> MoveOrderer<'a> {
        MoveOrderer {
            moves: moves.into_iter(),
            hash_move,
            stage: MoveOrderStage::HashMove,
        }
    }
}

impl<'a> Iterator for MoveOrderer<'a> {
    type Item = &'a Move;
    fn next(&mut self) -> Option<Self::Item> {
        match self.stage {
            MoveOrderStage::HashMove => {
                self.stage = MoveOrderStage::Rest;
                if let Some(hash_move) = self.hash_move {
                    return Some(hash_move);
                }
                self.stage = MoveOrderStage::Rest;
                self.next()
            }
            MoveOrderStage::Rest => {
                if let Some(hash_move) = self.hash_move {
                    for mv in &mut self.moves {
                        if mv != hash_move {
                            return Some(mv);
                        }
                    }
                }
                self.moves.next()
            }
        }
    }
}
