use std::sync::atomic::AtomicBool;
use std::time::Instant;

use arrayvec::ArrayVec;

use crate::chess::{Accumulator, Color, GameResult, Move, Position, Role, Square};
use crate::engine::eval::{self, nnue};
use crate::engine::limits::Limits;
use crate::engine::movepicker::{MAX_MOVES, MovePicker};
use crate::engine::time_management::SearchCop;
use crate::engine::tt::{Entry, EntryType, Table};

const MAX_DEPTH: u8 = 64;
pub const MAX_PLY: u8 = 128;

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
#[derive(Debug, Clone, Copy)]
pub struct SearchResult {
    pub bestmove: Move,
    pub score: i16,
    pub depth: i32,
    pub done_early: bool,
}

pub struct Stats {
    pub nodes: u64,
    pub effort: [[u64; Square::NUM]; Square::NUM],
    pub pv: [[Move; MAX_PLY as usize]; MAX_PLY as usize],
    pub pv_length: [u8; MAX_PLY as usize],
    pub start_time: Instant,
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            nodes: 0,
            effort: [[0; Square::NUM]; Square::NUM],
            pv: [[Move::NONE; MAX_PLY as usize]; MAX_PLY as usize],
            pv_length: [0; MAX_PLY as usize],
            start_time: Instant::now(),
        }
    }
}

impl Stats {
    fn reset(&mut self) {
        self.nodes = 0;
        self.effort = [[0; Square::NUM]; Square::NUM];
        self.pv = [[Move::NONE; MAX_PLY as usize]; MAX_PLY as usize];
        self.pv_length = [0; MAX_PLY as usize];
        self.start_time = Instant::now();
    }

    fn uci_info(&self, depth: i32, score: i16, hashfull: f64) {
        let elapsed = self.start_time.elapsed().as_millis() + 1;
        let nps = (self.nodes as u128 * 1000) / elapsed;
        let pv = (0..self.pv_length[0])
            .map(|i| self.pv[0][i as usize].to_string())
            .collect::<Vec<String>>()
            .join(" ");
        if score.abs() > eval::MATE_IN_PLY {
            let ply = score.signum() * (1 + eval::MATE - score.abs()) / 2;

            println!(
                "info depth {} score mate {} time {} nodes {} nps {} hashfull {} pv {}",
                depth, ply, elapsed, self.nodes, nps, hashfull, pv
            );
        } else {
            println!(
                "info depth {} score cp {} time {} nodes {} nps {}, hashfull {} pv {}",
                depth, score, elapsed, self.nodes, nps, hashfull, pv
            );
        }
    }

    pub fn uci_info_done_early(&self, mv: Move, depth: i32, score: i16, hashfull: f64) {
        let elapsed = self.start_time.elapsed().as_millis() + 1;
        let nps = (self.nodes as u128 * 1000) / elapsed;
        if score.abs() > eval::MATE_IN_PLY {
            let ply = score.signum() * (1 + eval::MATE - score.abs()) / 2;

            println!(
                "info depth {} score mate {} time {} nodes {} nps {} hashfull {} pv {}",
                depth, ply, elapsed, self.nodes, nps, hashfull, mv
            );
        } else {
            println!(
                "info depth {} score cp {} time {} nodes {} nps {}, hashfull {} pv {}",
                depth, score, elapsed, self.nodes, nps, hashfull, mv
            );
        }
    }
}

pub struct Search<'a> {
    pub stats: Stats,

    position: Position,
    accum: eval::nnue::NNUEAccumulator<'a, { eval::nnue::NNUE_HIDDEN_SIZE }>,

    current_move: [Move; MAX_PLY as usize],
    history: [[[i16; Square::NUM]; Square::NUM]; Color::NUM],
    killers: [[Move; 2]; MAX_PLY as usize],
    tt: &'a Table,

    tm: SearchCop,

    stop: &'a AtomicBool,

    silent: bool,
    thread_idx: usize,
}

impl<'a> Search<'a> {
    pub fn new(
        position: Position,
        limits: Limits,
        tt: &'a Table,
        stop: &'a AtomicBool,
        thread_idx: usize,
        silent: bool,
        net: &'a nnue::PerspectiveNet<{ nnue::NNUE_HIDDEN_SIZE }>,
    ) -> Self {
        let side = position.side;
        let mut accum = eval::nnue::NNUEAccumulator::new(net);
        accum.reset(&position);
        Search {
            accum,
            current_move: [Move::NONE; MAX_PLY as usize],
            history: [[[0; Square::NUM]; Square::NUM]; Color::NUM],
            killers: [[Move::NONE; 2]; MAX_PLY as usize],
            tm: SearchCop::new(limits, side),
            position,
            silent,
            stats: Stats::default(),
            stop,
            thread_idx,
            tt,
        }
    }

    pub fn think(&mut self) -> SearchResult {
        self.stats.reset();
        self.iterative_deepening()
    }

    fn iterative_deepening(&mut self) -> SearchResult {
        let max_depth = self.tm.depth.unwrap_or(MAX_DEPTH) as i32;
        let mut bestmove = Move::NONE;
        let mut score = 0;
        let mut depth_reached = 0;
        let mut done_early = false;

        for depth in 1..=max_depth {
            if self.done_thinking() {
                break;
            }

            let depth_score = self.aspiration(depth, score);

            if self.done_thinking() {
                done_early = true;
                break;
            }

            score = depth_score;
            bestmove = self.stats.pv[0][0];
            depth_reached = depth;

            if !self.silent && self.thread_idx == 0 {
                self.stats.uci_info(depth, score, self.tt.hashfull());
            }

            self.tm.adjust(&self.stats);
            if self.tm.time_up_deepening(&self.stats) {
                break;
            }
        }

        if bestmove == Move::NONE {
            bestmove = self.stats.pv[0][0];
        }

        SearchResult {
            bestmove,
            score,
            depth: depth_reached,
            done_early,
        }
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
                beta = alpha + (beta - alpha) / 2;
                alpha = (-eval::INFINITY).max(alpha.saturating_sub(delta));
            } else if score >= beta {
                beta = (eval::INFINITY).min(beta.saturating_add(delta));
            } else {
                return score;
            }

            delta = delta.saturating_add(delta / 2);
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
        beta: i16,
        ply: u8,
        is_pv: bool,
        is_root: bool,
    ) -> i16 {
        if self.done_thinking() {
            return 0;
        }
        if depth >= MAX_DEPTH as i32 || ply >= MAX_PLY {
            return eval::score_nnue(&self.position, &self.accum);
        }

        self.stats.pv_length[ply as usize] = ply;

        if depth <= 0 {
            return self.quiescence_search(alpha, beta, is_pv);
        }

        // Go to quiescence search if depth is 0
        self.stats.nodes += 1;

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

        let tt_hit = if let Some(hit) = self.tt.probe(self.position.key) {
            if !is_pv
                && self.position.halfmove_clock < 80
                && hit.depth as i32 >= depth
                && (hit.score_type == EntryType::Exact
                    || (hit.score_type == EntryType::LowerBound
                        && denormalize_score(hit.score, ply) >= beta)
                    || (hit.score_type == EntryType::UpperBound
                        && denormalize_score(hit.score, ply) <= alpha))
            {
                return denormalize_score(hit.score, ply);
            }
            Some(hit)
        } else {
            None
        };

        let static_eval;
        if self.position.in_check() {
            static_eval = eval::NO_VALUE;
        } else if let Some(Entry {
            score: tt_score,
            static_eval: tt_static_eval,
            ..
        }) = &tt_hit
        {
            if *tt_score != eval::NO_VALUE {
                static_eval = denormalize_score(*tt_score, ply);
            } else if *tt_static_eval != eval::NO_VALUE {
                static_eval = *tt_static_eval;
            } else {
                static_eval = eval::NO_VALUE;
            }
        } else {
            static_eval = eval::score_nnue(&self.position, &self.accum);

            self.tt.store(Entry::new(
                self.position.key,
                depth as u8,
                static_eval,
                static_eval,
                EntryType::Exact,
                Move::NONE,
            ));
        }

        let tt_move = tt_hit.map_or(Move::NONE, |tt| tt.best_move);

        if !is_root && depth >= 6 && !self.position.in_check() && tt_move == Move::NONE {
            depth -= 1;
        }

        let original_alpha = alpha;

        // Null move pruning
        if !is_pv
            && depth >= 3
            && self.position.non_pawn_material(self.position.side)
            && !self.position.in_check()
            && static_eval >= beta
            && (ply < 1 || self.current_move[(ply - 1) as usize] != Move::NULL)
        {
            self.position.make_null_move_with(&mut self.accum);
            self.current_move[ply as usize] = Move::NULL;

            let reduced_depth = depth - (3 + (depth / 5));
            let null_score = -self.search(reduced_depth, -beta, -beta + 1, ply + 1, false, false);

            self.position.unmake_null_move_with(&mut self.accum);
            self.current_move[ply as usize] = Move::NONE;

            if null_score >= beta {
                if null_score >= (eval::MATE_IN_PLY) {
                    return beta;
                }
                return null_score;
            }
        }

        // Reverse futility pruning
        if !is_pv
            && (-eval::MATE_IN_PLY..eval::MATE_IN_PLY).contains(&beta)
            && (-eval::MATE_IN_PLY..eval::MATE_IN_PLY).contains(&static_eval)
            && !self.position.in_check()
            && depth < 7
            && static_eval.saturating_sub(300 * depth as i16) >= beta
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
            let capture = mv.is_capture(&self.position);

            // Late Move Pruning: skip late quiet moves at shallow depths
            if !is_pv
                && !capture
                && !self.position.in_check()
                && depth <= 3
                && move_count > (3 + depth * depth) as u8
                && mv != self.killers[ply as usize][0]
                && mv != self.killers[ply as usize][1]
            {
                continue;
            }

            // store node count for effort calculation
            let before_nodes = self.stats.nodes;

            // TODO: extensions
            let mut new_depth = depth;
            if self.position.in_check() {
                new_depth += 1;
            }

            self.position.make_move_with(mv, &mut self.accum);
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
                score = -self.search(new_depth - 1, -alpha - 1, -alpha, ply + 1, false, false);
            }

            if is_pv && (move_count == 1 || score > alpha && score < beta) {
                score = -self.search(new_depth - 1, -beta, -alpha, ply + 1, true, false);
            }

            self.position.unmake_move_with(mv, &mut self.accum);
            self.current_move[ply as usize] = Move::NONE;

            // store effort at root
            if is_root {
                self.stats.effort[mv.from()][mv.to()] = self.stats.nodes - before_nodes;
            }

            if score > best {
                best = score;
                best_move = mv;

                self.stats.pv[ply as usize][ply as usize] = mv;
                for j in (ply + 1)..self.stats.pv_length[ply as usize + 1] {
                    self.stats.pv[ply as usize][j as usize] =
                        self.stats.pv[ply as usize + 1][j as usize];
                }

                self.stats.pv_length[ply as usize] = self.stats.pv_length[ply as usize + 1];

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
        } else if best > original_alpha {
            EntryType::Exact
        } else {
            EntryType::UpperBound
        };

        if !self.stop.load(std::sync::atomic::Ordering::Relaxed) {
            self.tt.store(Entry::new(
                self.position.key,
                depth as u8,
                static_eval,
                normalize_score(best, ply),
                entry_type,
                best_move,
            ));
        }
        best
    }

    fn quiescence_search(&mut self, mut alpha: i16, beta: i16, is_pv: bool) -> i16 {
        self.stats.nodes += 1;

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

        let tt_hit = if let Some(hit) = self.tt.probe(self.position.key) {
            if !is_pv
                && self.position.halfmove_clock < 80
                && (hit.score_type == EntryType::Exact
                    || (hit.score_type == EntryType::LowerBound
                        && denormalize_score(hit.score, MAX_PLY) >= beta)
                    || (hit.score_type == EntryType::UpperBound
                        && denormalize_score(hit.score, MAX_PLY) <= alpha))
            {
                return denormalize_score(hit.score, MAX_PLY);
            }
            Some(hit)
        } else {
            None
        };

        // Probe tt
        let stand_pat;

        if self.position.in_check() {
            stand_pat = -eval::INFINITY;
        } else if let Some(entry) = tt_hit {
            if entry.score != eval::NO_VALUE {
                stand_pat = denormalize_score(entry.score, MAX_PLY);
            } else if entry.static_eval != eval::NO_VALUE {
                stand_pat = entry.static_eval;
            } else {
                stand_pat = eval::score_nnue(&self.position, &self.accum);
            }
        } else {
            stand_pat = eval::score_nnue(&self.position, &self.accum);

            self.tt.store(Entry::new(
                self.position.key,
                0,
                stand_pat,
                stand_pat,
                EntryType::Exact,
                Move::NONE,
            ));
        }

        if stand_pat >= beta {
            return stand_pat;
        }

        let original_alpha = alpha;
        alpha = alpha.max(stand_pat);

        let mut best = stand_pat;
        let mut best_move = Move::NONE;

        if !self.position.in_check() {
            let best_case_score = {
                let mut value = eval::PIECE_VALUES[Role::Pawn as usize];

                for role in ((Role::Pawn as usize)..=(Role::Queen as usize)).rev() {
                    if self
                        .position
                        .by_color_role(self.position.side.opponent(), Role::new(role as u8))
                        .any()
                    {
                        value = eval::PIECE_VALUES[role];
                        break;
                    }
                }

                // check for promotions
                if (self.position.by_color_role(self.position.side, Role::Pawn)
                    & self.position.side.opponent().home_rank())
                .any()
                {
                    value += eval::PIECE_VALUES[Role::Queen as usize]
                        - eval::PIECE_VALUES[Role::Pawn as usize];
                }

                value
            };

            let delta_margin = alpha.saturating_sub(stand_pat).saturating_sub(425) as i32;
            if best_case_score < delta_margin {
                return stand_pat;
            }
        }

        let tt_move = tt_hit.map_or(Move::NONE, |tt| tt.best_move);

        let see_margin = alpha.saturating_sub(stand_pat).saturating_sub(500).max(1) as i32;
        let mut move_picker = MovePicker::new_quiescence(&self.position, tt_move, see_margin);
        while let Some(mv) = move_picker.next(&self.position, &self.history) {
            self.position.make_move_with(mv, &mut self.accum);
            let score = -self.quiescence_search(-beta, -alpha, is_pv);
            self.position.unmake_move_with(mv, &mut self.accum);

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

        if best == -eval::INFINITY && self.position.in_check() {
            return -eval::MATE + MAX_PLY as i16;
        }

        let entry_type = if best >= beta {
            EntryType::LowerBound
        } else if best > original_alpha {
            EntryType::Exact
        } else {
            EntryType::UpperBound
        };

        if !self.stop.load(std::sync::atomic::Ordering::Relaxed) {
            self.tt.store(Entry::new(
                self.position.key,
                0,
                stand_pat,
                normalize_score(best, MAX_PLY),
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
            || self.tm.nodes.is_some_and(|n| self.stats.nodes >= n)
        {
            return true;
        }

        if self.stats.nodes % 2048 == 0 && self.tm.time_up(&self.stats) {
            self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
            return true;
        }

        false
    }

    pub fn set_silent(&mut self, silent: bool) {
        self.silent = silent;
    }
}

fn normalize_score(score: i16, ply: u8) -> i16 {
    if (eval::MATE_IN_PLY..=eval::MATE).contains(&score) {
        score + ply as i16
    } else if (-eval::MATE..=-eval::MATE_IN_PLY).contains(&score) {
        score - ply as i16
    } else {
        score
    }
}

fn denormalize_score(score: i16, ply: u8) -> i16 {
    if (eval::MATE_IN_PLY..=eval::MATE).contains(&score) {
        score - ply as i16
    } else if (-eval::MATE..=-eval::MATE_IN_PLY).contains(&score) {
        score + ply as i16
    } else {
        score
    }
}
