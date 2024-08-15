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

    pub const fn make_square(file: File, rank: Rank) -> Square {
        Square::new_unchecked((rank as u8 * 8) + file as u8)
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
}

#[repr(u8)]
pub enum Color {
    White,
    Black,
    None,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[repr(u8)]
pub enum PieceType {
    None,
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[repr(u8)]
pub enum Piece {
    None,
    WPawn,
    WKnight,
    WBishop,
    WRook,
    WQueen,
    WKing,
    BPawn,
    BKnight,
    BBishop,
    BRook,
    BQueen,
    BKing,
}

impl Piece {
    pub fn color(&self) -> Color {
        match self {
            Piece::None => Color::None,
            Piece::WPawn
            | Piece::WKnight
            | Piece::WBishop
            | Piece::WRook
            | Piece::WQueen
            | Piece::WKing => Color::White,
            Piece::BPawn
            | Piece::BKnight
            | Piece::BBishop
            | Piece::BRook
            | Piece::BQueen
            | Piece::BKing => Color::Black,
        }
    }

    pub fn piece_type(&self) -> PieceType {
        match self {
            Piece::None => PieceType::None,
            Piece::WPawn | Piece::BPawn => PieceType::Pawn,
            Piece::WKnight | Piece::BKnight => PieceType::Knight,
            Piece::WBishop | Piece::BBishop => PieceType::Bishop,
            Piece::WRook | Piece::BRook => PieceType::Rook,
            Piece::WQueen | Piece::BQueen => PieceType::Queen,
            Piece::WKing | Piece::BKing => PieceType::King,
        }
    }
}
