use crate::bitboard::Bitboard;
use crate::chess::Square;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

fn rook_mask(sq: Square) -> Bitboard {
    let mut mask = Bitboard(0);

    let rank = sq.rank() as u8;
    let file = sq.file() as u8;

    for r in (rank + 1)..=6 {
        mask.0 |= 1 << (file + r * 8);
    }
    for f in (file + 1)..=6 {
        mask.0 |= 1 << (f + rank * 8);
    }
    if rank > 0 {
        for r in (1..=rank - 1).rev() {
            mask.0 |= 1 << (file + r * 8);
        }
    }
    if file > 0 {
        for f in (1..=file - 1).rev() {
            mask.0 |= 1 << (f + rank * 8);
        }
    }
    mask
}

fn bishop_mask(sq: Square) -> Bitboard {
    let mut mask = Bitboard(0);

    let rank = sq.rank() as u8;
    let file = sq.file() as u8;

    for i in 1..8 {
        if rank + i <= 6 && file + i <= 6 {
            mask.0 |= 1 << ((rank + i) * 8 + file + i);
        }
        if rank + i <= 6 && file > i {
            mask.0 |= 1 << ((rank + i) * 8 + file - i);
        }
        if rank > i && file + i <= 6 {
            mask.0 |= 1u64 << ((rank - i) * 8 + file + i);
        }
        if rank > i && file > i {
            mask.0 |= 1u64 << ((rank - i) * 8 + file - i);
        }
    }

    mask
}

fn rook_attacks(sq: Square, occ: Bitboard) -> Bitboard {
    let mut attacks = Bitboard(0);

    let rank = sq.rank() as u8;
    let file = sq.file() as u8;

    for r in (rank + 1)..8 {
        let bb = 1 << (file + r * 8);
        attacks.0 |= bb;
        if occ.0 & bb != 0 {
            break;
        }
    }
    for r in (1..=rank).rev() {
        let bb = 1 << (file + r * 8);
        attacks.0 |= bb;
        if occ.0 & bb != 0 {
            break;
        }
    }
    for f in (file + 1)..8 {
        let bb = 1 << (f + rank * 8);
        attacks.0 |= bb;
        if occ.0 & bb != 0 {
            break;
        }
    }
    for f in (1..=file).rev() {
        let bb = 1 << (f + rank * 8);
        attacks.0 |= bb;
        if occ.0 & bb != 0 {
            break;
        }
    }
    attacks
}

fn bishop_attacks(sq: Square, occ: Bitboard) -> Bitboard {
    let mut attacks = Bitboard(0);

    let rank = sq.rank() as u8;
    let file = sq.file() as u8;

    for i in 1..8 {
        if rank + i <= 7 && file + i <= 7 {
            let bb = 1 << ((rank + i) * 8 + file + i);
            attacks.0 |= bb;
            if occ.0 & bb != 0 {
                break;
            }
        }
        if rank + i <= 7 && file > i {
            let bb = 1 << ((rank + i) * 8 + file - i);
            attacks.0 |= bb;
            if occ.0 & bb != 0 {
                break;
            }
        }
        if rank > i && file + i <= 7 {
            let bb = 1 << ((rank - i) * 8 + file + i);
            attacks.0 |= bb;
            if occ.0 & bb != 0 {
                break;
            }
        }
        if rank > i && file > i {
            let bb = 1 << ((rank - i) * 8 + file - i);
            attacks.0 |= bb;
            if occ.0 & bb != 0 {
                break;
            }
        }
    }
    attacks
}

fn occupancy_bb(mask: &Bitboard, index: usize) -> Bitboard {
    let mut occ = Bitboard(0);

    // get indexes of all bits in mask
    let mut bits = Vec::new();
    let mut m = mask.0;
    while m != 0 {
        bits.push(m.trailing_zeros());
        m &= m - 1;
    }

    // set bits in occ according to index
    (0..bits.len()).for_each(|i| {
        if index & (1 << i) != 0 {
            occ.0 |= 1 << bits[i];
        }
    });
    occ
}

pub fn find_magic(sq: Square, shift: u8, bishop: bool, num_tries: usize) -> Option<u64> {
    let mut rng = SmallRng::from_entropy();
    let mut used = vec![0; 1 << shift];

    let mask = if bishop {
        bishop_mask(sq)
    } else {
        rook_mask(sq)
    };

    let n = mask.0.count_ones();
    // init occupancy array and attack array
    let mut occupancy = Vec::new();
    let mut attacks = Vec::new();
    for i in 0..(1 << n) {
        occupancy.push(occupancy_bb(&mask, i));
        attacks.push(if bishop {
            bishop_attacks(sq, occupancy[i])
        } else {
            rook_attacks(sq, occupancy[i])
        });
    }

    for _ in 0..num_tries {
        let magic = rng.gen::<u64>() & rng.gen::<u64>() & rng.gen::<u64>();
        used.iter_mut().for_each(|x| *x = 0);
        let mut fail = false;
        for i in 0..(1 << n) {
            let idx = (occupancy[i].0.wrapping_mul(magic) >> (64 - shift)) as usize;
            if used[idx] == 0 || used[idx] == attacks[i].0 {
                used[idx] = attacks[i].0;
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
        let sq = Square::make_square(File::D, Rank::R4);
        let mask = rook_mask(sq);
        assert_eq!(mask.0, 0x8080876080800);
    }

    #[test]
    fn test_rook_mask_2() {
        let sq = Square::make_square(File::A, Rank::R1);
        let mask = rook_mask(sq);
        assert_eq!(mask.0, 0x000101010101017e);
    }

    #[test]
    fn test_rook_mask_3() {
        let sq = Square::make_square(File::H, Rank::R7);
        let mask = rook_mask(sq);
        assert_eq!(mask.0, 0x007e808080808000);
    }

    #[test]
    fn test_bishop_mask_1() {
        let sq = Square::make_square(File::A, Rank::R1);
        let mask = bishop_mask(sq);
        assert_eq!(mask.0, 0x0040201008040200);
    }

    #[test]
    fn test_bishop_mask_2() {
        let sq = Square::make_square(File::G, Rank::R1);
        let mask = bishop_mask(sq);
        assert_eq!(mask.0, 0x20408102000);
    }

    #[test]
    fn test_index_to_u64_1() {
        let mask = Bitboard(0x0000000000000001);
        assert_eq!(occupancy_bb(&mask, 0).0, 0x0000000000000000);
        assert_eq!(occupancy_bb(&mask, 1).0, 0x0000000000000001);
    }

    #[test]
    fn test_index_to_u64_2() {
        let mask = Bitboard(0x001000000C000200);
        assert_eq!(occupancy_bb(&mask, 0).0, 0x0000000000000000);
        assert_eq!(occupancy_bb(&mask, 1).0, 0x200);
        assert_eq!(occupancy_bb(&mask, 2).0, 0x4000000);
        assert_eq!(occupancy_bb(&mask, 3).0, 0x4000200);
        assert_eq!(occupancy_bb(&mask, 4).0, 0x8000000);
        assert_eq!(occupancy_bb(&mask, 5).0, 0x8000200);
        assert_eq!(occupancy_bb(&mask, 6).0, 0xC000000);
        assert_eq!(occupancy_bb(&mask, 7).0, 0xC000200);
        assert_eq!(occupancy_bb(&mask, 8).0, 0x0010000000000000);
    }
}
