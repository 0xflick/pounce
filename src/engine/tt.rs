use std::sync::atomic::{AtomicU8, AtomicU64};

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
    UpperBound,
    LowerBound,
    Exact,
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
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
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
struct TTData {
    key: ZobristHash,
    depth: u8,
    score: i16,
    score_type: EntryType,
    best_move: Move,
    age: u8, // Age of the entry for LRU purposes
}

impl TTData {
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

    const AGE_SHIFT: u32 = 42;
    const AGE_BITS: u32 = 6;
    const AGE_MASK: u64 = (1 << Self::AGE_BITS) - 1;

    fn pack(&self) -> u64 {
        unsafe {
            ((std::mem::transmute::<Move, u16>(self.best_move) as u64) & Self::MOVE_MASK)
                | (((self.score as u64) & Self::SCORE_MASK) << Self::SCORE_SHIFT)
                | (((self.depth as u64) & Self::DEPTH_MASK) << Self::DEPTH_SHIFT)
                | ((self.score_type as u8 as u64 & Self::TYPE_MASK) << Self::TYPE_SHIFT)
                | ((self.age as u64 & Self::AGE_MASK) << Self::AGE_SHIFT)
        }
    }

    fn unpack(mem_key: u64, data: u64) -> Self {
        unsafe {
            let key = std::mem::transmute::<u64, ZobristHash>(mem_key ^ data);
            let best_move = std::mem::transmute::<u16, Move>((data & Self::MOVE_MASK) as u16);
            let score = ((data >> Self::SCORE_SHIFT) & Self::SCORE_MASK) as i16;
            let depth = ((data >> Self::DEPTH_SHIFT) & Self::DEPTH_MASK) as u8;
            let score_type = std::mem::transmute::<u8, EntryType>(
                ((data >> Self::TYPE_SHIFT) & Self::TYPE_MASK) as u8,
            );
            let age = ((data >> Self::AGE_SHIFT) & Self::AGE_MASK) as u8;

            Self {
                key,
                depth,
                score,
                score_type,
                best_move,
                age,
            }
        }
    }

    fn write(&self, mem: &TTMemory) {
        let data = self.pack();
        let key = u64::from(self.key) ^ data;
        mem.key.store(key, std::sync::atomic::Ordering::Relaxed);
        mem.data.store(data, std::sync::atomic::Ordering::Relaxed);
    }
}

impl From<&TTData> for TTMemory {
    fn from(tt_data: &TTData) -> Self {
        let data = tt_data.pack();
        let key = u64::from(tt_data.key) ^ data;
        TTMemory {
            key: AtomicU64::new(key),
            data: AtomicU64::new(data),
        }
    }
}

impl From<&TTMemory> for TTData {
    fn from(mem: &TTMemory) -> Self {
        let key = mem.key.load(std::sync::atomic::Ordering::Relaxed);
        let data = mem.data.load(std::sync::atomic::Ordering::Relaxed);
        Self::unpack(key, data)
    }
}

pub struct Table {
    entries: Vec<TTMemory>,
    age: AtomicU8,
}

impl Table {
    pub fn new(size: usize) -> Self {
        Self {
            entries: (0..size).map(|_| TTMemory::default()).collect(),
            age: AtomicU8::new(0),
        }
    }

    pub fn new_mb(size_mb: usize) -> Self {
        Self::new(size_mb * 1024 * 1024 / std::mem::size_of::<TTMemory>())
    }

    pub fn clear(&self) {
        self.entries.iter().for_each(|mem| {
            mem.key.store(0, std::sync::atomic::Ordering::Relaxed);
            mem.data.store(0, std::sync::atomic::Ordering::Relaxed);
        });
    }

    pub fn increment_age(&self) {
        self.age.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn index(&self, key: ZobristHash) -> usize {
        let big_key = u128::from(key);
        let len = self.entries.len() as u128;
        ((big_key * len) >> 64) as usize
    }

    pub fn probe(&self, key: ZobristHash) -> Option<Entry> {
        let idx = self.index(key);
        let data = TTData::from(&self.entries[idx]);
        match data.key == key {
            true => Some(Entry {
                key: data.key,
                depth: data.depth,
                score: data.score,
                score_type: data.score_type,
                best_move: data.best_move,
            }),
            false => None,
        }
    }

    pub fn store(&self, entry: Entry) {
        let idx = self.index(entry.key);
        let current_age = self.age.load(std::sync::atomic::Ordering::Relaxed);

        let existing_data = TTData::from(&self.entries[idx]);
        let age_diff = age_diff(current_age, existing_data.age) as u16;

        let entry_prio = entry.depth as u16 + entry.score_type as u16 + (age_diff * age_diff) / 4;
        let existing_prio = existing_data.depth as u16 + existing_data.score_type as u16;

        if entry.key != existing_data.key
            || (entry.score_type == EntryType::Exact
                && existing_data.score_type != EntryType::Exact)
            || entry_prio * 3 > existing_prio * 2
        {
            let data = TTData {
                key: entry.key,
                depth: entry.depth,
                score: entry.score,
                score_type: entry.score_type,
                best_move: entry.best_move,
                age: current_age,
            };

            data.write(&self.entries[idx]);
        }
    }

    pub fn hashfull(&self) -> f64 {
        self.entries[..1000]
            .iter()
            .filter(|mem| TTData::from(*mem).key != ZobristHash::default())
            .count() as f64
    }
}

fn age_diff(current_age: u8, old_age: u8) -> u8 {
    current_age.wrapping_sub(old_age) & 0x3F // Mask to 6 bits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table() {
        assert_eq!(std::mem::size_of::<TTMemory>(), 16);
    }
}
