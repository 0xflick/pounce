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

        #[arg(short, long)]
        threads: Option<u32>,

        #[arg(long, default_value_t = 16)]
        hash_size: u32,

        #[arg(
            long,
            help = "Maximum file size in MB before rolling over to a new file"
        )]
        max_file_size_mb: Option<u64>,

        #[arg(
            long,
            help = "Number of games per file before rolling over to a new file"
        )]
        games_per_file: Option<u32>,

        #[arg(long, help = "Interval in seconds for progress reports (default: 60)")]
        log_interval_secs: Option<u64>,
    },
    BinToPgn {
        in_file: PathBuf,
    },
}

fn main() -> Result<()> {
    pounce::init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Gen {
            depth,
            out_path,
            num_games,
            threads,
            hash_size,
            max_file_size_mb,
            games_per_file,
            log_interval_secs,
        } => datagen::datagen(DatagenConfig {
            limits: Limits {
                depth: Some(depth),
                ..Limits::new()
            },
            num_games,
            hash_size_mb: hash_size,
            threads: threads.unwrap_or(1),
            out_path,
            max_file_size_mb,
            games_per_file,
            log_interval_secs,
        }),
        Commands::BinToPgn { in_file } => datagen::bin_to_pgn(&in_file),
    }
}
