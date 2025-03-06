pub mod bitboard;
pub mod board;
pub mod chessmove;
pub mod color;
pub mod movegen;
pub mod piece;
pub mod position;

pub use board::{File, Rank, Square};
pub use chessmove::{Move, MoveType};
pub use color::Color;
pub use piece::{Piece, Role};
pub use position::Position;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum GameResult {
    Win,
    Loss,
    Draw,
}
