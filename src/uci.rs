use std::{
    borrow::Borrow,
    sync::{atomic::AtomicBool, Arc},
    thread,
};

use anyhow::{anyhow, Context, Result};
use rustyline::{error::ReadlineError, DefaultEditor};

use crate::{
    eval,
    fen::Fen,
    limits::Limits,
    movegen::{perft, MoveGen},
    moves::Move,
    position::Position,
    search::Search,
    tt::Table,
    util::engine_name,
};

pub struct Uci {
    position: Position,
    stop: Arc<AtomicBool>,
}

impl Uci {
    pub fn new() -> Self {
        let Fen(position) = Uci::STARTPOS.parse().unwrap();
        Uci {
            position,
            stop: Arc::new(AtomicBool::new(false)),
        }
    }

    pub const STARTPOS: &'static str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
}

impl Default for Uci {
    fn default() -> Self {
        Self::new()
    }
}

enum UciState {
    Continue,
    Quit,
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
                        Ok(UciState::Continue) => {}
                        Ok(UciState::Quit) => {
                            break;
                        }
                        Err(e) => {
                            eprintln!("Error: {:?}", e);
                        }
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

    fn handle_cmd<T>(&mut self, cmd: Option<&str>, rest: &[T]) -> Result<UciState>
    where
        T: AsRef<str> + Borrow<str>,
    {
        match cmd {
            Some("uci") => {
                println!("id name {}", engine_name());
                println!("id author alex flick");
                println!("uciok");
            }
            Some("isready") => {
                println!("readyok");
            }
            Some("quit") => {
                return Ok(UciState::Quit);
            }
            Some("position") => {
                self.cmd_position(rest)?;
            }
            Some("go") => {
                self.cmd_go(rest)?;
            }
            Some("eval") => {
                let eval = eval::eval(&self.position);
                println!("Eval: {}", eval);
            }
            Some("stop") => {
                self.cmd_stop();
            }
            Some(val) => {
                eprintln!("Unknown command: {}", val);
            }
            None => {}
        }
        Ok(UciState::Continue)
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

        let limits = if tokens.len() > 1 {
            Limits::from_tokens(&tokens[1..])?
        } else {
            Limits::new()
        };

        let stop = Arc::new(AtomicBool::new(false));
        let position = self.position.clone();
        let tt = Table::new_mb(64);

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
