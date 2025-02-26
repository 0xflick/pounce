use types::{BishopType, QueenType, RookType};

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
    fn pseudo_legal_moves<const BLACK: bool>(from: Square, pos: &Position) -> Bitboard {
        let rook_moves = RookType::pseudo_legal_moves::<BLACK>(from, pos);
        let bishop_moves = BishopType::pseudo_legal_moves::<BLACK>(from, pos);
        rook_moves | bishop_moves
    }
}
