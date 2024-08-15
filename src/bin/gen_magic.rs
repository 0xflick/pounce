use flichess::{chess::Square, magic::find_magic};
use std::env;
use std::io::Write;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
struct Magic {
    shift: u8,
    magic: Option<u64>,
}

fn main() {
    // Generate magic numbers. We continue to generate magic numbers for all
    // sliding pieces until the program quits

    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: {} <output_file>", args[0]);
        std::process::exit(1);
    }

    let output_file = &args[1];

    let mut bishop_magics = [Magic {
        shift: 0,
        magic: None,
    }; 64];
    let mut rook_magics = [Magic {
        shift: 0,
        magic: None,
    }; 64];

    let num_tries = 100_000;
    let first_shift = 14;

    loop {
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

            let bishop = find_magic(sq, bishop_shift, true, num_tries);
            if bishop.is_some() {
                bishop_magics[sq as usize] = Magic {
                    shift: bishop_shift,
                    magic: bishop,
                };
            }

            let rook = find_magic(sq, rook_shift, false, num_tries);
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
            let mut file = std::fs::File::create(output_file).unwrap();
            for sq in Square::ALL.into_iter() {
                let bishop = bishop_magics[sq as usize];
                let rook = rook_magics[sq as usize];
                writeln!(
                    file,
                    "{} 0x{:x} {} 0x{:x}",
                    bishop.shift,
                    bishop.magic.unwrap(),
                    rook.shift,
                    rook.magic.unwrap()
                )
                .unwrap();
            }
        }
    }
}
