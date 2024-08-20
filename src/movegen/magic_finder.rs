use magic::{bishop_attacks, occupancy_bb, rook_attacks};
use rand::{rngs::SmallRng, Rng, SeedableRng};

use crate::{bitboard::Bitboard, chess::Square, movegen::*};

pub struct Wizard {
    rng: SmallRng,

    r_masks: [Bitboard; 64],
    r_attacks: Vec<Vec<Bitboard>>,

    b_masks: [Bitboard; 64],
    b_attacks: Vec<Vec<Bitboard>>,
}

impl Wizard {
    pub fn new() -> Self {
        let rng = SmallRng::from_entropy();
        let mut r_masks = [Bitboard(0); 64];
        let mut b_masks = [Bitboard(0); 64];

        let mut r_attacks = vec![vec![Bitboard(0); 4096]; 64];
        let mut b_attacks = vec![vec![Bitboard(0); 4096]; 64];

        for sq in Square::ALL.into_iter() {
            let r_mask = rook_mask(sq);
            for i in 0..1 << r_mask.count() {
                let occupancy = occupancy_bb(&r_mask, i as usize);
                r_attacks[sq as usize][i as usize] = rook_attacks(sq, occupancy);
            }
            r_masks[sq as usize] = r_mask;

            let b_mask = bishop_mask(sq);
            for i in 0..1 << b_mask.count() {
                let occupancy = occupancy_bb(&b_mask, i as usize);
                b_attacks[sq as usize][i as usize] = bishop_attacks(sq, occupancy);
            }
            b_masks[sq as usize] = b_mask;
        }

        Wizard {
            rng,
            r_masks,
            r_attacks,
            b_masks,
            b_attacks,
        }
    }

    pub fn find_magic(
        &mut self,
        sq: Square,
        shift: u8,
        bishop: bool,
        num_tries: usize,
    ) -> Option<u64> {
        let mask = if bishop {
            self.b_masks[sq as usize]
        } else {
            self.r_masks[sq as usize]
        };

        let attacks = if bishop {
            &self.b_attacks[sq as usize]
        } else {
            &self.r_attacks[sq as usize]
        };

        let mut local_attacks: [Bitboard; 4096] = [Bitboard(0); 4096];
        for (i, bb) in attacks.iter().enumerate() {
            local_attacks[i] = *bb;
        }

        let mut used = vec![Bitboard(0); 1 << shift];

        for _ in 0..num_tries {
            let magic = self.rng.gen::<u64>() & self.rng.gen::<u64>() & self.rng.gen::<u64>();
            used.fill(Bitboard(0));

            let mut fail = false;
            let mut occ = Bitboard(0);

            let mut i = 0;
            loop {
                let idx = (occ.0.wrapping_mul(magic) >> (64 - shift)) as usize;

                if used[idx].none() {
                    used[idx] = local_attacks[i];
                } else if used[idx] != local_attacks[i] {
                    fail = true;
                    break;
                }

                occ = Bitboard(occ.0.wrapping_sub(mask.0)) & mask.0;
                if occ.none() {
                    break;
                }
                i += 1;
            }

            if !fail {
                return Some(magic);
            }
        }

        None
    }
}

impl Default for Wizard {
    fn default() -> Self {
        Self::new()
    }
}

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
