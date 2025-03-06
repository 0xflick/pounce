use crate::bitboard::Bitboard;
use crate::chess::movegen::types::{BishopType, Mover, QueenType, RookType};
use crate::chess::{Position, Role, Square};

impl Mover for QueenType {
    #[inline]
    fn into_piece() -> Role {
        Role::Queen
    }

    #[inline]
    fn pseudo_legal_moves<const BLACK: bool>(from: Square, pos: &Position) -> Bitboard {
        let rook_moves = RookType::pseudo_legal_moves::<BLACK>(from, pos);
        let bishop_moves = BishopType::pseudo_legal_moves::<BLACK>(from, pos);
        rook_moves | bishop_moves
    }
}
