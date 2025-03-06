pub mod bench;
pub mod chess;
pub mod limits;
pub mod search;
pub mod tt;
pub mod uci;

#[cfg(feature = "datagen")]
pub mod datagen;

mod eval;
mod movepicker;
mod utils;

use crate::chess::movegen;
use crate::chess::position::zobrist;

pub fn init() {
    movegen::init_tables();
    zobrist::init_zobrist();
    search::init_reductions();
}
