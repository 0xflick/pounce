pub mod nnue;

use crate::chess::Role;
use crate::chess::position::Position;
use crate::engine::search::MAX_PLY;

pub const NO_VALUE: i16 = 32_002;
pub const INFINITY: i16 = 32_001;
pub const MATE: i16 = 32_000;
pub const MATE_IN_PLY: i16 = MATE - MAX_PLY as i16;
pub const DRAW: i16 = 0;

#[inline]
pub fn score_nnue(
    pos: &Position,
    acc: &nnue::NNUEAccumulator<'_, { nnue::NNUE_HIDDEN_SIZE }>,
) -> i16 {
    (acc.net.forward(acc, pos.side) * 216.0) as i16
}

pub const PIECE_VALUES: [i32; Role::NUM] = [126, 781, 825, 1276, 2538, 0];
