use std::hint::black_box;
use std::time::Duration;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use pounce::fen::Fen;
use pounce::movegen::{MoveList, Mover, PawnType, init_tables};
use pounce::search::init_reductions;

fn bench_search(c: &mut Criterion) {
    pounce::init();
    let mut limit = pounce::limits::Limits::new();
    limit.depth = Some(5);
    c.bench_function("bench", |b| {
        b.iter(|| {
            pounce::bench::bench(16, 1, limit, true).unwrap();
        })
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
    targets = bench_search);
criterion_main!(benches);
