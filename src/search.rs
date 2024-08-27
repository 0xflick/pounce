use std::{
    sync::{atomic::AtomicBool, Arc},
    time::{Duration, Instant},
};

use crate::{
    chess::{Color, GameResult},
    eval,
    limits::Limits,
    movepicker::MovePicker,
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

impl SearchCop {
    pub fn new(
        depth: Option<u8>,
        nodes: Option<u64>,
        time_left: Option<i32>,
        inc: Option<u32>,
        movestogo: Option<u32>,
    ) -> Self {
        let time = match time_left {
            Some(left) => {
                let overhead = 30;

                let mtg = movestogo.unwrap_or(50);
                let total = left as u32 + (mtg - 1) * inc.unwrap_or(0) - mtg * overhead;

                Duration::from_millis((total / mtg) as u64)
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

    pv: [Move; MAX_DEPTH as usize],
    killers: [[Move; 2]; MAX_PLY as usize],

    start_time: Instant,
    stop: Arc<AtomicBool>,
    nodes: u64,
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
            pv: [Move::NULL; MAX_DEPTH as usize],
            killers: [[Move::NULL; 2]; MAX_PLY as usize],
            start_time: Instant::now(),
            stop,
            nodes: 0,
        }
    }

    pub fn think(&mut self) -> Move {
        self.start_time = Instant::now();

        self.iterative_deepening().expect("No best move found")
    }

    fn iterative_deepening(&mut self) -> Option<Move> {
        let max_depth = self.limits.depth.unwrap_or(MAX_DEPTH);
        let mut best_move = None;
        let mut best_score = -eval::INFINITY;

        let mut done_early = false;

        for depth in 1..=max_depth {
            if self.done_thinking() {
                done_early = true;
                break;
            }

            best_move = Some(self.pv[0]);
            if depth > 1 {
                self.uci_info(depth - 1, best_score);
            }

            let alpha = -eval::INFINITY;
            let beta = eval::INFINITY;

            let score = self.search(depth, alpha, beta, 0, true, true);
            best_score = score;
        }

        if max_depth == 1 {
            best_move = Some(self.pv[0]);
        }

        if !done_early {
            self.uci_info(max_depth, best_score);
        }

        best_move
    }

    fn search(
        &mut self,
        depth: u8,
        mut alpha: i16,
        mut beta: i16,
        ply: u8,
        is_pv: bool,
        is_root: bool,
    ) -> i16 {
        if self.done_thinking() {
            return 0;
        }
        self.nodes += 1;

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

        // TODO: Implement check extension

        // Go to quiescence search if depth is 0
        if depth == 0 {
            // return eval::eval(&self.position);
            return self.quiescence_search(alpha, beta, is_pv);
        }

        // Probe the transposition table
        let mut tt_move = Move::NULL;
        if let Some(entry) = self.tt.probe(self.position.key) {
            tt_move = entry.best_move;
            if entry.depth >= depth {
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

        // TODO: Implement null move pruning

        let mut best_move = Move::NULL;
        let mut best = -eval::INFINITY;
        let mut move_count = 0;

        let mut move_picker =
            MovePicker::new_ab_search(&self.position, tt_move, self.killers[ply as usize]);
        while let Some(mv) = move_picker.next(&self.position) {
            move_count += 1;

            self.position.make_move(mv);
            let mut score = -eval::INFINITY;
            if move_count > 1 || !is_pv {
                score = -self.search(depth - 1, -alpha - 1, -alpha, ply + 1, false, false);
            }

            if is_pv && (move_count == 1 || score > alpha && score < beta) {
                score = -self.search(depth - 1, -beta, -alpha, ply + 1, true, false);
            }

            self.position.unmake_move(mv);

            if score > best {
                best = score;
                best_move = mv;

                self.pv[ply as usize] = best_move;

                if score > alpha {
                    alpha = score;
                    if score >= beta {
                        self.update_killers(mv, ply);
                        break;
                    }
                }
            }
        }

        if move_count == 0 {
            if self.position.in_check() {
                return -eval::MATE + ply as i16;
            } else {
                return 0;
            }
        }

        let entry_type = if best <= alpha {
            EntryType::UpperBound
        } else if best >= beta {
            EntryType::LowerBound
        } else {
            EntryType::Exact
        };

        self.tt.set(Entry::new(
            self.position.key,
            depth,
            best,
            entry_type,
            best_move,
        ));
        best
    }

    fn quiescence_search(&mut self, mut alpha: i16, beta: i16, is_pv: bool) -> i16 {
        self.nodes += 1;

        if self.done_thinking() {
            return 0;
        }

        // TODO: check for repetition or draw or 50 move rule

        // Probe tt
        // Use tt move?
        if !is_pv {
            if let Some(entry) = self.tt.probe(self.position.key) {
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
        let mut best_move = Move::NULL;

        let mut move_picker = MovePicker::new_quiescence(&self.position);
        while let Some(mv) = move_picker.next(&self.position) {
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

        let entry_type = if best <= alpha {
            EntryType::UpperBound
        } else if best >= beta {
            EntryType::LowerBound
        } else {
            EntryType::Exact
        };

        self.tt.set(Entry::new(
            self.position.key,
            0,
            best,
            entry_type,
            best_move,
        ));

        best
    }

    pub fn update_killers(&mut self, mv: Move, ply: u8) {
        if self.position.piece_at(mv.to()).is_some() {
            self.killers[ply as usize][1] = self.killers[ply as usize][0];
            self.killers[ply as usize][0] = mv;
        }
    }

    pub fn done_thinking(&self) -> bool {
        self.start_time.elapsed() >= self.limits.time
            || self.stop.load(std::sync::atomic::Ordering::Relaxed)
            || self.limits.nodes.is_some_and(|n| self.nodes >= n)
    }

    fn uci_info(&self, depth: u8, score: i16) {
        let elapsed = self.start_time.elapsed().as_millis() + 1;
        let nps = (self.nodes as u128 * 1000) / elapsed;
        if score.abs() > eval::MATE - MAX_PLY as i16 {
            let ply = score.signum() * (eval::MATE - score.abs()) / 2;
            println!(
                "info depth {} score mate {} time {} nodes {} nps {} hashfull {}",
                depth,
                ply,
                elapsed,
                self.nodes,
                nps,
                self.tt.hashfull()
            );
        } else {
            println!(
                "info depth {} score cp {} time {} nodes {} nps {}, hashfull {}",
                depth,
                score,
                elapsed,
                self.nodes,
                nps,
                self.tt.hashfull(),
            );
        }
    }
}
