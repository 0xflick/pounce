use std::sync::atomic::AtomicBool;
use std::time::Instant;

use arrayvec::ArrayVec;

use crate::chess::movegen::MoveGen;
use crate::chess::{Accumulator, GameResult, Move, Position, Role, Square};
use crate::engine::eval::{self, nnue};
use crate::engine::history::HistoryTables;
use crate::engine::limits::Limits;
use crate::engine::movepicker::{MAX_MOVES, MovePicker, see};
use crate::engine::time_management::SearchCop;
use crate::engine::tt::{Entry, EntryType, Table};

const MAX_DEPTH: u8 = 64;
pub const MAX_PLY: usize = 128;
// Add 1 to array sizes to allow safe boundary access when accessing [ply + 1]
const SEARCH_ARRAY_SIZE: usize = MAX_PLY + 1;

static mut REDUCTIONS: [[u8; MAX_MOVES]; MAX_DEPTH as usize] = [[0; MAX_MOVES]; MAX_DEPTH as usize];

pub fn init_reductions() {
    unsafe {
        #[allow(clippy::needless_range_loop)]
        for m in 1..MAX_MOVES {
            for depth in 1..MAX_DEPTH as usize {
                let reduction = 0.4 + ((depth as f32).ln() * (m as f32).ln()) / 1.7;
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
    pub pv: [[Move; SEARCH_ARRAY_SIZE]; SEARCH_ARRAY_SIZE],
    pub pv_length: [u8; SEARCH_ARRAY_SIZE],
    pub start_time: Instant,
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            nodes: 0,
            effort: [[0; Square::NUM]; Square::NUM],
            pv: [[Move::NONE; SEARCH_ARRAY_SIZE]; SEARCH_ARRAY_SIZE],
            pv_length: [0; SEARCH_ARRAY_SIZE],
            start_time: Instant::now(),
        }
    }
}

impl Stats {
    fn reset(&mut self) {
        self.nodes = 0;
        self.effort = [[0; Square::NUM]; Square::NUM];
        self.pv = [[Move::NONE; SEARCH_ARRAY_SIZE]; SEARCH_ARRAY_SIZE];
        self.pv_length = [0; SEARCH_ARRAY_SIZE];
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

#[derive(Debug, Clone, Copy)]
pub struct Frame {
    pub mv: Move,
    pub moved: Option<Role>,
    pub eval: i16,
}

pub type Stack = [Frame; MAX_PLY];

impl Default for Frame {
    fn default() -> Self {
        Self {
            mv: Move::NONE,
            moved: None,
            eval: eval::NO_VALUE,
        }
    }
}

pub struct Search<'a> {
    pub stats: Stats,

    pub position: Position,
    accum: eval::nnue::NNUEAccumulator<'a, { eval::nnue::NNUE_HIDDEN_SIZE }>,

    pub stack: Stack,
    pub history: HistoryTables,

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
            history: HistoryTables::default(),
            stack: [Frame::default(); MAX_PLY],
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

            // reset pv
            self.stats.pv = [[Move::NONE; SEARCH_ARRAY_SIZE]; SEARCH_ARRAY_SIZE];
            self.stats.pv_length = [0; SEARCH_ARRAY_SIZE];

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

        // Safety: If we still don't have a move (timeout before depth 1 completes),
        // pick any legal move rather than returning Move::NONE
        if bestmove == Move::NONE {
            let mg = crate::chess::movegen::MoveGen::new(&self.position);
            bestmove = mg.into_iter().next().unwrap_or(Move::NONE);
        }

        SearchResult {
            bestmove,
            score,
            depth: depth_reached,
            done_early,
        }
    }

    fn aspiration(&mut self, depth: i32, prev: i16) -> i16 {
        let mut delta = 25;
        let (mut alpha, mut beta) = if depth > 5 {
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
                alpha = (-eval::INFINITY).max(score.saturating_sub(delta));
            } else if score >= beta {
                beta = (eval::INFINITY).min(score.saturating_add(delta));
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
        beta: i16,
        ply: usize,
        is_pv: bool,
        is_root: bool,
    ) -> i16 {
        if self.done_thinking() {
            return 0;
        }
        if depth >= MAX_DEPTH as i32 || ply >= MAX_PLY {
            return eval::score_nnue(&self.position, &self.accum);
        }

        self.stats.pv_length[ply] = ply as u8;

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
                && (hit.score_type() == EntryType::Exact
                    || (hit.score_type() == EntryType::LowerBound
                        && denormalize_score(hit.score, ply) >= beta)
                    || (hit.score_type() == EntryType::UpperBound
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
        } else if let Some(entry) = tt_hit {
            let eval = if entry.static_eval != eval::NO_VALUE {
                denormalize_score(entry.score, ply)
            } else {
                eval::score_nnue(&self.position, &self.accum)
            };

            let denormalized_score = denormalize_score(entry.score, ply);

            static_eval = match entry.score_type() {
                EntryType::Exact => denormalized_score,
                EntryType::LowerBound if denormalized_score > eval => denormalized_score,
                EntryType::UpperBound if denormalized_score < eval => denormalized_score,
                _ => eval,
            };
        } else {
            static_eval = eval::score_nnue(&self.position, &self.accum);

            self.tt.store(Entry::new(
                self.position.key,
                depth as u8,
                static_eval,
                static_eval,
                EntryType::None,
                Move::NONE,
            ));
        }

        self.stack[ply].eval = static_eval;

        let improving = if self.position.in_check() {
            false
        } else if ply > 1 && self.stack[ply - 2].eval != eval::NO_VALUE {
            self.stack[ply].eval > self.stack[ply - 2].eval
        } else if ply > 3 && self.stack[ply - 4].eval != eval::NO_VALUE {
            self.stack[ply].eval > self.stack[ply - 4].eval
        } else {
            true
        };

        let tt_move = {
            let mv = tt_hit.map_or(Move::NONE, |tt| tt.best_move);
            // Validate TT move - if it's corrupt/invalid, ignore it
            if mv != Move::NONE && !self.position.is_pseudo_legal(mv) {
                Move::NONE
            } else {
                mv
            }
        };

        // If we're at root and there's only one legal move, inform time management
        if is_root {
            let mg = MoveGen::new(&self.position);
            if mg.len() == 1 {
                self.tm.set_single_legal_move();
            }
        }

        if !is_root && depth >= 3 && !self.position.in_check() && tt_move == Move::NONE {
            depth -= 1;

            if is_pv {
                depth -= 1;
            }
        }

        // Reverse futility pruning
        if !is_pv
            && (-eval::MATE_IN_PLY..eval::MATE_IN_PLY).contains(&beta)
            && (-eval::MATE_IN_PLY..eval::MATE_IN_PLY).contains(&static_eval)
            && !self.position.in_check()
            && depth < 7
        {
            let margin = 80 * depth - (60 * improving as i32);
            if static_eval.saturating_sub(margin as i16) >= beta {
                return beta;
            }
        }

        // Null move pruning
        if !is_pv
            && depth >= 3
            && self.position.non_pawn_material(self.position.side)
            && !self.position.in_check()
            && static_eval >= beta
            && (ply < 1 || self.stack[ply - 1].mv != Move::NULL)
        {
            self.position.make_null_move_with(&mut self.accum);
            let prev_move = self.stack[ply].mv;
            let prev_moved = self.stack[ply].moved;

            self.stack[ply].mv = Move::NULL;
            self.stack[ply].moved = None;

            let mut reduction = 5 + depth / 5;
            // reduce more based on how far above beta we are
            reduction += ((static_eval - beta) / 200).min(3) as i32;

            let null_score =
                -self.search(depth - reduction, -beta, -beta + 1, ply + 1, false, false);

            self.position.unmake_null_move_with(&mut self.accum);
            self.stack[ply].mv = prev_move;
            self.stack[ply].moved = prev_moved;

            if null_score >= beta {
                return beta;
            }
        }

        let original_alpha = alpha;
        let mut best_move = Move::NONE;
        let mut best = -eval::INFINITY;
        let mut move_count = 0;
        let mut quiets: ArrayVec<Move, 64> = ArrayVec::new();

        let mut move_picker = MovePicker::new_ab_search(&self.position, ply, tt_move);
        while let Some(mv) = move_picker.next(self) {
            move_count += 1;
            let quiet = mv.is_quiet(&self.position);
            let capture = mv.is_capture(&self.position);

            // Late Move Pruning: skip late quiet moves at shallow depths
            if !is_pv
                && !capture
                && !self.position.in_check()
                && depth <= 3
                && move_count > (3 + depth * depth)
                && !self.history.is_killer(ply, mv)
            {
                continue;
            }

            // Futility pruning: skip moves that have no chance of raising alpha
            if !is_pv
                && best > -eval::MATE_IN_PLY
                && !self.position.in_check()
                && depth <= 5
                && mv.is_quiet(&self.position)
            {
                let margin = 100 + depth * 100 + 100;
                if static_eval + margin as i16 <= alpha {
                    continue;
                }
            }

            // SEE pruning: skip moves that aren't see positive (with a depth dependent margin)
            if !is_pv && best > -eval::MATE_IN_PLY && !self.position.in_check() && depth <= 5 {
                let margin = if quiet {
                    depth * 150
                } else {
                    depth * depth * 200
                };
                if !see::see(&self.position, mv, -margin) {
                    continue;
                }
            }

            let mut extension = 0;
            if self.position.in_check() {
                extension += 1
            }

            // store node count for effort calculation
            let before_nodes = self.stats.nodes;

            self.stack[ply].mv = mv;
            self.stack[ply].moved = self.position.role_at(mv.from());
            self.position.make_move_with(mv, &mut self.accum);

            let mut score = -eval::INFINITY;

            // LMR
            let needs_full_search = if depth >= 3 && move_count > (2 + is_pv as i32) {
                let mut reduction = self.reduction(depth, move_count);

                // Reduce more if we're not in a pv node
                if !is_pv {
                    reduction += 1;
                }

                // reduce less if the move gave check
                if self.position.in_check() {
                    reduction -= 1;
                }

                let rdepth = (depth + extension - reduction).clamp(1, depth + extension);

                // Do a zero window search
                score = -self.search(rdepth - 1, -alpha - 1, -alpha, ply + 1, false, false);

                // We need to re-search if we beat alpha, but only if the depth we searched is less
                // than the depth we'd get from a re-search
                score > alpha && rdepth < depth + extension - 1
            } else {
                // we always do a zero window search first if we've already searched this node, or
                // if we aren't in a pv node
                move_count > 1 || !is_pv
            };

            if needs_full_search {
                score = -self.search(
                    depth + extension - 1,
                    -alpha - 1,
                    -alpha,
                    ply + 1,
                    false,
                    false,
                );
            }

            // Do a full window search in pv nodes, if we haven't yet beaten beta
            if is_pv && (move_count == 1 || score > alpha && score < beta) {
                score = -self.search(depth + extension - 1, -beta, -alpha, ply + 1, true, false);
            }

            self.position.unmake_move_with(mv, &mut self.accum);
            self.stack[ply].mv = Move::NONE;
            self.stack[ply].moved = None;

            // store effort at root
            if is_root {
                self.stats.effort[mv.from()][mv.to()] = self.stats.nodes - before_nodes;
            }

            if score > best {
                best = score;
                best_move = mv;

                self.stats.pv[ply][ply] = mv;
                for j in (ply + 1)..self.stats.pv_length[ply + 1] as usize {
                    self.stats.pv[ply][j] = self.stats.pv[ply + 1][j];
                }

                self.stats.pv_length[ply] = self.stats.pv_length[ply + 1];

                if score > alpha {
                    alpha = score;
                    if score >= beta {
                        if !capture {
                            self.history.update_killers(mv, ply);
                            let bonus = 2000.min(350 * depth - 350);
                            self.history
                                .update(&self.position, &self.stack, ply, mv, bonus);

                            for quiet in quiets.iter() {
                                self.history.update(
                                    &self.position,
                                    &self.stack,
                                    ply,
                                    *quiet,
                                    -bonus / 2,
                                );
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
                && (hit.score_type() == EntryType::Exact
                    || (hit.score_type() == EntryType::LowerBound
                        && denormalize_score(hit.score, MAX_PLY) >= beta)
                    || (hit.score_type() == EntryType::UpperBound
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
            let eval = if entry.static_eval != eval::NO_VALUE {
                denormalize_score(entry.static_eval, MAX_PLY)
            } else {
                eval::score_nnue(&self.position, &self.accum)
            };

            let denormalized_score = denormalize_score(entry.score, MAX_PLY);

            stand_pat = match entry.score_type() {
                EntryType::Exact => denormalized_score,
                EntryType::LowerBound if denormalized_score > eval => denormalized_score,
                EntryType::UpperBound if denormalized_score < eval => denormalized_score,
                _ => eval,
            };
        } else {
            stand_pat = eval::score_nnue(&self.position, &self.accum);

            self.tt.store(Entry::new(
                self.position.key,
                0,
                stand_pat,
                stand_pat,
                EntryType::None,
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

        let tt_move = {
            let mv = tt_hit.map_or(Move::NONE, |tt| tt.best_move);
            // Validate TT move - if it's corrupt/invalid, ignore it
            if mv != Move::NONE && !self.position.is_pseudo_legal(mv) {
                Move::NONE
            } else {
                mv
            }
        };

        let see_margin = alpha.saturating_sub(stand_pat).saturating_sub(500).max(1) as i32;
        let mut move_picker = MovePicker::new_quiescence(&self.position, tt_move, see_margin);
        while let Some(mv) = move_picker.next(self) {
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

    fn reduction(&self, depth: i32, move_count: i32) -> i32 {
        unsafe { REDUCTIONS[depth as usize][move_count as usize] as i32 }
    }

    pub fn done_thinking(&self) -> bool {
        if self.stop.load(std::sync::atomic::Ordering::Relaxed) {
            return true;
        }

        if self.stats.nodes.is_multiple_of(2048) && self.tm.time_up(&self.stats) {
            self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
            return true;
        }

        false
    }

    pub fn set_silent(&mut self, silent: bool) {
        self.silent = silent;
    }
}

fn normalize_score(score: i16, ply: usize) -> i16 {
    if (eval::MATE_IN_PLY..=eval::MATE).contains(&score) {
        score + ply as i16
    } else if (-eval::MATE..=-eval::MATE_IN_PLY).contains(&score) {
        score - ply as i16
    } else {
        score
    }
}

fn denormalize_score(score: i16, ply: usize) -> i16 {
    if (eval::MATE_IN_PLY..=eval::MATE).contains(&score) {
        score - ply as i16
    } else if (-eval::MATE..=-eval::MATE_IN_PLY).contains(&score) {
        score + ply as i16
    } else {
        score
    }
}
