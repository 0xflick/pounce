use std::hint::black_box;
use std::time::Duration;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use pounce::chess::Fen;
use pounce::chess::fen::STARTPOS;
use pounce::movegen::{MoveList, Mover, PawnType, init_tables};

fn bench_pawn_movegen(c: &mut Criterion) {
    init_tables();
    let Fen(startpos) = STARTPOS.parse().unwrap();
    c.bench_function("pawn_movegen", |b| {
        b.iter_batched_ref(
            || MoveList::new(),
            |moves| {
                PawnType::legal_moves::<false, false>(black_box(&startpos), moves);
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
