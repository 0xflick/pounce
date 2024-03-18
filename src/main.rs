use log::{info, LevelFilter};
use std::io;
use std::time::Instant;

use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

use flichess::board::{parse_move_list, Board, Castle, Move, Position};
use flichess::uci::Uci;

fn main() -> rustyline::Result<()> {
    // simple_logging::log_to_file("test.log", LevelFilter::Info).unwrap();
    simple_logging::log_to_stderr(LevelFilter::Info);

    let mut rl = DefaultEditor::new()?;
    let mut board: Board = Default::default();

    let mut move_list: Vec<Move> = Vec::new();
    let mut uci = false;

    loop {
        let readline = rl.readline(">> ");
        match readline {
            Ok(line) => match line.as_str() {
                "uci" => {
                    uci = true;
                    break;
                }
                // "play" => {
                //     // set a 1 second time limit
                //     let mut search = Search::new(&mut board, Duration::from_secs(1));
                //     let mv = search.search();
                //     board.make_move(&mv);
                //     move_list.push(mv);
                // }
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
                                println!(
                                    "nodes/s: {:.2}M",
                                    (total as f64 / 1_000_000.0) / elapsed.as_secs_f64()
                                );
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

    if uci {
        uci_mode().expect("error in uci mode");
    }
    Ok(())
}

fn uci_mode() -> Result<(), ()> {
    info!("starting uci mode");
    let mut uci = Uci::new();
    uci.identify();

    let mut buf = String::new();
    loop {
        buf.clear();
        io::stdin()
            .read_line(&mut buf)
            .expect("error reading from stdin");
        let cmd_str = buf.clone();
        {
            uci.cmd(cmd_str);
        }
    }
}
