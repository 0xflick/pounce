use log::info;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::board::{Board, Move, MoveList, Piece, MAX_MOVES};
use crate::score::{score, MATE};
use crate::table::{Entry, ScoreType, Table};

const POS_INF: i32 = 9_999_999;
const NEG_INF: i32 = -POS_INF;

const MAX_DEPTH: usize = 64;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct StackFrame {
    ply: u8,
    killers: [Option<Move>; 2],
}

pub struct Search {
    board: Board,
    start_time: Instant,
    time_limit: Duration,
    table: Arc<Mutex<Table>>,
    stop: Arc<AtomicBool>,
    nodes: u64,
    score_nodes: u64,
    tt_cuts: u64,
    capture_cuts: u64,
    killer_cuts: u64,
    epoch: u16,
    stack: [StackFrame; MAX_DEPTH],
    silent: bool,
}

impl Search {
    pub fn new(
        board: Board,
        time_limit: Duration,
        stop: Arc<AtomicBool>,
        table: Arc<Mutex<Table>>,
    ) -> Search {
        let epoch = board.moves_played() as u16;
        let mut stack: [StackFrame; MAX_DEPTH] = [Default::default(); MAX_DEPTH];

        for (i, frame) in stack.iter_mut().enumerate() {
            frame.ply = i as u8;
        }

        Search {
            board,
            start_time: Instant::now(),
            time_limit,
            stop,
            table,
            nodes: 0,
            score_nodes: 0,
            tt_cuts: 0,
            capture_cuts: 0,
            killer_cuts: 0,
            epoch,
            stack,
            silent: false,
        }
    }

    pub fn set_silent(&mut self, silent: bool) {
        self.silent = silent;
    }

    pub fn search(&mut self) -> Option<Move> {
        self.start_time = Instant::now();

        let mut moves: MoveList = self.board.gen_moves();
        let mut best = NEG_INF;

        let mut best_move: Option<Move> = None;
        let mut depth_best_move: Option<Move> = None;
        let mut depth_best;

        info!("searching. time limit: {} ms", self.time_limit.as_millis());

        let mut canceled = false;
        let mut max_depth = 0;
        for depth in 0..200 {
            if self.should_stop() {
                info!("search canceled at depth: {}", depth);
                break;
            }
            let before_nodes = self.nodes;

            depth_best_move = None;
            depth_best = NEG_INF;

            info!("searching depth: {}", depth);
            let killers = self.stack[0].killers;
            for (_, mv) in MoveOrderer::new(
                &mut moves,
                best_move.as_ref(),
                [killers[0].as_ref(), killers[1].as_ref()],
            ) {
                if !self.board.is_legal(&mv) {
                    continue;
                }
                if depth_best_move.is_none() {
                    depth_best_move = Some(mv);
                }
                self.board.make_move(&mv);
                let extension = if self.board.is_check() { 1 } else { 0 };
                let res = self.nega_max(NEG_INF, -depth_best, depth + extension, extension, 0);
                self.board.unmake_move(&mv);
                match res {
                    Some(score) => {
                        if -score > depth_best {
                            info!("new best move: {}, score: {}, depth: {}", mv, -score, depth);
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
                let table = self.table.lock().unwrap();
                self.writeln(format!("info score cp {} nodes {} nps {} depth {} hashfull {} string hashhits {} string ttcuts {} string capturecuts {} string killercuts {}",
                        best, self.nodes,
                        (self.nodes as f64 / self.start_time.elapsed().as_secs_f64()) as usize,
                        depth,
                        table.per_mille_full(),
                        table.per_mille_hits(),
                        self.tt_cuts,
                        self.capture_cuts,
                        self.killer_cuts,
                    ));
                best = depth_best;
                best_move = depth_best_move;
                max_depth = depth;
            }
            let nodes_at_level = (self.nodes - before_nodes) as f64;
            self.writeln(format!(
                "info string ebf {}",
                nodes_at_level.powf(1.0 / (depth + 1) as f64)
            ));
        }
        let table = self.table.lock().unwrap();
        self.writeln(format!("info score cp {} nodes {} nps {} depth {} hashfull {} string hashhits {} string ttcuts {} string capturecuts {} string killercuts {} string score_nodes {}",
            best, self.nodes,
            (self.nodes as f64 / self.start_time.elapsed().as_secs_f64()) as usize,
            max_depth,
            table.per_mille_full(),
            table.per_mille_hits(),
            self.tt_cuts,
            self.capture_cuts,
            self.killer_cuts,
            self.score_nodes,
        ));
        info!(
            "search done. move {}",
            best_move.or(depth_best_move).unwrap_or_default()
        );
        best_move.or(depth_best_move)
    }

    fn nega_max(
        &mut self,
        mut alpha: i32,
        beta: i32,
        depth: u8,
        extensions: u8,
        ply_from_root: u8,
    ) -> Option<i32> {
        self.nodes += 1;
        if self.board.is_stalemate() {
            self.score_nodes += 1;
            return Some(0);
        }

        if depth == 0 {
            return self.quiesce(alpha, beta);
        }
        let mut best_move: Option<Move> = None;

        if let Some(hit) = self.table.lock().unwrap().probe(self.board.z_hash) {
            if hit.z_key == self.board.z_hash {
                if hit.depth >= depth {
                    match hit.score_type {
                        ScoreType::Exact => {
                            self.score_nodes += 1;
                            return Some(hit.score);
                        }
                        ScoreType::Alpha => {
                            if hit.score <= alpha {
                                self.score_nodes += 1;
                                return Some(alpha);
                            }
                        }
                        ScoreType::Beta => {
                            if hit.score >= beta {
                                self.score_nodes += 1;
                                return Some(beta);
                            }
                        }
                    }
                }

                if hit
                    .best_move
                    .is_some_and(|mv| mv.piece != Piece::NULL_PIECE)
                {
                    best_move = hit.best_move;
                }
            }
        }

        // null move
        if depth > 3 && !self.board.is_check() {
            self.board.make_move(&Move::NULL_MOVE);
            let res = self.nega_max(-beta, -alpha, depth - 1 - 3, ply_from_root + 1, extensions);
            self.board.unmake_move(&Move::NULL_MOVE);
            if res.is_some_and(|score| -score >= beta) {
                self.table.lock().unwrap().save(Entry {
                    z_key: self.board.z_hash,
                    best_move: Some(Move::NULL_MOVE),
                    depth,
                    score: -res.unwrap(),
                    score_type: ScoreType::Beta,
                    epoch: self.epoch,
                });
                return Some(beta);
            }
        }

        let mut moves: MoveList = self.board.gen_moves();

        let mut moved = false;
        let mut score_type = ScoreType::Alpha;
        let mut search_best = best_move.to_owned();
        let killers = self.stack[ply_from_root as usize].killers;
        for (stage, mv) in MoveOrderer::new(
            &mut moves,
            best_move.as_ref(),
            [killers[0].as_ref(), killers[1].as_ref()],
        ) {
            if !self.board.is_legal(&mv) {
                continue;
            }
            moved = true;
            if self.nodes % (1 << 17) == 0 {
                let table = self.table.lock().unwrap();
                self.writeln(format!(
                    "info nodes {} nps {} hashfull {} string hashhits {} string ttcuts {} string capturecuts {} string killercuts {}",
                    self.nodes,
                    (self.nodes as f64 / self.start_time.elapsed().as_secs_f64()) as usize,
                    table.per_mille_full(),
                    table.per_mille_hits(),
                    self.tt_cuts,
                    self.capture_cuts,
                    self.killer_cuts,
                ));
            }

            if self.nodes % (1 << 14) == 0 && self.must_stop() {
                return None;
            }

            self.board.make_move(&mv);
            let extension = if extensions < 16 && self.board.is_check() {
                1
            } else {
                0
            };
            let res = self.nega_max(
                -beta,
                -alpha,
                depth - 1 + extension,
                ply_from_root + 1,
                extensions + extension,
            );
            self.board.unmake_move(&mv);

            match res {
                Some(score) => {
                    if -score >= beta {
                        // fail hard beta-cutoff
                        self.table.lock().unwrap().save(Entry {
                            z_key: self.board.z_hash,
                            best_move: Some(mv),
                            depth,
                            score: -score,
                            score_type: ScoreType::Beta,
                            epoch: self.epoch,
                        });

                        match stage {
                            MoveStage::TT => self.tt_cuts += 1,
                            MoveStage::Capture => self.capture_cuts += 1,
                            MoveStage::Killer => self.killer_cuts += 1,
                            MoveStage::Quiet => {
                                self.update_killers(mv, ply_from_root);
                            }
                        }

                        return Some(beta);
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
        if !moved {
            self.score_nodes += 1;
            if self.board.is_check() {
                return Some(-MATE + ply_from_root as i32);
            }
            return Some(0);
        }
        self.table.lock().unwrap().save(Entry {
            z_key: self.board.z_hash,
            best_move: search_best,
            depth,
            score: alpha,
            score_type,
            epoch: self.epoch,
        });
        Some(alpha)
    }

    fn quiesce(&mut self, mut alpha: i32, beta: i32) -> Option<i32> {
        self.nodes += 1;
        if self.must_stop() {
            return None;
        }
        if self.board.is_stalemate() {
            self.score_nodes += 1;
            return Some(0);
        }
        let stand_pat = score(&self.board);
        if stand_pat >= beta {
            self.score_nodes += 1;
            return Some(beta);
        }
        if stand_pat > alpha {
            alpha = stand_pat;
        }
        let mut moves = self.board.gen_moves();
        let mut moved = false;
        for (_, mv) in MoveOrderer::new(&mut moves, None, [None, None]) {
            if mv.capture.is_none() || !self.board.is_legal(&mv) {
                continue;
            }
            moved = true;
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
        if !moved {
            self.score_nodes += 1;
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
            || (self.time_remaining() < self.time_limit / 6)
    }

    fn must_stop(&self) -> bool {
        self.stop.load(std::sync::atomic::Ordering::Relaxed)
            || (self.time_remaining() == Duration::from_secs(0))
    }

    fn update_killers(&mut self, mv: Move, ply: u8) {
        let killers = &mut self.stack[ply as usize].killers;
        if killers[0].is_none() || killers[0] == Some(mv) {
            killers[1] = killers[0];
            killers[0] = Some(mv);
        }
    }

    fn writeln(&self, s: String) {
        if !self.silent {
            println!("{}", s);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MoveStage {
    TT,
    Capture,
    Killer,
    Quiet,
}

const MVV_LVA: [[u16; 6]; 6] = [
    [15, 25, 35, 45, 55, 0], // attacker pawn, victim P, N, B, R, Q,  K
    [14, 24, 34, 44, 54, 0], // attacker knight, victim P, N, B, R, Q,  K
    [13, 23, 33, 43, 53, 0], // attacker bishop, victim P, N, B, R, Q,  K
    [12, 22, 32, 42, 52, 0], // attacker rook, victim P, N, B, R, Q,  K
    [11, 21, 31, 41, 51, 0], // attacker queen, victim P, N, B, R, Q,  K
    [10, 20, 30, 40, 50, 0], // attacker king, victim P, N, B, R, Q,  K
];

type MoveListWithScores<'a> = [Option<(&'a Move, u16)>; MAX_MOVES];

struct MoveOrderer<'a> {
    moves: MoveListWithScores<'a>,
    idx: usize,
    max_idx: usize,
    current_stage: MoveStage,
    hash_move: Option<&'a Move>,
    killers: [Option<&'a Move>; 2],
}

impl<'a> MoveOrderer<'a> {
    fn new(
        moves: &'a mut MoveList,
        hash_move: Option<&'a Move>,
        killers: [Option<&'a Move>; 2],
    ) -> MoveOrderer<'a> {
        let mut move_list = [None; MAX_MOVES];
        for (i, mv) in moves.iter().enumerate() {
            move_list[i] = Some((mv, 0));
        }

        MoveOrderer {
            moves: move_list,
            max_idx: moves.len(),
            hash_move,
            killers,
            idx: 0,
            current_stage: MoveStage::TT,
        }
    }

    fn score(&mut self) {
        for (mv, score) in self.moves.iter_mut().skip(self.idx).flatten() {
            if let Some(capture) = mv.capture {
                let attacker = mv.piece.kind() - 1;
                let victim = capture.kind() - 1;
                *score = MVV_LVA[attacker as usize][victim as usize];
            }
        }
    }

    fn sort(&mut self, begin: usize, end: usize) {
        self.moves[begin..end].sort_unstable_by(|a, b| b.unwrap().1.cmp(&a.unwrap().1))
    }

    fn select<F>(&mut self, f: F) -> Option<Move>
    where
        F: Fn(&Move) -> bool,
    {
        for i in self.idx..self.max_idx {
            if f(self.moves[i].unwrap().0) {
                self.moves.swap(self.idx, i);
                self.idx += 1;
                return Some(*self.moves[self.idx - 1].unwrap().0);
            }
        }
        None
    }
}

impl<'a> Iterator for MoveOrderer<'a> {
    type Item = (MoveStage, Move);
    fn next(&mut self) -> Option<Self::Item> {
        match self.current_stage {
            MoveStage::TT => {
                if let Some(hm) = self.hash_move {
                    if let Some(mv) = self.select(|mv| mv == hm) {
                        return Some((MoveStage::TT, mv));
                    }
                }
                self.current_stage = MoveStage::Capture;

                self.score();
                self.sort(self.idx, self.max_idx);
                self.next()
            }
            MoveStage::Capture => {
                let capture = self.select(|mv| mv.capture.is_some());
                if let Some(capture_mv) = capture {
                    return Some((MoveStage::Capture, capture_mv));
                }

                self.current_stage = MoveStage::Killer;
                self.next()
            }
            MoveStage::Killer => {
                let killers = self.killers;
                let killer = self.select(|mv| {
                    killers[0].is_some_and(|k| k == mv) || killers[1].is_some_and(|k| k == mv)
                });

                if let Some(killer_mv) = killer {
                    return Some((MoveStage::Killer, killer_mv));
                }

                self.current_stage = MoveStage::Quiet;
                self.next()
            }
            MoveStage::Quiet => self.select(|_| true).map(|mv| (MoveStage::Quiet, mv)),
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
        moves.push({
            let mut m = Move::default();
            m.piece = Piece::WHITE_QUEEN;
            m
        });
        moves.push({
            let mut m = Move::default();
            m.piece = Piece::BLACK_PAWN;
            m
        });

        let moves_copy = moves.clone();

        let mut orderer =
            MoveOrderer::new(&mut moves, moves_copy.get(3), [moves_copy.get(5), None]);

        assert_eq!(orderer.next(), Some((MoveStage::TT, (moves_copy[3]))));
        assert_eq!(orderer.next(), Some((MoveStage::Capture, moves_copy[1])));
        assert_eq!(orderer.next(), Some((MoveStage::Capture, moves_copy[2])));
        assert_eq!(orderer.next(), Some((MoveStage::Killer, moves_copy[5])));
        assert_eq!(orderer.next(), Some((MoveStage::Quiet, moves_copy[4])));
        assert_eq!(orderer.next(), Some((MoveStage::Quiet, moves_copy[0])));
        assert_eq!(orderer.next(), None);
    }
}
