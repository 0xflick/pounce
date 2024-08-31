use std::{
    fmt::{
        self,
        Display,
        Formatter,
    },
    ops::{
        BitXor,
        Index,
        IndexMut,
    },
    str::FromStr,
};

use bitflags::bitflags;
use thiserror::Error;

use crate::bitboard::Bitboard;

// A rank is a row on the chess board
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[repr(u8)]
pub enum Rank {
    R1,
    R2,
    R3,
    R4,
    R5,
    R6,
    R7,
    R8,
}

impl Rank {
    pub const fn new(rank: u8) -> Rank {
        assert!(rank < 8);
        Rank::new_unchecked(rank)
    }

    pub const fn new_unchecked(rank: u8) -> Rank {
        unsafe { std::mem::transmute(rank) }
    }

    pub const fn up(&self) -> Option<Rank> {
        if (*self as u8) < 7 {
            Some(Rank::new_unchecked(*self as u8 + 1))
        } else {
            None
        }
    }

    pub const fn down(&self) -> Option<Rank> {
        if (*self as u8) > 0 {
            Some(Rank::new_unchecked(*self as u8 - 1))
        } else {
            None
        }
    }

    pub fn distance(&self, other: Rank) -> u8 {
        let a = *self as u8;
        let b = other as u8;
        if a > b {
            a - b
        } else {
            b - a
        }
    }

    pub const fn from_char(c: char) -> Option<Rank> {
        match c {
            '1' => Some(Rank::R1),
            '2' => Some(Rank::R2),
            '3' => Some(Rank::R3),
            '4' => Some(Rank::R4),
            '5' => Some(Rank::R5),
            '6' => Some(Rank::R6),
            '7' => Some(Rank::R7),
            '8' => Some(Rank::R8),
            _ => None,
        }
    }

    pub const fn char(&self) -> char {
        match self {
            Rank::R1 => '1',
            Rank::R2 => '2',
            Rank::R3 => '3',
            Rank::R4 => '4',
            Rank::R5 => '5',
            Rank::R6 => '6',
            Rank::R7 => '7',
            Rank::R8 => '8',
        }
    }

    pub const ALL: [Rank; 8] = [
        Rank::R1,
        Rank::R2,
        Rank::R3,
        Rank::R4,
        Rank::R5,
        Rank::R6,
        Rank::R7,
        Rank::R8,
    ];

    pub const NUM: usize = 8;
}

impl<T> Index<Rank> for [T; Rank::NUM] {
    type Output = T;
    fn index(&self, index: Rank) -> &Self::Output {
        unsafe { self.get_unchecked(index as usize) }
    }
}

impl<T> IndexMut<Rank> for [T; Rank::NUM] {
    fn index_mut(&mut self, index: Rank) -> &mut Self::Output {
        unsafe { self.get_unchecked_mut(index as usize) }
    }
}

// A file is a column on the chess board
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[repr(u8)]
pub enum File {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
}

impl File {
    pub const fn new(file: u8) -> File {
        assert!(file < 8);
        File::new_unchecked(file)
    }

    pub const fn new_unchecked(file: u8) -> File {
        unsafe { std::mem::transmute(file) }
    }

    pub fn east(&self) -> Option<File> {
        if (*self as u8) < 7 {
            Some(File::new_unchecked((*self as u8 + 1) % 8))
        } else {
            None
        }
    }

    pub fn east_wrapped(&self) -> File {
        File::new_unchecked((*self as u8 + 1) % 8)
    }

    pub fn west(&self) -> Option<File> {
        if (*self as u8) > 0 {
            Some(File::new_unchecked((*self as u8 - 1) % 8))
        } else {
            None
        }
    }

    pub fn distance(&self, other: File) -> u8 {
        let a = *self as u8;
        let b = other as u8;
        if a > b {
            a - b
        } else {
            b - a
        }
    }

    pub fn direction(&self, other: File) -> i8 {
        let a = *self as i8;
        let b = other as i8;
        b - a
    }

    pub const fn from_char(c: char) -> Option<File> {
        match c {
            'a' | 'A' => Some(File::A),
            'b' | 'B' => Some(File::B),
            'c' | 'C' => Some(File::C),
            'd' | 'D' => Some(File::D),
            'e' | 'E' => Some(File::E),
            'f' | 'F' => Some(File::F),
            'g' | 'G' => Some(File::G),
            'h' | 'H' => Some(File::H),
            _ => None,
        }
    }

    pub const fn char(&self) -> char {
        match self {
            File::A => 'a',
            File::B => 'b',
            File::C => 'c',
            File::D => 'd',
            File::E => 'e',
            File::F => 'f',
            File::G => 'g',
            File::H => 'h',
        }
    }

    pub const ALL: [File; 8] = [
        File::A,
        File::B,
        File::C,
        File::D,
        File::E,
        File::F,
        File::G,
        File::H,
    ];

    pub const NUM: usize = 8;
}

impl<T> Index<File> for [T; File::NUM] {
    type Output = T;
    fn index(&self, index: File) -> &Self::Output {
        unsafe { self.get_unchecked(index as usize) }
    }
}

impl<T> IndexMut<File> for [T; File::NUM] {
    fn index_mut(&mut self, index: File) -> &mut Self::Output {
        unsafe { self.get_unchecked_mut(index as usize) }
    }
}

// A square is a position on the chess board
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[repr(u8)]
#[rustfmt::skip]
pub enum Square {
    A1 = 0, B1, C1, D1, E1, F1, G1, H1,
    A2, B2, C2, D2, E2, F2, G2, H2,
    A3, B3, C3, D3, E3, F3, G3, H3,
    A4, B4, C4, D4, E4, F4, G4, H4,
    A5, B5, C5, D5, E5, F5, G5, H5,
    A6, B6, C6, D6, E6, F6, G6, H6,
    A7, B7, C7, D7, E7, F7, G7, H7,
    A8, B8, C8, D8, E8, F8, G8, H8,
}

impl Square {
    pub const fn new(square: u8) -> Square {
        assert!(square < 64);
        Square::new_unchecked(square)
    }

    pub const fn new_unchecked(square: u8) -> Square {
        unsafe { std::mem::transmute(square) }
    }

    pub const fn rank(&self) -> Rank {
        Rank::new((*self as u8) / 8)
    }

    pub const fn file(&self) -> File {
        File::new((*self as u8) % 8)
    }

    pub const fn bb(&self) -> Bitboard {
        Bitboard(1 << (*self as u8))
    }

    pub const fn make(file: File, rank: Rank) -> Square {
        Square::new_unchecked((rank as u8 * 8) + file as u8)
    }

    pub fn north(&self) -> Option<Square> {
        self.rank().up().map(|r| Square::make(self.file(), r))
    }

    pub fn south(&self) -> Option<Square> {
        self.rank().down().map(|r| Square::make(self.file(), r))
    }

    pub fn east(&self) -> Option<Square> {
        self.file().east().map(|f| Square::make(f, self.rank()))
    }

    pub fn west(&self) -> Option<Square> {
        self.file().west().map(|f| Square::make(f, self.rank()))
    }

    pub fn same_color(&self, other: Square) -> bool {
        (9 * (*self as u16 ^ other as u16)) & 8 == 0
    }

    #[inline]
    pub fn up(&self, color: Color) -> Option<Square> {
        match color {
            Color::White => self.north(),
            Color::Black => self.south(),
        }
    }

    pub fn down(&self, color: Color) -> Option<Square> {
        match color {
            Color::White => self.south(),
            Color::Black => self.north(),
        }
    }

    #[rustfmt::skip]
    pub const ALL: [Square; 64] = [
        Square::A1, Square::B1, Square::C1, Square::D1, Square::E1, Square::F1, Square::G1, Square::H1,
        Square::A2, Square::B2, Square::C2, Square::D2, Square::E2, Square::F2, Square::G2, Square::H2,
        Square::A3, Square::B3, Square::C3, Square::D3, Square::E3, Square::F3, Square::G3, Square::H3,
        Square::A4, Square::B4, Square::C4, Square::D4, Square::E4, Square::F4, Square::G4, Square::H4,
        Square::A5, Square::B5, Square::C5, Square::D5, Square::E5, Square::F5, Square::G5, Square::H5,
        Square::A6, Square::B6, Square::C6, Square::D6, Square::E6, Square::F6, Square::G6, Square::H6,
        Square::A7, Square::B7, Square::C7, Square::D7, Square::E7, Square::F7, Square::G7, Square::H7,
        Square::A8, Square::B8, Square::C8, Square::D8, Square::E8, Square::F8, Square::G8, Square::H8,
    ];

    pub const NUM: usize = 64;
}

#[derive(Error, Debug)]
pub enum ParseSquareError {
    #[error("expected 2 characters, got {0}")]
    InvalidLength(usize),
    #[error("invalid file")]
    InvalidFile,
    #[error("invalid rank")]
    InvalidRank,
}

impl std::str::FromStr for Square {
    type Err = ParseSquareError;
    fn from_str(s: &str) -> Result<Square, ParseSquareError> {
        if s.len() != 2 {
            return Err(ParseSquareError::InvalidLength(s.len()));
        }

        match (
            File::from_char(s.chars().nth(0).unwrap()),
            Rank::from_char(s.chars().nth(1).unwrap()),
        ) {
            (Some(file), Some(rank)) => Ok(Square::make(file, rank)),
            (None, _) => Err(ParseSquareError::InvalidFile),
            (_, None) => Err(ParseSquareError::InvalidRank),
        }
    }
}

impl Display for Square {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}{}", self.file().char(), self.rank().char())
    }
}

impl From<Bitboard> for Square {
    fn from(bb: Bitboard) -> Square {
        debug_assert_ne!(bb, Bitboard::EMPTY);
        Square::new(bb.0.trailing_zeros() as u8)
    }
}

impl From<u8> for Square {
    fn from(sq: u8) -> Square {
        Square::new(sq)
    }
}

impl<T> Index<Square> for [T; Square::NUM] {
    type Output = T;
    fn index(&self, index: Square) -> &Self::Output {
        unsafe { self.get_unchecked(index as usize) }
    }
}

impl<T> IndexMut<Square> for [T; Square::NUM] {
    fn index_mut(&mut self, index: Square) -> &mut Self::Output {
        unsafe { self.get_unchecked_mut(index as usize) }
    }
}

impl<T> BitXor<T> for Square
where
    T: Into<Square>,
{
    type Output = Self;
    #[inline]
    fn bitxor(self, rhs: T) -> Self {
        let rhs = rhs.into();
        Square::new_unchecked(self as u8 ^ rhs as u8)
    }
}

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

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum GameResult {
    Win,
    Loss,
    Draw,
}

#[cfg(test)]
mod test {
    use crate::bitboard::Bitboard;

    #[test]
    fn from_sq_a1() {
        let sq = super::Square::A1;
        assert_eq!(sq.rank(), super::Rank::R1);
        assert_eq!(sq.file(), super::File::A);

        let bb = Bitboard::from(sq);
        assert_eq!(bb, Bitboard(1));
        assert_eq!(super::Square::from(bb), sq);
    }

    #[test]
    fn from_sq_h8() {
        let sq = super::Square::H8;
        assert_eq!(sq.rank(), super::Rank::R8);
        assert_eq!(sq.file(), super::File::H);

        let bb = Bitboard::from(sq);
        assert_eq!(bb, Bitboard(1 << 63));
        assert_eq!(super::Square::from(bb), sq);
    }
}
