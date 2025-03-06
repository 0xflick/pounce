use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use threadpool::ThreadPool;

use crate::chess::{Move, Position};
use crate::engine::limits::Limits;
use crate::engine::search::{Search, SearchResult};
use crate::engine::tt::Table;

pub struct SearchManager {
    pool: ThreadPool,
    silent: bool,
}

impl SearchManager {
    pub fn new(num_threads: usize) -> Self {
        SearchManager {
            pool: ThreadPool::new(num_threads),
            silent: false,
        }
    }

    pub fn set_silent(&mut self, silent: bool) {
        self.silent = silent;
    }

    pub fn set_num_threads(&mut self, num_threads: usize) {
        self.pool.set_num_threads(num_threads);
    }

    pub fn think(&self, position: Position, limits: Limits, tt: Arc<Table>, stop: Arc<AtomicBool>) {
        for thread_idx in 0..self.pool.max_count() {
            let position = position.clone();
            let tt = tt.clone();
            let stop = stop.clone();
            let silent = self.silent;
            self.pool.execute(move || {
                let mut search = Search::new(position, limits, tt, stop, thread_idx);
                if silent {
                    search.set_silent(true);
                }
                let result = search.think();
                if thread_idx == 0 {
                    println!("bestmove {}", result.bestmove);
                }
            });
        }
    }

    pub fn think_sync(
        &self,
        position: Position,
        limits: Limits,
        tt: Arc<Table>,
        stop: Arc<AtomicBool>,
    ) -> (SearchResult, u64) {
        let (tx, rx) = std::sync::mpsc::channel();
        for thread_idx in 0..self.pool.max_count() {
            let tx = tx.clone();
            let position = position.clone();
            let tt = tt.clone();
            let stop = stop.clone();
            let silent = self.silent;
            self.pool.execute(move || {
                let mut search = Search::new(position, limits, tt, stop, thread_idx);
                if silent {
                    search.set_silent(true);
                }
                let result = search.think();
                tx.send((thread_idx, result, search.nodes)).unwrap();
            });
        }

        self.pool.join();

        let mut nodes = 0;
        let mut result = SearchResult {
            bestmove: Move::NONE,
            score: 0,
        };
        for _ in 0..self.pool.max_count() {
            let (thread_idx, worker_result, worker_nodes) = rx.recv().unwrap();
            if thread_idx == 0 {
                result = worker_result;
            }
            nodes += worker_nodes;
        }

        (result, nodes)
    }
}
