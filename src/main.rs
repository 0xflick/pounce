use anyhow::Result;
use pounce::{movegen::gen_all_tables, uci::Uci};

fn main() -> Result<()> {
    gen_all_tables();

    let mut uci = Uci::new();

    uci.run_loop()
}
