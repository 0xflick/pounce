use std::ops::ShlAssign;

use crate::chess::{Color, Move, Position, Role, Square};
use crate::engine::eval;
use crate::engine::movepicker::{KILLER_1_SCORE, KILLER_2_SCORE};
use crate::engine::search::{Frame, MAX_PLY, Stack};

pub const HISTORY_MAX: i32 = 16384;

const CORRECTION_BITS: u32 = 14;
const CORRECTION_GRAIN: i32 = 256;
const CORRECTION_MAX: i32 = 32 * CORRECTION_GRAIN;
const CORRECTION_MAX_WEIGHT: i32 = 16;

type Sided<T> = [T; Color::NUM];
type Butterfly<T> = [[T; Square::NUM]; Square::NUM];
type RoleTo<T> = [[T; Square::NUM]; Role::NUM];
type HistoryTable<T, const MAX: i32> = Sided<Butterfly<HistScore<T, MAX>>>;
type ContinuationTable<T, const MAX: i32> = Sided<RoleTo<RoleTo<HistScore<T, MAX>>>>;

pub struct HistoryTables {
    killers: [[Move; 2]; MAX_PLY],
    history: HistoryTable<i16, HISTORY_MAX>,
    continuation: Box<[ContinuationTable<i16, HISTORY_MAX>; 1]>,
    /// How far searches have found the net's eval to be off, by side and pawn structure.
    correction: Box<Sided<[i16; 1 << CORRECTION_BITS]>>,
}

impl Default for HistoryTables {
    fn default() -> Self {
        // this is janky but without this rust tries to initialize the array, *and then* put it
        // into the box, which blows up the stack on debug builds
        let mut boxed = Box::<[ContinuationTable<i16, HISTORY_MAX>; 1]>::new_uninit();
        let continuation = unsafe {
            boxed.as_mut_ptr().write_bytes(0u8, 1);
            boxed.assume_init()
        };

        Self {
            killers: [[Move::NONE; 2]; MAX_PLY],
            history: [[[HistScore::<i16, HISTORY_MAX>::default(); Square::NUM]; Square::NUM];
                Color::NUM],
            continuation,
            correction: Box::new([[0; 1 << CORRECTION_BITS]; Color::NUM]),
        }
    }
}

impl HistoryTables {
    pub fn update(&mut self, position: &Position, stack: &Stack, ply: usize, mv: Move, bonus: i32) {
        let hist_idx = self.history_index(position, mv);
        self.history[hist_idx.0][hist_idx.1][hist_idx.2] <<= bonus;

        for i in 1..=self.continuation.len() {
            if ply < i {
                break;
            }
            if let Some(c_idx) = self.continuation_index(position, &stack[ply - i], mv) {
                self.continuation[i - 1][c_idx.0][c_idx.1][c_idx.2][c_idx.3][c_idx.4] <<= bonus;
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

        value / 2
    }

    /// The net's eval, adjusted by how far off it has been in this pawn structure.
    pub fn corrected_eval(&self, position: &Position, raw_eval: i16) -> i16 {
        let correction =
            self.correction[position.side][correction_index(position)] as i32 / CORRECTION_GRAIN;
        let limit = eval::MATE_IN_PLY as i32 - 1;
        (raw_eval as i32 + correction).clamp(-limit, limit) as i16
    }

    /// Moves the correction towards what a search of this depth found.
    pub fn update_correction(
        &mut self,
        position: &Position,
        raw_eval: i16,
        score: i16,
        depth: i32,
    ) {
        let target = (score as i32 - raw_eval as i32) * CORRECTION_GRAIN;
        let weight = (depth + 1).min(CORRECTION_MAX_WEIGHT);
        let entry = &mut self.correction[position.side][correction_index(position)];
        let updated = (*entry as i32 * (256 - weight) + target * weight) / 256;
        *entry = updated.clamp(-CORRECTION_MAX, CORRECTION_MAX) as i16;
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
    ) -> Option<(Color, Role, Square, Role, Square)> {
        match frame.moved {
            Some(prev_role) => {
                let prev_to = frame.mv.to();

                let role = position.role_at(mv.from()).expect("No role at position");

                Some((position.side, prev_role, prev_to, role, mv.to()))
            }
            // Null move
            None => None,
        }
    }
}

fn correction_index(position: &Position) -> usize {
    let pawns = position.by_role[Role::Pawn];
    let white = (pawns & position.by_color[Color::White]).0;
    let black = (pawns & position.by_color[Color::Black]).0;
    let hash = white.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ black.wrapping_mul(0xC2B2_AE3D_27D4_EB4F).rotate_left(23);
    (hash.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> (64 - CORRECTION_BITS)) as usize
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
