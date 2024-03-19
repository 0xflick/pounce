use arrayvec::ArrayVec;
use log::info;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::board::{Board, Move, MoveList, MAX_MOVES};
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

        let mut moves: MoveList = self.board.gen_moves();
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

            for mv in MoveOrderer::new(&mut moves, best_move.as_ref()) {
                if !self.board.is_legal(&mv) {
                    continue;
                }
                self.board.make_move(&mv);
                let res = self.nega_max(NEG_INF, POS_INF, depth, 0);
                self.board.unmake_move(&mv);
                match res {
                    Some(score) => {
                        if -score > depth_best {
                            // info!("new best move: {}, score: {}, depth: {}", mv, -score, depth);
                            depth_best = -score;
                            depth_best_move = Some(mv);
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
        if self.board.is_stalemate() {
            return Some(0);
        }

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

        let mut moves: MoveList = self.board.gen_moves();
        if moves.is_empty() {
            if self.board.is_check() {
                return Some(-MATE + ply_from_root as i32);
            }
            return Some(0);
        }
        let mut score_type = ScoreType::Alpha;
        let mut search_best = best_move.to_owned();
        for mv in MoveOrderer::new(&mut moves, best_move.as_ref()) {
            if !self.board.is_legal(&mv) {
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

            self.board.make_move(&mv);
            let res = self.nega_max(-beta, -alpha, depth - 1, ply_from_root + 1);
            self.board.unmake_move(&mv);

            match res {
                Some(score) => {
                    if -score >= beta {
                        self.table.lock().unwrap().save(Entry {
                            z_key: self.board.z_hash,
                            best_move: Some(mv),
                            depth,
                            score: -score,
                            score_type: ScoreType::Beta,
                        });
                        return Some(beta); // fail hard beta-cutoff
                    }
                    if -score > alpha {
                        score_type = ScoreType::Exact;
                        search_best = Some(mv);
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
        if self.board.is_stalemate() {
            return Some(0);
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

type ScoreList = ArrayVec<i32, MAX_MOVES>;

const HASH_MOVE_SCORE: i32 = 1000;
const MVV_LVA_SCORE: i32 = 900;
const REST_SCORE: i32 = 0;

const MVV_LVA: [[i32; 6]; 6] = [
    [15, 25, 35, 45, 55, 0], // attacker pawn, victim P, N, B, R, Q,  K
    [14, 24, 34, 44, 54, 0], // attacker knight, victim P, N, B, R, Q,  K
    [13, 23, 33, 43, 53, 0], // attacker bishop, victim P, N, B, R, Q,  K
    [12, 22, 32, 42, 52, 0], // attacker rook, victim P, N, B, R, Q,  K
    [11, 21, 31, 41, 51, 0], // attacker queen, victim P, N, B, R, Q,  K
    [10, 20, 30, 40, 50, 0], // attacker king, victim P, N, B, R, Q,  K
];

struct MoveOrderer<'a> {
    moves: &'a mut MoveList,
    scores: ScoreList,
    idx: usize,
}

impl<'a> MoveOrderer<'a> {
    fn new(moves: &'a mut MoveList, hash_move: Option<&Move>) -> MoveOrderer<'a> {
        let mut scores: ScoreList = ArrayVec::new();

        for mv in moves.as_ref() {
            let score = if hash_move.is_some_and(|hm| hm == mv) {
                HASH_MOVE_SCORE
            } else if let Some(capture) = mv.capture {
                let attacker = mv.piece.kind() - 1;
                let victim = capture.kind() - 1;
                MVV_LVA_SCORE + MVV_LVA[attacker as usize][victim as usize]
            } else {
                REST_SCORE
            };
            scores.push(score);
        }

        MoveOrderer {
            moves,
            scores,
            idx: 0,
        }
    }
}

impl<'a> Iterator for MoveOrderer<'a> {
    type Item = Move;
    fn next(&mut self) -> Option<Self::Item> {
        let max_idx = self
            .scores
            .iter()
            .enumerate()
            .skip(self.idx)
            .max_by_key(|(_, s)| *s)
            .map(|(i, _)| i);

        if let Some(max_idx) = max_idx {
            self.moves.swap(self.idx, max_idx);
            self.scores.swap(self.idx, max_idx);
            self.idx += 1;
            Some(self.moves[self.idx - 1])
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Piece;

    #[test]
    fn test_move_orderer() {
        let mut moves = MoveList::default();
        moves.push({
            let mut m = Move::default();
            m.piece = Piece::WHITE_PAWN;
            m
        });
        moves.push({
            let mut m = Move::default();
            m.piece = Piece::WHITE_PAWN;
            m.capture = Some(Piece::BLACK_KNIGHT);
            m
        });
        moves.push({
            let mut m = Move::default();
            m.piece = Piece::WHITE_KNIGHT;
            m.capture = Some(Piece::BLACK_PAWN);
            m
        });
        moves.push({
            let mut m = Move::default();
            m.piece = Piece::BLACK_KING;
            m.capture = Some(Piece::BLACK_KING);
            m
        });

        let moves_copy = moves.clone();

        let mut orderer = MoveOrderer::new(&mut moves, moves_copy.get(3));
        assert_eq!(orderer.next(), Some(moves_copy[3]));
        assert_eq!(orderer.next(), Some(moves_copy[1]));
        assert_eq!(orderer.next(), Some(moves_copy[2]));
        assert_eq!(orderer.next(), Some(moves_copy[0]));
        assert_eq!(orderer.next(), None);
    }
}
