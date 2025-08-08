mod format;
mod utils;

use std::fmt::Debug;
use std::fs::{self, OpenOptions};
use std::io::{BufWriter, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64};
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
static CURRENT_FILE_SIZE: AtomicU64 = AtomicU64::new(0);
static CURRENT_FILE_GAMES: AtomicU32 = AtomicU32::new(0);

enum WriterWrapper {
    Simple(BufWriter<fs::File>),
    Rotating {
        base_path: PathBuf,
        current_path: PathBuf,
        writer: BufWriter<fs::File>,
        config: DatagenConfig,
    },
}

impl WriterWrapper {
    fn write_game(&mut self, game: &CompressedGame) -> anyhow::Result<()> {
        match self {
            WriterWrapper::Simple(writer) => {
                game.serialize_into(writer)?;
            }
            WriterWrapper::Rotating { base_path, current_path, writer, config } => {
                let max_size_bytes = config.max_file_size_mb.map(|mb| mb * 1024 * 1024);
                let current_size = CURRENT_FILE_SIZE.load(std::sync::atomic::Ordering::Relaxed);
                let current_games = CURRENT_FILE_GAMES.load(std::sync::atomic::Ordering::Relaxed);
                
                let should_rotate = max_size_bytes.map_or(false, |max| current_size >= max)
                    || config.games_per_file.map_or(false, |max| current_games >= max);
                
                if should_rotate {
                    writer.flush()?;
                    
                    // Create timestamp for new filename
                    let timestamp = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    
                    // Move temp file to final name with timestamp
                    let stem = base_path.file_stem().unwrap_or_default().to_string_lossy();
                    let ext = base_path.extension().map_or("".to_string(), |e| format!(".{}", e.to_string_lossy()));
                    let parent = base_path.parent().unwrap_or(Path::new("."));
                    let final_path = parent.join(format!("{}_{}_{}{}", stem, timestamp, current_games, ext));
                    
                    fs::rename(&current_path, &final_path)?;
                    
                    // Print notification for external watcher
                    println!("\n=== FILE COMPLETE ===");
                    println!("Path: {}", final_path.display());
                    println!("Games: {}", current_games);
                    println!("Size: {} MB", current_size as f64 / (1024.0 * 1024.0));
                    println!("===================\n");
                    
                    // Reset counters
                    CURRENT_FILE_SIZE.store(0, std::sync::atomic::Ordering::Relaxed);
                    CURRENT_FILE_GAMES.store(0, std::sync::atomic::Ordering::Relaxed);
                    
                    // Create new temp file
                    let new_temp_path = parent.join(format!(".{}_temp", stem));
                    let new_file = OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(true)
                        .open(&new_temp_path)?;
                    
                    *writer = BufWriter::new(new_file);
                    *current_path = new_temp_path;
                }
                
                // Write the game
                let size_before = writer.stream_position().unwrap_or(0);
                game.serialize_into(writer)?;
                let size_after = writer.stream_position().unwrap_or(0);
                
                let bytes_written = size_after - size_before;
                CURRENT_FILE_SIZE.fetch_add(bytes_written, std::sync::atomic::Ordering::Relaxed);
                CURRENT_FILE_GAMES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        }
        Ok(())
    }
}

fn create_rotating_writer(config: &DatagenConfig) -> anyhow::Result<Arc<Mutex<WriterWrapper>>> {
    let parent = config.out_path.parent().unwrap_or(Path::new("."));
    let stem = config.out_path.file_stem().unwrap_or_default().to_string_lossy();
    
    // Create temp file for writing
    let temp_path = parent.join(format!(".{}_temp", stem));
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&temp_path)?;
    
    Ok(Arc::new(Mutex::new(WriterWrapper::Rotating {
        base_path: config.out_path.clone(),
        current_path: temp_path,
        writer: BufWriter::new(file),
        config: config.clone(),
    })))
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub struct DatagenConfig {
    pub limits: Limits,
    pub num_games: u32,
    pub hash_size_mb: u32,
    pub threads: u32,
    pub out_path: PathBuf,
    pub max_file_size_mb: Option<u64>,
    pub games_per_file: Option<u32>,
}

pub fn datagen(config: DatagenConfig) -> anyhow::Result<()> {
    ctrlc::set_handler(move || {
        STOP.store(true, std::sync::atomic::Ordering::Relaxed);
    })?;

    if let Some(parent) = config.out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    println!();
    
    let writer = if config.max_file_size_mb.is_some() || config.games_per_file.is_some() {
        create_rotating_writer(&config)?
    } else {
        let file = OpenOptions::new()
            .read(true)
            .create(true)
            .append(true)
            .open(&config.out_path)
            .expect("Failed to open output file");
        Arc::new(Mutex::new(WriterWrapper::Simple(BufWriter::new(file))))
    };

    std::thread::scope(|s| {
        for i in 0..config.threads {
            s.spawn({
                let config = config.clone();
                let writer_clone = writer.clone();
                move || thread_worker(i, &config, writer_clone)
            });
        }
        println!("{}/{} threads started", config.threads, config.threads);
        println!();
    });
    
    // Finalize any remaining file
    if config.max_file_size_mb.is_some() || config.games_per_file.is_some() {
        let mut writer_guard = writer.lock().unwrap();
        if let WriterWrapper::Rotating { base_path, current_path, writer, .. } = &mut *writer_guard {
            writer.flush()?;
            
            let current_games = CURRENT_FILE_GAMES.load(std::sync::atomic::Ordering::Relaxed);
            let current_size = CURRENT_FILE_SIZE.load(std::sync::atomic::Ordering::Relaxed);
            
            if current_games > 0 {
                // Move final temp file to timestamped name
                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                
                let stem = base_path.file_stem().unwrap_or_default().to_string_lossy();
                let ext = base_path.extension().map_or("".to_string(), |e| format!(".{}", e.to_string_lossy()));
                let parent = base_path.parent().unwrap_or(Path::new("."));
                let final_path = parent.join(format!("{}_{}_{}{}", stem, timestamp, current_games, ext));
                
                fs::rename(current_path, &final_path)?;
                
                // Print final notification
                println!("\n=== FILE COMPLETE ===");
                println!("Path: {}", final_path.display());
                println!("Games: {}", current_games);
                println!("Size: {} MB", current_size as f64 / (1024.0 * 1024.0));
                println!("===================\n");
            } else {
                // No games written to temp file, remove it
                let _ = fs::remove_file(current_path);
            }
        }
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

#[derive(Debug, Serialize, Deserialize)]
struct DatagenState {
    white_wins: u32,
    black_wins: u32,
    draws: u32,

    config: DatagenConfig,
}

fn thread_worker(
    id: u32,
    config: &DatagenConfig,
    writer: Arc<Mutex<WriterWrapper>>,
) -> anyhow::Result<()> {
    let mut last_log = std::time::Instant::now();

    let Fen(pos) = STARTPOS.parse().unwrap();

    while TOTAL_GAMES.load(std::sync::atomic::Ordering::Relaxed) < config.num_games {
        if STOP.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(());
        }

        if id == 0 && last_log.elapsed() > Duration::from_secs(60) {
            last_log = std::time::Instant::now();

            let white_wins = WHITE_WINS.load(std::sync::atomic::Ordering::Relaxed);
            let black_wins = BLACK_WINS.load(std::sync::atomic::Ordering::Relaxed);
            let draws = DRAWS.load(std::sync::atomic::Ordering::Relaxed);

            let total = white_wins + black_wins + draws;

            println!();
            println!(
                "{}/{} Games, White wins: {}, Black wins: {}, Draws: {}",
                total, config.num_games, white_wins, black_wins, draws
            );
        }

        if let Ok(game) = playout(&pos, config.limits, config.hash_size_mb as usize) {
            TOTAL_GAMES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

            let mut writer_guard = writer.lock().unwrap();
            writer_guard.write_game(&game)?;
        }
    }

    Ok(())
}

fn playout(
    startpos: &Position,
    limits: Limits,
    hash_size_mb: usize,
) -> anyhow::Result<CompressedGame> {
    let mut pos = startpos.clone();
    let mut rng = SmallRng::from_os_rng();

    // make random moves
    let num_random = if rng.random_bool(0.5) { 8 } else { 9 };

    for _ in 0..num_random {
        let m = MoveGen::new(&pos).collect::<Vec<_>>();
        let mv = match m.choose(&mut rng) {
            Some(mv) => *mv,
            None => return Err(anyhow::anyhow!("No moves")),
        };
        pos.make_move(mv);
    }
    let startpos = pos.clone();
    let num_moves = MoveGen::new(&pos).len();
    if num_moves == 0 {
        return Err(anyhow::anyhow!("No moves"));
    }

    match pos.is_draw() {
        Some(GameResult::Loss) => {
            return Err(anyhow::anyhow!("Loss"));
        }
        Some(GameResult::Draw) => return Err(anyhow::anyhow!("Draw")),
        Some(GameResult::Win) => unreachable!(),
        None => {}
    }

    let mut search = SearchManager::new(1, hash_size_mb);
    search.set_silent(true);
    let (res, _) = search.think(limits);

    // break early if eval is too extreme
    if res.score.abs() > 1_500 {
        return Err(anyhow::anyhow!("Extreme score"));
    }

    let mut game = CompressedGame::new(startpos);

    let mut drawish_count = 0;

    let result = loop {
        if STOP.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(anyhow::anyhow!("Stopped"));
        }
        let num_moves = MoveGen::new(&pos).len();
        if num_moves == 0 {
            if pos.in_check() {
                match pos.side {
                    Color::Black => break Wdl::WhiteWin,
                    Color::White => break Wdl::BlackWin,
                }
            }
            break Wdl::Draw;
        }

        match pos.is_draw() {
            Some(GameResult::Loss) => match pos.side {
                Color::Black => break Wdl::WhiteWin,
                Color::White => break Wdl::BlackWin,
            },
            Some(GameResult::Draw) => break Wdl::Draw,
            Some(GameResult::Win) => unreachable!(),
            None => {}
        }

        if pos.is_repetition(2) {
            break Wdl::Draw;
        }

        let (res, _) = search.think(limits);
        // exit if we find a mate score
        if res.score < (-eval::MATE_IN_PLY) {
            // current side is losing
            match pos.side {
                Color::Black => break Wdl::WhiteWin,
                Color::White => break Wdl::BlackWin,
            }
        } else if res.score > eval::MATE_IN_PLY {
            // current side is winning
            match pos.side {
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

        game.push_move(res.bestmove, {
            if pos.side == Color::White {
                res.score
            } else {
                -res.score
            }
        });
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
