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
    UpperBound,
    LowerBound,
    Exact,
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

    // New bit layout with age:
    // move: bits 0-15
    // score: bits 16-31
    // depth: bits 32-39
    // type: bits 40-41
    // age: bits 42-47
    const MOVE_MASK: u64 = 0xFFFF;
    const SCORE_SHIFT: u32 = 16;
    const SCORE_MASK: u64 = 0xFFFF;
    const DEPTH_SHIFT: u32 = 32;
    const DEPTH_MASK: u64 = 0xFF;
    const TYPE_SHIFT: u32 = 40;
    const TYPE_MASK: u64 = 0x3;
    const AGE_SHIFT: u32 = 42;
    const AGE_MASK: u64 = 0x3F;

    fn read_from(mem: &TTMemory, _current_age: u8) -> (Entry, u8) {
        let mem_key = mem.key.load(std::sync::atomic::Ordering::Relaxed);
        let mem_data = mem.data.load(std::sync::atomic::Ordering::Relaxed);

        unsafe {
            let key = std::mem::transmute::<u64, ZobristHash>(mem_key ^ mem_data);
            let best_move = std::mem::transmute::<u16, Move>((mem_data & Self::MOVE_MASK) as u16);
            let score = ((mem_data >> Self::SCORE_SHIFT) & Self::SCORE_MASK) as i16;
            let depth = ((mem_data >> Self::DEPTH_SHIFT) & Self::DEPTH_MASK) as u8;
            let score_type = std::mem::transmute::<u8, EntryType>(
                ((mem_data >> Self::TYPE_SHIFT) & Self::TYPE_MASK) as u8,
            );
            let age = ((mem_data >> Self::AGE_SHIFT) & Self::AGE_MASK) as u8;

            (
                Entry {
                    key,
                    depth,
                    score,
                    score_type,
                    best_move,
                },
                age,
            )
        }
    }

    fn write_to(&self, mem: &TTMemory, age: u8) {
        unsafe {
            let best_move = std::mem::transmute::<Move, u16>(self.best_move) as u64;
            let score = self.score as u64 & Self::SCORE_MASK;
            let depth = self.depth as u64 & Self::DEPTH_MASK;
            let score_type = self.score_type as u64 & Self::TYPE_MASK;
            let age = age as u64 & Self::AGE_MASK;

            let data = (best_move & Self::MOVE_MASK)
                | (score << Self::SCORE_SHIFT)
                | (depth << Self::DEPTH_SHIFT)
                | (score_type << Self::TYPE_SHIFT)
                | (age << Self::AGE_SHIFT);
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
    age: AtomicU8,
}

impl Table {
    pub fn new(size: usize) -> Table {
        let mut entries = Vec::with_capacity(size);
        for _ in 0..size {
            entries.push(TTMemory {
                key: AtomicU64::new(0),
                data: AtomicU64::new(0),
            });
        }
        Table {
            entries,
            max_size: size,
            age: AtomicU8::new(0),
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
        let current_age = self.age.load(std::sync::atomic::Ordering::Relaxed);
        let (entry, _entry_age) = Entry::read_from(&self.entries[idx], current_age);
        match entry.key == key {
            true => Some(entry),
            false => None,
        }
    }

    // Now with replacement logic from problematic commit
    pub fn store(&self, entry: Entry) {
        let idx = self.index(entry.key);
        let current_age = self.age.load(std::sync::atomic::Ordering::Relaxed);

        let (existing_entry, existing_age) = Entry::read_from(&self.entries[idx], current_age);
        let age_diff = age_diff(current_age, existing_age) as u16;

        let entry_prio = entry.depth as u16 + entry.score_type as u16 + (age_diff * age_diff) / 4;
        let existing_prio = existing_entry.depth as u16 + existing_entry.score_type as u16;

        if entry.key != existing_entry.key
            || (entry.score_type == EntryType::Exact
                && existing_entry.score_type != EntryType::Exact)
            || entry_prio * 3 > existing_prio * 2
        {
            entry.write_to(&self.entries[idx], current_age);
        }
    }

    // Keep old name for compatibility
    pub fn set(&self, entry: Entry) {
        self.store(entry);
    }

    pub fn hashfull(&self) -> f64 {
        let current_age = self.age.load(std::sync::atomic::Ordering::Relaxed);
        self.entries[..1000]
            .iter()
            .filter(|entry| Entry::read_from(entry, current_age).0.key != ZobristHash::default())
            .count() as f64
    }

    pub fn size_mb(&self) -> usize {
        self.max_size * std::mem::size_of::<Entry>() / 1024 / 1024
    }
}

fn age_diff(current_age: u8, old_age: u8) -> u8 {
    current_age.wrapping_sub(old_age) & 0x3F
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
