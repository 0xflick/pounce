use core::fmt;
use std::fmt::{Display, Formatter};

use crate::chess::{File, Rank, Square};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct Bitboard(pub u64);

impl Display for Bitboard {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        for rank in (Rank::R1 as u8..=Rank::R8 as u8).rev() {
            for file in File::ALL.iter() {
                let sq = Square::make_square(*file, Rank::new(rank));
                if self.0 & sq.bb().0 != 0 {
                    write!(f, "1")?;
                } else {
                    write!(f, ".")?;
                }
            }
            writeln!(f)?;
        }
        writeln!(f, "hex: 0x{:x}", self.0)
    }
}
