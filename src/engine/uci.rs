use std::borrow::Borrow;
use std::collections::HashMap;
use std::fmt::Display;
use std::ops::ControlFlow;
use std::sync::{Arc, Mutex};
use std::{io, thread};

use anyhow::{Context, Result, anyhow};

use crate::chess::Move;
use crate::chess::movegen::{MoveGen, perft};
use crate::chess::position::Accumulator;
use crate::chess::position::fen::{Fen, STARTPOS};
use crate::engine::bench::bench;
use crate::engine::limits::Limits;
use crate::engine::utils::engine_name;
use crate::engine::{SearchManager, eval};

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
                    "option name {name} type spin default {default} min {min} max {max}"
                )
            }
        }
    }
}

#[derive(Default)]
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

impl Display for UciOptionSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for option in &self.options {
            writeln!(f, "{option}")?;
        }
        Ok(())
    }
}

pub struct Uci {
    manager: Arc<Mutex<SearchManager>>,
    options: UciOptionSet,
    stop: Arc<std::sync::atomic::AtomicBool>,
}

impl Uci {
    pub fn new() -> Self {
        let mut options = UciOptionSet::new();
        options.add_option(UciOption::Spin {
            name: "Threads",
            default: 1,
            min: 1,
            max: 128,
        });
        options.add_option(UciOption::Spin {
            name: "Hash",
            default: 16,
            min: 1,
            max: 16384,
        });

        let manager = Arc::new(Mutex::new(SearchManager::new(1, 16)));

        Uci {
            manager,
            options,
            stop: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }
}

impl Default for Uci {
    fn default() -> Self {
        Self::new()
    }
}

impl Uci {
    pub fn run_loop(&mut self) -> Result<()> {
        println!("{}", engine_name());

        loop {
            let mut line = String::new();
            match io::stdin().read_line(&mut line) {
                Ok(0) => {
                    // EOF reached
                    break;
                }
                Ok(_) => {
                    let line = line.trim(); // Remove newline and whitespace
                    if line.is_empty() {
                        continue;
                    }

                    let mut tokens = line.split_whitespace();
                    let cmd = tokens.next().map(|s| s.to_string());
                    let rest = tokens.collect::<Vec<&str>>();

                    match self.handle_cmd(cmd.as_deref(), &rest) {
                        Err(e) => {
                            eprintln!("Error: {e:?}");
                        }
                        Ok(ControlFlow::Break(())) => {
                            break;
                        }
                        Ok(ControlFlow::Continue(())) => {}
                    }
                }
                Err(e) => {
                    return Err(e).context("Error reading input");
                }
            }
        }

        println!("Exiting...");
        Ok(())
    }

    pub fn run_once(&mut self, input: &str) -> Result<()> {
        println!("{}", engine_name());
        let mut tokens = input.split_whitespace();
        let cmd = tokens.next().map(|s| s.to_string());
        let rest = tokens.collect::<Vec<&str>>();

        match self.handle_cmd(cmd.as_deref(), &rest) {
            Err(e) => Err(e),
            Ok(_) => Ok(()),
        }
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
                match self.manager.try_lock() {
                    Ok(mut manager) => {
                        if let Some(hash_size) = self.options.get_int("Hash") {
                            manager.set_tt_size_mb(hash_size as usize);
                        }

                        if let Some(threads) = self.options.get_int("Threads") {
                            manager.set_num_threads(threads as usize);
                        }
                    }
                    Err(_) => {
                        Err(anyhow!("Failed to lock search manager"))?;
                    }
                }
            }
            Some("quit") => {
                return Ok(ControlFlow::Break(()));
            }
            Some("position") => {
                self.cmd_position(rest)?;
            }
            Some("bench") => {
                self.cmd_bench()?;
            }
            Some("go") => {
                self.cmd_go(rest)?;
            }
            Some("eval") => match self.manager.try_lock() {
                Ok(manager) => {
                    let pos = &manager.position;
                    let mut psqt_accumulator = eval::PSQTAccumulator::new();
                    psqt_accumulator.reset(pos);
                    let eval = eval::score(pos, &psqt_accumulator);
                    println!("Eval: {eval}");
                }
                Err(_) => {
                    Err(anyhow!("Failed to lock search manager"))?;
                }
            },
            Some("stop") => {
                self.cmd_stop()?;
            }
            Some("ucinewgame") => match self.manager.try_lock() {
                Ok(mut manager) => {
                    let Fen(position) = STARTPOS.parse().unwrap();
                    manager.position = position;
                }
                Err(_) => {
                    Err(anyhow!("Failed to lock search manager"))?;
                }
            },
            Some("zobrist") => match self.manager.try_lock() {
                Ok(manager) => {
                    let hash = manager.position.zobrist_hash();
                    println!("Zobrist hash: {:x}", u64::from(hash));
                    println!("Zobrist hash: {:x}", u64::from(manager.position.key));
                }
                Err(_) => {
                    Err(anyhow!("Failed to lock search manager"))?;
                }
            },
            Some(val) => {
                eprintln!("Unknown command: {val}");
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
        match self.manager.try_lock() {
            Ok(mut manager) => {
                if !fen.is_empty() {
                    let fen_str = fen.join(" ");
                    let Fen(position) = Fen::parse(fen_str.as_str())?;
                    manager.position = position;
                } else {
                    let Fen(position) = STARTPOS.parse().unwrap();
                    manager.position = position;
                }

                for mv in moves {
                    manager.position.make_move(mv);
                }
            }
            Err(_) => {
                return Err(anyhow!("Failed to lock search manager"));
            }
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

        let manager = match self.manager.try_lock() {
            Ok(manager) => manager,
            Err(_) => {
                return Err(anyhow!("Failed to lock search manager"));
            }
        };

        if depth > 0 {
            let mut pos = manager.position.clone();
            let mg = MoveGen::new(&pos);

            for mv in mg {
                pos.make_move(mv);
                let count = perft(&mut pos, depth - 1);
                nodes += count;
                pos.unmake_move(mv);
                println!("{mv}: {count}");
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

    fn cmd_bench(&mut self) -> Result<()> {
        let limits = Limits {
            depth: Some(7),
            ..Default::default()
        };
        println!("Running benchmark...");
        bench(16, 1, limits, false)
    }

    fn cmd_go<T>(&mut self, tokens: &[T]) -> Result<()>
    where
        T: AsRef<str> + Borrow<str>,
    {
        if !tokens.is_empty() && tokens[0].as_ref() == "perft" {
            self.cmd_perft(&tokens[1..])?;
            return Ok(());
        }

        let limits = if !tokens.is_empty() {
            Limits::from_tokens(tokens)?
        } else {
            let mut limits = Limits::new();
            limits.infinite = true;
            limits
        };
        self.stop.store(false, std::sync::atomic::Ordering::Relaxed);
        let stop = self.stop.clone();
        let manager = self.manager.clone();
        thread::spawn(move || match manager.try_lock() {
            Ok(manager) => {
                manager.think_with_stop(limits, stop);
            }
            Err(_) => {
                eprintln!("Failed to lock search manager");
            }
        });

        Ok(())
    }

    fn cmd_stop(&mut self) -> Result<()> {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }
}
