use std::{error::Error, fmt::Display, io, result::Result};

use crate::util::engine_name;

#[derive(Debug)]
pub struct UciError;
impl Error for UciError {}

impl Display for UciError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "UCI Error")
    }
}

pub fn uci_loop() -> Result<(), UciError> {
    println!("{}", engine_name());
    for line in io::stdin().lines() {
        match line {
            Ok(line) => match line.split_whitespace().next() {
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
                _ => {
                    println!("Unknown command: {}", line);
                }
            },
            Err(_) => {
                println!("Error reading line");
                return Err(UciError);
            }
        }
    }

    Ok(())
}
