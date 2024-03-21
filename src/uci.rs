use log::info;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::board::{Board, Move, ParseMoveError};
use crate::search::Search;
use crate::table::Table;

pub struct Uci {
    board: Option<Board>,
    abort: Arc<AtomicBool>,
    table: Arc<Mutex<Table>>,
    options: Options,
}

#[derive(Copy, Clone)]
struct Options {
    hash: usize,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            hash: 64 * 1024 * 1024,
        }
    }
}

impl Uci {
    pub fn new() -> Uci {
        Uci {
            board: None,
            abort: Arc::new(AtomicBool::new(false)),
            table: Arc::new(Mutex::new(Table::new(64 * 1024 * 1024))),
            options: Options::default(),
        }
    }

    pub fn cmd(&mut self, cmd: String) {
        info!("cmd: {}", cmd);
        let mut parts = cmd.split_whitespace();
        match parts.next() {
            Some("uci") => self.cmd_uci(),
            Some("isready") => self.cmd_isready(),
            Some("setoption") => self.cmd_setoption(&mut parts),
            Some("position") => self.cmd_position(&mut parts),
            Some("go") => self.cmd_go(&mut parts),
            Some("stop") => self.cmd_stop(),
            Some("quit") => self.cmd_quit(),
            Some("ucinewgame") => self.cmd_ucinewgame(),
            Some("board") => {
                if let Some(board) = &self.board {
                    println!("{}", board);
                }
            }
            _ => println!("unknown command: {}", cmd),
        }
    }

    pub fn cmd_uci(&self) {
        println!("id name flichess");
        println!("id author alex flick");
        println!("option name Hash type spin default 64 min 1 max 1024");
        println!("uciok");
    }

    fn cmd_ucinewgame(&mut self) {
        self.board = None;
        self.table = Arc::new(Mutex::new(Table::new(self.options.hash)));
    }

    fn cmd_isready(&self) {
        println!("readyok");
    }

    fn cmd_setoption<'b, I>(&mut self, parts: &mut I)
    where
        I: Iterator<Item = &'b str>,
    {
        if parts.next() == Some("name") {
            let name = parts.next().unwrap();
            if name == "Hash" && parts.next() == Some("value") {
                let value = parts.next().unwrap();
                if let Ok(hash) = value.parse::<usize>() {
                    self.options.hash = hash * 1024 * 1024;
                }
            }
        }
    }

    fn cmd_position<'b, I>(&mut self, parts: &mut I)
    where
        I: Iterator<Item = &'b str>,
    {
        match parts.next() {
            Some("startpos") => {
                self.board = Some(Board::default());
            }
            Some("fen") => {
                let mut fen = String::new();
                fen.push_str(&parts.take(6).collect::<Vec<_>>().join(" "));
                match fen.parse() {
                    Ok(board) => {
                        self.board = Some(board);
                    }
                    Err(e) => {
                        println!("error parsing fen: {:?}", e);
                        return;
                    }
                }
            }
            _ => println!("unknown position command"),
        }
        if let Some("moves") = parts.next() {
            let mv_list: Vec<Result<Move, ParseMoveError>> = parts.map(|s| s.parse()).collect();
            for mv in mv_list {
                match mv {
                    Ok(mv) => {
                        let annotated_move = self.board.as_ref().unwrap().annotate_move(&mv);
                        self.board.as_mut().unwrap().make_move(&annotated_move);
                    }
                    Err(e) => println!("error parsing move: {:?}", e),
                }
            }
        }
    }

    fn cmd_go<'b, I>(&mut self, parts: &mut I)
    where
        I: Iterator<Item = &'b str>,
    {
        if self.board.is_none() {
            self.board = Some(Board::default());
        }

        self.abort
            .store(false, std::sync::atomic::Ordering::Relaxed);

        let tl = self
            .parse_time_control(parts)
            .time_limit(self.board.as_ref().unwrap().is_white_turn)
            .unwrap_or(Duration::from_secs(1));

        let abort = self.abort.clone();
        let board = self.board.as_ref().unwrap().clone();
        let table = self.table.clone();
        thread::spawn(move || {
            let mut search = Search::new(board, tl, abort, table);
            match search.search() {
                Some(best_move) => {
                    println!("bestmove {}", best_move);
                }
                None => println!("bestmove (none)"),
            }
        });
    }

    fn parse_time_control<'b, I>(&self, parts: &mut I) -> TimeControl
    where
        I: Iterator<Item = &'b str>,
    {
        let mut time_control = TimeControl {
            wtime: None,
            btime: None,
            winc: None,
            binc: None,
            movestogo: None,
            move_time: None,
            infinite: false,
        };
        while let Some(part) = parts.next() {
            match part {
                "wtime" => time_control.wtime = parts.next().map(|s| s.parse().unwrap_or_default()),
                "btime" => time_control.btime = parts.next().map(|s| s.parse().unwrap_or_default()),
                "winc" => time_control.winc = parts.next().map(|s| s.parse().unwrap_or_default()),
                "binc" => time_control.binc = parts.next().map(|s| s.parse().unwrap_or_default()),
                "movestogo" => {
                    time_control.movestogo = parts.next().map(|s| s.parse().unwrap_or_default())
                }
                "movetime" => {
                    time_control.move_time = parts.next().map(|s| s.parse().unwrap_or_default())
                }
                "infinite" => time_control.infinite = true,
                _ => println!("unknown go command"),
            }
        }
        time_control
    }

    fn cmd_quit(&self) {
        std::process::exit(0);
    }

    fn cmd_stop(&mut self) {
        self.abort.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

impl Default for Uci {
    fn default() -> Self {
        Uci::new()
    }
}

struct TimeControl {
    wtime: Option<u64>,
    btime: Option<u64>,
    winc: Option<u64>,
    binc: Option<u64>,
    movestogo: Option<u64>,
    move_time: Option<u64>,
    infinite: bool,
}

impl TimeControl {
    fn time_left(&self, is_white: bool) -> Option<u64> {
        match is_white {
            true => self.wtime,
            false => self.btime,
        }
    }

    fn time_increment(&self, is_white: bool) -> Option<u64> {
        match is_white {
            true => self.winc,
            false => self.binc,
        }
    }

    fn moves_to_go(&self) -> u32 {
        match self.movestogo {
            Some(m) => m as u32,
            None => 30,
        }
    }

    pub fn time_limit(&self, is_white: bool) -> Option<Duration> {
        if self.infinite {
            return Some(Duration::MAX);
        }
        if self.move_time.is_some() {
            return Some(Duration::from_millis(self.move_time.unwrap()));
        }

        let time_left = self.time_left(is_white);
        let time_increment = self.time_increment(is_white);
        match time_left {
            Some(tl) => match time_increment {
                Some(ti) => {
                    let time_left = Duration::from_millis(tl);
                    let time_increment = Duration::from_millis(ti) * self.moves_to_go();
                    Some((time_left + time_increment) / (self.moves_to_go() + 2))
                }
                None => Some(Duration::from_millis(tl) / (self.moves_to_go() + 2)),
            },
            None => None,
        }
    }
}
