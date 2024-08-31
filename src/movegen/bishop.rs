use types::BishopType;

use crate::{
    bitboard::Bitboard,
    chess::{
        Color,
        Role,
        Square,
    },
    movegen::*,
    position::Position,
};

impl Mover for BishopType {
    #[inline]
    fn into_piece() -> Role {
        Role::Bishop
    }

    #[inline]
    fn pseudo_legal_moves<const BLACK: bool>(from: Square, pos: &Position) -> Bitboard {
        let side = match BLACK {
            true => Color::Black,
            false => Color::White,
        };
        get_bishop_moves(from, pos.occupancy) & !pos.by_color[side]
    }
}
