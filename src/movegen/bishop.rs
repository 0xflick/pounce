use types::{BishopType, ColorType};

use crate::{
    bitboard::Bitboard,
    chess::{Role, Square},
    movegen::*,
    position::Position,
};

impl Mover for BishopType {
    #[inline]
    fn into_piece() -> Role {
        Role::Bishop
    }

    #[inline]
    fn pseudo_legal_moves<CO: ColorType>(from: Square, pos: &Position) -> Bitboard {
        get_bishop_moves(from, pos.occupancy) & !pos.by_color[CO::COLOR]
    }
}
