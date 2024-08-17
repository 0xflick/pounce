use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    num::NonZeroU32,
};

use crate::{
    board::Board,
    chess::{CastleRights, Color, File, Piece, Rank, Square},
    position::Position,
};

#[derive(Debug)]
pub enum ParseFenError {
    InvalidPartCount,
    InvalidBoard,
    InvalidColor,
    InvalidCastle,
    InvalidEpSquare,
    InvalidHalfmoveClock,
    InvalidFullmoveNumber,
}

impl Error for ParseFenError {}

impl Display for ParseFenError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            ParseFenError::InvalidPartCount => {
                write!(f, "FEN string must contain 6 parts separated by whitespace")
            }
            ParseFenError::InvalidBoard => write!(f, "invalid board part of FEN string"),
            ParseFenError::InvalidColor => write!(f, "invalid color part of FEN"),
            ParseFenError::InvalidCastle => write!(f, "invalid castling part of FEN"),
            ParseFenError::InvalidEpSquare => write!(f, "invalid en passant square part of FEN"),
            ParseFenError::InvalidHalfmoveClock => write!(f, "invalid halfmove clock part of FEN"),
            ParseFenError::InvalidFullmoveNumber => {
                write!(f, "invalid fullmove number part of FEN")
            }
        }
    }
}

pub struct Fen(pub Position);

impl Fen {
    pub fn parse(fen: &str) -> Result<Fen, ParseFenError> {
        let parts: Vec<&str> = fen.split_whitespace().collect();
        if parts.len() != 6 {
            return Err(ParseFenError::InvalidPartCount);
        }
        let board_str = parts[0];
        let side_str = parts[1];
        let castling_str = parts[2];
        let ep_square_str = parts[3];
        let halfmove_clock_str = parts[4];
        let fullmove_number_str = parts[5];

        let mut position = Position::new();
        position.board = parse_board_part(board_str)?;
        position.side = parse_side_part(side_str)?;
        position.castling = parse_castle_part(castling_str)?;
        position.ep_square = parse_ep_part(ep_square_str)?;
        position.halfmove_clock = parse_halfmove_clock_part(halfmove_clock_str)?;
        position.fullmove_number = parse_fullmove_number_part(fullmove_number_str)?;

        Ok(Fen(position))
    }
}

fn parse_board_part(board_str: &str) -> Result<Board, ParseFenError> {
    let iter = board_str.chars();
    let mut file = File::A;
    let mut rank = Rank::R8;

    let mut board = Board::new();

    for c in iter {
        match c {
            '/' => {
                rank = rank.down().ok_or(ParseFenError::InvalidBoard)?;
            }
            '1'..='8' => {
                let n = c.to_digit(10).unwrap() as u8;
                for _ in 0..n {
                    file = file.right_wrapped()
                }
            }
            _ => {
                let piece = Piece::from_char(c).ok_or(ParseFenError::InvalidBoard)?;
                board.set(Square::make(file, rank), piece);
                file = file.right_wrapped();
            }
        }
    }

    Ok(board)
}

fn parse_side_part(side_str: &str) -> Result<Color, ParseFenError> {
    match side_str {
        "w" => Ok(Color::White),
        "b" => Ok(Color::Black),
        _ => Err(ParseFenError::InvalidBoard),
    }
}

fn parse_castle_part(castle_str: &str) -> Result<CastleRights, ParseFenError> {
    let mut castling = CastleRights::empty();
    for c in castle_str.chars() {
        match c {
            'K' => castling.insert(CastleRights::WHITE_KING_SIDE),
            'Q' => castling.insert(CastleRights::WHITE_QUEEN_SIDE),
            'k' => castling.insert(CastleRights::BLACK_KING_SIDE),
            'q' => castling.insert(CastleRights::BLACK_QUEEN_SIDE),
            _ => return Err(ParseFenError::InvalidBoard),
        }
    }
    Ok(castling)
}

fn parse_ep_part(ep_str: &str) -> Result<Option<Square>, ParseFenError> {
    if ep_str == "-" {
        Ok(None)
    } else {
        let ep_square = ep_str.parse().map_err(|_| ParseFenError::InvalidEpSquare)?;
        Ok(Some(ep_square))
    }
}

fn parse_halfmove_clock_part(halfmove_clock_str: &str) -> Result<u16, ParseFenError> {
    halfmove_clock_str
        .parse()
        .map_err(|_| ParseFenError::InvalidHalfmoveClock)
}

fn parse_fullmove_number_part(fullmove_number_str: &str) -> Result<NonZeroU32, ParseFenError> {
    fullmove_number_str
        .parse()
        .map_err(|_| ParseFenError::InvalidFullmoveNumber)
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
