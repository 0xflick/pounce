use crate::{
    bitboard::Bitboard,
    chess::Square,
    movegen::magic_gen::{BISHOP_MAGICS, ROOK_MAGICS},
};

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
        let magic = ROOK_MAGICS[sq];
        let mut occ = Bitboard(0);
        loop {
            let attack = rook_attacks(Square::new(sq as u8), occ);
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
        let magic = BISHOP_MAGICS[sq];
        let mut occ = Bitboard(0);
        loop {
            let attack = bishop_attacks(Square::new(sq as u8), occ);
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

pub const fn rook_attacks(sq: Square, occ: Bitboard) -> Bitboard {
    let mut attacks = Bitboard(0);

    let rank = sq.rank() as u8;
    let file = sq.file() as u8;

    {
        let mut r = rank + 1;
        while r < 8 {
            let bb = 1 << (file + r * 8);
            attacks.0 |= bb;
            if (occ.0 & bb) != 0 {
                break;
            }
            r += 1;
        }
    }
    {
        let mut r = rank;
        while r > 0 {
            r -= 1;
            let bb = 1 << (file + r * 8);
            attacks.0 |= bb;
            if (occ.0 & bb) != 0 {
                break;
            }
        }
    }
    {
        let mut f = file + 1;
        while f < 8 {
            let bb = 1 << (f + rank * 8);
            attacks.0 |= bb;
            if (occ.0 & bb) != 0 {
                break;
            }
            f += 1;
        }
    }
    {
        let mut f = file;
        while f > 0 {
            f -= 1;
            let bb = 1 << (f + rank * 8);
            attacks.0 |= bb;
            if (occ.0 & bb) != 0 {
                break;
            }
        }
    }
    attacks
}

pub const fn bishop_attacks(sq: Square, occ: Bitboard) -> Bitboard {
    let mut attacks = Bitboard(0);

    let rank = sq.rank() as u8;
    let file = sq.file() as u8;

    let mut i = 1;
    while i < 8 {
        if rank + i <= 7 && file + i <= 7 {
            let bb = 1 << ((rank + i) * 8 + file + i);
            attacks.0 |= bb;
            if (occ.0 & bb) != 0 {
                break;
            }
        }
        i += 1;
    }

    i = 1;
    while i < 8 {
        if rank + i <= 7 && file >= i {
            let bb = 1 << ((rank + i) * 8 + file - i);
            attacks.0 |= bb;
            if (occ.0 & bb) != 0 {
                break;
            }
        }
        i += 1;
    }

    i = 1;
    while i < 8 {
        if rank >= i && file + i <= 7 {
            let bb = 1 << ((rank - i) * 8 + file + i);
            attacks.0 |= bb;
            if (occ.0 & bb) != 0 {
                break;
            }
        }
        i += 1;
    }

    i = 1;
    while i < 8 {
        if rank >= i && file >= i {
            let bb = 1 << ((rank - i) * 8 + file - i);
            attacks.0 |= bb;
            if (occ.0 & bb) != 0 {
                break;
            }
        }
        i += 1;
    }
    attacks
}

pub fn occupancy_bb(mask: &Bitboard, index: usize) -> Bitboard {
    let mut occ = Bitboard(0);

    // get indexes of all bits in mask
    let mut bits = Vec::new();
    let mut m = *mask;
    while m.any() {
        bits.push(m.0.trailing_zeros());
        m &= m.0 - 1;
    }

    // set bits in occ according to index
    (0..bits.len()).for_each(|i| {
        if index & (1 << i) != 0 {
            occ |= 1 << bits[i];
        }
    });
    occ
}
