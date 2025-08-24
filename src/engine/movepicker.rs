mod see;

use arrayvec::ArrayVec;

use crate::chess::movegen::MoveGen;
use crate::chess::{Move, Position, Role};
use crate::engine::history::HistoryTables;
use crate::engine::search::{Search, Stack};

const TT_MOVE_SCORE: i32 = 30_000;
const GOOD_TACTICAL_SCORE: i32 = 22_000;
const QUEEN_PROMO_BONUS: i32 = 21_002;
pub const KILLER_1_SCORE: i32 = 21_001;
pub const KILLER_2_SCORE: i32 = 21_000;
const BAD_TACTICAL_SCORE: i32 = 17_000;

pub const MAX_MOVES: usize = 256;

const MVV_LVA: [[i32; 6]; 6] = [
    [15, 25, 35, 45, 55, 0], // attacker pawn, victim P, N, B, R, Q,  K
    [14, 24, 34, 44, 54, 0], // attacker knight, victim P, N, B, R, Q,  K
    [13, 23, 33, 43, 53, 0], // attacker bishop, victim P, N, B, R, Q,  K
    [12, 22, 32, 42, 52, 0], // attacker rook, victim P, N, B, R, Q,  K
    [11, 21, 31, 41, 51, 0], // attacker queen, victim P, N, B, R, Q,  K
    [10, 20, 30, 40, 50, 0], // attacker king, victim P, N, B, R, Q,  K
];

#[derive(Debug)]
struct MoveWithScore {
    m: Move,
    score: i32,
}

type MoveList = ArrayVec<MoveWithScore, MAX_MOVES>;

enum MovePickerStage {
    TT,
    ScoreTacticals,
    Tacticals,
    ScoreQuiets,
    Quiets,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MovePickerMode {
    Normal { ply: usize },
    Quiescence,
    QuiescenceCheck,
}

pub struct MovePicker {
    move_generator: MoveGen,
    stage: MovePickerStage,
    mode: MovePickerMode,
    tt_move: Move,
    margin: i32,

    scored_moves: MoveList,
    scored_index: usize,
    sorted_index: usize,
}

impl MovePicker {
    pub fn new(pos: &Position, mode: MovePickerMode, tt_move: Move, margin: i32) -> MovePicker {
        let mg = MoveGen::new(pos);
        MovePicker {
            move_generator: mg,
            stage: MovePickerStage::TT,
            mode,
            tt_move,
            margin,
            scored_moves: ArrayVec::new(),
            scored_index: 0,
            sorted_index: 0,
        }
    }

    pub fn new_quiescence(pos: &Position, mut tt_move: Move, margin: i32) -> MovePicker {
        // If the tt move isn't a capture or promotion, we can't use it in quiescence search
        if tt_move != Move::NONE && !(tt_move.is_capture(pos) || tt_move.is_promotion()) {
            tt_move = Move::NONE;
        }

        let mode = if pos.in_check() {
            // if in check, we need to search all moves
            MovePickerMode::QuiescenceCheck
        } else {
            MovePickerMode::Quiescence
        };

        MovePicker::new(pos, mode, tt_move, margin)
    }

    pub fn new_ab_search(pos: &Position, ply: usize, tt_move: Move) -> MovePicker {
        MovePicker::new(pos, MovePickerMode::Normal { ply }, tt_move, 1)
    }

    fn mvv_lva(&self, m: Move, position: &Position) -> i32 {
        let attacker = position.role_at(m.from());
        let victim = m.captured_role(position);

        match (attacker, victim) {
            (None, _) => 0,
            (_, None) => 0,
            (Some(attacker), Some(victim)) => MVV_LVA[attacker][victim],
        }
    }

    fn score_tacticals(&mut self, position: &Position) {
        for i in 0..self.scored_moves.len() {
            self.scored_moves[i].score = {
                if self.scored_moves[i].m == self.tt_move {
                    TT_MOVE_SCORE
                } else if self.scored_moves[i].m.is_promotion() {
                    match self.scored_moves[i].m.promotion() {
                        Some(Role::Queen) => {
                            if see::see(position, self.scored_moves[i].m, self.margin) {
                                QUEEN_PROMO_BONUS + GOOD_TACTICAL_SCORE
                            } else {
                                QUEEN_PROMO_BONUS + BAD_TACTICAL_SCORE
                            }
                        }
                        _ => BAD_TACTICAL_SCORE,
                    }
                } else if see::see(position, self.scored_moves[i].m, self.margin) {
                    self.mvv_lva(self.scored_moves[i].m, position) + GOOD_TACTICAL_SCORE
                } else {
                    self.mvv_lva(self.scored_moves[i].m, position) + BAD_TACTICAL_SCORE
                }
            }
        }
        self.scored_index = self.scored_moves.len();
    }

    fn score_quiets(
        &mut self,
        history: &HistoryTables,
        position: &Position,
        stack: &Stack,
        ply: usize,
    ) {
        for i in self.scored_index..self.scored_moves.len() {
            let m = self.scored_moves[i].m;
            self.scored_moves[i].score = history.score(position, stack, ply, m);
        }
        self.scored_index = self.scored_moves.len();
    }

    #[inline]
    fn select_sorted(&mut self) -> Option<Move> {
        self.select_sorted_min(i32::MIN)
    }

    #[inline]
    fn select_sorted_min(&mut self, min_score: i32) -> Option<Move> {
        let mut best_score = i32::MIN;
        let mut best_index = 0;

        for i in self.sorted_index..self.scored_moves.len() {
            let move_score = &self.scored_moves[i];
            if move_score.score > best_score {
                best_score = move_score.score;
                best_index = i;
            }
        }

        if best_score <= min_score {
            return None;
        }

        // swap
        self.scored_moves.swap(self.sorted_index, best_index);
        self.sorted_index += 1;

        Some(self.scored_moves[self.sorted_index - 1].m)
    }

    pub fn next(&mut self, search: &Search) -> Option<Move> {
        match self.stage {
            MovePickerStage::TT => {
                self.stage = MovePickerStage::ScoreTacticals;
                if self.tt_move != Move::NONE {
                    return Some(self.tt_move);
                }
                self.next(search)
            }
            MovePickerStage::ScoreTacticals => {
                self.stage = MovePickerStage::Tacticals;
                self.scored_moves.clear();

                self.move_generator
                    .set_tacticals_only(search.position.occupancy);

                for m in self.move_generator.by_ref() {
                    self.scored_moves.push(MoveWithScore { m, score: 0 });
                }

                self.score_tacticals(&search.position);
                self.next(search)
            }
            MovePickerStage::Tacticals => {
                // Don't need to filter this to enemies, right?
                match self.select_sorted_min(GOOD_TACTICAL_SCORE) {
                    Some(m) => {
                        if m == self.tt_move {
                            return self.next(search);
                        }
                        Some(m)
                    }
                    None => {
                        // If we're in quiescence search, end now
                        if self.mode == MovePickerMode::Quiescence {
                            return None;
                        }
                        self.stage = MovePickerStage::ScoreQuiets;
                        self.next(search)
                    }
                }
            }
            MovePickerStage::ScoreQuiets => {
                self.stage = MovePickerStage::Quiets;
                self.move_generator.disable_tacticals_only();

                for m in self.move_generator.by_ref() {
                    self.scored_moves.push(MoveWithScore { m, score: 0 });
                }

                // We only need to score if we're in normal mode
                if let MovePickerMode::Normal { ply } = self.mode {
                    self.score_quiets(&search.history, &search.position, &search.stack, ply);
                }

                self.next(search)
            }
            MovePickerStage::Quiets => match self.select_sorted() {
                Some(m) => {
                    if m == self.tt_move {
                        return self.next(search);
                    }
                    Some(m)
                }
                None => None,
            },
        }
    }
}
