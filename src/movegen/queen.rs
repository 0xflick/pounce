use types::{BishopType, ColorType, QueenType, RookType};

use crate::{
    bitboard::Bitboard,
    chess::{Role, Square},
    movegen::*,
};

impl Mover for QueenType {
    #[inline]
    fn into_piece() -> Role {
        Role::Queen
    }

    #[inline]
    fn pseudo_legal_moves<CO: ColorType>(from: Square, pos: &Position) -> Bitboard {
        let rook_moves = RookType::pseudo_legal_moves::<CO>(from, pos);
        let bishop_moves = BishopType::pseudo_legal_moves::<CO>(from, pos);
        rook_moves | bishop_moves
    }
}
