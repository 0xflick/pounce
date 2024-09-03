use rand::seq::SliceRandom;
use rand::{rngs::SmallRng, Rng, SeedableRng};

use crate::eval;
use crate::{
    bitboard::Bitboard,
    chess::{Color, GameResult},
    fen::Fen,
    limits::Limits,
    movegen::MoveGen,
    position::Position,
    search::Search,
    tt::Table,
};
use std::{
    fmt::{self, Debug, Formatter},
    fs::OpenOptions,
    io::{Read, Write},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU32},
        Arc,
    },
};

static STOP: AtomicBool = AtomicBool::new(false);
static TOTAL_GAMES: AtomicU32 = AtomicU32::new(0);
static WHITE_WINS: AtomicU32 = AtomicU32::new(0);
static BLACK_WINS: AtomicU32 = AtomicU32::new(0);
static DRAWS: AtomicU32 = AtomicU32::new(0);

const STARTPOS: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Wdl {
    BlackWin,
    Draw,
    WhiteWin,
}

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct CompressedPosition {
    occ: Bitboard,
    pieces: [u8; 16],
    score: i16,
    pub wdl: u8,
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

pub struct DatagenConfig {
    pub limits: Limits,
    pub num_games: u32,
    pub tt_size_mb: u32,
    pub concurrency: u32,
    pub out_path: String,
}

pub fn datagen(
    DatagenConfig {
        limits,
        num_games,
        tt_size_mb,
        concurrency,
        out_path,
    }: DatagenConfig,
) -> anyhow::Result<()> {
    // start playout threads, share global state, print results
    ctrlc::set_handler(move || {
        STOP.store(true, std::sync::atomic::Ordering::Relaxed);
    })?;

    let out_dir = PathBuf::from("data").join(out_path);
    std::fs::create_dir_all(&out_dir)?;
    println!("Output directory: {:?}", out_dir);

    let games_per_thread = (num_games / concurrency).max(1);
    println!("Games per thread: {}", games_per_thread);

    std::thread::scope(|s| {
        for i in 0..concurrency {
            let out_path = out_dir.join(format!("{}.dat", i));
            s.spawn(move || {
                thread_worker(limits, tt_size_mb, games_per_thread, out_path, i);
            });
        }
    });
    println!(
        "Total: {}, White wins: {}, Black wins: {}, Draws: {}",
        TOTAL_GAMES.load(std::sync::atomic::Ordering::Relaxed),
        WHITE_WINS.load(std::sync::atomic::Ordering::Relaxed),
        BLACK_WINS.load(std::sync::atomic::Ordering::Relaxed),
        DRAWS.load(std::sync::atomic::Ordering::Relaxed)
    );
    Ok(())
}

fn thread_worker(limits: Limits, tt_size_mb: u32, num_games: u32, out_path: PathBuf, id: u32) {
    let tt = Arc::new(Table::new_mb(tt_size_mb as usize));
    let Fen(pos) = STARTPOS.parse().unwrap();
    let mut completed = 0;
    while completed < num_games {
        if STOP.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }

        if id == 0 && completed % 10 == 0 {
            println!(
                "Total: {}, White wins: {}, Black wins: {}, Draws: {}",
                TOTAL_GAMES.load(std::sync::atomic::Ordering::Relaxed),
                WHITE_WINS.load(std::sync::atomic::Ordering::Relaxed),
                BLACK_WINS.load(std::sync::atomic::Ordering::Relaxed),
                DRAWS.load(std::sync::atomic::Ordering::Relaxed)
            );
        }

        tt.clear();
        if let Ok(positions) = playout(&pos, limits, tt.clone()) {
            completed += 1;
            TOTAL_GAMES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let mut file = OpenOptions::new()
                .read(true)
                .create(true)
                .append(true)
                .open(&out_path)
                .unwrap();

            for p in positions {
                file.write_all(p.as_bytes())
                    .expect("Failed to write to file");
            }
        }
    }
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
