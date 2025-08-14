use std::ops::{Index, IndexMut};
use std::str::FromStr;

use thiserror::Error;

use crate::chess::Rank;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
#[repr(u8)]
pub enum Color {
    #[default]
    White = 0,
    Black,
}

impl Color {
    pub const fn new(color: u8) -> Color {
        assert!(color < 2);
        unsafe { std::mem::transmute(color) }
    }

    #[inline]
    pub fn home_rank(&self) -> Rank {
        match self {
            Color::White => Rank::R2,
            Color::Black => Rank::R7,
        }
    }

    #[inline]
    pub fn back_rank(&self) -> Rank {
        match self {
            Color::White => Rank::R1,
            Color::Black => Rank::R8,
        }
    }

    #[inline]
    pub fn double_pawn_rank(&self) -> Rank {
        match self {
            Color::White => Rank::R4,
            Color::Black => Rank::R5,
        }
    }

    #[inline]
    pub fn opponent(&self) -> Color {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }

    pub const ALL: [Color; 2] = [Color::White, Color::Black];
    pub const NUM: usize = 2;
}

#[derive(Error, Debug)]
#[error("invalid color: {0}")]
pub struct ParseColorError(String);

impl FromStr for Color {
    type Err = ParseColorError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "w" => Ok(Color::White),
            "b" => Ok(Color::Black),
            _ => Err(ParseColorError(s.to_string())),
        }
    }
}

impl<T> Index<Color> for [T; Color::NUM] {
    type Output = T;
    fn index(&self, index: Color) -> &Self::Output {
        unsafe { self.get_unchecked(index as usize) }
    }
}

impl<T> IndexMut<Color> for [T; Color::NUM] {
    fn index_mut(&mut self, index: Color) -> &mut Self::Output {
        unsafe { self.get_unchecked_mut(index as usize) }
    }
}

impl std::ops::Not for Color {
    type Output = Color;
    fn not(self) -> Self::Output {
        self.opponent()
    }
}
