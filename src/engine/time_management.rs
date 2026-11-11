use std::time::Duration;

use crate::chess::Color;
use crate::engine::limits::Limits;
use crate::engine::search::Stats;
use crate::engine::tune;

pub struct SearchCop {
    pub depth: Option<u8>,
    pub nodes: Option<u64>,
    soft_nodes: Option<u64>,
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
            soft_nodes,
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
                soft_nodes,
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
                soft_nodes,
                adjust: false,
                optimal_time: Some(Duration::from_millis(movetime.max(0) as u64)),
                max_time: Some(Duration::from_millis(movetime.max(0) as u64)),
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
                soft_nodes,
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

        let opt = if let Some(mtg) = movestogo {
            // multiple time controls
            let scale = 0.7 / mtg as f32;
            (time_left as f32 * scale).min(0.8 * time_left as f32) as u64
        } else {
            // one time control for the whole game
            let scale = tune::time_base() as f32 / 10_000.0;
            (time_left as f32 * scale).min(0.2 * time_left as f32) as u64
        };

        let max = (opt * tune::time_max_factor() as u64 / 10).min((0.8 * time_left as f32) as u64);
        // the clock can be nearly out, or negative. These saturate because underflowing here gives a
        // huge hard limit, which would lose on time.
        let max =
            max.min((time_remaining.unwrap().max(0) as u64).saturating_sub(2 * overhead as u64));

        SearchCop {
            depth,
            nodes,
            soft_nodes,
            adjust: true,
            optimal_time: Some(Duration::from_millis(opt)),
            max_time: Some(Duration::from_millis(
                max.saturating_sub(overhead as u64).max(1),
            )),
            scale: 1.0,
        }
    }

    pub fn set_single_legal_move(&mut self) {
        if self.adjust {
            // If there's only one legal move, cap search time to 250ms
            self.max_time = self.max_time.map(|t| t.min(Duration::from_millis(250)));
        }
    }

    pub fn time_up(&self, stats: &Stats) -> bool {
        if let Some(n) = self.nodes
            && stats.nodes >= n
        {
            return true;
        }

        if let Some(time) = self.max_time
            && stats.start_time.elapsed() >= time
        {
            return true;
        }

        false
    }

    pub fn time_up_deepening(&self, stats: &Stats) -> bool {
        if let Some(n) = self.soft_nodes
            && stats.nodes >= n
        {
            return true;
        }

        if let Some(time) = self.optimal_time
            && stats.start_time.elapsed() >= time.mul_f32(self.scale)
        {
            return true;
        }
        if let Some(time) = self.max_time
            && stats.start_time.elapsed() >= time.mul_f32(0.8)
        {
            return true;
        }

        false
    }

    pub fn adjust(&mut self, stats: &Stats) {
        if self.adjust {
            let best_move = stats.pv[0][0];

            // Safety: Don't access effort array if we don't have a valid move
            if best_move == crate::chess::Move::NONE || best_move == crate::chess::Move::NULL {
                return;
            }

            let bm_nodes = stats.effort[best_move.from()][best_move.to()];
            let bm_frac = bm_nodes as f32 / stats.nodes as f32;

            let base = tune::time_scale_base() as f32 / 100.0;
            let range = tune::time_scale_range() as f32 / 100.0;
            self.scale = (base + range * (1. - bm_frac)).max(0.5);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hard_limit_ms(wtime: i32, winc: u32) -> u128 {
        let limits = Limits {
            wtime: Some(wtime),
            winc: Some(winc),
            ..Limits::new()
        };
        let cop = SearchCop::new(limits, Color::White);
        cop.max_time.unwrap().as_millis()
    }

    #[test]
    fn hard_limit_never_exceeds_a_nearly_empty_clock() {
        for wtime in [-5, 0, 1, 5, 19, 20, 21, 29, 30, 31, 50] {
            for winc in [0, 100, 1000] {
                let limit = hard_limit_ms(wtime, winc);
                assert!(
                    limit <= wtime.max(1) as u128,
                    "{limit} ms limit with {wtime} ms left and {winc} ms increment"
                );
                assert!(limit >= 1);
            }
        }
    }

    #[test]
    fn hard_limit_is_unchanged_with_a_normal_clock() {
        // Neither cap applies: opt is TimeBase ten-thousandths of (60000 + 50 * 100 - 50 * 10), and
        // max is TimeMaxFactor tenths of opt, less the overhead
        let opt = (64_500.0 * (tune::time_base() as f32 / 10_000.0)) as u128;
        let max = opt * tune::time_max_factor() as u128 / 10 - 10;
        assert_eq!(hard_limit_ms(60_000, 100), max);
    }

    #[test]
    fn negative_movetime_is_not_a_huge_limit() {
        let limits = Limits {
            movetime: Some(-1),
            ..Limits::new()
        };
        let cop = SearchCop::new(limits, Color::White);
        assert_eq!(cop.max_time, Some(Duration::ZERO));
    }
}
