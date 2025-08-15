pub mod format;

mod utils;

use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Context;
use rand::prelude::IndexedRandom;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};

use crate::chess::movegen::MoveGen;
use crate::chess::position::fen::{Fen, STARTPOS};
use crate::chess::{Color, GameResult, Position};
use crate::datagen::format::{CompressedGame, Wdl};
use crate::engine::limits::Limits;
use crate::engine::{SearchManager, eval};

static STOP: AtomicBool = AtomicBool::new(false);
static TOTAL_GAMES: AtomicU32 = AtomicU32::new(0);
static WHITE_WINS: AtomicU32 = AtomicU32::new(0);
static BLACK_WINS: AtomicU32 = AtomicU32::new(0);
static DRAWS: AtomicU32 = AtomicU32::new(0);

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub struct DatagenConfig {
    pub limits: Limits,
    pub duration_mins: u32,
    pub hash_size_mb: u32,
    pub threads: u32,
    pub out_path: PathBuf,
    pub log_interval_secs: Option<u64>,
}

pub fn datagen(config: DatagenConfig) -> anyhow::Result<()> {
    ctrlc::set_handler(move || {
        STOP.store(true, std::sync::atomic::Ordering::Relaxed);
    })?;

    if let Some(parent) = config.out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    println!();

    let games: Arc<Mutex<Vec<CompressedGame>>> = Arc::new(Mutex::new(Vec::new()));
    let start_time = std::time::Instant::now();

    std::thread::scope(|s| {
        for i in 0..config.threads {
            s.spawn({
                let config = config.clone();
                let games_clone = games.clone();
                move || thread_worker(i, &config, games_clone, start_time)
            });
        }
        println!("{}/{} threads started", config.threads, config.threads);
        println!();
    });

    // Write all collected games to file
    let games_guard = games.lock().unwrap();
    let total_games = games_guard.len();
    
    if total_games > 0 {
        // Create timestamp for filename
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let stem = config.out_path.file_stem().unwrap_or_default().to_string_lossy();
        let ext = config.out_path
            .extension()
            .map_or("".to_string(), |e| format!(".{}", e.to_string_lossy()));
        let parent = config.out_path.parent().unwrap_or(Path::new("."));
        let final_path = parent.join(format!("{stem}_{timestamp}_{total_games}{ext}"));

        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&final_path)?;
        let mut writer = BufWriter::new(file);

        for game in games_guard.iter() {
            game.serialize_into(&mut writer)?;
        }
        writer.flush()?;
        writer.get_ref().sync_all()?;

        // Print notification
        println!("\n=== FILE COMPLETE ===");
        println!("Path: {}", final_path.display());
        println!("Games: {total_games}");
        if STOP.load(std::sync::atomic::Ordering::Relaxed) {
            println!("Note: Finalized on interruption");
        }
        println!("===================\n");
    }

    if STOP.load(std::sync::atomic::Ordering::Relaxed) {
        println!("Stopped by user");
    } else {
        println!("Datagen finished");
    }

    println!();
    println!(
        "Total: {}, White wins: {}, Black wins: {}, Draws: {}",
        TOTAL_GAMES.load(std::sync::atomic::Ordering::Relaxed),
        WHITE_WINS.load(std::sync::atomic::Ordering::Relaxed),
        BLACK_WINS.load(std::sync::atomic::Ordering::Relaxed),
        DRAWS.load(std::sync::atomic::Ordering::Relaxed)
    );
    Ok(())
}


fn thread_worker(
    id: u32,
    config: &DatagenConfig,
    games: Arc<Mutex<Vec<CompressedGame>>>,
    start_time: std::time::Instant,
) -> anyhow::Result<()> {
    let mut last_log = std::time::Instant::now();
    let duration = Duration::from_secs(config.duration_mins as u64 * 60);

    let Fen(pos) = STARTPOS.parse().unwrap();

    while start_time.elapsed() < duration {
        if STOP.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(());
        }

        let log_interval = Duration::from_secs(config.log_interval_secs.unwrap_or(60));
        if id == 0 && last_log.elapsed() > log_interval {
            last_log = std::time::Instant::now();

            let white_wins = WHITE_WINS.load(std::sync::atomic::Ordering::Relaxed);
            let black_wins = BLACK_WINS.load(std::sync::atomic::Ordering::Relaxed);
            let draws = DRAWS.load(std::sync::atomic::Ordering::Relaxed);

            let total = white_wins + black_wins + draws;
            let elapsed = start_time.elapsed();
            let remaining = duration.saturating_sub(elapsed);

            println!();
            println!("=== PROGRESS REPORT ===");
            println!(
                "Time: {:.1}/{:.1} minutes ({:.1}%)",
                elapsed.as_secs_f64() / 60.0,
                config.duration_mins,
                (elapsed.as_secs_f64() / (config.duration_mins as f64 * 60.0)) * 100.0
            );
            println!("Remaining: {:.1} minutes", remaining.as_secs_f64() / 60.0);
            println!("Total games: {total}");
            println!("Results - White: {white_wins}, Black: {black_wins}, Draws: {draws}",);
            println!(
                "Win rates - White: {:.1}%, Black: {:.1}%, Draw: {:.1}%",
                if total > 0 {
                    (white_wins as f64 / total as f64) * 100.0
                } else {
                    0.0
                },
                if total > 0 {
                    (black_wins as f64 / total as f64) * 100.0
                } else {
                    0.0
                },
                if total > 0 {
                    (draws as f64 / total as f64) * 100.0
                } else {
                    0.0
                }
            );
            println!("=====================\n");
        }

        if let Ok(game) = playout(&pos, config.limits, config.hash_size_mb as usize) {
            let mut games_guard = games.lock().unwrap();
            games_guard.push(game);
            TOTAL_GAMES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    Ok(())
}

fn playout(
    startpos: &Position,
    limits: Limits,
    hash_size_mb: usize,
) -> anyhow::Result<CompressedGame> {
    let mut initial_pos = startpos.clone();
    let mut rng = SmallRng::from_os_rng();

    // make random moves
    let num_random = if rng.random_bool(0.5) { 8 } else { 9 };

    for _ in 0..num_random {
        let m = MoveGen::new(&initial_pos).collect::<Vec<_>>();
        let mv = match m.choose(&mut rng) {
            Some(mv) => *mv,
            None => return Err(anyhow::anyhow!("No moves")),
        };
        initial_pos.make_move(mv);
    }
    let num_moves = MoveGen::new(&initial_pos).len();
    if num_moves == 0 {
        return Err(anyhow::anyhow!("No moves"));
    }

    match initial_pos.is_draw() {
        Some(GameResult::Loss) => {
            return Err(anyhow::anyhow!("Loss"));
        }
        Some(GameResult::Draw) => return Err(anyhow::anyhow!("Draw")),
        Some(GameResult::Win) => unreachable!(),
        None => {}
    }

    let mut game = CompressedGame::new(&initial_pos);
    let mut search = SearchManager::new_from_position(1, hash_size_mb, initial_pos);
    search.set_silent(true);
    let (res, _) = search.think(limits);

    // break early if eval is too extreme
    if res.score.abs() > 1_000 {
        return Err(anyhow::anyhow!("Extreme score"));
    }

    let mut drawish_count = 0;

    let result = loop {
        if STOP.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(anyhow::anyhow!("Stopped"));
        }
        let num_moves = MoveGen::new(&search.position).len();
        if num_moves == 0 {
            if search.position.in_check() {
                match search.position.side {
                    Color::Black => break Wdl::WhiteWin,
                    Color::White => break Wdl::BlackWin,
                }
            }
            break Wdl::Draw;
        }

        match search.position.is_draw() {
            Some(GameResult::Loss) => match search.position.side {
                Color::Black => break Wdl::WhiteWin,
                Color::White => break Wdl::BlackWin,
            },
            Some(GameResult::Draw) => break Wdl::Draw,
            Some(GameResult::Win) => unreachable!(),
            None => {}
        }

        if search.position.is_repetition(2) {
            break Wdl::Draw;
        }

        let (res, _) = search.think(limits);
        // exit if we find a mate score
        if res.score < (-eval::MATE_IN_PLY) {
            // current side is losing
            match search.position.side {
                Color::Black => break Wdl::WhiteWin,
                Color::White => break Wdl::BlackWin,
            }
        } else if res.score > eval::MATE_IN_PLY {
            // current side is winning
            match search.position.side {
                Color::Black => break Wdl::BlackWin,
                Color::White => break Wdl::WhiteWin,
            }
        } else if res.score.abs() < 10 {
            drawish_count += 1;
            if drawish_count > 14 {
                break Wdl::Draw;
            }
        } else {
            drawish_count = 0;
        }

        game.push_move(res.bestmove, res.score);
        search.position.make_move(res.bestmove);
    };

    game.set_win(result);

    match result {
        Wdl::WhiteWin => WHITE_WINS.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        Wdl::BlackWin => BLACK_WINS.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        Wdl::Draw => DRAWS.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        Wdl::Unknown => unreachable!(),
    };
    Ok(game)
}

pub fn bin_to_pgn(input: &PathBuf) -> anyhow::Result<()> {
    let mut file = std::fs::File::open(input).context(format!("Failed to open {input:?}"))?;
    while let Ok(game) = CompressedGame::deserialize_from(&mut file) {
        println!("{game}");
    }

    Ok(())
}

pub fn count_bin(input: &PathBuf) -> anyhow::Result<(u64, u64)> {
    let mut file = std::fs::File::open(input).context(format!("Failed to open {input:?}"))?;
    let mut game_count = 0;
    let mut position_count = 0;

    while let Ok(game) = CompressedGame::deserialize_from(&mut file) {
        game_count += 1;
        position_count += game.moves.len() as u64;
    }

    Ok((game_count, position_count))
}

pub fn count_bins(inputs: &[PathBuf]) -> anyhow::Result<()> {
    let mut total_games = 0;
    let mut total_positions = 0;

    for input in inputs {
        match count_bin(input) {
            Ok((games, positions)) => {
                println!("{}: {} games, {} positions", input.display(), games, positions);
                total_games += games;
                total_positions += positions;
            }
            Err(e) => {
                eprintln!("Error reading {}: {}", input.display(), e);
            }
        }
    }

    if inputs.len() > 1 {
        println!();
        println!("Total: {} games, {} positions", total_games, total_positions);
    }

    Ok(())
}
