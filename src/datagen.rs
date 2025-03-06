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
use crate::engine::eval;
use crate::engine::limits::Limits;
use crate::engine::search::Search;
use crate::engine::tt::Table;

static STOP: AtomicBool = AtomicBool::new(false);
static TOTAL_GAMES: AtomicU32 = AtomicU32::new(0);
static WHITE_WINS: AtomicU32 = AtomicU32::new(0);
static BLACK_WINS: AtomicU32 = AtomicU32::new(0);
static DRAWS: AtomicU32 = AtomicU32::new(0);
static NUM_AT_RESTART: AtomicU32 = AtomicU32::new(0);

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub struct DatagenConfig {
    pub limits: Limits,
    pub num_games: u32,
    pub tt_size_mb: u32,
    pub concurrency: u32,
    pub out_path: PathBuf,
    pub state_path: Option<PathBuf>,
}

pub fn datagen(mut config: DatagenConfig) -> anyhow::Result<()> {
    // start playout threads, share global state, print results
    ctrlc::set_handler(move || {
        STOP.store(true, std::sync::atomic::Ordering::Relaxed);
    })?;

    if let Some(parent) = config.out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    println!("Output location: {:?}", config.out_path);

    if let Some(ref state_path) = config.state_path {
        let state: DatagenState = match std::fs::read_to_string(state_path) {
            Ok(s) => {
                println!("Loaded state from {:?}", state_path);
                let state: DatagenState = serde_json::from_str(&s)?;
                if state.config != config {
                    return Err(anyhow::anyhow!("Config mismatch"));
                }
                println!(
                    "Found {} previous games.\nWhite wins: {}, Black wins: {}, Draws: {}",
                    state.white_wins + state.black_wins + state.draws,
                    state.white_wins,
                    state.black_wins,
                    state.draws
                );
                println!();

                config = state.config.to_owned();

                state
            }
            Err(_) => {
                println!("Creating new state file at {:?}", state_path);
                println!();
                DatagenState {
                    white_wins: 0,
                    black_wins: 0,
                    draws: 0,
                    config: config.clone(),
                }
            }
        };
        WHITE_WINS.store(state.white_wins, std::sync::atomic::Ordering::Relaxed);
        BLACK_WINS.store(state.black_wins, std::sync::atomic::Ordering::Relaxed);
        DRAWS.store(state.draws, std::sync::atomic::Ordering::Relaxed);
        TOTAL_GAMES.store(
            state.white_wins + state.black_wins + state.draws,
            std::sync::atomic::Ordering::Relaxed,
        );
        NUM_AT_RESTART.store(
            state.white_wins + state.black_wins + state.draws,
            std::sync::atomic::Ordering::Relaxed,
        );
    }

    let games_remaing = config.num_games - TOTAL_GAMES.load(std::sync::atomic::Ordering::Relaxed);

    println!("Starting datagen with the following configuration:");
    println!("Limits: {:?}", config.limits);
    println!("TT size: {} MB", config.tt_size_mb);
    println!("Concurrency: {}", config.concurrency);
    println!("Output path: {:?}", config.out_path);
    if let Some(ref state_path) = config.state_path {
        println!("State path: {:?}", state_path);
    } else {
        println!("State path: None");
    }
    println!("Total games: {}", config.num_games);
    println!("Games remaining: {}", games_remaing);
    println!();
    let file = OpenOptions::new()
        .read(true)
        .create(true)
        .append(true)
        .open(&config.out_path)
        .unwrap();

    std::thread::scope(|s| {
        println!("Starting threads");
        let buf_writer = BufWriter::new(file);
        let shared_writer = Arc::new(Mutex::new(buf_writer));
        for i in 0..config.concurrency {
            s.spawn({
                let config = config.clone();
                let writer_clone = shared_writer.clone();
                move || thread_worker(i, &config, writer_clone)
            });
        }
        println!(
            "{}/{} threads started",
            config.concurrency, config.concurrency
        );
        println!();
        println!("Let 'er rip!!!!");
    });

    if STOP.load(std::sync::atomic::Ordering::Relaxed) {
        println!("Stopped by user");
    } else {
        println!("All games finished");
    }

    if let Some(ref state_path) = config.state_path {
        println!("Saving state to {:?}", config.state_path);
        let state = DatagenState {
            white_wins: WHITE_WINS.load(std::sync::atomic::Ordering::Relaxed),
            black_wins: BLACK_WINS.load(std::sync::atomic::Ordering::Relaxed),
            draws: DRAWS.load(std::sync::atomic::Ordering::Relaxed),
            config: config.clone(),
        };
        let state = serde_json::to_string(&state)?;
        std::fs::write(state_path, state)?;
    };

    println!();
    println!(
        "Total: {}, White wins: {}, Black wins: {}, Draws: {}",
        TOTAL_GAMES.load(std::sync::atomic::Ordering::Relaxed),
        WHITE_WINS.load(std::sync::atomic::Ordering::Relaxed),
        BLACK_WINS.load(std::sync::atomic::Ordering::Relaxed),
        DRAWS.load(std::sync::atomic::Ordering::Relaxed)
    );
    println!("See ya!");
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
    let tt = Arc::new(Table::new_mb(config.tt_size_mb as usize));
    let start = std::time::Instant::now();
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
            let num_since_restart =
                total - NUM_AT_RESTART.load(std::sync::atomic::Ordering::Relaxed);

            let games_per_min = (num_since_restart as f64)
                / (std::time::Instant::now() - start).as_secs_f64()
                * 60.0;
            let est_remaining = (config.num_games - num_since_restart) as f64 / games_per_min;

            println!();
            println!(
                "{}/{} Games, White wins: {}, Black wins: {}, Draws: {}",
                total, config.num_games, white_wins, black_wins, draws
            );
            println!("Games per minute: {:.1}", games_per_min);
            println!("Estimated time remaining: {:.1} minutes", est_remaining);

            if let Some(ref state_path) = config.state_path {
                let state = DatagenState {
                    white_wins: WHITE_WINS.load(std::sync::atomic::Ordering::Relaxed),
                    black_wins: BLACK_WINS.load(std::sync::atomic::Ordering::Relaxed),
                    draws: DRAWS.load(std::sync::atomic::Ordering::Relaxed),
                    config: config.to_owned(),
                };

                let state = serde_json::to_string(&state).unwrap();
                std::fs::write(state_path, state)?;
            };
        }

        tt.clear();
        if let Ok(game) = playout(&pos, config.limits, tt.clone()) {
            TOTAL_GAMES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

            let mut writer_guard = writer.lock().unwrap();
            game.serialize_into(writer_guard.by_ref())?;
        }
    }

    Ok(())
}

fn playout(startpos: &Position, limits: Limits, tt: Arc<Table>) -> anyhow::Result<CompressedGame> {
    let mut pos = startpos.clone();
    let mut rng = SmallRng::from_os_rng();

    let stop = Arc::new(AtomicBool::new(false));

    // make random moves
    let num_random = if rng.random_bool(0.5) { 8 } else { 9 };

    for _ in 0..num_random {
        let m = MoveGen::new(&pos).collect::<Vec<_>>();
        if m.is_empty() {
            return Err(anyhow::anyhow!("No moves"));
        }
        let mv = *m.choose(&mut rng).unwrap();
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

    // break early if eval is too extreme
    let mut search = Search::new(pos.clone(), limits, tt.clone(), stop.clone(), 0);
    search.set_silent(true);
    let res = search.think();
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

        let mut search = Search::new(pos.clone(), limits, tt.clone(), stop.clone(), 0);
        search.set_silent(true);
        let res = search.think();
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
        pos.make_move(res.bestmove);
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
    let mut file = std::fs::File::open(input).context(format!("Failed to open {:?}", input))?;
    while let Ok(game) = CompressedGame::deserialize_from(&mut file) {
        println!("{}", game);
    }

    Ok(())
}
