use std::time::Duration;

use crate::chess::Color;
use crate::engine::limits::Limits;
use crate::engine::search::Stats;

pub struct SearchCop {
    pub depth: Option<u8>,
    pub nodes: Option<u64>,
    adjust: bool,
    pub optimal_time: Option<Duration>,
    pub max_time: Option<Duration>,

    pub scale: f32,
}

impl SearchCop {
    pub fn new(
        Limits {
            depth,
            nodes,
            wtime,
            btime,
            winc,
            binc,
            movestogo,
            movetime,
            infinite,
        }: Limits,
        side: Color,
    ) -> Self {
        if infinite {
            return SearchCop {
                depth,
                nodes,
                adjust: false,
                optimal_time: None,
                max_time: None,
                scale: 1.0,
            };
        }

        if let Some(movetime) = movetime {
            return SearchCop {
                depth,
                nodes,
                adjust: false,
                optimal_time: Some(Duration::from_millis(movetime as u64)),
                max_time: Some(Duration::from_millis(movetime as u64)),
                scale: 1.0,
            };
        }

        let (time_remaining, inc) = match side {
            Color::White => (wtime, winc.unwrap_or(0) as i32),
            Color::Black => (btime, binc.unwrap_or(0) as i32),
        };

        // if time remaining was not set, return as if infinite
        if time_remaining.is_none() {
            return SearchCop {
                depth,
                nodes,
                adjust: false,
                optimal_time: None,
                max_time: None,
                scale: 1.0,
            };
        }

        // inspired by weiss
        let overhead = 10;

        // plan as if there are at most 50 moves left
        let mtg = 50.min(movestogo.unwrap_or(50)) as i32;

        let time_left = time_remaining
            .unwrap()
            .max(time_remaining.unwrap() + mtg * inc - mtg * overhead);

        let opt = if movestogo.is_none() {
            // one time control for the whole game
            let scale = 0.04;
            (time_left as f32 * scale).min(0.2 * time_left as f32) as u64
        } else {
            // multiple time controls
            let scale = 0.7;
            (time_left as f32 * scale).min(0.8 * time_left as f32) as u64
        };

        let max = (3 * opt).min((0.8 * time_left as f32) as u64);
        let max = max.min(time_remaining.unwrap() as u64 - 3 * overhead as u64);

        SearchCop {
            depth,
            nodes,
            adjust: true,
            optimal_time: Some(Duration::from_millis(opt)),
            max_time: Some(Duration::from_millis(max)),
            scale: 1.0,
        }
    }

    pub fn time_up(&self, stats: &Stats) -> bool {
        if let Some(time) = self.max_time {
            if stats.start_time.elapsed() >= time {
                return true;
            }
        }
        false
    }

    pub fn time_up_deepening(&self, stats: &Stats) -> bool {
        if let Some(time) = self.optimal_time {
            if stats.start_time.elapsed() >= time.mul_f32(self.scale) {
                return true;
            }
        }
        if let Some(time) = self.max_time {
            if stats.start_time.elapsed() >= time.mul_f32(0.8) {
                return true;
            }
        }

        false
    }

    pub fn adjust(&mut self, stats: &Stats) {
        if self.adjust {
            let bm_nodes = stats.effort[stats.pv[0][0].from()][stats.pv[0][0].to()];
            let bm_frac = bm_nodes as f32 / stats.nodes as f32;

            self.scale = (0.4 + 2. * (1. - bm_frac)).max(0.5);
        }
    }
}
