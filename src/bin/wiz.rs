use std::io::Write;

use clap::Parser;
use pounce::{
    chess::Square,
    movegen::magic_finder::{bishop_mask, rook_mask, Wizard},
};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
struct Magic {
    shift: u8,
    magic: Option<u64>,
}

#[derive(Parser)]
struct Cli {
    // Output file for generated magics
    output_file: String,

    // Number of iteration rounds
    rounds: usize,
}

fn main() {
    let args = Cli::parse();

    let mut bishop_magics = [Magic {
        shift: 0,
        magic: None,
    }; 64];
    let mut rook_magics = [Magic {
        shift: 0,
        magic: None,
    }; 64];

    let num_tries = 100_000;
    let first_shift = 12;

    let mut wizard = Wizard::new();

    for _ in 0..args.rounds {
        for sq in Square::ALL.into_iter() {
            let bishop_shift = if bishop_magics[sq as usize].magic.is_none() {
                first_shift
            } else {
                bishop_magics[sq as usize].shift - 1
            };

            let rook_shift = if rook_magics[sq as usize].magic.is_none() {
                first_shift
            } else {
                rook_magics[sq as usize].shift - 1
            };

            let bishop = wizard.find_magic(sq, bishop_shift, true, num_tries);
            if bishop.is_some() {
                bishop_magics[sq as usize] = Magic {
                    shift: bishop_shift,
                    magic: bishop,
                };
            }

            let rook = wizard.find_magic(sq, rook_shift, false, num_tries);
            if rook.is_some() {
                rook_magics[sq as usize] = Magic {
                    shift: rook_shift,
                    magic: rook,
                };
            }
        }

        let bishop_found = bishop_magics.iter().filter(|m| m.magic.is_some()).count();
        let bishop_total = bishop_magics.len();

        let mut bishop_size = 0;
        let mut bishop_best_shift = 255;
        let mut bishop_worst_shift = 0;

        let rook_found = rook_magics.iter().filter(|m| m.magic.is_some()).count();
        let rook_total = rook_magics.len();

        let mut rook_size = 0;
        let mut rook_best_shift = 255;
        let mut rook_worst_shift = 0;

        for sq in Square::ALL.into_iter() {
            if bishop_magics[sq as usize].magic.is_some() {
                let shift = bishop_magics[sq as usize].shift;
                let entry_size = 1 << shift;
                bishop_size += entry_size * 8;

                if shift > bishop_worst_shift {
                    bishop_worst_shift = shift;
                }
                if shift < bishop_best_shift {
                    bishop_best_shift = shift;
                }
            }
            if rook_magics[sq as usize].magic.is_some() {
                let shift = rook_magics[sq as usize].shift;
                let entry_size = 1 << shift;
                rook_size += entry_size * 8;

                if shift > rook_worst_shift {
                    rook_worst_shift = shift;
                }
                if shift < rook_best_shift {
                    rook_best_shift = shift;
                }
            }
        }

        println!();
        println!(
            "Bishop: {}/{} ({} kb), best shift: {}, worst shift: {}",
            bishop_found,
            bishop_total,
            bishop_size / 1024,
            bishop_best_shift,
            bishop_worst_shift
        );

        println!(
            "Rook: {}/{} ({} kb), best shift: {}, worst shift: {}",
            rook_found,
            rook_total,
            rook_size / 1204,
            rook_best_shift,
            rook_worst_shift
        );

        if bishop_found == bishop_total && rook_found == rook_total {
            // write magics to file
            let mut file = std::fs::File::create(&args.output_file).unwrap();
            writeln!(file, "use crate::bitboard::Bitboard;").unwrap();
            writeln!(file, "use crate::magic::Magic;").unwrap();
            writeln!(file).unwrap();

            writeln!(file, "#[rustfmt::skip]").unwrap();
            writeln!(file, "pub const BISHOP_MAGICS: [Magic; 64] = [").unwrap();
            let mut offset = 0;
            for sq in Square::ALL.into_iter() {
                let bishop = bishop_magics[sq as usize];
                let mask = bishop_mask(sq);
                writeln!(
                    file,
                    "    Magic {{ mask: Bitboard(0x{:x}), shift: 0x{:x}, magic: 0x{:x}, offset: 0x{:x} }},",
                    mask,
                    bishop.shift,
                    bishop.magic.unwrap(),
                    offset,
                )
                .unwrap();

                offset += 1 << bishop.shift;
            }
            writeln!(file, "];").unwrap();
            writeln!(file).unwrap();

            writeln!(file, "#[rustfmt::skip]").unwrap();
            writeln!(file, "pub const ROOK_MAGICS: [Magic; 64] = [").unwrap();
            let mut offset = 0;
            for sq in Square::ALL.into_iter() {
                let rook = rook_magics[sq as usize];
                let mask = rook_mask(sq);
                writeln!(
                    file,
                    "    Magic {{ mask: Bitboard(0x{:x}), shift: 0x{:x}, magic: 0x{:x}, offset: 0x{:x} }},",
                    mask,
                    rook.shift,
                    rook.magic.unwrap(),
                    offset,
                )
                .unwrap();

                offset += 1 << rook.shift;
            }
            writeln!(file, "];").unwrap();
        }
    }
}
