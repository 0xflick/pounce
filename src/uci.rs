use std::{
    borrow::Borrow,
    collections::HashMap,
    fmt::Display,
    ops::ControlFlow,
    sync::{atomic::AtomicBool, Arc},
    thread,
};

use anyhow::{anyhow, Context, Result};
use rustyline::{error::ReadlineError, DefaultEditor};

use crate::{
    bench::bench,
    fen::Fen,
    limits::Limits,
    movegen::{perft, MoveGen},
    moves::Move,
    position::Position,
    search::Search,
    tt::Table,
    util::engine_name,
};

#[derive(Debug, Clone, Copy)]
pub enum UciOption {
    Spin {
        name: &'static str,
        default: i32,
        min: i32,
        max: i32,
    },
}

impl Display for UciOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UciOption::Spin {
                name,
                default,
                min,
                max,
            } => {
                write!(
                    f,
                    "option name {} type spin default {} min {} max {}",
                    name, default, min, max
                )
            }
        }
    }
}

struct UciOptionSet {
    options: Vec<UciOption>,
    values: HashMap<String, String>,
}

impl UciOptionSet {
    pub fn new() -> Self {
        UciOptionSet::default()
    }
    pub fn add_option(&mut self, option: UciOption) {
        match option {
            UciOption::Spin { name, default, .. } => {
                self.values.insert(name.to_string(), default.to_string());
            }
        }

        self.options.push(option);
    }

    pub fn parse<T>(&mut self, tokens: &[T]) -> Result<()>
    where
        T: AsRef<str> + Borrow<str>,
    {
        enum ParseStage {
            Pre,
            Name,
            Value,
        }

        let mut parse_stage = ParseStage::Pre;

        let mut name = String::new();
        let mut value = String::new();

        for token in tokens {
            match token.as_ref() {
                "name" => {
                    parse_stage = ParseStage::Name;
                }
                "value" => {
                    parse_stage = ParseStage::Value;
                }
                _ => match parse_stage {
                    ParseStage::Name => {
                        name = token.as_ref().to_string();
                    }
                    ParseStage::Value => {
                        value = token.as_ref().to_string();
                    }
                    _ => {}
                },
            }
        }

        self.values.insert(name, value);
        Ok(())
    }

    pub fn get_int(&self, name: &str) -> Option<i32> {
        self.values
            .get(name)
            .and_then(|val| val.parse::<i32>().ok())
    }
}

impl Default for UciOptionSet {
    fn default() -> Self {
        UciOptionSet {
            options: Vec::new(),
            values: HashMap::new(),
        }
    }
}

impl Display for UciOptionSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for option in &self.options {
            writeln!(f, "{}", option)?;
        }
        Ok(())
    }
}

pub struct Uci {
    position: Position,
    stop: Arc<AtomicBool>,
    tt: Arc<Table>,
    options: UciOptionSet,
}

impl Uci {
    pub fn new() -> Self {
        let Fen(position) = Uci::STARTPOS.parse().unwrap();

        let mut options = UciOptionSet::new();
        options.add_option(UciOption::Spin {
            name: "Hash",
            default: 64,
            min: 1,
            max: 16384,
        });

        let tt = Table::new_mb(options.get_int("Hash").unwrap() as usize);

        Uci {
            position,
            stop: Arc::new(AtomicBool::new(false)),
            tt: Arc::new(tt),
            options,
        }
    }

    pub const STARTPOS: &'static str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
}

impl Default for Uci {
    fn default() -> Self {
        Self::new()
    }
}

impl Uci {
    pub fn run_loop(&mut self) -> Result<()> {
        println!("{}", engine_name());

        let mut rl = DefaultEditor::new()?;

        loop {
            match rl.readline("") {
                Ok(line) => {
                    rl.add_history_entry(&line)?;

                    let mut tokens = line.split_whitespace();
                    let cmd = tokens.next().map(|s| s.to_string());
                    let rest = tokens.collect::<Vec<&str>>();

                    match self.handle_cmd(cmd.as_deref(), &rest) {
                        Err(e) => {
                            eprintln!("Error: {:?}", e);
                        }
                        Ok(ControlFlow::Break(())) => {
                            break;
                        }
                        Ok(ControlFlow::Continue(())) => {}
                    }
                }
                Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => {
                    break;
                }
                Err(e) => return Err(e).context("Error reading input"),
            }
        }
        println!("Exiting...");
        Ok(())
    }

    fn handle_cmd<T>(&mut self, cmd: Option<&str>, rest: &[T]) -> Result<ControlFlow<()>>
    where
        T: AsRef<str> + Borrow<str>,
    {
        match cmd {
            Some("uci") => {
                println!("id name {}", engine_name());
                println!("id author alex flick");
                println!("{}", self.options);
                println!("uciok");
            }
            Some("isready") => {
                println!("readyok");
            }
            Some("setoption") => {
                self.options.parse(rest)?;

                if let Some(hash_size) = self.options.get_int("Hash") {
                    if self.tt.size_mb() != hash_size.try_into().unwrap() {
                        self.tt = Arc::new(Table::new_mb(hash_size as usize));
                    }
                }
                self.tt = Arc::new(Table::new_mb(self.options.get_int("Hash").unwrap() as usize));
            }
            Some("quit") => {
                return Ok(ControlFlow::Break(()));
            }
            Some("position") => {
                self.cmd_position(rest)?;
            }
            Some("go") => {
                self.cmd_go(rest)?;
            }
            Some("eval") => {
                let eval = self.position.eval();
                let psqt_mg = self.position.psqt_mg;
                let psqt_eg = self.position.psqt_eg;
                let psqt_mg_calc = self.position.psqt_mg();
                let psqt_eg_calc = self.position.psqt_eg();
                println!(
                    "Eval: {}, PSQT MG: {} - {}, PSQT EG: {} - {}",
                    eval, psqt_mg, psqt_mg_calc, psqt_eg, psqt_eg_calc
                );
            }
            Some("stop") => {
                self.cmd_stop();
            }
            Some("ucinewgame") => {
                self.tt.clear();
            }
            Some("zobrist") => {
                let hash = self.position.zobrist_hash();
                println!("Zobrist hash: {:x}", u64::from(hash));
                println!("Zobrist hash: {:x}", u64::from(self.position.key));
            }
            Some(val) => {
                eprintln!("Unknown command: {}", val);
            }
            None => {}
        }
        Ok(ControlFlow::Continue(()))
    }

    fn cmd_position<T>(&mut self, tokens: &[T]) -> Result<()>
    where
        T: AsRef<str> + Borrow<str>,
    {
        enum ParseStage {
            Pre,
            Startpos,
            Fen,
            Moves,
        }

        let mut parse_stage = ParseStage::Pre;
        let mut fen: Vec<&str> = Vec::new();
        let mut moves: Vec<Move> = Vec::new();

        for token in tokens {
            match token.as_ref() {
                "startpos" => {
                    parse_stage = ParseStage::Startpos;
                }
                "fen" => {
                    parse_stage = ParseStage::Fen;
                }
                "moves" => {
                    parse_stage = ParseStage::Moves;
                }
                _ => match parse_stage {
                    ParseStage::Fen => {
                        fen.push(token.borrow());
                    }
                    ParseStage::Moves => {
                        moves.push(token.borrow().parse::<Move>()?);
                    }
                    _ => {}
                },
            }
        }

        if !fen.is_empty() {
            let fen_str = fen.join(" ");
            let Fen(position) = Fen::parse(fen_str.as_str())?;
            self.position = position;
        } else {
            let Fen(position) = Uci::STARTPOS.parse().unwrap();
            self.position = position;
        }

        for mv in moves {
            self.position.make_move(mv);
        }
        Ok(())
    }

    fn cmd_perft<T>(&mut self, tokens: &[T]) -> Result<()>
    where
        T: AsRef<str> + Borrow<str>,
    {
        let depth = tokens
            .first()
            .ok_or(anyhow!("No depth provided"))?
            .as_ref()
            .parse::<u8>()?;

        let mut nodes = 0;
        let now = std::time::Instant::now();

        if depth > 0 {
            let mg = MoveGen::new(&self.position);

            for mv in mg {
                self.position.make_move(mv);
                let count = perft(&mut self.position, depth - 1);
                nodes += count;
                self.position.unmake_move(mv);
                println!("{}: {}", mv, count);
            }
        }

        let elapsed = now.elapsed();
        println!();
        println!(
            "Nodes: {}, Time: {}s {}ms, Nodes/s: {:.2}M",
            nodes,
            elapsed.as_secs(),
            elapsed.subsec_millis(),
            (nodes as f64 / elapsed.as_secs_f64() / 1_000_000.0)
        );
        Ok(())
    }

    fn cmd_go<T>(&mut self, tokens: &[T]) -> Result<()>
    where
        T: AsRef<str> + Borrow<str>,
    {
        if !tokens.is_empty() && tokens[0].as_ref() == "perft" {
            self.cmd_perft(&tokens[1..])?;
            return Ok(());
        }

        if !tokens.is_empty() && tokens[0].as_ref() == "bench" {
            return bench(self.tt.size_mb() as u32);
        }

        let limits = if !tokens.is_empty() {
            Limits::from_tokens(tokens)?
        } else {
            let mut limits = Limits::new();
            limits.infinite = true;
            limits
        };

        let stop = Arc::new(AtomicBool::new(false));
        self.stop = stop.clone();
        let tt = self.tt.clone();

        let position = self.position.clone();

        thread::spawn(move || {
            let mut search = Search::new(position, limits, tt, stop.clone());
            let best_move = search.think();
            println!("bestmove {}", best_move);
        });
        Ok(())
    }

    fn cmd_stop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}
