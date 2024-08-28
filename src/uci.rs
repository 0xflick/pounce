use std::{
    borrow::Borrow,
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

pub struct Uci {
    position: Position,
    stop: Arc<AtomicBool>,
    tt: Arc<Table>,
}

impl Uci {
    pub fn new() -> Self {
        let Fen(position) = Uci::STARTPOS.parse().unwrap();
        let tt = Table::new_mb(512);
        Uci {
            position,
            stop: Arc::new(AtomicBool::new(false)),
            tt: Arc::new(tt),
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
                println!("uciok");
            }
            Some("isready") => {
                println!("readyok");
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
            return bench();
        }

        let limits = if !tokens.is_empty() {
            Limits::from_tokens(tokens)?
        } else {
            Limits::new()
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
