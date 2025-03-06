use anyhow::Result;
use pounce::engine::Uci;

fn main() -> Result<()> {
    pounce::init();

    let mut uci = Uci::new();
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 {
        uci.run_once(&args[1])
    } else {
        uci.run_loop()
    }
}
