use std::fmt::{self, Debug, Display, Formatter};
use std::fs::OpenOptions;
use std::io::BufWriter;
use std::num::NonZeroU16;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rand::prelude::IndexedRandom;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};

use crate::bitboard::Bitboard;
use crate::chess::{CastleRights, Color, GameResult, Piece, Role, Square};
use crate::eval;
use crate::fen::Fen;
use crate::limits::Limits;
use crate::movegen::MoveGen;
use crate::moves::Move;
use crate::position::Position;
use crate::search::Search;
use crate::tt::Table;

static STOP: AtomicBool = AtomicBool::new(false);
static TOTAL_GAMES: AtomicU32 = AtomicU32::new(0);
static WHITE_WINS: AtomicU32 = AtomicU32::new(0);
static BLACK_WINS: AtomicU32 = AtomicU32::new(0);
static DRAWS: AtomicU32 = AtomicU32::new(0);
static NUM_AT_RESTART: AtomicU32 = AtomicU32::new(0);

const STARTPOS: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

#[derive(Copy, Clone, Default, PartialEq, Eq)]
#[repr(transparent)]
struct U4Array32([u8; 16]);

impl U4Array32 {
    pub const fn get(&self, i: usize) -> u8 {
        (self.0[i / 2] >> (4 * (i % 2))) & 0b1111
    }

    pub const fn set(&mut self, i: usize, val: u8) {
        debug_assert!(val < 0x10);
        let shift = 4 * (i % 2);
        let idx = i / 2;
        self.0[idx] &= !(0b1111 << shift);
        self.0[idx] |= val << shift;
    }
}

impl Debug for U4Array32 {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let mut dbg_list = f.debug_list();
        for i in 0..32 {
            dbg_list.entry(&format!("{:#02x}", self.get(i)));
        }
        dbg_list.finish()
    }
}

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Wdl {
    BlackWin,
    Draw,
    WhiteWin,
    Unknown,
}

impl Display for Wdl {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Wdl::Unknown => write!(f, "Unknown"),
            Wdl::BlackWin => write!(f, "0-1"),
            Wdl::Draw => write!(f, "1/2-1/2"),
            Wdl::WhiteWin => write!(f, "1-0"),
        }
    }
}

impl From<u8> for Wdl {
    fn from(val: u8) -> Self {
        match val {
            0 => Wdl::BlackWin,
            1 => Wdl::Draw,
            2 => Wdl::WhiteWin,
            _ => Wdl::Unknown,
        }
    }
}

// 32 bytes (needs to be a multiple of 8 because that's the alignment of Bitboard)
#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq, Default)]
pub struct CompressedPosition {
    occ: Bitboard,      // 8 bytes
    pieces: U4Array32,  // 16 bytes
    score: i16,         // 2 bytes
    halfmove_clock: u8, // 1 byte
    pub wdl: u8,        // 1 byte
    ply: u16,           // 2 byte
    stm_ep_square: u8,  // 1 byte
    castling: u8,       // 1 bytes
}

impl CompressedPosition {
    pub fn new(pos: &Position, score: i16, wdl: Wdl) -> Self {
        let mut pieces = U4Array32::default();
        for (idx, sq) in pos.occupancy.enumerate() {
            let pc = pos.mailbox[sq].unwrap();
            let bit_pc = ((pc.color as u8) << 3) | (pc.role as u8);

            pieces.set(idx, bit_pc);
        }

        Self {
            occ: pos.occupancy,
            pieces,
            score,
            wdl: wdl as u8,
            stm_ep_square: pos.ep_square.map_or(u8::MAX, |sq| sq as u8),
            halfmove_clock: pos.halfmove_clock,
            ply: (pos.fullmove_number.get() - 1) * 2 + pos.side as u16,
            castling: pos.castling.bits(),
        }
    }

    pub fn flip(&self) -> Self {
        let mut mailbox: [Option<Piece>; 64] = [None; 64];

        for (idx, sq) in self.occ.enumerate() {
            let pc = self.pieces.get(idx);

            let color = if pc >> 3 == 0 {
                Color::White
            } else {
                Color::Black
            };

            mailbox[(sq) as usize] = Some(Piece {
                color,
                role: unsafe { std::mem::transmute::<u8, Role>(pc & 0b111) },
            });
        }

        let occ = self.occ.flip();
        let mut pieces = U4Array32::default();
        for (idx, sq) in occ.enumerate() {
            let pc = mailbox[sq ^ 56].unwrap();
            let bit_pc = ((1 - pc.color as u8) << 3) | (pc.role as u8);

            pieces.set(idx, bit_pc);
        }

        let ep_square = if self.stm_ep_square == u8::MAX {
            u8::MAX
        } else {
            self.stm_ep_square ^ 56
        };

        let new_ply = if self.ply % 2 == 0 {
            self.ply + 1
        } else {
            self.ply - 1
        };

        Self {
            occ,
            pieces,
            score: -self.score,
            wdl: 2 - self.wdl,
            stm_ep_square: ep_square,
            halfmove_clock: self.halfmove_clock,
            ply: new_ply,
            castling: self.castling,
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
        f.debug_struct("CompressedPosition")
            .field("occ", &self.occ)
            .field("pieces", &self.pieces)
            .field("score", &self.score)
            .field("wdl", &Wdl::from(self.wdl))
            .field("halfmove_clock", &self.halfmove_clock)
            .field("ply", &self.ply)
            .field(
                "stm_ep_square",
                &if self.stm_ep_square == u8::MAX {
                    "None".to_string()
                } else {
                    format!("{}", Square::from(self.stm_ep_square))
                },
            )
            .field("castling", &format!("{:04b}", self.castling))
            .finish()
    }
}

// 4 bytes
#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq, Default)]
pub struct CompressedMove {
    mv: Move,   // 2 bytes
    score: i16, // 2 bytes
}

impl CompressedMove {
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

const NULL_TERMINATOR: [u8; 4] = [0; 4];

pub struct CompressedGame {
    initial: CompressedPosition,
    moves: Vec<CompressedMove>,
}

impl CompressedGame {
    pub fn new(initial: Position) -> Self {
        Self {
            initial: CompressedPosition::new(&initial, 0, Wdl::Unknown),
            moves: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.moves.len()
    }

    pub fn is_empty(&self) -> bool {
        self.moves.is_empty()
    }

    pub fn push_move(&mut self, mv: Move, score: i16) {
        self.moves.push(CompressedMove { mv, score });
    }

    pub fn set_win(&mut self, wdl: Wdl) {
        self.initial.wdl = wdl as u8;
    }

    pub fn serialize_into(&self, file: &mut impl std::io::Write) -> std::io::Result<()> {
        file.write_all(self.initial.as_bytes())?;
        for m in &self.moves {
            file.write_all(m.as_bytes())?;
        }
        file.write_all(&NULL_TERMINATOR)?;
        Ok(())
    }

    pub fn deserialize_from(file: &mut impl std::io::Read) -> std::io::Result<Self> {
        let mut initial = CompressedPosition::default();
        file.read_exact(initial.as_mut_bytes())?;

        let mut moves = Vec::new();
        loop {
            let mut unparsed_move = [0; std::mem::size_of::<CompressedMove>()];
            file.read_exact(&mut unparsed_move)?;
            if unparsed_move == NULL_TERMINATOR {
                break;
            }
            let mut m = CompressedMove::default();
            m.as_mut_bytes().copy_from_slice(&unparsed_move);
            moves.push(m);
        }

        Ok(Self { initial, moves })
    }
}

impl From<CompressedPosition> for Position {
    fn from(value: CompressedPosition) -> Self {
        let mut pos = Self::default();

        for (idx, sq) in value.occ.enumerate() {
            let pc = value.pieces.get(idx);

            let color = if pc >> 3 == 0 {
                Color::White
            } else {
                Color::Black
            };

            let role = unsafe { std::mem::transmute::<u8, Role>(pc & 0b111) };

            pos.set(sq, Piece { color, role });
        }

        pos.fullmove_number = NonZeroU16::new(value.ply / 2 + 1).unwrap();
        pos.ep_square = match value.stm_ep_square {
            u8::MAX => None,
            sq => Some(Square::from(sq)),
        };
        pos.halfmove_clock = value.halfmove_clock;
        pos.side = if value.ply % 2 == 0 {
            Color::White
        } else {
            Color::Black
        };
        pos.refresh_checks_and_pins();
        pos.key = pos.zobrist_hash();
        pos.castling = CastleRights::from_bits_retain(value.castling);

        pos
    }
}

impl Display for CompressedGame {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let mut pos: Position = self.initial.into();
        let header = format!(
            r#"[Event "game"]
[Site "NA"]
[Date "NA"]
[White "pounce"]
[Black "pounce"]
[Result "{}"]
[FEN "{}"]"#,
            Wdl::from(self.initial.wdl),
            pos.to_fen(),
        );
        writeln!(f, "{}", header)?;

        let mut move_count = 0;
        for m in &self.moves {
            if move_count % 12 == 0 && pos.side == Color::White {
                writeln!(f)?;
            }
            if pos.side == Color::White {
                write!(f, "{}. ", pos.fullmove_number.get())?;
            } else {
                move_count += 1;
            }

            let san = pos.san(m.mv).unwrap();
            write!(f, "{} {{{:+.2}}} ", san, m.score as f64 / 100.0)?;
            pos.make_move(m.mv);
        }
        write!(f, "{}\n\n", Wdl::from(self.initial.wdl))?;
        Ok(())
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
    let mut search = Search::new(pos.clone(), limits, tt.clone(), stop.clone());
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
    let mut file = std::fs::File::open(input)?;
    while let Ok(game) = CompressedGame::deserialize_from(&mut file) {
        println!("{}", game);
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::fen::Fen;

    const STARTPOS: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

    #[test]
    fn test_size() {
        assert_eq!(std::mem::size_of::<CompressedPosition>(), 32);
        assert_eq!(std::mem::size_of::<CompressedMove>(), 4);
    }

    #[test]
    fn test_startpos() {
        let Fen(pos) = Fen::parse(STARTPOS).unwrap();

        let comp = CompressedPosition::new(&pos, 0, Wdl::BlackWin);

        assert_eq!(comp, comp.flip().flip());

        assert_eq!(comp.occ, comp.flip().occ);
        assert_eq!(comp.pieces, comp.flip().pieces);
        assert_eq!(comp.score, -comp.flip().score);
    }

    const POS_1: &str = "b3r1k1/5pbp/6p1/1NP5/8/5N2/2Q3PP/3q2K1 b - - 0 31";
    const POS_1_FLIPPED: &str = "3Q2k1/2q3pp/5n2/8/1np5/6P1/5PBP/B3R1K1 w - - 0 31";

    #[test]
    fn test_flip() {
        let Fen(pos) = Fen::parse(POS_1).unwrap();
        let comp = CompressedPosition::new(&pos, 600, Wdl::BlackWin);

        let Fen(flip) = POS_1_FLIPPED.parse().unwrap();
        let comp_flip = CompressedPosition::new(&flip, -600, Wdl::WhiteWin);

        assert_eq!(comp_flip, comp.flip());
    }
}
