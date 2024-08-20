use pounce::{movegen::gen_all_tables, uci::Uci};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    gen_all_tables();

    let mut uci = Uci::new();

    Ok((uci.run_loop())?)
}
