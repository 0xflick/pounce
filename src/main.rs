use anyhow::Result;
use clap::Parser;
use pounce::{
    fen::Fen,
    movegen::{gen_all_tables, perft},
    uci::Uci,
};

#[derive(Parser)]
struct Cli {
    /// Run perft test
    #[clap(long)]
    perft: Option<u8>,
}

fn main() -> Result<()> {
    gen_all_tables();

    let args = Cli::parse();
    if let Some(depth) = args.perft {
        let Fen(pos) = Uci::STARTPOS.parse()?;
        let now = std::time::Instant::now();
        let nodes = perft(pos, depth);
        let elapsed = now.elapsed();
        println!(
            "Nodes: {}, Time: {}s {}ms, Nodes/s: {:.2}M",
            nodes,
            elapsed.as_secs(),
            elapsed.subsec_millis(),
            (nodes as f64 / elapsed.as_secs_f64() / 1_000_000.0)
        );
        return Ok(());
    }

    let mut uci = Uci::new();

    uci.run_loop()
}
