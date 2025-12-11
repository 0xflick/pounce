use std::cell::UnsafeCell;

use crate::chess::Move;
use crate::zobrist::ZobristHash;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum EntryType {
    #[default]
    None = 0,
    UpperBound = 1,
    LowerBound = 2,
    Exact = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
#[repr(C)]
pub struct Entry {
    pub key: ZobristHash,
    pub static_eval: i16,
    pub score: i16,
    pub best_move: Move,
    pub depth: u8,
    age_and_type: u8, // Top 2 bits: EntryType, Bottom 6 bits: age
}

impl Entry {
    pub fn new(
        key: ZobristHash,
        depth: u8,
        static_eval: i16,
        score: i16,
        score_type: EntryType,
        best_move: Move,
    ) -> Self {
        Self {
            key,
            depth,
            static_eval,
            score,
            best_move,
            age_and_type: (score_type as u8) << 6, // Type in top 2 bits, age=0
        }
    }

    #[inline]
    pub fn score_type(&self) -> EntryType {
        // Extract top 2 bits
        match self.age_and_type >> 6 {
            0 => EntryType::None,
            1 => EntryType::UpperBound,
            2 => EntryType::LowerBound,
            3 => EntryType::Exact,
            _ => unreachable!(),
        }
    }

    #[inline]
    pub fn age(&self) -> u8 {
        // Extract bottom 6 bits
        self.age_and_type & 0x3F
    }

    #[inline]
    fn set_age_and_type(&mut self, age: u8, score_type: EntryType) {
        // Combine type (top 2 bits) and age (bottom 6 bits)
        self.age_and_type = ((score_type as u8) << 6) | (age & 0x3F);
    }
}

pub struct Table {
    entries: Vec<UnsafeCell<Entry>>,
    pub age: u8, // Only uses bottom 6 bits
}

// We're naughty
unsafe impl Sync for Table {}
unsafe impl Send for Table {}

impl Table {
    pub fn new(size: usize) -> Self {
        let mut entries = Vec::with_capacity(size);
        for _ in 0..size {
            entries.push(UnsafeCell::new(Entry::default()));
        }
        Self { entries, age: 0 }
    }

    pub fn new_mb(size_mb: usize) -> Self {
        Self::new(size_mb * 1024 * 1024 / std::mem::size_of::<Entry>())
    }

    pub fn clear(&mut self) {
        self.entries.iter_mut().for_each(|cell| {
            *cell.get_mut() = Entry::default();
        });
    }

    pub fn increment_age(&mut self) {
        // Wrap at 6 bits (0-63)
        self.age = (self.age + 1) & 0x3F;
    }

    fn index(&self, key: ZobristHash) -> usize {
        let big_key = u128::from(key);
        let len = self.entries.len() as u128;
        ((big_key * len) >> 64) as usize
    }

    pub fn probe(&self, key: ZobristHash) -> Option<Entry> {
        let idx = self.index(key);
        unsafe {
            let entry = *self.entries[idx].get();
            if entry.key == key { Some(entry) } else { None }
        }
    }

    pub fn store(&self, mut entry: Entry) {
        let idx = self.index(entry.key);
        let score_type = entry.score_type();
        entry.set_age_and_type(self.age, score_type);

        let should_write = if let Some(existing_entry) = self.probe(entry.key) {
            let age_diff = age_diff(self.age, existing_entry.age()) as u16;

            let entry_prio = entry.depth as u16 + score_type as u16 + (age_diff * age_diff) / 4;
            let existing_prio = existing_entry.depth as u16 + existing_entry.score_type() as u16;

            entry.key != existing_entry.key
                || (score_type == EntryType::Exact
                    && existing_entry.score_type() != EntryType::Exact)
                || entry_prio * 3 > existing_prio * 2
        } else {
            true
        };

        if should_write {
            unsafe { *self.entries[idx].get() = entry }
        }
    }

    pub fn hashfull(&self) -> f64 {
        self.entries[..1000]
            .iter()
            .filter(|e| unsafe {
                let ptr = e.get();
                (*ptr).key != ZobristHash::default() && (*ptr).age() == self.age
            })
            .count() as f64
    }
}

fn age_diff(current_age: u8, old_age: u8) -> u8 {
    // Both ages are already 6-bit values
    current_age.wrapping_sub(old_age) & 0x3F
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table() {
        // Now should be smaller without the separate score_type field
        assert_eq!(std::mem::size_of::<Entry>(), 16);
    }

    #[test]
    fn test_age_and_type_packing() {
        let mut entry = Entry::default();

        // Test setting different types and ages
        entry.set_age_and_type(42, EntryType::Exact);
        assert_eq!(entry.age(), 42);
        assert_eq!(entry.score_type(), EntryType::Exact);

        entry.set_age_and_type(63, EntryType::LowerBound);
        assert_eq!(entry.age(), 63);
        assert_eq!(entry.score_type(), EntryType::LowerBound);

        // Test age wrapping
        entry.set_age_and_type(64, EntryType::UpperBound);
        assert_eq!(entry.age(), 0); // 64 & 0x3F = 0
        assert_eq!(entry.score_type(), EntryType::UpperBound);
    }
}
