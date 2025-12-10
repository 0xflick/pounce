use crate::chess::bitboard::Bitboard;
use crate::chess::movegen::magic::{BISHOP_ATTACKS, ROOK_ATTACKS};
use crate::chess::movegen::magic_gen::{BISHOP_MAGICS, ROOK_MAGICS};
use crate::chess::movegen::tables::{
    BETWEEN, BISHOP_RAYS, KING_MOVES, KINGSIDE_CASTLE, KNIGHT_MOVES, LINE, PAWN_ATTACKS,
    PAWN_MOVES, QUEENSIDE_CASTLE, ROOK_RAYS,
};
use crate::chess::{Color, Square};

#[inline(always)]
pub fn get_pawn_moves(sq: Square, color: Color) -> Bitboard {
    PAWN_MOVES[color as usize][sq as usize]
}

#[inline(always)]
pub fn get_pawn_attacks(sq: Square, color: Color) -> Bitboard {
    PAWN_ATTACKS[color as usize][sq as usize]
}

#[inline(always)]
pub fn get_rook_moves(sq: Square, occ: Bitboard) -> Bitboard {
    unsafe {
        let magic = ROOK_MAGICS.get_unchecked(sq as usize);
        let occ = occ & magic.mask;
        *ROOK_ATTACKS.get_unchecked(magic.index(occ))
    }
}

#[inline(always)]
pub fn get_bishop_moves(sq: Square, occ: Bitboard) -> Bitboard {
    unsafe {
        let magic = BISHOP_MAGICS.get_unchecked(sq as usize);
        let occ = occ & magic.mask;
        *BISHOP_ATTACKS.get_unchecked(magic.index(occ))
    }
}

#[inline(always)]
pub fn get_knight_moves(sq: Square) -> Bitboard {
    KNIGHT_MOVES[sq as usize]
}

#[inline(always)]
pub fn get_king_moves(sq: Square) -> Bitboard {
    KING_MOVES[sq as usize]
}

#[inline(always)]
pub fn between(from: Square, to: Square) -> Bitboard {
    BETWEEN[from as usize][to as usize]
}

#[inline(always)]
pub fn line(from: Square, to: Square) -> Bitboard {
    LINE[from as usize][to as usize]
}

#[inline(always)]
pub fn bishop_rays(sq: Square) -> Bitboard {
    BISHOP_RAYS[sq as usize]
}

#[inline(always)]
pub fn rook_rays(sq: Square) -> Bitboard {
    ROOK_RAYS[sq as usize]
}

#[inline(always)]
pub fn get_kingside_castle_through_squares(color: Color) -> Bitboard {
    KINGSIDE_CASTLE[color as usize]
}

#[inline(always)]
pub fn get_queenside_castle_throught_squares(color: Color) -> Bitboard {
    QUEENSIDE_CASTLE[color as usize]
}
