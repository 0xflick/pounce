use std::time::{Duration, Instant};

use crate::chess::Color;
use crate::engine::limits::Limits;

pub struct SearchCop {
    pub depth: Option<u8>,
    pub nodes: Option<u64>,
    pub adjust: bool,
    pub optimal_time: Option<Duration>,
    pub max_time: Option<Duration>,
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
            };
        }

        if let Some(movetime) = movetime {
            return SearchCop {
                depth,
                nodes,
                adjust: false,
                optimal_time: Some(Duration::from_millis(movetime as u64)),
                max_time: Some(Duration::from_millis(movetime as u64)),
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
            };
        }

        // inspired by weiss
        let overhead = 10;

        // plan as if there are at most 50 moves left
        let mtg = 50.min(movestogo.unwrap_or(50)) as i32;

        let time_left = 0.max(time_remaining.unwrap() + mtg * inc - mtg * overhead);

        let opt = if movestogo.is_none() {
            // one time control for the whole game
            let scale = 0.04;
            (time_left as f32 * scale).min(0.2 * time_left as f32) as u64
        } else {
            // multiple time controls
            let scale = 0.7;
            (time_left as f32 * scale).min(0.8 * time_left as f32) as u64
        };

        let max = (opt).min((0.8 * time_left as f32) as u64);
        let max = max.min(time_remaining.unwrap() as u64 - 3 * overhead as u64);

        SearchCop {
            depth,
            nodes,
            adjust: true,
            optimal_time: Some(Duration::from_millis(opt)),
            max_time: Some(Duration::from_millis(max)),
        }
    }

    pub fn time_up(&self, start_time: Instant) -> bool {
        if let Some(time) = self.max_time {
            return start_time.elapsed() >= time;
        }
        false
    }
}
