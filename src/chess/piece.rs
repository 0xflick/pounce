use std::fmt::{self, Display, Formatter};
use std::ops::{Index, IndexMut};
use std::str::FromStr;

use bitflags::bitflags;
use thiserror::Error;

use crate::chess::{Color, Square};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
#[repr(u8)]
pub enum Role {
    #[default]
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

impl Role {
    pub const fn new(role: u8) -> Role {
        assert!(role < 6);
        unsafe { std::mem::transmute(role) }
    }
}

impl Role {
    pub const ALL: [Role; 6] = [
        Role::Pawn,
        Role::Knight,
        Role::Bishop,
        Role::Rook,
        Role::Queen,
        Role::King,
    ];

    pub const NUM: usize = 6;
}

impl Display for Role {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let c = match self {
            Role::Pawn => 'P',
            Role::Knight => 'N',
            Role::Bishop => 'B',
            Role::Rook => 'R',
            Role::Queen => 'Q',
            Role::King => 'K',
        };
        write!(f, "{}", c)
    }
}

impl<T> Index<Role> for [T; Role::NUM] {
    type Output = T;
    fn index(&self, index: Role) -> &Self::Output {
        unsafe { self.get_unchecked(index as usize) }
    }
}

impl<T> IndexMut<Role> for [T; Role::NUM] {
    fn index_mut(&mut self, index: Role) -> &mut Self::Output {
        unsafe { self.get_unchecked_mut(index as usize) }
    }
}

#[derive(Error, Debug)]
#[error("invalid role: {0}")]
pub struct ParseRoleError(String);

impl FromStr for Role {
    type Err = ParseRoleError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "P" | "p" => Ok(Role::Pawn),
            "N" | "n" => Ok(Role::Knight),
            "B" | "b" => Ok(Role::Bishop),
            "R" | "r" => Ok(Role::Rook),
            "Q" | "q" => Ok(Role::Queen),
            "K" | "k" => Ok(Role::King),
            _ => Err(ParseRoleError(s.to_string())),
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Piece {
    pub color: Color,
    pub role: Role,
}

impl Piece {
    pub fn new(color: Color, role: Role) -> Piece {
        Piece { color, role }
    }
}

impl Display for Piece {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match (self.role, self.color) {
            (Role::Pawn, Color::White) => write!(f, "P"),
            (Role::Knight, Color::White) => write!(f, "N"),
            (Role::Bishop, Color::White) => write!(f, "B"),
            (Role::Rook, Color::White) => write!(f, "R"),
            (Role::Queen, Color::White) => write!(f, "Q"),
            (Role::King, Color::White) => write!(f, "K"),
            (Role::Pawn, Color::Black) => write!(f, "p"),
            (Role::Knight, Color::Black) => write!(f, "n"),
            (Role::Bishop, Color::Black) => write!(f, "b"),
            (Role::Rook, Color::Black) => write!(f, "r"),
            (Role::Queen, Color::Black) => write!(f, "q"),
            (Role::King, Color::Black) => write!(f, "k"),
        }
    }
}

#[derive(Error, Debug)]
#[error("invalid piece: {0}")]
pub struct ParsePieceError(String);

impl FromStr for Piece {
    type Err = ParsePieceError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "P" => Ok(Piece::new(Color::White, Role::Pawn)),
            "N" => Ok(Piece::new(Color::White, Role::Knight)),
            "B" => Ok(Piece::new(Color::White, Role::Bishop)),
            "R" => Ok(Piece::new(Color::White, Role::Rook)),
            "Q" => Ok(Piece::new(Color::White, Role::Queen)),
            "K" => Ok(Piece::new(Color::White, Role::King)),
            "p" => Ok(Piece::new(Color::Black, Role::Pawn)),
            "n" => Ok(Piece::new(Color::Black, Role::Knight)),
            "b" => Ok(Piece::new(Color::Black, Role::Bishop)),
            "r" => Ok(Piece::new(Color::Black, Role::Rook)),
            "q" => Ok(Piece::new(Color::Black, Role::Queen)),
            "k" => Ok(Piece::new(Color::Black, Role::King)),
            _ => Err(ParsePieceError(s.to_string())),
        }
    }
}

bitflags! {
    #[derive(Debug, Copy, Clone, Eq, PartialEq)]
    pub struct CastleRights: u8 {
        const WHITE_KING_SIDE = 0b0001;
        const WHITE_QUEEN_SIDE = 0b0010;
        const BLACK_KING_SIDE = 0b0100;
        const BLACK_QUEEN_SIDE = 0b1000;
    }
}

impl CastleRights {
    pub fn new() -> CastleRights {
        CastleRights::all()
    }
}

impl Default for CastleRights {
    fn default() -> CastleRights {
        CastleRights::new()
    }
}

impl CastleRights {
    pub fn discard_color(&mut self, color: Color) {
        match color {
            Color::White => {
                self.remove(CastleRights::WHITE_KING_SIDE | CastleRights::WHITE_QUEEN_SIDE);
            }
            Color::Black => {
                self.remove(CastleRights::BLACK_KING_SIDE | CastleRights::BLACK_QUEEN_SIDE);
            }
        }
    }

    pub fn discard_square(&mut self, square: Square) {
        match square {
            Square::A1 => self.remove(CastleRights::WHITE_QUEEN_SIDE),
            Square::H1 => self.remove(CastleRights::WHITE_KING_SIDE),
            Square::A8 => self.remove(CastleRights::BLACK_QUEEN_SIDE),
            Square::H8 => self.remove(CastleRights::BLACK_KING_SIDE),
            _ => {}
        }
    }

    pub fn can_castle_kingside(&self, color: Color) -> bool {
        match color {
            Color::White => self.contains(CastleRights::WHITE_KING_SIDE),
            Color::Black => self.contains(CastleRights::BLACK_KING_SIDE),
        }
    }

    pub fn can_castle_queenside(&self, color: Color) -> bool {
        match color {
            Color::White => self.contains(CastleRights::WHITE_QUEEN_SIDE),
            Color::Black => self.contains(CastleRights::BLACK_QUEEN_SIDE),
        }
    }
}
