use std::sync::atomic::AtomicU64;

use crate::chess::Move;
use crate::zobrist::ZobristHash;

#[derive(Debug)]
#[repr(C)]
struct TTMemory {
    key: AtomicU64,
    data: AtomicU64,
}

impl Default for TTMemory {
    fn default() -> TTMemory {
        TTMemory {
            key: AtomicU64::new(0),
            data: AtomicU64::new(0),
        }
    }
}

impl TTMemory {
    fn clear(&self) {
        self.key.store(0, std::sync::atomic::Ordering::Relaxed);
        self.data.store(0, std::sync::atomic::Ordering::Relaxed);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum EntryType {
    None,
    Exact,
    LowerBound,
    UpperBound,
}

#[derive(Clone, Copy, Debug, PartialEq)]
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

    fn read_from(mem: &TTMemory) -> Entry {
        let mem_key = mem.key.load(std::sync::atomic::Ordering::Relaxed);
        let mem_data = mem.data.load(std::sync::atomic::Ordering::Relaxed);

        unsafe {
            let key = std::mem::transmute::<u64, ZobristHash>(mem_key ^ mem_data);
            let depth = (mem_data >> 48) as u8;
            let score = ((mem_data >> 32) & 0xffff) as i16;
            let score_type = std::mem::transmute::<u8, EntryType>(((mem_data >> 24) & 0xff) as u8);
            let best_move = std::mem::transmute::<u16, Move>(mem_data as u16);

            Entry {
                key,
                depth,
                score,
                score_type,
                best_move,
            }
        }
    }

    fn write_to(&self, mem: &TTMemory) {
        unsafe {
            let depth = self.depth as u64;

            let score = self.score as u64 & 0xffff;
            let score_type = self.score_type as u64;
            let best_move = std::mem::transmute::<Move, u16>(self.best_move) as u64;

            let data = (depth << 48) | (score << 32) | (score_type << 24) | best_move;
            let key = std::mem::transmute::<ZobristHash, u64>(self.key) ^ data;

            mem.key.store(key, std::sync::atomic::Ordering::Relaxed);
            mem.data.store(data, std::sync::atomic::Ordering::Relaxed);
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
    entries: Vec<TTMemory>,
    max_size: usize,
}

impl Table {
    pub fn new(size: usize) -> Table {
        Table {
            entries: (0..size).map(|_| TTMemory::default()).collect(),
            max_size: size,
        }
    }

    pub fn new_mb(size_mb: usize) -> Table {
        Table::new(size_mb * 1024 * 1024 / std::mem::size_of::<Entry>())
    }

    pub fn clear(&self) {
        self.entries.iter().for_each(|entry| {
            entry.clear();
        });
    }

    fn index(&self, key: ZobristHash) -> usize {
        let big_key = u128::from(key);
        let len = self.entries.len() as u128;
        ((big_key * len) >> 64) as usize
    }

    pub fn probe(&self, key: ZobristHash) -> Option<Entry> {
        let idx = self.index(key);
        let entry = Entry::read_from(&self.entries[idx]);
        match entry.key == key {
            true => Some(entry),
            false => None,
        }
    }

    pub fn set(&self, entry: Entry) {
        let idx = self.index(entry.key);
        let val = &self.entries[idx];
        entry.write_to(val);
    }

    pub fn hashfull(&self) -> f64 {
        self.entries[..1000]
            .iter()
            .filter(|entry| Entry::read_from(entry).key != ZobristHash::default())
            .count() as f64
    }

    pub fn size_mb(&self) -> usize {
        self.max_size * std::mem::size_of::<Entry>() / 1024 / 1024
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chess::position::fen::{Fen, STARTPOS};
    use crate::chess::position::zobrist::init_zobrist;

    fn random_key() -> ZobristHash {
        init_zobrist();
        let Fen(pos) = STARTPOS.parse().unwrap();
        pos.key
    }

    #[test]
    fn test_table() {
        assert_eq!(std::mem::size_of::<Entry>(), 16);
    }

    #[test]
    fn test_insert() {
        let tt = Table::new_mb(1);
        assert_eq!(tt.size_mb(), 1);
        let key = random_key();

        let e = Entry::new(key, 20, -150, EntryType::Exact, Move::NULL);

        tt.set(e);
        assert_eq!(tt.probe(key), Some(e));
    }
}
