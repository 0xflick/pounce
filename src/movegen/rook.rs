use types::{ColorType, RookType};

use crate::{
    bitboard::Bitboard,
    chess::{Role, Square},
    movegen::*,
};

impl Mover for RookType {
    #[inline]
    fn into_piece() -> Role {
        Role::Rook
    }

    #[inline]
    fn pseudo_legal_moves<CO: ColorType>(from: Square, pos: &Position) -> Bitboard {
        get_rook_moves(from, pos.occupancy) & !pos.by_color[CO::COLOR as usize]
    }
}
