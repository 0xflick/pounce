use std::{
    sync::{atomic::AtomicBool, Arc},
    time::{Duration, Instant},
};

use crate::{
    chess::Color,
    eval::{self, eval},
    limits::Limits,
    movegen::MoveGen,
    moves::Move,
    position::Position,
    tt::{EntryType, Table},
};

pub struct SearchCop {
    pub depth: Option<u8>,
    pub nodes: Option<u64>,
    pub time: Duration,
}

const MAX_DEPTH: u8 = 64;

impl SearchCop {
    pub fn new(
        depth: Option<u8>,
        nodes: Option<u64>,
        time_left: Option<u64>,
        inc: Option<u64>,
        movestogo: Option<u64>,
    ) -> Self {
        let time = match time_left {
            Some(left) => {
                let overhead = 10;

                let mtg = movestogo.unwrap_or(50);
                let total = left + mtg * inc.unwrap_or(0) - mtg * overhead;

                Duration::from_millis(total / mtg)
            }
            None => Duration::MAX,
        };

        SearchCop { depth, nodes, time }
    }
}

pub struct Search {
    position: Position,
    limits: SearchCop,
    tt: Table,

    start_time: Instant,
    stop: Arc<AtomicBool>,
    nodes: u64,
}

impl Search {
    pub fn new(position: Position, limits: Limits, tt: Table, stop: Arc<AtomicBool>) -> Self {
        let (time_left, inc) = match position.side {
            Color::White => (limits.wtime, limits.winc),
            Color::Black => (limits.btime, limits.binc),
        };

        Search {
            position,
            limits: SearchCop::new(limits.depth, limits.nodes, time_left, inc, limits.movestogo),
            tt,
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
        let mut best_move = None;

        let max_depth = self.limits.depth.unwrap_or(MAX_DEPTH);

        for depth in 1..=max_depth {
            if self.done_thinking() {
                break;
            }

            let alpha = -eval::INFINITY;
            let beta = eval::INFINITY;

            let score = self.search(depth, alpha, beta);
            self.uci_info(depth, score);
        }

        best_move
    }

    fn search(&mut self, depth: u8, mut alpha: i16, mut beta: i16) -> i16 {
        if self.done_thinking() {
            return 0;
        }

        // TODO: Implement draw detection

        // Extension if in check
        // TODO: Implement check extension

        // Go to quiescence search if depth is 0
        if depth == 0 {
            return self.quiescence_search(0, alpha, beta);
        }

        // Probe the transposition table
        if let Some(entry) = self.tt.probe(self.position.key) {
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

        // Null move pruning
        // TODO: Implement null move pruning

        // Need movepicker

        unimplemented!()
    }

    fn quiescence_search(&mut self, depth: u8, alpha: i16, beta: i16) -> i16 {
        todo!()
    }

    pub fn done_thinking(&self) -> bool {
        self.start_time.elapsed() >= self.limits.time
            || self.stop.load(std::sync::atomic::Ordering::Relaxed)
            || self.limits.nodes.is_some_and(|n| self.nodes >= n)
    }

    fn uci_info(&self, depth: u8, score: i16) {
        let elapsed = self.start_time.elapsed().as_millis();
        let nps = (self.nodes as u128 * 1000) / elapsed;
        println!(
            "info depth {} score cp {} time {} nodes {} nps {}",
            depth, score, elapsed, self.nodes, nps
        );
    }
}
