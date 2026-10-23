use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use crate::chess::Position;
use crate::chess::position::fen::{self, Fen};
use crate::engine::eval::nnue;
use crate::engine::limits::Limits;
use crate::engine::search::{Search, SearchResult};
use crate::engine::tt::Table;

pub type Net = nnue::PerspectiveNet<{ nnue::NNUE_HIDDEN_SIZE }>;

/// Owns the persistent, cross-game search state: the transposition table
/// (shared lockless via `Arc<Table>` so it's reused without any mutex), the
/// NNUE net (`Arc`, read-only), the current root position and the config.
/// The UCI layer owns one directly (no mutex) and hands *snapshots* — a
/// cloned position + cloned `Arc`s — to the search thread, so the UCI thread
/// never shares a lock with a running search.
pub struct SearchManager {
    num_threads: usize,
    silent: bool,
    pub position: Position,
    tt: Arc<Table>,
    net: Arc<Net>,
}

impl SearchManager {
    pub fn new(num_threads: usize, hash_size: usize) -> Self {
        let Fen(position) = fen::STARTPOS.parse().unwrap();
        Self::new_from_position(num_threads, hash_size, position)
    }

    pub fn new_from_position(num_threads: usize, hash_size: usize, position: Position) -> Self {
        let net = Net::load().expect("Failed to load NNUE network");
        SearchManager {
            num_threads,
            silent: false,
            tt: Arc::new(Table::new_mb(hash_size)),
            position,
            net: Arc::new(net),
        }
    }

    pub fn set_silent(&mut self, silent: bool) {
        self.silent = silent;
    }

    pub fn set_num_threads(&mut self, num_threads: usize) {
        self.num_threads = num_threads;
    }

    pub fn set_tt_size_mb(&mut self, size_mb: usize) {
        self.tt = Arc::new(Table::new_mb(size_mb));
    }

    pub fn clear_tt(&mut self) {
        // UCI joins any in-flight search before calling this, so we hold the
        // sole Arc here (bench/datagen are single-threaded).
        Arc::get_mut(&mut self.tt)
            .expect("clear_tt while a search is in flight")
            .clear();
    }

    /// A snapshot to hand to a background search: cloned root position +
    /// cloned `Arc`s for the shared TT/net + config. The search runs on
    /// these alone and never touches the manager again, so the UCI thread is
    /// free to mutate `position`/config afterwards with no lock.
    pub(crate) fn search_inputs(&self) -> (Position, Arc<Table>, Arc<Net>, usize, bool) {
        (
            self.position.clone(),
            Arc::clone(&self.tt),
            Arc::clone(&self.net),
            self.num_threads,
            self.silent,
        )
    }

    /// Synchronous search — used by `bench` / `datagen`, and as the core for
    /// the async UCI path (`search_core`). Returns the result + node count.
    pub fn think_with_stop(&mut self, limits: Limits, stop: Arc<AtomicBool>) -> (SearchResult, u64) {
        stop.store(false, Ordering::Relaxed);
        self.tt.bump_age();
        search_core(
            &self.position,
            &self.tt,
            &self.net,
            self.num_threads,
            self.silent,
            &stop,
            limits,
        )
    }

    pub fn think(&mut self, limits: Limits) -> (SearchResult, u64) {
        self.think_with_stop(limits, Arc::new(AtomicBool::new(false)))
    }
}

/// The Lazy-SMP search: spawn `num_threads` workers (each on its own position
/// clone, sharing the TT/net by `&`), join, pick the best result, print
/// `bestmove` once. Free function so the async UCI path can run it on a
/// detached search thread (owning the `Arc`s) just like bench runs it inline.
pub(crate) fn search_core(
    position: &Position,
    tt: &Table,
    net: &Net,
    num_threads: usize,
    silent: bool,
    stop: &AtomicBool,
    limits: Limits,
) -> (SearchResult, u64) {
    thread::scope(|s| {
        let mut handles = Vec::with_capacity(num_threads);
        for thread_idx in 0..num_threads {
            let position = position.clone();
            let handle = s.spawn(move || {
                let mut search = Search::new(position, limits, tt, stop, thread_idx, silent, net);
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

        if !silent {
            if best.0.done_early {
                best.1.uci_info_done_early(
                    best.0.bestmove,
                    best.0.depth,
                    best.0.score,
                    tt.hashfull(),
                );
            }
            println!("bestmove {}", best.0.bestmove);
        }

        (best.0, nodes)
    })
}
