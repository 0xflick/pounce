use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use pounce::datagen::{self, DatagenConfig};
use pounce::engine::limits::Limits;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Gen {
        #[arg(short, long, default_value_t = 7)]
        depth: u8,

        #[arg(short, long)]
        out_path: PathBuf,

        #[arg(short, long)]
        num_games: u32,

        #[arg(short, long, default_value_t = 1)]
        threads: u32,

        #[arg(short, long, default_value_t = 16)]
        table_size: u32,

        #[arg(long)]
        state: Option<PathBuf>,
    },
    BinToPgn {
        in_file: PathBuf,
    },
}

fn main() -> Result<()> {
    pounce::init();

    let cli = Cli::parse();
    match &cli.command {
        Commands::Gen {
            depth,
            out_path,
            num_games,
            threads: concurrency,
            table_size,
            state,
        } => datagen::datagen(DatagenConfig {
            limits: Limits {
                depth: Some(*depth),
                ..Limits::new()
            },
            num_games: num_games.to_owned(),
            tt_size_mb: *table_size,
            concurrency: concurrency.to_owned(),
            out_path: out_path.to_owned(),
            state_path: state.clone(),
        }),
        Commands::BinToPgn { in_file } => datagen::bin_to_pgn(in_file),
    }
}
