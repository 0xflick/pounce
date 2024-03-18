fn main() {
    let mut board = "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8"
        .parse::<flichess::board::Board>()
        .unwrap();
    board.perft(5);
}
