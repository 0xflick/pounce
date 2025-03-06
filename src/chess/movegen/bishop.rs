use crate::bitboard::Bitboard;
use crate::chess::movegen::types::{BishopType, Mover};
use crate::chess::movegen::utils::get_bishop_moves;
use crate::chess::{Color, Position, Role, Square};

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
