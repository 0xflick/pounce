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
    unsafe { PAWN_MOVES[color][sq] }
}

#[inline(always)]
pub fn get_pawn_attacks(sq: Square, color: Color) -> Bitboard {
    unsafe { PAWN_ATTACKS[color][sq] }
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
    unsafe { KNIGHT_MOVES[sq] }
}

#[inline(always)]
pub fn get_king_moves(sq: Square) -> Bitboard {
    unsafe { KING_MOVES[sq] }
}

#[inline(always)]
pub fn between(from: Square, to: Square) -> Bitboard {
    unsafe { BETWEEN[from][to] }
}

#[inline(always)]
pub fn line(from: Square, to: Square) -> Bitboard {
    unsafe { LINE[from][to] }
}

#[inline(always)]
pub fn bishop_rays(sq: Square) -> Bitboard {
    unsafe { BISHOP_RAYS[sq] }
}

#[inline(always)]
pub fn rook_rays(sq: Square) -> Bitboard {
    unsafe { ROOK_RAYS[sq] }
}

#[inline(always)]
pub fn get_kingside_castle_through_squares(color: Color) -> Bitboard {
    unsafe { KINGSIDE_CASTLE[color] }
}

#[inline(always)]
pub fn get_queenside_castle_throught_squares(color: Color) -> Bitboard {
    unsafe { QUEENSIDE_CASTLE[color] }
}
