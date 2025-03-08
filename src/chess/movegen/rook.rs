use crate::chess::bitboard::Bitboard;
use crate::chess::movegen::types::{Mover, RookType};
use crate::chess::movegen::utils::get_rook_moves;
use crate::chess::{Color, Position, Role, Square};

impl Mover for RookType {
    #[inline]
    fn into_piece() -> Role {
        Role::Rook
    }

    #[inline]
    fn pseudo_legal_moves<const BLACK: bool>(from: Square, pos: &Position) -> Bitboard {
        let side = match BLACK {
            true => Color::Black,
            false => Color::White,
        };
        get_rook_moves(from, pos.occupancy) & !pos.by_color[side]
    }
}
