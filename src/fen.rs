use std::{
    fmt::{self, Display, Formatter},
    num::NonZeroU32,
    str::FromStr,
};

use thiserror::Error;

use crate::{
    chess::{CastleRights, Color, File, ParsePieceError, ParseSquareError, Rank, Square},
    position::Position,
};

#[derive(Debug, Error)]
pub enum ParseFenError {
    #[error("found `{0}` parts in FEN string, expected 6")]
    InvalidPartCount(usize),
    #[error("too many slashes")]
    TooManySlashesInBoard,
    #[error("could not parse piece character")]
    CouldNotParsePiece(#[from] ParsePieceError),
    #[error("could not parse color: '{0}'")]
    CouldNotParseColor(String),
    #[error("could not parse castling rights: '{0}'")]
    CouldNotParseCastle(String),
    #[error("invalid en-passant square")]
    InvalidEpSquare(#[from] ParseSquareError),
    #[error("invalid halfmove clock")]
    InvalidHalfmoveClock(#[source] std::num::ParseIntError),
    #[error("invalid fullmove number")]
    InvalidFullmoveNumber(#[source] std::num::ParseIntError),
}

type Result<T, E = ParseFenError> = std::result::Result<T, E>;

pub struct Fen(pub Position);

impl Fen {
    pub fn parse(fen: &str) -> Result<Fen> {
        let parts: Vec<&str> = fen.split_whitespace().collect();
        if parts.len() != 6 {
            return Err(ParseFenError::InvalidPartCount(parts.len()));
        }
        let board_str = parts[0];
        let side_str = parts[1];
        let castling_str = parts[2];
        let ep_square_str = parts[3];
        let halfmove_clock_str = parts[4];
        let fullmove_number_str = parts[5];

        let mut position = parse_board_part(board_str)?;
        position.side = parse_side_part(side_str)?;
        position.castling = parse_castle_part(castling_str)?;
        position.ep_square = parse_ep_part(ep_square_str)?;
        position.halfmove_clock = parse_halfmove_clock_part(halfmove_clock_str)?;
        position.fullmove_number = parse_fullmove_number_part(fullmove_number_str)?;

        position.refresh_checks_and_pins();
        position.key = position.zobrist_hash();

        Ok(Fen(position))
    }
}

impl FromStr for Fen {
    type Err = ParseFenError;
    fn from_str(fen: &str) -> Result<Self, Self::Err> {
        Fen::parse(fen)
    }
}

impl Display for Fen {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let Fen(position) = self;
        write!(f, "{}", position.to_fen())
    }
}

fn parse_board_part(board_str: &str) -> Result<Position> {
    let iter = board_str.chars();
    let mut file = File::A;
    let mut rank = Rank::R8;

    let mut position = Position::new();

    for c in iter {
        match c {
            '/' => {
                rank = rank.down().ok_or(ParseFenError::TooManySlashesInBoard)?;
            }
            '1'..='8' => {
                let n = c.to_digit(10).unwrap() as u8;
                for _ in 0..n {
                    file = file.east_wrapped()
                }
            }
            _ => {
                let piece = c.to_string().parse()?;
                position.set(Square::make(file, rank), piece);
                file = file.east_wrapped();
            }
        }
    }

    Ok(position)
}

fn parse_side_part(side_str: &str) -> Result<Color> {
    match side_str {
        "w" => Ok(Color::White),
        "b" => Ok(Color::Black),
        _ => Err(ParseFenError::CouldNotParseColor(side_str.to_string())),
    }
}

fn parse_castle_part(castle_str: &str) -> Result<CastleRights> {
    let mut castling = CastleRights::empty();
    for c in castle_str.chars() {
        match c {
            'K' => castling.insert(CastleRights::WHITE_KING_SIDE),
            'Q' => castling.insert(CastleRights::WHITE_QUEEN_SIDE),
            'k' => castling.insert(CastleRights::BLACK_KING_SIDE),
            'q' => castling.insert(CastleRights::BLACK_QUEEN_SIDE),
            '-' => castling = CastleRights::empty(),
            _ => return Err(ParseFenError::CouldNotParseCastle(castle_str.to_string())),
        }
    }
    Ok(castling)
}

fn parse_ep_part(ep_str: &str) -> Result<Option<Square>> {
    if ep_str == "-" {
        Ok(None)
    } else {
        let ep_square = ep_str.parse()?;
        Ok(Some(ep_square))
    }
}

fn parse_halfmove_clock_part(halfmove_clock_str: &str) -> Result<u16> {
    halfmove_clock_str
        .parse()
        .map_err(ParseFenError::InvalidHalfmoveClock)
}

fn parse_fullmove_number_part(fullmove_number_str: &str) -> Result<NonZeroU32> {
    fullmove_number_str
        .parse()
        .map_err(ParseFenError::InvalidFullmoveNumber)
}

impl Position {
    pub fn to_fen(&self) -> String {
        let mut fen = String::new();
        for rank in Rank::ALL.iter().rev() {
            let mut empty = 0;
            for file in File::ALL.iter() {
                let square = Square::make(*file, *rank);
                match self.piece_at(square) {
                    Some(piece) => {
                        if empty > 0 {
                            fen.push_str(&empty.to_string());
                            empty = 0;
                        }
                        fen.push_str(&piece.to_string())
                    }
                    None => {
                        empty += 1;
                    }
                }
            }
            if empty > 0 {
                fen.push_str(&empty.to_string());
            }
            if *rank != Rank::R1 {
                fen.push('/');
            }
        }
        format!(
            "{} {} {} {} {} {}",
            fen,
            self.side.to_fen(),
            self.castling.to_fen(),
            self.ep_square
                .map_or_else(|| "-".to_string(), |s| s.to_string()),
            self.halfmove_clock,
            self.fullmove_number
        )
    }
}

impl Color {
    fn to_fen(self) -> &'static str {
        match self {
            Color::White => "w",
            Color::Black => "b",
        }
    }
}

impl CastleRights {
    fn to_fen(self) -> String {
        if self.is_empty() {
            "-".to_string()
        } else {
            let mut s = String::new();
            if self.contains(CastleRights::WHITE_KING_SIDE) {
                s.push('K');
            }
            if self.contains(CastleRights::WHITE_QUEEN_SIDE) {
                s.push('Q');
            }
            if self.contains(CastleRights::BLACK_KING_SIDE) {
                s.push('k');
            }
            if self.contains(CastleRights::BLACK_QUEEN_SIDE) {
                s.push('q');
            }
            s
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_fen_parse() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let Fen(position) = Fen::parse(fen).unwrap();
        println!("{:?}", position);
        assert_eq!(position.side, Color::White);
        assert_eq!(position.castling, CastleRights::all());
        assert_eq!(position.ep_square, None);
        assert_eq!(position.halfmove_clock, 0);
        assert_eq!(position.fullmove_number, NonZeroU32::new(1).unwrap());
    }
}
