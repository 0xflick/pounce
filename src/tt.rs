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
            best_move: Move::NULL,
        }
    }
}

pub(crate) struct Table {
    entries: Vec<Entry>,
    max_size: usize,
}

impl Table {
    pub fn new(size: usize) -> Table {
        Table {
            entries: vec![Entry::default(); size],
            max_size: size,
        }
    }

    pub fn new_mb(size_mb: usize) -> Table {
        Table::new(size_mb * 1024 * 1024 / std::mem::size_of::<Entry>())
    }

    pub fn clear(&mut self) {
        self.entries.iter_mut().for_each(|entry| {
            *entry = Entry::default();
        });
    }

    fn index(&self, key: ZobristHash) -> usize {
        usize::from(key) % self.max_size
    }

    pub fn probe(&self, key: ZobristHash) -> Option<&Entry> {
        let idx = self.index(key);
        if self.entries[idx].key == key {
            Some(&self.entries[idx])
        } else {
            None
        }
    }

    pub fn set(&mut self, entry: Entry) {
        let idx = self.index(entry.key);
        self.entries[idx] = entry;
    }

    pub fn hashfull(&self) -> f64 {
        self.entries[..1000]
            .iter()
            .filter(|entry| entry.score_type != EntryType::None)
            .count() as f64
    }
}
