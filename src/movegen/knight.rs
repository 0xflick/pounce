use types::KnightType;

use crate::{
    bitboard::Bitboard,
    chess::{Color, Role, Square},
    movegen::*,
    position::Position,
};

impl Mover for KnightType {
    #[inline]
    fn into_piece() -> Role {
        Role::Knight
    }

    #[inline]
    fn pseudo_legal_moves<const BLACK: bool>(from: Square, pos: &Position) -> Bitboard {
        let side = match BLACK {
            true => Color::Black,
            false => Color::White,
        };
        get_knight_moves(from) & !pos.by_color[side]
    }
}
