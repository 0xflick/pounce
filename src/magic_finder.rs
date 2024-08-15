use crate::bitboard::Bitboard;
use crate::chess::Square;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

pub fn rook_mask(sq: Square) -> Bitboard {
    let mut mask = Bitboard(0);

    let rank = sq.rank() as u8;
    let file = sq.file() as u8;

    for r in (rank + 1)..=6 {
        mask |= 1 << (file + r * 8);
    }
    for f in (file + 1)..=6 {
        mask |= 1 << (f + rank * 8);
    }
    if rank > 0 {
        for r in (1..=rank - 1).rev() {
            mask |= 1 << (file + r * 8);
        }
    }
    if file > 0 {
        for f in (1..=file - 1).rev() {
            mask |= 1 << (f + rank * 8);
        }
    }
    mask
}

pub fn bishop_mask(sq: Square) -> Bitboard {
    let mut mask = Bitboard(0);

    let rank = sq.rank() as u8;
    let file = sq.file() as u8;

    for i in 1..8 {
        if rank + i <= 6 && file + i <= 6 {
            mask |= 1 << ((rank + i) * 8 + file + i);
        }
        if rank + i <= 6 && file > i {
            mask |= 1 << ((rank + i) * 8 + file - i);
        }
        if rank > i && file + i <= 6 {
            mask |= 1 << ((rank - i) * 8 + file + i);
        }
        if rank > i && file > i {
            mask |= 1 << ((rank - i) * 8 + file - i);
        }
    }

    mask
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

pub fn find_magic(sq: Square, shift: u8, bishop: bool, num_tries: usize) -> Option<u64> {
    let mut rng = SmallRng::from_entropy();

    let mask = if bishop {
        bishop_mask(sq)
    } else {
        rook_mask(sq)
    };

    let n = mask.count();
    let mut used = vec![Bitboard(0); 1 << shift];
    let mut occupancy = vec![Bitboard(0); 1 << n];
    let mut attacks = vec![Bitboard(0); 1 << n];
    // init occupancy array and attack array
    for i in 0..(1 << n) {
        occupancy[i] = occupancy_bb(&mask, i);
        attacks[i] = if bishop {
            bishop_attacks(sq, occupancy[i])
        } else {
            rook_attacks(sq, occupancy[i])
        };
    }

    for _ in 0..num_tries {
        let magic = rng.gen::<u64>() & rng.gen::<u64>() & rng.gen::<u64>();
        used.iter_mut().for_each(|u| *u = Bitboard(0));
        let mut fail = false;
        for i in 0..(1 << n) {
            let idx = (occupancy[i].0.wrapping_mul(magic) >> (64 - shift)) as usize;

            if used[idx].none() || used[idx] == attacks[i] {
                used[idx] = attacks[i];
            } else {
                fail = true;
                break;
            }
        }
        if !fail {
            return Some(magic);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chess::{File, Rank};
    #[test]
    fn test_rook_mask_1() {
        let sq = Square::make(File::D, Rank::R4);
        let mask = rook_mask(sq);
        assert_eq!(mask, Bitboard(0x8080876080800));
    }

    #[test]
    fn test_rook_mask_2() {
        let sq = Square::make(File::A, Rank::R1);
        let mask = rook_mask(sq);
        assert_eq!(mask, Bitboard(0x000101010101017e));
    }

    #[test]
    fn test_rook_mask_3() {
        let sq = Square::make(File::H, Rank::R7);
        let mask = rook_mask(sq);
        assert_eq!(mask, Bitboard(0x007e808080808000));
    }

    #[test]
    fn test_bishop_mask_1() {
        let sq = Square::make(File::A, Rank::R1);
        let mask = bishop_mask(sq);
        assert_eq!(mask, Bitboard(0x0040201008040200));
    }

    #[test]
    fn test_bishop_mask_2() {
        let sq = Square::make(File::G, Rank::R1);
        let mask = bishop_mask(sq);
        assert_eq!(mask, Bitboard(0x20408102000));
    }

    #[test]
    fn test_index_to_u64_1() {
        let mask = Bitboard(0x0000000000000001);
        assert_eq!(occupancy_bb(&mask, 0), Bitboard(0x0000000000000000));
        assert_eq!(occupancy_bb(&mask, 1), Bitboard(0x0000000000000001));
    }

    #[test]
    fn test_index_to_u64_2() {
        let mask = Bitboard(0x001000000C000200);
        assert_eq!(occupancy_bb(&mask, 0), Bitboard(0x0000000000000000));
        assert_eq!(occupancy_bb(&mask, 1), Bitboard(0x200));
        assert_eq!(occupancy_bb(&mask, 2), Bitboard(0x4000000));
        assert_eq!(occupancy_bb(&mask, 3), Bitboard(0x4000200));
        assert_eq!(occupancy_bb(&mask, 4), Bitboard(0x8000000));
        assert_eq!(occupancy_bb(&mask, 5), Bitboard(0x8000200));
        assert_eq!(occupancy_bb(&mask, 6), Bitboard(0xC000000));
        assert_eq!(occupancy_bb(&mask, 7), Bitboard(0xC000200));
        assert_eq!(occupancy_bb(&mask, 8), Bitboard(0x0010000000000000));
    }
}
