use types::{ColorType, KnightType};

use crate::{
    bitboard::Bitboard,
    chess::{Role, Square},
    movegen::*,
    position::Position,
};

impl Mover for KnightType {
    #[inline]
    fn into_piece() -> Role {
        Role::Knight
    }

    #[inline]
    fn pseudo_legal_moves<CO: ColorType>(from: Square, pos: &Position) -> Bitboard {
        get_knight_moves(from) & !pos.by_color[CO::COLOR as usize]
    }
}
