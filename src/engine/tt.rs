use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};

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
    None = 0,
    Exact = 1,
    LowerBound = 2,
    UpperBound = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Entry {
    pub key: ZobristHash,
    pub depth: u8,
    pub score: i16,
    pub entry_type: EntryType,
    pub best_move: Move,
    pub static_eval: i16,
}

impl Entry {
    pub fn new(
        key: ZobristHash,
        depth: u8,
        score: i16,
        entry_type: EntryType,
        best_move: Move,
        static_eval: i16,
    ) -> Self {
        Self {
            key,
            depth,
            score,
            entry_type,
            best_move,
            static_eval,
        }
    }

    // Pack entry into u64 (age will be added by Table)
    fn pack(&self, age: u8) -> u64 {
        let move_bits = unsafe { std::mem::transmute::<Move, u16>(self.best_move) } as u64;
        let score_bits = self.score as u16 as u64;
        let eval_bits = self.static_eval as u16 as u64;

        move_bits
            | (score_bits << 16)
            | ((self.depth as u64) << 32)
            | ((age as u64) << 40)
            | ((self.entry_type as u64) << 46)
            | (eval_bits << 48)
    }

    // Unpack from TTMemory
    fn unpack(mem: &TTMemory) -> (Self, u8) {
        let mem_key = mem.key.load(Ordering::Relaxed);
        let data = mem.data.load(Ordering::Relaxed);

        let key = unsafe { std::mem::transmute::<u64, ZobristHash>(mem_key ^ data) };
        let best_move = unsafe { std::mem::transmute::<u16, Move>(data as u16) };
        let score = ((data >> 16) & 0xFFFF) as i16;
        let depth = ((data >> 32) & 0xFF) as u8;
        let age = ((data >> 40) & 0x3F) as u8;
        let score_type =
            unsafe { std::mem::transmute::<u8, EntryType>(((data >> 46) & 0x3) as u8) };
        let static_eval = ((data >> 48) & 0xFFFF) as i16;

        (
            Self {
                key,
                depth,
                score,
                entry_type: score_type,
                best_move,
                static_eval,
            },
            age,
        )
    }

    fn write(&self, mem: &TTMemory, age: u8) {
        let data = self.pack(age);
        let key = unsafe { std::mem::transmute::<ZobristHash, u64>(self.key) } ^ data;
        mem.key.store(key, Ordering::Relaxed);
        mem.data.store(data, Ordering::Relaxed);
    }
}

pub struct Table {
    entries: Vec<TTMemory>,
    age: AtomicU8,
}

impl Table {
    pub fn new_mb(size_mb: usize) -> Self {
        let size = size_mb * 1024 * 1024 / std::mem::size_of::<TTMemory>();
        Self {
            entries: (0..size).map(|_| TTMemory::default()).collect(),
            age: AtomicU8::new(0),
        }
    }

    pub fn clear(&self) {
        self.age.fetch_add(1, Ordering::Relaxed);
        self.entries.iter().for_each(|e| {
            e.key.store(0, Ordering::Relaxed);
            e.data.store(0, Ordering::Relaxed);
        });
    }

    pub fn new_search(&self) {
        self.age.fetch_add(1, Ordering::Relaxed);
    }

    fn index(&self, key: ZobristHash) -> usize {
        let hash = unsafe { std::mem::transmute::<ZobristHash, u64>(key) };
        ((hash as u128 * self.entries.len() as u128) >> 64) as usize
    }

    pub fn probe(&self, key: ZobristHash) -> Option<Entry> {
        let (entry, _) = Entry::unpack(&self.entries[self.index(key)]);
        (entry.key == key).then_some(entry)
    }

    pub fn store(&self, entry: Entry) {
        let idx = self.index(entry.key);
        let current_age = self.age.load(Ordering::Relaxed);

        let (existing, old_age) = Entry::unpack(&self.entries[idx]);
        // Effective depth = actual depth + bonus for exact nodes
        let effective_depth = |d: u8, t: EntryType| d + (t == EntryType::Exact) as u8 * 2;
        let replace = existing.key == ZobristHash::default()
            || existing.key == entry.key
            || current_age.wrapping_sub(old_age) > 3
            || effective_depth(entry.depth, entry.entry_type)
                >= effective_depth(existing.depth, existing.entry_type) - 2;
        if replace {
            entry.write(&self.entries[idx], current_age);
        }
    }

    pub fn hashfull(&self) -> f64 {
        let sample = &self.entries[..1000.min(self.entries.len())];
        sample
            .iter()
            .filter(|e| Entry::unpack(e).0.key != ZobristHash::default())
            .count() as f64
            / sample.len() as f64
    }

    pub fn size_mb(&self) -> usize {
        self.entries.len() * std::mem::size_of::<TTMemory>() / (1024 * 1024)
    }
}
