use log::{info, LevelFilter};
use std::io;
use std::time::{Duration, Instant};

use regex::Regex;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

use flichess::board::{Board, Castle, Move, Position};
use flichess::search::search;

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
                "play" => {
                    // set a 1 second time limit
                    let mv = search(&mut board, Duration::from_secs(1));
                    board.make_move(&mv);
                    move_list.push(mv);
                }
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
        uci_mode(&mut board).expect("error in uci mode");
    }
    Ok(())
}

fn uci_mode(board: &mut Board) -> Result<(), ()> {
    info!("starting uci mode");
    println!("id name flichess");
    println!("id author flichess");
    println!("uciok");
    let mut buf = String::new();
    loop {
        buf.clear();
        io::stdin()
            .read_line(&mut buf)
            .expect("error reading from stdin");
        match buf.trim() {
            "isready" => println!("readyok"),
            "quit" => break,
            "ucinewgame" => {
                *board = Default::default();
            }
            s => {
                if s.starts_with("go") {
                    let wtime_regex = Regex::new(r"wtime (\d+)").unwrap();
                    let btime_regex = Regex::new(r"btime (\d+)").unwrap();

                    let wtime = wtime_regex
                        .captures(s)
                        .and_then(|cap| cap.get(1))
                        .and_then(|time_match| time_match.as_str().parse::<u64>().ok());

                    let btime = btime_regex
                        .captures(s)
                        .and_then(|cap| cap.get(1))
                        .and_then(|time_match| time_match.as_str().parse::<u64>().ok());

                    let timeout = match board.is_white_turn {
                        true => wtime.map(|time| Duration::from_millis(time / 20)),
                        false => btime.map(|time| Duration::from_millis(time / 20)),
                    };
                    // set a time out of 5% of the time limit
                    let mv = search(board, timeout.unwrap_or(Duration::from_secs(1)));
                    board.make_move(&mv);
                    info!("bestmove {}", mv);
                    println!("bestmove {}", mv);
                } else if s.starts_with("position") {
                    let split = s.split_once(' ');
                    if let Some((_, rest)) = split {
                        let split = rest.split_once("moves ");
                        if let Some((_, moves)) = split {
                            let ml = parse_move_list(moves);
                            if let Ok(moves) = ml {
                                *board = Default::default();

                                moves.iter().for_each(|m| {
                                    let annotated_move = board.annotate_move(m);
                                    board.make_move(&annotated_move);
                                });
                            }
                        }
                    }
                }
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
