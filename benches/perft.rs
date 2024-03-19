use criterion::{black_box, criterion_group, criterion_main, Criterion};

pub fn perft_benchmark(c: &mut Criterion) {
    let mut board = flichess::board::Board::default();

    c.bench_function("perft 3", |b| b.iter(|| board.perft(black_box(3))));
}

pub fn perft_alt_benchmark(c: &mut Criterion) {
    let mut board = "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8"
        .parse::<flichess::board::Board>()
        .unwrap();
    c.bench_function("perft 3", |b| b.iter(|| board.perft(black_box(3))));
}

criterion_group!(perft, perft_benchmark, perft_alt_benchmark);
criterion_main!(perft);
