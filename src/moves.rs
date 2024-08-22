use std::{
    fmt,
    fmt::{Display, Formatter},
    str::FromStr,
};

use thiserror::Error;

use crate::chess::{ParseRoleError, ParseSquareError, Role, Square};

#[derive(Debug, Clone, Copy)]
pub struct Move(u16);

impl Move {
    #[inline]
    pub fn new(from: Square, to: Square, promotion: Option<Role>) -> Move {
        let from = from as u16;
        let to = to as u16;
        let promotion = promotion
            .map(|role| role as u16)
            .unwrap_or(Role::NUM as u16);
        Move(from | (to << 6) | (promotion << 12))
    }

    #[inline]
    pub fn from(self) -> Square {
        Square::new_unchecked((self.0 & 0x3f) as u8)
    }

    #[inline]
    pub fn to(self) -> Square {
        Square::new_unchecked(((self.0 >> 6) & 0x3f) as u8)
    }

    #[inline]
    pub fn promotion(self) -> Option<Role> {
        unsafe { std::mem::transmute((self.0 >> 12) as u8) }
    }

    #[inline]
    pub fn reverse(self) -> Move {
        Move::new(self.to(), self.from(), None)
    }
}

impl Display for Move {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}{}", self.from(), self.to())?;
        if let Some(promotion) = self.promotion() {
            write!(f, "{}", promotion)?;
        }
        Ok(())
    }
}

impl From<Move> for u16 {
    fn from(mv: Move) -> u16 {
        mv.0
    }
}

#[derive(Debug, Error)]
pub enum ParseMoveError {
    #[error("expected 4 or 5 characters, found {0}")]
    InvalidLength(usize),
    #[error("invalid square")]
    InvalidSquare(#[from] ParseSquareError),
    #[error("invalid role")]
    InvalidRole(#[from] ParseRoleError),
}

impl FromStr for Move {
    type Err = ParseMoveError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.len() {
            4 => {
                let from = Square::from_str(&s[0..2])?;
                let to = Square::from_str(&s[2..4])?;
                Ok(Move::new(from, to, None))
            }
            5 => {
                let from = Square::from_str(&s[0..2])?;
                let to = Square::from_str(&s[2..4])?;
                let promotion = Role::from_str(&s[4..5])?;
                Ok(Move::new(from, to, Some(promotion)))
            }
            _ => Err(ParseMoveError::InvalidLength(s.len())),
        }
    }
}
