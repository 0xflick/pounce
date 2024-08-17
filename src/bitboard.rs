use core::fmt;
use std::fmt::Formatter;

use crate::chess::{Color, File, Rank, Square};

#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
pub struct Bitboard(pub u64);

impl Bitboard {
    #[inline]
    pub const fn any(self) -> bool {
        self.0 != 0
    }

    #[inline]
    pub const fn none(self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub fn contains(self, sq: Square) -> bool {
        (self & Bitboard::from(sq)).any()
    }

    #[inline]
    pub const fn count(self) -> u32 {
        self.0.count_ones()
    }

    #[inline]
    pub fn set(&mut self, sq: Square) {
        *self |= Bitboard::from(sq);
    }

    #[inline]
    pub fn clear(&mut self, sq: Square) {
        *self &= !Bitboard::from(sq);
    }

    #[inline]
    pub fn toggle(&mut self, sq: Square) {
        *self ^= Bitboard::from(sq);
    }

    #[inline]
    pub fn north(self) -> Bitboard {
        self << 8
    }

    #[inline]
    pub fn south(self) -> Bitboard {
        self >> 8
    }

    #[inline]
    pub fn up(self, color: Color) -> Bitboard {
        if color == Color::White {
            self.north()
        } else {
            self.south()
        }
    }

    #[inline]
    pub fn down(self, color: Color) -> Bitboard {
        if color == Color::White {
            self.south()
        } else {
            self.north()
        }
    }

    pub const EMPTY: Bitboard = Bitboard(0);
    pub const FULL: Bitboard = Bitboard(0xFFFFFFFFFFFFFFFF);
}

impl fmt::Debug for Bitboard {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        for rank in Rank::ALL.iter().rev() {
            for file in File::ALL.iter() {
                let sq = Square::make(*file, *rank);
                if self.contains(sq) {
                    write!(f, "1")?;
                } else {
                    write!(f, ".")?;
                }
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

impl fmt::LowerHex for Bitboard {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        fmt::LowerHex::fmt(&self.0, f)
    }
}

impl fmt::UpperHex for Bitboard {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        fmt::UpperHex::fmt(&self.0, f)
    }
}

impl fmt::Binary for Bitboard {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        fmt::Binary::fmt(&self.0, f)
    }
}

impl From<Bitboard> for u64 {
    #[inline]
    fn from(bb: Bitboard) -> Self {
        bb.0
    }
}

impl From<Rank> for Bitboard {
    #[inline]
    fn from(rank: Rank) -> Self {
        Bitboard(0xFF << (rank as u8 * 8))
    }
}

impl From<File> for Bitboard {
    #[inline]
    fn from(file: File) -> Self {
        Bitboard(0x0101010101010101 << file as u8)
    }
}

impl From<Square> for Bitboard {
    #[inline]
    fn from(sq: Square) -> Self {
        Bitboard(1 << sq as u8)
    }
}

impl From<u64> for Bitboard {
    #[inline]
    fn from(bb: u64) -> Self {
        Bitboard(bb)
    }
}

impl<T> std::ops::BitAnd<T> for Bitboard
where
    T: Into<Bitboard>,
{
    type Output = Bitboard;
    #[inline]
    fn bitand(self, rhs: T) -> Bitboard {
        let Bitboard(rhs) = rhs.into();
        Bitboard(self.0 & rhs)
    }
}

impl<T> std::ops::BitAndAssign<T> for Bitboard
where
    T: Into<Bitboard>,
{
    #[inline]
    fn bitand_assign(&mut self, rhs: T) {
        let Bitboard(rhs) = rhs.into();
        self.0 &= rhs;
    }
}

impl<T> std::ops::BitOr<T> for Bitboard
where
    T: Into<Bitboard>,
{
    type Output = Bitboard;
    #[inline]
    fn bitor(self, rhs: T) -> Bitboard {
        let Bitboard(rhs) = rhs.into();
        Bitboard(self.0 | rhs)
    }
}

impl<T> std::ops::BitOrAssign<T> for Bitboard
where
    T: Into<Bitboard>,
{
    #[inline]
    fn bitor_assign(&mut self, rhs: T) {
        let Bitboard(rhs) = rhs.into();
        self.0 |= rhs;
    }
}

impl<T> std::ops::BitXor<T> for Bitboard
where
    T: Into<Bitboard>,
{
    type Output = Bitboard;
    #[inline]
    fn bitxor(self, rhs: T) -> Bitboard {
        let Bitboard(rhs) = rhs.into();
        Bitboard(self.0 ^ rhs)
    }
}

impl<T> std::ops::BitXorAssign<T> for Bitboard
where
    T: Into<Bitboard>,
{
    #[inline]
    fn bitxor_assign(&mut self, rhs: T) {
        let Bitboard(rhs) = rhs.into();
        self.0 ^= rhs;
    }
}

impl std::ops::Not for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn not(self) -> Self::Output {
        Bitboard(!self.0)
    }
}

impl std::ops::Shl<usize> for Bitboard {
    type Output = Self;
    #[inline]
    fn shl(self, rhs: usize) -> Bitboard {
        Bitboard(self.0 << rhs)
    }
}

impl std::ops::ShlAssign<usize> for Bitboard {
    #[inline]
    fn shl_assign(&mut self, rhs: usize) {
        self.0 <<= rhs;
    }
}

impl std::ops::Shr<usize> for Bitboard {
    type Output = Self;
    #[inline]
    fn shr(self, rhs: usize) -> Bitboard {
        Bitboard(self.0 >> rhs)
    }
}

impl std::ops::ShrAssign<usize> for Bitboard {
    #[inline]
    fn shr_assign(&mut self, rhs: usize) {
        self.0 >>= rhs;
    }
}

impl Iterator for Bitboard {
    type Item = Square;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.0 == 0 {
            return None;
        }
        let sq = Square::new_unchecked(self.0.trailing_zeros() as u8);
        *self ^= Bitboard::from(sq);
        Some(sq)
    }
}
