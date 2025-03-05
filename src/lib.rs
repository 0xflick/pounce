pub mod bench;
pub mod bitboard;
pub mod chess;
pub mod limits;
pub mod movegen;
pub mod search;
pub mod tt;
pub mod uci;

mod eval;
mod movepicker;
mod util;

#[cfg(feature = "datagen")]
pub mod datagen;

use crate::chess::position::zobrist;

pub fn init() {
    movegen::init_tables();
    zobrist::init_zobrist();
    search::init_reductions();
}
