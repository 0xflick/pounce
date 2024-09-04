use std::path::PathBuf;

use anyhow::{
    Ok,
    Result,
};
use clap::{
    Parser,
    Subcommand,
};
use pounce::{
    bench::bench,
    datagen::{
        self,
        DatagenConfig,
    },
    fen::Fen,
    limits::Limits,
    movegen::{
        init_tables,
        perft,
    },
    search::init_reductions,
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
    Perft {
        depth: u8,
    },
    Bench {
        #[arg(default_value_t = 7)]
        depth: u8,
    },
    Datagen {
        #[arg(short, long, default_value_t = 7)]
        depth: u8,

        #[arg(short, long)]
        out_file: String,

        #[arg(short, long)]
        num_games: u32,

        #[arg(short, long, default_value_t = 1)]
        concurrency: u32,

        #[arg(short, long, default_value_t = 16)]
        table_size: u32,
    },
    Datamix {
        in_files: Vec<PathBuf>,
        #[arg(short, long, required = true)]
        out_file: PathBuf,
    },
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
        Some(Commands::Bench { depth }) => {
            let limits = Limits {
                depth: Some(*depth),
                ..Default::default()
            };
            return bench(16, limits);
        }
        Some(Commands::Datagen {
            depth,
            out_file,
            num_games,
            concurrency,
            table_size,
        }) => {
            return datagen::datagen(DatagenConfig {
                limits: Limits {
                    depth: Some(*depth),
                    ..Limits::new()
                },
                num_games: num_games.to_owned(),
                tt_size_mb: *table_size,
                concurrency: concurrency.to_owned(),
                out_path: out_file.to_owned(),
            });
        }
        Some(Commands::Datamix { in_files, out_file }) => {
            datagen::shuffle_interleave(in_files, out_file);
            return Ok(());
        }

        _ => {}
    }

    let mut uci = Uci::new();

    uci.run_loop()
}

fn init() {
    init_tables();
    init_reductions();
    init_zobrist();
}
