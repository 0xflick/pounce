use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use flichess::{
    fen::Fen,
    movegen::{gen_all_tables, MoveList, Mover, NotCheck, PawnType, WhiteType},
};
use std::{hint::black_box, time::Duration};

fn bench_pawn_movegen(c: &mut Criterion) {
    gen_all_tables();
    let Fen(startpos) = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
        .parse()
        .unwrap();
    c.bench_function("pawn_movegen", |b| {
        b.iter_batched_ref(
            || MoveList::new(),
            |moves| {
                PawnType::legal_moves::<NotCheck, WhiteType>(black_box(&startpos), moves);
            },
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(
    name=benches;
    config = {
        let mut conf = Criterion::default();
        conf = conf.measurement_time(Duration::from_secs(20));
        conf = conf.noise_threshold(0.005);
        conf = conf.confidence_level(0.98);
        conf = conf.significance_level(0.005);
        conf
    };
    targets = bench_pawn_movegen);
criterion_main!(benches);
