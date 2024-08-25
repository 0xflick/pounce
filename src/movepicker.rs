use std::cell::RefCell;

use arrayvec::ArrayVec;

use crate::{bitboard::Bitboard, movegen::MoveGen, moves::Move, position::Position};

const CAPTURE_SCORE: u16 = 1000;

const MAX_MOVES: usize = 256;

const MVV_LVA: [[u16; 6]; 6] = [
    [15, 25, 35, 45, 55, 0], // attacker pawn, victim P, N, B, R, Q,  K
    [14, 24, 34, 44, 54, 0], // attacker knight, victim P, N, B, R, Q,  K
    [13, 23, 33, 43, 53, 0], // attacker bishop, victim P, N, B, R, Q,  K
    [12, 22, 32, 42, 52, 0], // attacker rook, victim P, N, B, R, Q,  K
    [11, 21, 31, 41, 51, 0], // attacker queen, victim P, N, B, R, Q,  K
    [10, 20, 30, 40, 50, 0], // attacker king, victim P, N, B, R, Q,  K
];

struct MoveWithScore {
    m: Move,
    score: u16,
}

type MoveList = ArrayVec<MoveWithScore, MAX_MOVES>;

// TODO: killers, history, etc.
enum MovePickerStage {
    TT,
    ScoreCaptures,
    Captures,
    Quiets,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MovePickerMode {
    Normal,
    Quiescence,
}

pub struct MovePicker {
    occ: Bitboard,
    move_generator: MoveGen,
    stage: MovePickerStage,
    mode: MovePickerMode,
    tt_move: Move,

    scored_moves: RefCell<MoveList>,
    scored_len: usize,
    scored_index: usize,

    pos: RefCell<Position>,
}

impl MovePicker {
    pub fn new(pos: RefCell<Position>, mode: MovePickerMode, tt_move: Move) -> MovePicker {
        let occ = pos.borrow().occupancy;
        let mg = MoveGen::new(&pos.borrow());
        MovePicker {
            occ,
            move_generator: mg,
            stage: MovePickerStage::TT,
            mode,
            tt_move,
            scored_moves: RefCell::new(ArrayVec::new()),
            scored_len: 0,
            scored_index: 0,
            pos,
        }
    }

    pub fn new_quiescence(pos: RefCell<Position>) -> MovePicker {
        MovePicker::new(pos, MovePickerMode::Quiescence, Move::NULL)
    }

    pub fn new_ab_search(pos: RefCell<Position>, tt_move: Move) -> MovePicker {
        MovePicker::new(pos, MovePickerMode::Normal, tt_move)
    }

    fn mvv_lva(&self, m: Move) -> u16 {
        let attacker = self.pos.borrow().role_at(m.from());
        let victim = self.pos.borrow().role_at(m.to());

        match (attacker, victim) {
            (None, _) => 0,
            (_, None) => 0,
            (Some(attacker), Some(victim)) => CAPTURE_SCORE + MVV_LVA[attacker][victim],
        }
    }

    fn score(&self) {
        for move_score in self.scored_moves.borrow_mut().iter_mut() {
            move_score.score = self.mvv_lva(move_score.m);
        }
    }

    fn select_sorted(&mut self) -> Option<Move> {
        let mut best_score = 0;
        let mut best_index = 0;

        for (i, move_score) in self
            .scored_moves
            .borrow()
            .iter()
            .skip(self.scored_index)
            .enumerate()
        {
            if move_score.score > best_score {
                best_score = move_score.score;
                best_index = i;
            }
        }

        if best_score == 0 {
            return None;
        }

        // swap
        self.scored_moves
            .borrow_mut()
            .swap(self.scored_index, best_index);
        self.scored_index += 1;

        return Some(self.scored_moves.borrow()[self.scored_index - 1].m);
    }
}

impl Iterator for MovePicker {
    type Item = Move;

    // TODO: filter out tt move after making it
    fn next(&mut self) -> Option<Self::Item> {
        match self.stage {
            MovePickerStage::TT => {
                self.stage = MovePickerStage::ScoreCaptures;
                if self.tt_move != Move::NULL {
                    return Some(self.tt_move);
                }
                self.next()
            }
            MovePickerStage::ScoreCaptures => {
                self.stage = MovePickerStage::Captures;
                self.scored_moves.borrow_mut().clear();

                self.move_generator.set_mask(self.occ);

                for m in self.move_generator.by_ref() {
                    self.scored_moves
                        .borrow_mut()
                        .push(MoveWithScore { m, score: 0 });
                    self.scored_len += 1;
                }

                self.score();
                self.next()
            }
            MovePickerStage::Captures => {
                // Don't need to filter this to enemies, right?
                match self.select_sorted() {
                    Some(m) => Some(m),
                    None => {
                        if self.mode == MovePickerMode::Quiescence {
                            return None;
                        }
                        self.stage = MovePickerStage::Quiets;
                        self.move_generator.set_mask(Bitboard::FULL);
                        self.next()
                    }
                }
            }
            MovePickerStage::Quiets => self.move_generator.next(),
        }
    }
}
