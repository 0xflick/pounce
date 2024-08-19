use flichess::{
    chess::{Color, Square},
    fen::Fen,
    movegen::{
        between, bishop_rays, gen_all_tables, line as move_line, perft, rook_rays, MoveGen,
        PAWN_MOVES,
    },
    position::Position,
};
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    gen_all_tables();
    let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    let Fen(mut pos) = Fen::parse(fen).unwrap();

    perft(pos, 6);
    return Ok(());

    let mut rl = DefaultEditor::new()?;

    let mut mv_stack = Vec::new();

    loop {
        let readline = rl.readline(">> ");
        match readline {
            Ok(line) => {
                let args: Vec<&str> = line.split_whitespace().collect();

                if args.is_empty() {
                    continue;
                }

                match args[0] {
                    "make" => {
                        if args.len() != 2 {
                            println!("Usage: move <move>");
                            continue;
                        }

                        match args[1].parse() {
                            Ok(chess_move) => {
                                pos.make_move(chess_move);
                                mv_stack.push(chess_move);
                            }
                            Err(_) => println!("Invalid move format"),
                        }
                    }
                    // "unmake" => {
                    //     pos.unmake_move(mv_stack.pop().unwrap());
                    // }
                    "fen" => {
                        if args.len() != 7 {
                            println!("Usage: fen");
                            continue;
                        }

                        let f = Fen::parse(args[1..7].join(" ").as_str()).unwrap();
                        println!("{}", f);
                        pos = f.0;
                    }
                    "print" => println!("{:?}", pos),
                    "perft" => {
                        if args.len() != 2 {
                            println!("Usage: perft <depth>");
                            continue;
                        }

                        if let Ok(depth) = args[1].parse() {
                            move_perft(&mut pos, depth);
                        } else {
                            println!("Invalid depth");
                        }
                    }
                    "between" => {
                        if args.len() != 3 {
                            println!("Usage: between <start> <end>");
                            continue;
                        }
                        let start = args[1].parse::<Square>().unwrap();
                        let end = args[2].parse::<Square>().unwrap();
                        println!("{:?}", between(start, end));
                    }
                    "line" => {
                        if args.len() != 3 {
                            println!("Usage: line <move>");
                            continue;
                        }

                        let start = args[1].parse::<Square>().unwrap();
                        let end = args[2].parse::<Square>().unwrap();
                        println!("{:?}", move_line(start, end));
                    }
                    "b_rays" => {
                        if args.len() != 2 {
                            println!("Usage: b_rays <square>");
                            continue;
                        }

                        let square = args[1].parse::<Square>().unwrap();
                        println!("{:?}", bishop_rays(square));
                    }
                    "r_rays" => {
                        if args.len() != 2 {
                            println!("Usage: b_rays <square>");
                            continue;
                        }

                        let square = args[1].parse::<Square>().unwrap();
                        println!("{:?}", rook_rays(square));
                    }
                    "p_moves" => {
                        if args.len() != 3 {
                            println!("Usage: p_moves <color> <square>");
                            continue;
                        }
                        let color: Color = args[1].parse().unwrap();
                        let square = args[2].parse::<Square>().unwrap();
                        unsafe {
                            println!("{:?}", PAWN_MOVES[color as usize][square as usize]);
                        }
                    }
                    "exit" => break,
                    _ => println!("Unknown command"),
                }
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

fn move_perft(pos: &mut Position, depth: u8) {
    if depth == 0 {
        println!("Total: 1");
        return;
    }

    let mg = MoveGen::new(pos);

    let mut total = 0;

    for mv in mg {
        let mut new_pos = *pos;
        new_pos.make_move(mv);
        let count = perft(new_pos, depth - 1);
        total += count;
        println!("{}: nodes: {}", mv, count);
        // pos.unmake_move(mv);
    }
    println!("Total: {}", total);
}
