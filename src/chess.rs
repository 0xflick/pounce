pub mod board;
pub mod chessmove;
pub mod color;
pub mod movegen;
pub mod piece;
pub mod position;

pub use board::*;
pub use chessmove::*;
pub use color::*;
pub use movegen::*;
pub use piece::*;
pub use position::*;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum GameResult {
    Win,
    Loss,
    Draw,
}
