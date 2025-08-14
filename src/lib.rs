pub mod chess;
pub mod engine;

#[cfg(feature = "datagen")]
pub mod datagen;

use crate::chess::movegen;
use crate::chess::position::zobrist;
#[cfg(feature = "datagen")]
pub use crate::datagen::format::{CompressedGame, Wdl};
use crate::engine::search;

pub fn init() {
    movegen::init_tables();
    zobrist::init_zobrist();
    search::init_reductions();
}
