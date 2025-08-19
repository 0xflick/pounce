use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread;

use crate::chess::Position;
use crate::chess::position::fen::{self, Fen};
use crate::engine::eval::nnue;
use crate::engine::limits::Limits;
use crate::engine::search::{Search, SearchResult};
use crate::engine::tt::Table;

pub struct SearchManager {
    num_threads: usize,
    silent: bool,
    pub position: Position,
    tt: Table,
    net: nnue::PerspectiveNet<{ nnue::NNUE_HIDDEN_SIZE }>,
}

impl SearchManager {
    pub fn new(num_threads: usize, hash_size: usize) -> Self {
        let Fen(position) = fen::STARTPOS.parse().unwrap();
        Self::new_from_position(num_threads, hash_size, position)
    }

    pub fn new_from_position(num_threads: usize, hash_size: usize, position: Position) -> Self {
        let net = nnue::PerspectiveNet::load().expect("Failed to load NNUE network");
        SearchManager {
            num_threads,
            silent: false,
            tt: Table::new_mb(hash_size),
            position,
            net,
        }
    }

    pub fn set_silent(&mut self, silent: bool) {
        self.silent = silent;
    }

    pub fn set_num_threads(&mut self, num_threads: usize) {
        self.num_threads = num_threads;
    }

    pub fn set_tt_size_mb(&mut self, size_mb: usize) {
        self.tt = Table::new_mb(size_mb);
    }

    pub fn clear_tt(&mut self) {
        self.tt.clear();
    }

    pub fn think_with_stop(&self, limits: Limits, stop: Arc<AtomicBool>) -> (SearchResult, u64) {
        stop.store(false, std::sync::atomic::Ordering::Relaxed);
        self.tt.new_search();
        thread::scope(|s| {
            let mut handles = Vec::with_capacity(self.num_threads);

            for thread_idx in 0..self.num_threads {
                let position = self.position.clone();
                let tt = &self.tt;
                let silent = self.silent;
                let stop = &stop;

                let handle = s.spawn(move || {
                    let mut search =
                        Search::new(position, limits, tt, stop, thread_idx, silent, &self.net);
                    let result = search.think();
                    (result, search.stats)
                });

                handles.push(handle);
            }

            let moves_and_stats = handles
                .into_iter()
                .map(|handle| handle.join().expect("Thread panicked"))
                .collect::<Vec<_>>();

            let nodes: u64 = moves_and_stats.iter().map(|(_, stats)| stats.nodes).sum();
            let best = moves_and_stats
                .iter()
                .max_by_key(|(result, _)| result.score)
                .expect("should have at least one result");

            if !self.silent {
                if best.0.done_early {
                    best.1.uci_info_done_early(
                        best.0.bestmove,
                        best.0.depth,
                        best.0.score,
                        self.tt.hashfull(),
                    );
                }
                println!("bestmove {}", best.0.bestmove);
            }

            (best.0, nodes)
        })
    }

    pub fn think(&self, limits: Limits) -> (SearchResult, u64) {
        self.think_with_stop(limits, Arc::new(AtomicBool::new(false)))
    }
}
