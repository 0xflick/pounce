use std::mem::size_of;

use crate::board::Move;

#[derive(Clone, Debug)]
pub enum ScoreType {
    Exact,
    Alpha,
    Beta,
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub z_key: u64,
    pub best_move: Option<Move>,
    pub depth: u8,
    pub score: i32,
    pub score_type: ScoreType,
    pub epoch: u16,
}

pub struct Table {
    entries: Vec<Option<Entry>>,
    num_entries: usize,
    num_hits: usize,
    num_probes: usize,
}

impl Table {
    pub fn new(size: usize) -> Table {
        let entries = size / size_of::<Entry>();

        Table {
            entries: vec![None; entries.next_power_of_two() >> 1],
            num_entries: 0,
            num_hits: 0,
            num_probes: 0,
        }
    }
    pub fn probe(&mut self, z_key: u64) -> Option<&Entry> {
        self.num_probes += 1;
        let index = (z_key as usize) % self.entries.len();
        if let Some(ref e) = self.entries[index] {
            if e.z_key == z_key {
                self.num_hits += 1;
            }
        }

        self.entries[index].as_ref()
    }

    pub fn save(&mut self, entry: Entry) {
        let index = (entry.z_key as usize) % self.entries.len();
        let should_replace = match self.entries[index] {
            Some(ref e) => e.epoch + 1 < entry.epoch || e.depth <= entry.depth + 1,
            None => {
                self.num_entries += 1;
                true
            }
        };

        if should_replace {
            self.entries[index] = Some(entry);
        }
    }

    pub fn per_mille_full(&self) -> usize {
        ((self.num_entries as f64 / self.entries.len() as f64) * 1000.0) as usize
    }

    pub fn per_mille_hits(&self) -> usize {
        ((self.num_hits as f64 / self.num_probes as f64) * 1000.0) as usize
    }
}
