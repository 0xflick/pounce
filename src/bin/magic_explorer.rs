use std::error::Error;

use flichess::bitboard::Bitboard;
use flichess::chess::Square;
use flichess::magic::{BISHOP_ATTACKS, ROOK_ATTACKS};
use flichess::magic_gen::{BISHOP_MAGICS, ROOK_MAGICS};
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

fn main() -> Result<(), Box<dyn Error>> {
    // magic bit board explorer.
    // user will enter a square and an occupancy bit board
    // and the program will print out the magic bit board
    // for that square.

    let mut rl = DefaultEditor::new()?;

    loop {
        let readline = rl.readline(">> ");
        match readline {
            Ok(line) => {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() != 2 {
                    println!("Please enter a square and an occupancy bit board.");
                    continue;
                }

                let square = Square::from_str(parts[0])?;
                let occupancy = Bitboard(u64::from_str_radix(parts[1], 16).unwrap());

                let bishop_magic = BISHOP_MAGICS[square as usize];
                let bishop_attack = BISHOP_ATTACKS[bishop_magic.index(occupancy)];

                let rook_magic = ROOK_MAGICS[square as usize];
                let rook_attack = ROOK_ATTACKS[rook_magic.index(occupancy)];

                println!("Square: {:?}", square);
                println!("Occupancy:\n{:?}", occupancy);

                println!("Bishop Magic: {:016x}", bishop_magic.magic);
                println!("Bishop Attack:\n{:?}", bishop_attack);

                println!("Rook Magic: {:016x}", rook_magic.magic);
                println!("Rook Attack:\n{:?}", rook_attack);
            }
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }
    Ok(())
}
