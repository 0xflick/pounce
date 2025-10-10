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
        #[arg(short, long, conflicts_with = "nodes")]
        depth: Option<u8>,

        #[arg(long, conflicts_with = "depth")]
        nodes: Option<u64>,

        #[arg(short, long)]
        out_path: PathBuf,

        #[arg(short, long)]
        num_games: u32,

        #[arg(short, long)]
        threads: Option<u32>,

        #[arg(long, default_value_t = 16)]
        hash_size: u32,

        #[arg(long, help = "Interval in seconds to save collected games to file")]
        save_interval_secs: u64,

        #[arg(long, help = "Interval in seconds for progress reports (default: 60)")]
        log_interval_secs: Option<u64>,
    },
    BinToPgn {
        in_file: PathBuf,
    },
    Count {
        in_files: Vec<PathBuf>,
    },
}

fn main() -> Result<()> {
    pounce::init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Gen {
            depth,
            nodes,
            out_path,
            num_games,
            threads,
            hash_size,
            save_interval_secs,
            log_interval_secs,
        } => {
            let limits = if let Some(n) = nodes {
                Limits {
                    nodes: Some(n),
                    ..Limits::new()
                }
            } else {
                Limits {
                    depth: Some(depth.unwrap_or(7)), // default to depth 7 if nothing
                    // specified
                    ..Limits::new()
                }
            };

            datagen::datagen(DatagenConfig {
                limits,
                num_games,
                hash_size_mb: hash_size,
                threads: threads.unwrap_or(1),
                out_path,
                save_interval_secs,
                log_interval_secs,
            })
        }
        Commands::BinToPgn { in_file } => datagen::bin_to_pgn(&in_file),
        Commands::Count { in_files } => datagen::count_bins(&in_files),
    }
}
