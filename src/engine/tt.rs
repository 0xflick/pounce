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
    fn default() -> Self {
        Self {
            key: AtomicU64::new(0),
            data: AtomicU64::new(0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum EntryType {
    #[default]
    None,
    Exact,
    LowerBound,
    UpperBound,
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
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
    ) -> Self {
        Self {
            key,
            depth,
            score,
            score_type,
            best_move,
        }
    }

    const MOVE_BITS: u32 = 16;
    const MOVE_MASK: u64 = (1 << Self::MOVE_BITS) - 1;

    const SCORE_SHIFT: u32 = 16;
    const SCORE_BITS: u32 = 16;
    const SCORE_MASK: u64 = (1 << Self::SCORE_BITS) - 1;

    const DEPTH_SHIFT: u32 = 32;
    const DEPTH_BITS: u32 = 8;
    const DEPTH_MASK: u64 = (1 << Self::DEPTH_BITS) - 1;

    const TYPE_SHIFT: u32 = 40;
    const TYPE_BITS: u32 = 2;
    const TYPE_MASK: u64 = (1 << Self::TYPE_BITS) - 1;

    fn read_from(mem: &TTMemory) -> Self {
        let mem_key = mem.key.load(std::sync::atomic::Ordering::Relaxed);
        let mem_data = mem.data.load(std::sync::atomic::Ordering::Relaxed);

        unsafe {
            let key = std::mem::transmute::<u64, ZobristHash>(mem_key ^ mem_data);

            let depth = ((mem_data >> Self::DEPTH_SHIFT) & Self::DEPTH_MASK) as u8;
            let score = ((mem_data >> Self::SCORE_SHIFT) & Self::SCORE_MASK) as i16;
            let score_type = std::mem::transmute::<u8, EntryType>(
                ((mem_data >> Self::TYPE_SHIFT) & Self::TYPE_MASK) as u8,
            );
            let best_move = std::mem::transmute::<u16, Move>((mem_data & Self::MOVE_MASK) as u16);

            Self {
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
            let data = ((std::mem::transmute::<Move, u16>(self.best_move) as u64)
                & Self::MOVE_MASK)
                | (((self.score as u64) & Self::SCORE_MASK) << Self::SCORE_SHIFT)
                | (((self.depth as u64) & Self::DEPTH_MASK) << Self::DEPTH_SHIFT)
                | ((self.score_type as u8 as u64 & Self::TYPE_MASK) << Self::TYPE_SHIFT);

            let key = std::mem::transmute::<ZobristHash, u64>(self.key) ^ data;

            mem.key.store(key, std::sync::atomic::Ordering::Relaxed);
            mem.data.store(data, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

pub struct Table {
    entries: Vec<TTMemory>,
}

impl Table {
    pub fn new(size: usize) -> Self {
        Self {
            entries: (0..size).map(|_| TTMemory::default()).collect(),
        }
    }

    pub fn new_mb(size_mb: usize) -> Self {
        Self::new(size_mb * 1024 * 1024 / std::mem::size_of::<TTMemory>())
    }

    pub fn clear(&self) {
        self.entries.iter().for_each(|entry| {
            entry.key.store(0, std::sync::atomic::Ordering::Relaxed);
            entry.data.store(0, std::sync::atomic::Ordering::Relaxed);
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
        assert_eq!(std::mem::size_of::<TTMemory>(), 16);
    }
}
