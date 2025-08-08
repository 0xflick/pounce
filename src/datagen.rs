mod format;
mod utils;

use std::fmt::Debug;
use std::fs::OpenOptions;
use std::io::BufWriter;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::{Arc, Mutex};
use std::time::Duration;

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
    pub num_games: u32,
    pub hash_size_mb: u32,
    pub threads: u32,
    pub out_path: PathBuf,
}

pub fn datagen(config: DatagenConfig) -> anyhow::Result<()> {
    ctrlc::set_handler(move || {
        STOP.store(true, std::sync::atomic::Ordering::Relaxed);
    })?;

    if let Some(parent) = config.out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    println!();
    let file = OpenOptions::new()
        .read(true)
        .create(true)
        .append(true)
        .open(&config.out_path)
        .expect("Failed to open output file");

    std::thread::scope(|s| {
        let buf_writer = BufWriter::new(file);
        let shared_writer = Arc::new(Mutex::new(buf_writer));
        for i in 0..config.threads {
            s.spawn({
                let config = config.clone();
                let writer_clone = shared_writer.clone();
                move || thread_worker(i, &config, writer_clone)
            });
        }
        println!("{}/{} threads started", config.threads, config.threads);
        println!();
    });

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
    writer: Arc<Mutex<impl std::io::Write>>,
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
            game.serialize_into(writer_guard.by_ref())?;
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
