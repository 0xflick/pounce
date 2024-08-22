use std::borrow::Borrow;

use anyhow::Result;
use rand::seq::SliceRandom;
use rustyline::DefaultEditor;

use crate::{
    fen::Fen,
    movegen::{perft, MoveGen},
    moves::Move,
    position::Position,
    util::engine_name,
};

pub struct Uci {
    position: Position,
}

impl Uci {
    pub fn new() -> Self {
        let Fen(position) = Uci::STARTPOS.parse().unwrap();
        Uci { position }
    }

    const STARTPOS: &'static str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
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
            let line = rl.readline("")?;
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
                self.cmd_go(rest);
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

    fn cmd_go<T>(&mut self, tokens: &[T])
    where
        T: AsRef<str> + Borrow<str>,
    {
        enum ParseStage {
            Pre,
            Perft, // Depth,
                   // Movetime,
                   // Nodes,
                   // Infinite,
        }

        let mut parse_stage = ParseStage::Pre;
        let mut perft_depth = 0;

        for token in tokens {
            match token.as_ref() {
                "perft" => {
                    parse_stage = ParseStage::Perft;
                }
                _ => match parse_stage {
                    ParseStage::Perft => {
                        if let Ok(depth) = token.as_ref().parse::<u8>() {
                            perft_depth = depth;
                            parse_stage = ParseStage::Pre;
                        }
                    }
                    _ => {}
                },
            }
        }

        if perft_depth > 0 {
            let mg = MoveGen::new(&self.position);
            let mut nodes = 0;

            for mv in mg {
                let mut new_pos = self.position;
                new_pos.make_move(mv);
                let count = perft(new_pos, perft_depth - 1);
                nodes += count;
                println!("{}: nodes: {}", mv, count);
            }

            println!("Total: {}", nodes);
            return;
        }

        // random best move
        let bm = MoveGen::new(&self.position)
            .collect::<Vec<Move>>()
            .choose(&mut rand::thread_rng())
            .cloned();

        println!("info score cp 0 depth 1");

        if let Some(bm) = bm {
            println!("bestmove {}", bm);
        }
    }
}
