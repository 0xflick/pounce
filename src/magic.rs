use crate::magic_finder::{bishop_attacks, rook_attacks};
use crate::magic_gen::BISHOP_MAGICS;
use crate::{bitboard::Bitboard, chess::Square, magic_gen::ROOK_MAGICS};

const fn calc_size(magic_arr: &[Magic; 64]) -> usize {
    let mut size = 0;
    let mut i = 0;
    while i < 64 {
        size += 1 << magic_arr[i].shift;
        i += 1;
    }
    size
}

#[derive(Debug, Clone, Copy)]
pub struct Magic {
    pub mask: Bitboard,
    pub shift: u8,
    pub magic: u64,
    pub offset: usize,
}

impl Magic {
    #[inline]
    pub const fn index(&self, occ: Bitboard) -> usize {
        let masked = occ.0 & self.mask.0;
        (masked.wrapping_mul(self.magic) >> (64 - self.shift)) as usize + self.offset
    }
}

const ROOK_TABLE_SIZE: usize = calc_size(&ROOK_MAGICS);
const BISHOP_TABLE_SIZE: usize = calc_size(&BISHOP_MAGICS);

pub static ROOK_ATTACKS: [Bitboard; ROOK_TABLE_SIZE] = init_rook_magics();
pub static BISHOP_ATTACKS: [Bitboard; BISHOP_TABLE_SIZE] = init_bishop_magics();

const fn init_rook_magics() -> [Bitboard; ROOK_TABLE_SIZE] {
    let mut table = [Bitboard(0); ROOK_TABLE_SIZE];

    let mut sq = 0;
    while sq < 64 {
        let magic = ROOK_MAGICS[sq as usize];
        let mut occ = Bitboard(0);
        loop {
            let attack = rook_attacks(Square::new(sq), occ);
            let idx = magic.index(occ);

            if table[idx].0 == Bitboard(0).0 {
                table[idx] = attack;
            }
            occ.0 = occ.0.wrapping_sub(magic.mask.0) & magic.mask.0;
            if occ.none() {
                break;
            }
        }
        sq += 1;
    }

    table
}

const fn init_bishop_magics() -> [Bitboard; BISHOP_TABLE_SIZE] {
    let mut table = [Bitboard(0); BISHOP_TABLE_SIZE];

    let mut sq = 0;
    while sq < 64 {
        let magic = BISHOP_MAGICS[sq as usize];
        let mut occ = Bitboard(0);
        loop {
            let attack = bishop_attacks(Square::new(sq), occ);
            let idx = magic.index(occ);

            if table[idx].0 == Bitboard(0).0 {
                table[idx] = attack;
            }
            occ.0 = occ.0.wrapping_sub(magic.mask.0) & magic.mask.0;
            if occ.none() {
                break;
            }
        }
        sq += 1;
    }

    table
}
