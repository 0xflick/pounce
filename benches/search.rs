use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};

pub fn search_benchmark(c: &mut Criterion) {
    let board = flichess::board::Board::default();

    let table = flichess::table::Table::new(16 * 1024 * 1024);
    let mut search = flichess::search::Search::new(
        board,
        Duration::from_millis(200),
        Arc::new(AtomicBool::new(false)),
        Arc::new(Mutex::new(table)),
    );

    c.bench_function("search", |b| b.iter(|| search.search()));
}

criterion_group!(search, search_benchmark);
criterion_main!(search);
