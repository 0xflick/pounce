mod eval;
mod movepicker;
mod util;

pub mod bench;
pub mod bitboard;
pub mod chess;
pub mod fen;
pub mod limits;
pub mod movegen;
pub mod moves;
pub mod position;
pub mod san;
pub mod search;
pub mod tt;
pub mod uci;
pub mod zobrist;

#[cfg(feature = "datagen")]
pub mod datagen;

pub fn init() {
    movegen::init_tables();
    zobrist::init_zobrist();
    search::init_reductions();
}
