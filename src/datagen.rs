use std::{
    fmt::{self, Debug, Formatter},
    fs::OpenOptions,
    io::{Read, Write},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU32},
        Arc,
    },
    time::Duration,
};

use rand::{rngs::SmallRng, seq::SliceRandom, Rng, SeedableRng};
use serde::{Deserialize, Serialize};

use crate::{
    bitboard::Bitboard,
    chess::{Color, GameResult},
    eval,
    fen::Fen,
    limits::Limits,
    movegen::MoveGen,
    position::Position,
    search::Search,
    tt::Table,
};

static STOP: AtomicBool = AtomicBool::new(false);
static TOTAL_GAMES: AtomicU32 = AtomicU32::new(0);
static WHITE_WINS: AtomicU32 = AtomicU32::new(0);
static BLACK_WINS: AtomicU32 = AtomicU32::new(0);
static DRAWS: AtomicU32 = AtomicU32::new(0);
static NUM_AT_RESTART: AtomicU32 = AtomicU32::new(0);

const STARTPOS: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Wdl {
    BlackWin,
    Draw,
    WhiteWin,
}

// 32 bytes (needs to be a multiple of 8 because that's the alignment of Bitboard)
#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct CompressedPosition {
    occ: Bitboard,    // 8 bytes
    pieces: [u8; 16], // 16 bytes
    score: i16,       // 2 bytes
    pub wdl: u8,      // 1 byte
    extra: [u8; 5],   // 5 bytes
}

impl CompressedPosition {
    pub fn new(pos: &Position, score: i16, wdl: Wdl) -> Self {
        let mut occ = pos.occupancy;
        let mailbox = pos.mailbox;
        let mut wdl = wdl as u8;

        // if side to move is black, we want to flip the orientation of the board
        if pos.side == Color::Black {
            occ = occ.flip();
            wdl = 2 - wdl;
        }

        let mut pieces = [0; 16];
        for (idx, mut sq) in occ.enumerate() {
            if pos.side == Color::Black {
                sq = sq ^ 56;
            };

            let pc = mailbox[sq].unwrap();
            let bit_pc = (((pos.side != pc.color) as u8) << 3) | (pc.role as u8);

            let shift = 4 * (idx % 2);
            let idx = idx / 2;

            pieces[idx] |= bit_pc << shift;
        }

        Self {
            occ,
            pieces,
            score,
            wdl,
            extra: [0; 5],
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(self as *const _ as *const u8, std::mem::size_of::<Self>())
        }
    }

    pub fn as_mut_bytes(&mut self) -> &mut [u8] {
        unsafe {
            std::slice::from_raw_parts_mut(self as *mut _ as *mut u8, std::mem::size_of::<Self>())
        }
    }
}

impl Debug for CompressedPosition {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        writeln!(f, "occ:")?;
        writeln!(f, "{:?}", self.occ)?;
        writeln!(f, "pieces:")?;
        for p in self.pieces {
            writeln!(f, "{:#010b}", p)?;
        }
        writeln!(f, "score: {}, wdl: {}", self.score, self.wdl)
    }
}

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

    std::fs::create_dir_all(&config.out_path)?;
    println!("Output directory: {:?}", config.out_path);

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

    std::thread::scope(|s| {
        println!("Starting threads");
        for i in 0..config.concurrency {
            s.spawn({
                let config = config.clone();
                move || thread_worker(i, &config)
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

fn thread_worker(id: u32, config: &DatagenConfig) -> anyhow::Result<()> {
    let out_path = config.out_path.join(format!("{}.dat", id));
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
        if let Ok(positions) = playout(&pos, config.limits, tt.clone()) {
            TOTAL_GAMES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let mut file = OpenOptions::new()
                .read(true)
                .create(true)
                .append(true)
                .open(&out_path)
                .unwrap();

            for p in positions {
                file.write_all(p.as_bytes())?;
            }
        }
    }

    Ok(())
}

fn playout(
    startpos: &Position,
    limits: Limits,
    tt: Arc<Table>,
) -> anyhow::Result<Vec<CompressedPosition>> {
    let mut pos = startpos.clone();
    let mut rng = SmallRng::from_entropy();

    let stop = Arc::new(AtomicBool::new(false));

    let mut positions = Vec::new();

    // make random moves
    let num_random = if rng.gen_bool(0.5) { 8 } else { 9 };

    for _ in 0..num_random {
        let moves = MoveGen::new(&pos).collect::<Vec<_>>();
        if moves.is_empty() {
            return Err(anyhow::anyhow!("No moves"));
        }
        let mv = *moves.choose(&mut rng).unwrap();
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
    let mut search = Search::new(pos.clone(), limits, tt.clone(), stop.clone());
    search.set_silent(true);
    let res = search.think();
    if res.score.abs() > 1_500 {
        return Err(anyhow::anyhow!("Extreme score"));
    }

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

        let mut search = Search::new(pos.clone(), limits, tt.clone(), stop.clone());
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
        }

        pos.make_move(res.bestmove);
        positions.push(res);
    };

    match result {
        Wdl::WhiteWin => WHITE_WINS.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        Wdl::BlackWin => BLACK_WINS.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        Wdl::Draw => DRAWS.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
    };

    let mut compressed_positions = Vec::with_capacity(positions.len());
    let mut pos = startpos.clone();
    // build up CompressedPositions
    for p in positions {
        compressed_positions.push(CompressedPosition::new(&pos, p.score, result));
        pos.make_move(p.bestmove);
    }

    Ok(compressed_positions)
}

pub fn shuffle_interleave(inputs: &[PathBuf], output: &PathBuf) {
    let mut rng = SmallRng::from_entropy();

    let mut all_positions = Vec::new();
    for input in inputs.iter() {
        let file = std::fs::File::open(input).unwrap();
        let mut reader = std::io::BufReader::new(file);
        loop {
            let mut cp = CompressedPosition {
                occ: Bitboard::EMPTY,
                pieces: [0; 16],
                score: 0,
                wdl: 0,
                extra: [0; 5],
            };

            let bytes_read = reader.read(cp.as_mut_bytes()).unwrap();
            if bytes_read == 0 {
                break;
            }
            if cp.score.abs() > 20_000 {
                continue;
            }

            all_positions.push(cp);
        }
    }

    all_positions.shuffle(&mut rng);

    let mut file = OpenOptions::new()
        .read(true)
        .create(true)
        .append(true)
        .open(output)
        .unwrap();

    for p in all_positions.iter() {
        file.write_all(p.as_bytes())
            .expect("Failed to write to file");
    }

    println!(
        "Shuffled and interleaved {} positions into {}",
        all_positions.len(),
        output.display()
    );
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::fen::Fen;

    const STARTPOS: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    const STARTPOS_FLIPPED: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

    #[test]
    fn test_startpos() {
        let Fen(pos) = Fen::parse(STARTPOS).unwrap();
        let Fen(flipped_pos) = Fen::parse(STARTPOS_FLIPPED).unwrap();

        let comp = CompressedPosition::new(&pos, 0, Wdl::BlackWin);
        let comp_flipped = CompressedPosition::new(&flipped_pos, 0, Wdl::BlackWin);

        assert_eq!(comp, comp_flipped);
    }
}
