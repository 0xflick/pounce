use std::borrow::Borrow;
use std::fmt::Display;
use std::ops::ControlFlow;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::{io, thread};

use anyhow::{Context, Result, anyhow};

use crate::chess::Move;
use crate::chess::movegen::{MoveGen, perft};
use crate::chess::position::Accumulator;
use crate::chess::position::fen::{Fen, STARTPOS};
use crate::engine::bench::bench;
use crate::engine::limits::Limits;
use crate::engine::search_manager::search_core;
#[cfg(feature = "tune")]
use crate::engine::tune;
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
}

impl UciOptionSet {
    pub fn new() -> Self {
        UciOptionSet::default()
    }
    pub fn add_option(&mut self, option: UciOption) {
        self.options.push(option);
    }

    /// Reads a `setoption` command, returning the option's name and its value clamped to range.
    /// Names are case insensitive, as UCI requires.
    pub fn parse<T>(&self, tokens: &[T]) -> Result<(&'static str, i32)>
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

        for option in &self.options {
            let UciOption::Spin {
                name: known,
                min,
                max,
                ..
            } = *option;
            if known.eq_ignore_ascii_case(&name) {
                let value: i32 = value
                    .parse()
                    .with_context(|| format!("Invalid value for {known}: {value}"))?;
                return Ok((known, value.clamp(min, max)));
            }
        }

        Err(anyhow!("Unknown option: {name}"))
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
    /// Owned directly by the UCI thread — no mutex. The UCI thread mutates
    /// `position`/config here; a `go` hands a *snapshot* to a search thread.
    manager: SearchManager,
    options: UciOptionSet,
    /// Atomic stop flag — the only thing `stop`/`quit` touch on a running
    /// search. Never contended with `position`/`setoption`.
    stop: Arc<AtomicBool>,
    /// The in-flight search thread, owned and joined (never abandoned).
    search_handle: Option<JoinHandle<()>>,
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

        #[cfg(feature = "tune")]
        for tunable in tune::TUNABLES {
            options.add_option(UciOption::Spin {
                name: tunable.name,
                default: tunable.default,
                min: tunable.min,
                max: tunable.max,
            });
        }

        Uci {
            manager: SearchManager::new(1, 16),
            options,
            stop: Arc::new(AtomicBool::new(false)),
            search_handle: None,
        }
    }

    /// Applies an option that was just set. Only that option is applied, because resizing the
    /// hash table clears it, and a tuning run sets many options.
    fn apply_option(&mut self, name: &str, value: i32) {
        match name {
            "Hash" => self.manager.set_tt_size_mb(value as usize),
            "Threads" => self.manager.set_num_threads(value as usize),
            #[cfg(feature = "tune")]
            _ => {
                if let Some(tunable) = tune::TUNABLES.iter().find(|t| t.name == name) {
                    tunable.set(value);
                }
            }
            #[cfg(not(feature = "tune"))]
            _ => {}
        }
    }

    /// Stop any in-flight search and join it. Cheap when the search already
    /// finished (just reaps the thread); bounded otherwise because `stop` is
    /// set first. Called before every state mutation / `go` / `quit`, so the
    /// UCI thread and a search never race over `position` or the TT.
    fn wait_search(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.search_handle.take() {
            let _ = h.join();
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
                let (name, value) = self.options.parse(rest)?;
                self.wait_search();
                self.apply_option(name, value);
            }
            Some("quit") => {
                self.wait_search();
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
            Some("eval") => {
                self.wait_search();
                let pos = &self.manager.position;
                let net = eval::nnue::PerspectiveNet::<{ eval::nnue::NNUE_HIDDEN_SIZE }>::load()?;
                let mut nnue_accumulator = eval::nnue::NNUEAccumulator::new(&net);
                nnue_accumulator.reset(pos);
                let nnue_eval = eval::score_nnue(pos, &nnue_accumulator);
                println!("NNUE Eval: {nnue_eval}");
            }
            Some("stop") => {
                self.cmd_stop()?;
            }
            Some("ucinewgame") => {
                self.wait_search();
                let Fen(position) = STARTPOS.parse().unwrap();
                self.manager.position = position;
                self.manager.clear_tt();
            }
            Some("zobrist") => {
                self.wait_search();
                let hash = self.manager.position.zobrist_hash();
                println!("Zobrist hash: {:x}", u64::from(hash));
                println!("Zobrist hash: {:x}", u64::from(self.manager.position.key));
            }
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
        // No lock: the UCI thread owns `manager`. We only join any in-flight
        // search first so a `position` can never be applied while a search
        // is running (which, with the old `try_lock`, silently dropped the
        // update and left the next `go` searching the stale board).
        self.wait_search();
        if !fen.is_empty() {
            let fen_str = fen.join(" ");
            let Fen(position) = Fen::parse(fen_str.as_str())?;
            self.manager.position = position;
        } else {
            let Fen(position) = STARTPOS.parse().unwrap();
            self.manager.position = position;
        }
        for mv in moves {
            self.manager.position.make_move(mv);
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

        self.wait_search();
        let manager = &self.manager;

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
            depth: Some(13),
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
        // Reap any prior search, then launch this one on a snapshot: a cloned
        // position + cloned `Arc`s for the shared TT/net. The search touches
        // nothing the UCI thread owns, so `position`/`stop`/`quit` never
        // contend with it. We own the JoinHandle (joined on the next mutation
        // / `quit`) — no abandoned thread, exactly one `bestmove`.
        self.wait_search();
        self.stop.store(false, Ordering::Relaxed);
        let (pos, tt, net, num_threads, silent) = self.manager.search_inputs();
        let stop = Arc::clone(&self.stop);
        self.search_handle = Some(thread::spawn(move || {
            tt.bump_age();
            search_core(&pos, &tt, &net, num_threads, silent, &stop, limits);
        }));

        Ok(())
    }

    fn cmd_stop(&mut self) -> Result<()> {
        self.stop.store(true, Ordering::Relaxed);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> UciOptionSet {
        let mut options = UciOptionSet::new();
        options.add_option(UciOption::Spin {
            name: "Hash",
            default: 16,
            min: 1,
            max: 64,
        });
        options
    }

    fn parse(command: &str) -> Result<(&'static str, i32)> {
        let tokens: Vec<&str> = command.split_whitespace().collect();
        options().parse(&tokens)
    }

    #[test]
    fn setoption_is_case_insensitive() {
        assert_eq!(parse("name hash value 32").unwrap(), ("Hash", 32));
    }

    #[test]
    fn setoption_clamps_to_range() {
        assert_eq!(parse("name Hash value 0").unwrap(), ("Hash", 1));
        assert_eq!(parse("name Hash value 1000").unwrap(), ("Hash", 64));
    }

    #[test]
    fn setoption_rejects_unknown_names_and_bad_values() {
        assert!(parse("name Hsah value 32").is_err());
        assert!(parse("name Hash value 3.5").is_err());
    }
}
