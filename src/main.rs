use anyhow::Result;
use clap::{Parser, Subcommand};
use pounce::{
    bench::bench,
    fen::Fen,
    movegen::{init_tables, perft},
    uci::Uci,
    zobrist::init_zobrist,
};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[clap(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Perft { depth: u8 },
    Bench,
}

fn main() -> Result<()> {
    init();

    let cli = Cli::parse();
    match &cli.command {
        Some(Commands::Perft { depth }) => {
            let Fen(mut pos) = Uci::STARTPOS.parse()?;
            let now = std::time::Instant::now();
            let nodes = perft(&mut pos, *depth);
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
        Some(Commands::Bench) => {
            return bench(16);
        }

        _ => {}
    }

    let mut uci = Uci::new();

    uci.run_loop()
}

fn init() {
    init_tables();
    init_zobrist();
}
