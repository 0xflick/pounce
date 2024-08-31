use std::{
    sync::{atomic::AtomicBool, Arc},
    time::{Duration, Instant},
};

use arrayvec::ArrayVec;

use crate::{
    chess::{Color, GameResult, Square},
    eval,
    limits::Limits,
    movepicker::{MovePicker, MAX_MOVES},
    moves::Move,
    position::Position,
    tt::{Entry, EntryType, Table},
};

pub struct SearchCop {
    pub depth: Option<u8>,
    pub nodes: Option<u64>,
    pub time: Duration,
}

const MAX_DEPTH: u8 = 64;
const MAX_PLY: u8 = 128;

static mut REDUCTIONS: [[u8; MAX_MOVES]; MAX_DEPTH as usize] = [[0; MAX_MOVES]; MAX_DEPTH as usize];

pub fn init_reductions() {
    unsafe {
        #[allow(clippy::needless_range_loop)]
        for m in 1..MAX_MOVES {
            for depth in 1..MAX_DEPTH as usize {
                let reduction = 1. + ((depth as f32).ln() * (m as f32).ln()) / 2.;
                REDUCTIONS[depth][m] = reduction as u8;
            }
        }
    }
}

impl SearchCop {
    pub fn new(
        depth: Option<u8>,
        nodes: Option<u64>,
        time_remaining: Option<i32>,
        inc: Option<u32>,
        movestogo: Option<u32>,
    ) -> Self {
        let time = match time_remaining {
            Some(time_remaining) => {
                let overhead = 10;
                let mtg = movestogo.unwrap_or(20);
                let total = time_remaining as u32 + (mtg - 1) * inc.unwrap_or(0) - mtg * overhead;

                let mut goal = total / mtg;
                if goal > time_remaining as u32 {
                    goal = time_remaining as u32 / 2
                }

                Duration::from_millis(goal as u64)
            }
            None => Duration::MAX,
        };

        SearchCop { depth, nodes, time }
    }
}

pub struct Search {
    position: Position,
    limits: SearchCop,
    tt: Arc<Table>,

    pv: [[Move; MAX_PLY as usize]; MAX_PLY as usize],
    pv_length: [u8; MAX_PLY as usize],
    killers: [[Move; 2]; MAX_PLY as usize],
    current_move: [Move; MAX_PLY as usize],
    history: [[[i16; Square::NUM]; Square::NUM]; Color::NUM],
    start_time: Instant,
    stop: Arc<AtomicBool>,
    silent: bool,

    pub nodes: u64,
}

impl Search {
    pub fn new(position: Position, limits: Limits, tt: Arc<Table>, stop: Arc<AtomicBool>) -> Self {
        let (time_left, inc) = match position.side {
            Color::White => (limits.wtime, limits.winc),
            Color::Black => (limits.btime, limits.binc),
        };

        Search {
            position,
            limits: SearchCop::new(limits.depth, limits.nodes, time_left, inc, limits.movestogo),
            tt,
            pv: [[Move::NONE; MAX_PLY as usize]; MAX_PLY as usize],
            pv_length: [0; MAX_PLY as usize],
            killers: [[Move::NONE; 2]; MAX_PLY as usize],
            current_move: [Move::NONE; MAX_PLY as usize],
            history: [[[0; Square::NUM]; Square::NUM]; Color::NUM],
            start_time: Instant::now(),
            stop,
            silent: false,
            nodes: 0,
        }
    }

    pub fn think(&mut self) -> Move {
        self.start_time = Instant::now();

        self.iterative_deepening().expect("No best move found")
    }

    fn iterative_deepening(&mut self) -> Option<Move> {
        let max_depth = self.limits.depth.unwrap_or(MAX_DEPTH) as i32;
        let mut best_move = None;
        let mut score = 0;

        let mut done_early = false;

        for depth in 1..=max_depth {
            if self.done_thinking() {
                done_early = true;
                break;
            }

            best_move = Some(self.pv[0][0]);
            if depth > 1 {
                self.uci_info(depth - 1, score);
            }

            score = self.aspiration(depth, score);
        }

        if max_depth == 1 {
            best_move = Some(self.pv[0][0]);
        }

        if !done_early {
            self.uci_info(max_depth, score);
        }

        best_move
    }

    fn aspiration(&mut self, depth: i32, prev: i16) -> i16 {
        let mut delta = 50;
        let (mut alpha, mut beta) = if depth > 6 {
            (prev - delta, prev + delta)
        } else {
            (-eval::INFINITY, eval::INFINITY)
        };

        loop {
            if self.done_thinking() {
                return 0;
            }

            let score = self.search(depth, alpha, beta, 0, true, true);

            if score <= alpha {
                beta = (alpha + beta) / 2;
                alpha = (-eval::INFINITY).max(alpha - delta);
            } else if score >= beta {
                beta = (eval::INFINITY).min(beta + delta);
            } else {
                return score;
            }

            delta += delta / 2;
            if delta > 1000 {
                alpha = -eval::INFINITY;
                beta = eval::INFINITY;
            }
        }
    }

    fn search(
        &mut self,
        mut depth: i32,
        mut alpha: i16,
        mut beta: i16,
        ply: u8,
        is_pv: bool,
        is_root: bool,
    ) -> i16 {
        if self.done_thinking() {
            return 0;
        }
        if depth >= MAX_DEPTH as i32 || ply >= MAX_PLY {
            return self.position.eval();
        }
        self.nodes += 1;

        self.pv_length[ply as usize] = ply;

        debug_assert!(alpha < beta);
        debug_assert_eq!(self.position.key, self.position.zobrist_hash());

        if !is_root {
            match self.position.is_draw() {
                Some(GameResult::Draw) => return eval::DRAW,
                // test if this is between alpha and beta?
                Some(GameResult::Loss) => return -eval::MATE + ply as i16,
                _ => {}
            }

            let repetition_count = if is_pv { 2 } else { 1 };
            if self.position.is_repetition(repetition_count) {
                return eval::DRAW;
            }
        }

        if self.position.in_check() {
            depth += 1;
            if depth >= MAX_DEPTH as i32 {
                return self.position.eval();
            }
        }

        // Go to quiescence search if depth is 0
        if depth <= 0 {
            return self.quiescence_search(alpha, beta, is_pv);
        }

        // Probe the transposition table
        let mut tt_eval = None;
        let mut tt_move = Move::NONE;
        if let Some(entry) = self.tt.probe(self.position.key) {
            tt_move = entry.best_move;
            tt_eval = Some(entry.score);
            if entry.depth as i32 >= depth
                && !is_pv
                && self.current_move[ply as usize - 1] != Move::NULL
            {
                match entry.score_type {
                    // Exact score
                    EntryType::Exact => return entry.score,
                    // Lower bound
                    EntryType::LowerBound => alpha = alpha.max(entry.score),
                    // Upper bound
                    EntryType::UpperBound => beta = beta.min(entry.score),
                    EntryType::None => {}
                }
                if alpha >= beta {
                    return entry.score;
                }
            }
        }

        let static_eval = tt_eval.unwrap_or(self.position.eval());

        // internal iterative reduction
        if !is_root && depth >= 6 && !self.position.in_check() && tt_move == Move::NONE {
            depth -= 1;
        }

        // Null move pruning
        if !is_pv
            && depth >= 3
            && self.position.non_pawn_material(self.position.side)
            && !self.position.in_check()
            && static_eval >= beta
            && (ply < 1 || self.current_move[(ply - 1) as usize] != Move::NULL)
        {
            self.position.make_null_move();
            self.current_move[ply as usize] = Move::NULL;

            let reduced_depth = depth - (3 + (depth / 5));
            let null_score = -self.search(reduced_depth, -beta, -beta + 1, ply + 1, false, false);

            self.position.unmake_null_move();
            self.current_move[ply as usize] = Move::NONE;

            if null_score >= beta {
                if null_score >= (eval::MATE - MAX_PLY as i16) {
                    return beta;
                }
                return null_score;
            }
        }

        // Reverse futility pruning
        if !is_pv
            && (-31_000..31_000).contains(&beta)
            && !self.position.in_check()
            && depth < 7
            && (static_eval - 300 * depth as i16) >= beta
        {
            return static_eval - 300 * depth as i16;
        }

        let mut best_move = Move::NONE;
        let mut best = -eval::INFINITY;
        let mut move_count = 0;
        let mut quiets: ArrayVec<Move, 64> = ArrayVec::new();

        let mut move_picker =
            MovePicker::new_ab_search(&self.position, tt_move, self.killers[ply as usize]);
        while let Some(mv) = move_picker.next(&self.position, &self.history) {
            move_count += 1;
            let capture = (self.position.occupancy & mv.to()).any();

            self.position.make_move(mv);
            self.current_move[ply as usize] = mv;

            let mut score = -eval::INFINITY;

            // LMR
            let needs_full_search = if depth >= 3 && !self.position.in_check() && move_count > 4 {
                let reduction = self.reduction(depth, move_count);
                let mut rdepth = (depth - 1 - reduction).clamp(1, depth - 2);

                // Reduce less in PV nodes
                if is_pv {
                    rdepth += 1;
                }

                // reduce more in non-capture moves
                if move_count > 15 && !capture {
                    rdepth -= 1;
                }

                score = -self.search(rdepth, -alpha - 1, -alpha, ply + 1, false, false);

                score > alpha && rdepth < depth - 1
            } else {
                move_count > 1 || !is_pv
            };

            if needs_full_search {
                score = -self.search(depth - 1, -alpha - 1, -alpha, ply + 1, false, false);
            }

            if is_pv && (move_count == 1 || score > alpha && score < beta) {
                score = -self.search(depth - 1, -beta, -alpha, ply + 1, true, false);
            }

            self.position.unmake_move(mv);
            self.current_move[ply as usize] = Move::NONE;

            if score > best {
                best = score;
                best_move = mv;

                self.pv[ply as usize][ply as usize] = mv;
                for j in (ply + 1)..self.pv_length[ply as usize + 1] {
                    self.pv[ply as usize][j as usize] = self.pv[ply as usize + 1][j as usize];
                }

                self.pv_length[ply as usize] = self.pv_length[ply as usize + 1];

                if score > alpha {
                    alpha = score;
                    if score >= beta {
                        if !capture {
                            self.update_killers(mv, ply);
                            let bonus = 2000.min(350 * depth as i16 - 350);
                            self.update_history(mv, bonus);

                            for quiet in quiets.iter() {
                                self.update_history(*quiet, -bonus / 2);
                            }
                        }

                        break;
                    }
                }
            }

            if !capture && quiets.len() < quiets.capacity() {
                quiets.push(mv);
            }
        }

        if move_count == 0 {
            if self.position.in_check() {
                return -eval::MATE + ply as i16;
            } else {
                return 0;
            }
        }

        let entry_type = if best >= beta {
            EntryType::LowerBound
        } else if is_pv && best_move != Move::NULL {
            EntryType::Exact
        } else {
            EntryType::LowerBound
        };

        if !self.stop.load(std::sync::atomic::Ordering::Relaxed) {
            self.tt.set(Entry::new(
                self.position.key,
                depth as u8,
                best,
                entry_type,
                best_move,
            ));
        }
        best
    }

    fn quiescence_search(&mut self, mut alpha: i16, beta: i16, is_pv: bool) -> i16 {
        self.nodes += 1;

        if self.done_thinking() {
            return 0;
        }

        match self.position.is_draw() {
            Some(GameResult::Draw) => return eval::DRAW,
            // don't have ply here so this is a guess
            Some(GameResult::Loss) => return -eval::MATE + MAX_PLY as i16,
            _ => {}
        }

        let repetition_count = if is_pv { 2 } else { 1 };
        if self.position.is_repetition(repetition_count) {
            return eval::DRAW;
        }

        // Probe tt
        let mut tt_move = Move::NONE;
        if let Some(entry) = self.tt.probe(self.position.key) {
            tt_move = entry.best_move;
            if !is_pv {
                match entry.score_type {
                    EntryType::Exact => return entry.score,
                    EntryType::LowerBound => {
                        if entry.score >= beta {
                            return entry.score;
                        }
                    }
                    EntryType::UpperBound => {
                        if entry.score <= alpha {
                            return entry.score;
                        }
                    }
                    _ => {}
                }
            }
        }

        let stand_pat = self.position.eval();
        if stand_pat >= beta {
            return stand_pat;
        }

        if stand_pat > alpha {
            alpha = stand_pat;
        }

        let mut best = stand_pat;
        let mut best_move = Move::NONE;

        let mut move_picker = MovePicker::new_quiescence(&self.position, tt_move);
        while let Some(mv) = move_picker.next(&self.position, &self.history) {
            // delta pruning
            let captured = self.position.role_at(mv.to()).unwrap();
            if mv.promotion().is_none()
                && !self.position.in_check()
                && ((stand_pat + 500 + eval::PIECE_VALUES_EG[captured] as i16) < alpha)
                && self.position.non_pawn_material(self.position.side)
            {
                continue;
            }

            self.position.make_move(mv);
            let score = -self.quiescence_search(-beta, -alpha, is_pv);
            self.position.unmake_move(mv);

            if score > best {
                best = score;
                best_move = mv;
                if score > alpha {
                    alpha = score;
                    if score >= beta {
                        break;
                    }
                }
            }
        }

        let entry_type = if best >= beta {
            EntryType::LowerBound
        } else {
            EntryType::UpperBound
        };

        if !self.stop.load(std::sync::atomic::Ordering::Relaxed) {
            self.tt.set(Entry::new(
                self.position.key,
                0,
                best,
                entry_type,
                best_move,
            ));
        }

        best
    }

    pub fn update_killers(&mut self, mv: Move, ply: u8) {
        self.killers[ply as usize][1] = self.killers[ply as usize][0];
        self.killers[ply as usize][0] = mv;
    }

    fn update_history(&mut self, mv: Move, bonus: i16) {
        self.history[self.position.side][mv.from()][mv.to()] += bonus
            - ((self.history[self.position.side][mv.from()][mv.to()] as i32 * bonus.abs() as i32)
                / 16384) as i16;
    }

    fn reduction(&self, depth: i32, move_count: u8) -> i32 {
        unsafe { REDUCTIONS[depth as usize][move_count as usize] as i32 }
    }

    pub fn done_thinking(&self) -> bool {
        if self.stop.load(std::sync::atomic::Ordering::Relaxed)
            || self.limits.nodes.is_some_and(|n| self.nodes >= n)
        {
            return true;
        }

        if self.nodes % 2048 == 0 && self.start_time.elapsed() >= self.limits.time {
            self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
            return true;
        }

        false
    }

    pub fn set_silent(&mut self, silent: bool) {
        self.silent = silent;
    }

    fn uci_info(&self, depth: i32, score: i16) {
        if self.silent {
            return;
        }

        let elapsed = self.start_time.elapsed().as_millis() + 1;
        let nps = (self.nodes as u128 * 1000) / elapsed;
        let pv = (0..self.pv_length[0])
            .map(|i| self.pv[0][i as usize].to_string())
            .collect::<Vec<String>>()
            .join(" ");
        if score.abs() > eval::MATE - MAX_PLY as i16 {
            let ply = score.signum() * (eval::MATE - score.abs()) / 2;

            println!(
                "info depth {} score mate {} time {} nodes {} nps {} hashfull {} pv {}",
                depth,
                ply,
                elapsed,
                self.nodes,
                nps,
                self.tt.hashfull(),
                pv
            );
        } else {
            println!(
                "info depth {} score cp {} time {} nodes {} nps {}, hashfull {} pv {}",
                depth,
                score,
                elapsed,
                self.nodes,
                nps,
                self.tt.hashfull(),
                pv
            );
        }
    }
}
