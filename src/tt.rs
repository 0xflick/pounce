use std::sync::Mutex;

use crate::{moves::Move, zobrist::ZobristHash};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum EntryType {
    None,
    Exact,
    LowerBound,
    UpperBound,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Entry {
    pub key: ZobristHash,
    pub depth: u8,
    pub score: i16,
    pub score_type: EntryType,
    pub best_move: Move,
}

impl Entry {
    pub fn new(
        key: ZobristHash,
        depth: u8,
        score: i16,
        score_type: EntryType,
        best_move: Move,
    ) -> Entry {
        Entry {
            key,
            depth,
            score,
            score_type,
            best_move,
        }
    }
}

impl Default for Entry {
    fn default() -> Entry {
        Entry {
            key: ZobristHash::new(),
            depth: 0,
            score: 0,
            score_type: EntryType::None,
            best_move: Move::NONE,
        }
    }
}

pub struct Table {
    entries: Mutex<Vec<Entry>>,
    max_size: usize,
}

impl Table {
    pub fn new(size: usize) -> Table {
        Table {
            entries: Mutex::new(vec![Entry::default(); size]),
            max_size: size,
        }
    }

    pub fn new_mb(size_mb: usize) -> Table {
        Table::new(size_mb * 1024 * 1024 / std::mem::size_of::<Entry>())
    }

    pub fn clear(&self) {
        self.entries.lock().unwrap().iter_mut().for_each(|entry| {
            *entry = Entry::default();
        });
    }

    fn index(&self, key: ZobristHash) -> usize {
        usize::from(key) % self.max_size
    }

    pub fn probe(&self, key: ZobristHash) -> Option<Entry> {
        let idx = self.index(key);
        let entry = &self.entries.lock().unwrap()[idx];
        match entry.key == key {
            true => Some(*entry),
            false => None,
        }
    }

    pub fn set(&self, entry: Entry) {
        let idx = self.index(entry.key);
        self.entries.lock().unwrap()[idx] = entry;
    }

    pub fn hashfull(&self) -> f64 {
        self.entries.lock().unwrap()[..1000]
            .iter()
            .filter(|entry| entry.score_type != EntryType::None)
            .count() as f64
    }

    pub fn size_mb(&self) -> usize {
        self.max_size * std::mem::size_of::<Entry>() / 1024 / 1024
    }
}
