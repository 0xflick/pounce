use std::{
    sync::{atomic::AtomicBool, Arc},
    time::{Duration, Instant},
};

use crate::{
    chess::Color, eval::eval, limits::Limits, movegen::MoveGen, moves::Move, position::Position,
};

pub struct SearchCop {
    pub depth: Option<u32>,
    pub nodes: Option<u64>,
    pub time: Duration,
}

impl SearchCop {
    pub fn new(
        depth: Option<u32>,
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
    start_time: Instant,
    stop: Arc<AtomicBool>,
    nodes: u64,
}

impl Search {
    pub fn new(position: Position, limits: Limits, stop: Arc<AtomicBool>) -> Self {
        let (time_left, inc) = match position.side {
            Color::White => (limits.wtime, limits.winc),
            Color::Black => (limits.btime, limits.binc),
        };

        Search {
            position,
            limits: SearchCop::new(limits.depth, limits.nodes, time_left, inc, limits.movestogo),
            start_time: Instant::now(),
            stop,
            nodes: 0,
        }
    }

    pub fn think(&mut self) -> Move {
        self.start_time = Instant::now();

        let mut best_move = None;
        let mut best_score = i32::MIN;
        for mv in MoveGen::new(&self.position) {
            let mut next_pos = self.position;
            next_pos.make_move(mv);
            let score = eval(&next_pos);
            if score > best_score {
                best_move = Some(mv);
                best_score = score;
            }
        }

        best_move.unwrap()
    }

    pub fn done_thinking(&self) -> bool {
        self.start_time.elapsed() >= self.limits.time
            || self.stop.load(std::sync::atomic::Ordering::Relaxed)
            || self.limits.nodes.is_some_and(|n| self.nodes >= n)
    }
}
