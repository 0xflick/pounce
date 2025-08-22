mod see;

use arrayvec::ArrayVec;

use crate::chess::movegen::MoveGen;
use crate::chess::{Color, Move, Position, Role, Square};
use crate::engine::search::{MAX_PLY, Search};

const TT_MOVE_SCORE: i16 = 30_000;
const GOOD_TACTICAL_SCORE: i16 = 22_000;
const QUEEN_PROMO_BONUS: i16 = 21_002;
const KILLER_1_SCORE: i16 = 21_001;
const KILLER_2_SCORE: i16 = 21_000;
const BAD_TACTICAL_SCORE: i16 = 17_000;

pub const MAX_MOVES: usize = 256;

const MVV_LVA: [[i16; 6]; 6] = [
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
    Normal,
    Quiescence,
}

pub struct MovePicker {
    move_generator: MoveGen,
    stage: MovePickerStage,
    mode: MovePickerMode,
    tt_move: Move,
    killers: [Move; 2],
    margin: i32,

    scored_moves: MoveList,
    scored_index: usize,
    sorted_index: usize,
}

impl MovePicker {
    pub fn new(
        pos: &Position,
        mode: MovePickerMode,
        tt_move: Move,
        killers: [Move; 2],
        margin: i32,
    ) -> MovePicker {
        let mg = MoveGen::new(pos);
        MovePicker {
            move_generator: mg,
            stage: MovePickerStage::TT,
            mode,
            tt_move,
            killers,
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
            MovePickerMode::Normal
        } else {
            MovePickerMode::Quiescence
        };

        MovePicker::new(pos, mode, tt_move, [Move::NONE; 2], margin)
    }

    pub fn new_ab_search(pos: &Position, tt_move: Move, killers: [Move; 2]) -> MovePicker {
        MovePicker::new(pos, MovePickerMode::Normal, tt_move, killers, 1)
    }

    fn mvv_lva(&self, m: Move, position: &Position) -> i16 {
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
                    TT_MOVE_SCORE as i32
                } else if self.scored_moves[i].m.is_promotion() {
                    match self.scored_moves[i].m.promotion() {
                        Some(Role::Queen) => {
                            if see::see(position, self.scored_moves[i].m, self.margin) {
                                QUEEN_PROMO_BONUS as i32 + GOOD_TACTICAL_SCORE as i32
                            } else {
                                QUEEN_PROMO_BONUS as i32 + BAD_TACTICAL_SCORE as i32
                            }
                        }
                        _ => BAD_TACTICAL_SCORE as i32,
                    }
                } else if see::see(position, self.scored_moves[i].m, self.margin) {
                    self.mvv_lva(self.scored_moves[i].m, position) as i32
                        + GOOD_TACTICAL_SCORE as i32
                } else {
                    self.mvv_lva(self.scored_moves[i].m, position) as i32
                        + BAD_TACTICAL_SCORE as i32
                }
            }
        }
        self.scored_index = self.scored_moves.len();
    }

    fn score_quiets(
        &mut self,
        position: &Position,
        history: &[[[i16; Square::NUM]; Square::NUM]; Color::NUM],
        continuation: &[[[[[i16; Square::NUM]; Role::NUM]; Square::NUM]; Role::NUM]; Color::NUM],
        ply: u8,
        current_move: &[(Move, Option<Role>); MAX_PLY as usize],
    ) {
        for i in self.scored_index..self.scored_moves.len() {
            let m = self.scored_moves[i].m;
            if m == self.killers[0] {
                self.scored_moves[i].score = KILLER_1_SCORE as i32;
            } else if m == self.killers[1] {
                self.scored_moves[i].score = KILLER_2_SCORE as i32;
            } else {
                let history_bonus = history[position.side][m.from()][m.to()] as i32;
                let counter_move_bonus = if ply == 0 || ply == MAX_PLY {
                    0
                } else {
                    match current_move[ply as usize - 1] {
                        (prev_mv, Some(role)) if prev_mv != Move::NULL && prev_mv != Move::NONE => {
                            let current_role = position.role_at(m.from()).unwrap();
                            continuation[position.side][role][prev_mv.to()][current_role][m.to()]
                                as i32
                        }
                        _ => 0,
                    }
                };

                self.scored_moves[i].score = (history_bonus / 2) + (counter_move_bonus / 2);
            };
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

    pub fn next(&mut self, search: &Search, ply: u8) -> Option<Move> {
        match self.stage {
            MovePickerStage::TT => {
                self.stage = MovePickerStage::ScoreTacticals;
                if self.tt_move != Move::NONE {
                    return Some(self.tt_move);
                }
                self.next(search, ply)
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
                self.next(search, ply)
            }
            MovePickerStage::Tacticals => {
                // Don't need to filter this to enemies, right?
                match self.select_sorted_min(GOOD_TACTICAL_SCORE as i32) {
                    Some(m) => {
                        if m == self.tt_move {
                            return self.next(search, ply);
                        }
                        Some(m)
                    }
                    None => {
                        if self.mode == MovePickerMode::Quiescence {
                            return None;
                        }
                        self.stage = MovePickerStage::ScoreQuiets;
                        self.next(search, ply)
                    }
                }
            }
            MovePickerStage::ScoreQuiets => {
                self.stage = MovePickerStage::Quiets;
                self.move_generator.disable_tacticals_only();

                for m in self.move_generator.by_ref() {
                    self.scored_moves.push(MoveWithScore { m, score: 0 });
                }

                self.score_quiets(
                    &search.position,
                    &search.history,
                    &search.continuation,
                    ply,
                    &search.current_move,
                );
                self.next(search, ply)
            }
            MovePickerStage::Quiets => match self.select_sorted() {
                Some(m) => {
                    if m == self.tt_move {
                        return self.next(search, ply);
                    }
                    Some(m)
                }
                None => None,
            },
        }
    }
}
