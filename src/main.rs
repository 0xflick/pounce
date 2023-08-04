use std::time::Instant;

use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

use flichess::{Board, Castle, Move, Position};

fn main() -> rustyline::Result<()> {
    let mut rl = DefaultEditor::new()?;
    let mut board: Board = Default::default();

    let mut move_list: Vec<Move> = Vec::new();

    loop {
        let readline = rl.readline(">> ");
        match readline {
            Ok(line) => match line.as_str() {
                "board" => println!("{}", board),
                "moves" => {
                    for mv in board.gen_moves().iter().filter(|m| board.is_legal(m)) {
                        if mv.castle != Castle::No {
                            print!("castle: ")
                        }
                        print!("{} ", mv);
                    }
                    println!();
                }
                "reset" => board = Default::default(),
                "unmake" => {
                    let mv = move_list.pop();
                    if let Some(m) = mv {
                        println!("unmake {:?}", m);
                        board.unmake_move(&m);
                    }
                }
                "set black" => {
                    board.is_white_turn = false;
                }
                "set white" => {
                    board.is_white_turn = true;
                }
                "check" => {
                    println!("check: {}", board.is_check())
                }
                "debug" => {
                    println!("{:?}", board)
                }
                s => {
                    if s.starts_with("perft") {
                        let split = s.split_once(' ');
                        if let Some((_, digit)) = split {
                            if let Ok(depth) = digit.parse::<usize>() {
                                let mut total = 0;
                                let mut captures = 0;
                                let mut ep = 0;
                                let mut castles = 0;

                                let now = Instant::now();

                                for mv in board
                                    .gen_moves()
                                    .iter()
                                    .filter(|m| board.is_legal(m))
                                    .collect::<Vec<&Move>>()
                                {
                                    if depth < 2 {
                                        println!(
                                            "{}: {} --- captures {}",
                                            mv,
                                            1,
                                            if mv.capture.is_some() { 1 } else { 0 }
                                        );
                                        total += 1;
                                    } else {
                                        board.make_move(mv);
                                        let (count, capture_count, ep_count, cc) =
                                            board.perft(depth - 1);
                                        board.unmake_move(mv);
                                        println!(
                                            "{}: {} --- captures: {}, ep: {}, castles: {}",
                                            mv, count, capture_count, ep_count, cc
                                        );
                                        total += count;
                                        captures += capture_count;
                                        ep += ep_count;
                                        castles += cc;
                                    }
                                }

                                let elapsed = now.elapsed();
                                println!(
                                    "total: {}, captures: {}, ep: {}, castles: {}",
                                    total, captures, ep, castles
                                );
                                println!("time: {} ms", elapsed.as_millis());
                            };
                        }
                    }
                    if let Ok(fen) = s.parse::<Board>() {
                        println!("parsing fen");
                        board = fen;
                    } else if let Ok(ml) = parse_move_list(s) {
                        ml.iter().for_each(|m| {
                            let annotated_move = board.annotate_move(m);
                            board.make_move(&annotated_move);
                            move_list.push(annotated_move);
                        })
                    } else if let Ok(m) = s.parse::<Move>() {
                        let annotated_move = board.annotate_move(&m);
                        board.make_move(&annotated_move);
                        move_list.push(annotated_move);
                        println!("{}", annotated_move);
                        continue;
                    } else if let Ok(p) = s.parse::<Position>() {
                        println!("{}", board.get(p));
                        continue;
                    }
                }
            },
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

struct ParseMoveListError;
fn parse_move_list(list: &str) -> Result<Vec<Move>, ParseMoveListError> {
    let mut res: Vec<Move> = Vec::new();
    for s in list.split(' ') {
        match s.parse::<Move>() {
            Ok(m) => res.push(m),
            _ => return Err(ParseMoveListError),
        }
    }

    Ok(res)
}
