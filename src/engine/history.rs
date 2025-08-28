use std::ops::ShlAssign;

use crate::chess::{Color, Move, Position, Role, Square};
use crate::engine::movepicker::{KILLER_1_SCORE, KILLER_2_SCORE};
use crate::engine::search::{Frame, MAX_PLY, Stack};

pub const HISTORY_MAX: i32 = 16384;

const NUM_CONTINUATION_TABLES: usize = 2;
pub const TOTAL_HISTORY: i32 = HISTORY_MAX * NUM_CONTINUATION_TABLES as i32;

type Sided<T> = [T; Color::NUM];
type Butterfly<T> = [[T; Square::NUM]; Square::NUM];
type RoleTo<T> = [[T; Square::NUM]; Role::NUM];
type HistoryTable<T, const MAX: i32> = Sided<Butterfly<HistScore<T, MAX>>>;
type ContinuationTable<T, const MAX: i32> = Sided<RoleTo<Butterfly<HistScore<T, MAX>>>>;

pub struct HistoryTables {
    killers: [[Move; 2]; MAX_PLY],
    history: HistoryTable<i16, HISTORY_MAX>,
    continuation: Box<[ContinuationTable<i16, HISTORY_MAX>; NUM_CONTINUATION_TABLES]>,
}

impl Default for HistoryTables {
    fn default() -> Self {
        // this is janky but without this rust tries to initialize the array, *and then* put it
        // into the box, which blows up the stack on debug builds
        let mut boxed =
            Box::<[ContinuationTable<i16, HISTORY_MAX>; NUM_CONTINUATION_TABLES]>::new_uninit();
        let continuation = unsafe {
            boxed.as_mut_ptr().write_bytes(0u8, 1);
            boxed.assume_init()
        };

        Self {
            killers: [[Move::NONE; 2]; MAX_PLY],
            history: [[[HistScore::<i16, HISTORY_MAX>::default(); Square::NUM]; Square::NUM];
                Color::NUM],
            continuation,
        }
    }
}

impl HistoryTables {
    pub fn update(&mut self, position: &Position, stack: &Stack, ply: usize, mv: Move, bonus: i32) {
        let hist_idx = self.history_index(position, mv);
        self.history[hist_idx.0][hist_idx.1][hist_idx.2] <<= bonus;

        const CONT_HISTORY_WEIGHTS: [i32; NUM_CONTINUATION_TABLES] = [1024, 512];

        for i in 1..=self.continuation.len() {
            if ply < i {
                break;
            }
            if let Some(c_idx) = self.continuation_index(position, &stack[ply - i], mv) {
                self.continuation[i - 1][c_idx.0][c_idx.1][c_idx.2][c_idx.3][c_idx.4] <<=
                    bonus * CONT_HISTORY_WEIGHTS[i - 1] / 1024;
            }
        }
    }

    pub fn update_killers(&mut self, mv: Move, ply: usize) {
        self.killers[ply][1] = self.killers[ply][0];
        self.killers[ply][0] = mv;
    }

    pub fn score(&self, position: &Position, stack: &Stack, ply: usize, mv: Move) -> i32 {
        if mv == self.killers[ply][0] {
            return KILLER_1_SCORE;
        } else if mv == self.killers[ply][1] {
            return KILLER_2_SCORE;
        }

        let mut value = 0;
        let hist_idx = self.history_index(position, mv);
        value += self.history[hist_idx.0][hist_idx.1][hist_idx.2].0 as i32;

        for i in 1..=self.continuation.len() {
            if ply < i {
                break;
            }

            if let Some(c_idx) = self.continuation_index(position, &stack[ply - i], mv) {
                value +=
                    self.continuation[i - 1][c_idx.0][c_idx.1][c_idx.2][c_idx.3][c_idx.4].0 as i32;
            }
        }

        value
    }

    pub fn is_killer(&self, ply: usize, mv: Move) -> bool {
        mv == self.killers[ply][0] || mv == self.killers[ply][1]
    }

    fn history_index(&self, position: &Position, mv: Move) -> (Color, Square, Square) {
        (position.side, mv.from(), mv.to())
    }

    fn continuation_index(
        &self,
        position: &Position,
        frame: &Frame,
        mv: Move,
    ) -> Option<(Color, Role, Square, Square, Square)> {
        match frame.moved {
            Some(prev_role) => {
                let prev_to = frame.mv.to();
                Some((position.side, prev_role, prev_to, mv.from(), mv.to()))
            }
            // Null move
            None => None,
        }
    }
}

#[derive(Default, Clone, Copy)]
struct HistScore<T: Default, const MAX: i32>(T);

impl<const MAX: i32> ShlAssign<i32> for HistScore<i16, MAX> {
    fn shl_assign(&mut self, rhs: i32) {
        self.0 += (rhs - (self.0 as i32) * rhs.abs() / MAX) as i16;
    }
}

impl<const MAX: i32> ShlAssign<i32> for HistScore<i32, MAX> {
    fn shl_assign(&mut self, rhs: i32) {
        self.0 += rhs - self.0 * rhs.abs() / MAX;
    }
}
