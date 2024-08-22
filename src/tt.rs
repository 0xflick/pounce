use crate::moves::Move;

struct TTEntry {
    hash: u64,
    depth: u8,
    score: i16,
    best_move: Move,
}
