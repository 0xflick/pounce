use rand::seq::SliceRandom;
use std::{borrow::Borrow, error::Error, fmt::Display, io, result::Result};

use crate::{
    fen::Fen,
    movegen::{perft, MoveGen},
    moves::Move,
    position::Position,
    util::engine_name,
};

#[derive(Debug)]
pub struct UciError;
impl Error for UciError {}

impl Display for UciError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "UCI Error")
    }
}

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

impl Uci {
    pub fn run_loop(&mut self) -> Result<(), UciError> {
        println!("{}", engine_name());
        for line in io::stdin().lines() {
            let (cmd, tokens) = match line {
                Ok(line) => {
                    let mut tokens = line.split_whitespace();
                    let cmd = tokens.next().map(|s| s.to_string());
                    (cmd, tokens.map(|s| s.to_string()).collect::<Vec<String>>())
                }
                Err(_) => {
                    println!("Error reading line");
                    return Err(UciError);
                }
            };
            match cmd.as_deref() {
                Some("uci") => {
                    println!("id name {}", engine_name());
                    println!("id author alex flick");
                    println!("uciok");
                }
                Some("isready") => {
                    println!("readyok");
                }
                Some("quit") => {
                    break;
                }
                Some("position") => {
                    self.cmd_position(&tokens);
                }
                Some("go") => {
                    self.cmd_go(&tokens);
                }
                Some(_) | None => {}
            }
        }
        Ok(())
    }

    fn cmd_position<T>(&mut self, tokens: &[T])
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
                        if let Ok(mv) = token.borrow().parse::<Move>() {
                            moves.push(mv);
                        }
                    }
                    _ => {}
                },
            }
        }

        if !fen.is_empty() {
            let fen_str = fen.join(" ");
            if let Ok(Fen(position)) = Fen::parse(fen_str.as_str()) {
                self.position = position;
            }
        } else {
            let Fen(position) = Uci::STARTPOS.parse().unwrap();
            self.position = position;
        }

        for mv in moves {
            self.position.make_move(mv);
        }
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
