mod see;

use arrayvec::ArrayVec;

use crate::chess::movegen::MoveGen;
use crate::chess::{Color, Move, Position, Square};

const TT_MOVE_SCORE: i16 = 30_000;
const GOOD_CAPTURE_SCORE: i16 = 29_000;
const KILLER_1_SCORE: i16 = 28_001;
const KILLER_2_SCORE: i16 = 28_000;
const BAD_CAPTURE_SCORE: i16 = 27_000;

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
    ScoreCaptures,
    Captures,
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
        // If the tt move isn't a capture, we can't use it in quiescence search
        if tt_move != Move::NONE && !tt_move.is_capture(pos) {
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

    fn score_captures(&mut self, position: &Position) {
        for i in 0..self.scored_moves.len() {
            self.scored_moves[i].score = {
                if self.scored_moves[i].m == self.tt_move {
                    TT_MOVE_SCORE as i32
                } else if see::see(position, self.scored_moves[i].m, self.margin) {
                    self.mvv_lva(self.scored_moves[i].m, position) as i32
                        + GOOD_CAPTURE_SCORE as i32
                } else {
                    self.mvv_lva(self.scored_moves[i].m, position) as i32 + BAD_CAPTURE_SCORE as i32
                }
            }
        }
        self.scored_index = self.scored_moves.len();
    }

    fn score_quiets(
        &mut self,
        position: &Position,
        history: &[[[i16; Square::NUM]; Square::NUM]; Color::NUM],
    ) {
        for i in self.scored_index..self.scored_moves.len() {
            let m = self.scored_moves[i].m;
            if m == self.killers[0] {
                self.scored_moves[i].score = KILLER_1_SCORE as i32;
            } else if m == self.killers[1] {
                self.scored_moves[i].score = KILLER_2_SCORE as i32;
            } else {
                self.scored_moves[i].score = history[position.side][m.from()][m.to()] as i32;
            }
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

    pub fn next(
        &mut self,
        position: &Position,
        history: &[[[i16; Square::NUM]; Square::NUM]; Color::NUM],
    ) -> Option<Move> {
        match self.stage {
            MovePickerStage::TT => {
                self.stage = MovePickerStage::ScoreCaptures;
                if self.tt_move != Move::NONE {
                    return Some(self.tt_move);
                }
                self.next(position, history)
            }
            MovePickerStage::ScoreCaptures => {
                self.stage = MovePickerStage::Captures;
                self.scored_moves.clear();

                self.move_generator.set_captures_only(position.occupancy);

                for m in self.move_generator.by_ref() {
                    self.scored_moves.push(MoveWithScore { m, score: 0 });
                }

                self.score_captures(position);
                self.next(position, history)
            }
            MovePickerStage::Captures => {
                // Don't need to filter this to enemies, right?
                match self.select_sorted_min(GOOD_CAPTURE_SCORE as i32) {
                    Some(m) => {
                        if m == self.tt_move {
                            return self.next(position, history);
                        }
                        Some(m)
                    }
                    None => {
                        if self.mode == MovePickerMode::Quiescence {
                            return None;
                        }
                        self.stage = MovePickerStage::ScoreQuiets;
                        self.next(position, history)
                    }
                }
            }
            MovePickerStage::ScoreQuiets => {
                self.stage = MovePickerStage::Quiets;
                self.move_generator.disable_captures_only();

                for m in self.move_generator.by_ref() {
                    self.scored_moves.push(MoveWithScore { m, score: 0 });
                }

                self.score_quiets(position, history);
                self.next(position, history)
            }
            MovePickerStage::Quiets => match self.select_sorted() {
                Some(m) => {
                    if m == self.tt_move {
                        return self.next(position, history);
                    }
                    Some(m)
                }
                None => None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::chess::movegen::init_tables;
    use crate::chess::position::fen::Fen;
    use crate::init;
    use crate::zobrist::init_zobrist;

    #[test]
    fn move_order() {
        init_tables();
        init_zobrist();

        let Fen(pos) = "rnb1kbnr/pppp1ppp/8/3qp3/2PQ4/8/PPP1PPPP/RNB1KBNR w KQkq - 0 1"
            .parse()
            .unwrap();

        let mut mp = super::MovePicker::new_ab_search(
            &pos,
            "d4e5".parse().unwrap(),
            ["c1e3".parse().unwrap(), "g1f3".parse().unwrap()],
        );

        let mut moves = Vec::new();

        while let Some(m) = mp.next(&pos, &[[[0; 64]; 64]; 2]) {
            moves.push(m);
        }

        assert_eq!(moves.len(), 41);
        // queen takes pawn (tt move)
        assert_eq!(moves[0], "d4e5".parse().unwrap());

        // pawn takes queen
        assert_eq!(moves[1], "c4d5".parse().unwrap());
        // queen takes queen
        assert_eq!(moves[2], "d4d5".parse().unwrap());

        // killer 1
        assert_eq!(moves[3], "c1e3".parse().unwrap());
        // killer 2
        assert_eq!(moves[4], "g1f3".parse().unwrap());

        // queen takes pawn and cand be recaptured
        assert_eq!(moves[5], "d4a7".parse().unwrap());
    }

    #[test]
    fn quiescence() {
        init();

        let Fen(pos) = "2b1kbnr/5ppp/4p3/q1pP1Q1r/6P1/1NP5/PP2PP1P/R1B1KBNR w - c6 0 1"
            .parse()
            .unwrap();

        let mut mp = super::MovePicker::new_quiescence(&pos, "e2e3".parse().unwrap(), 15);

        let mut moves = Vec::new();
        while let Some(m) = mp.next(&pos, &[[[0; 64]; 64]; 2]) {
            moves.push(m);
        }

        // Should get 4 good captures including en passant
        // tt move is not a capture, so it should be skipped
        assert_eq!(moves.len(), 4);

        // Check that all expected moves are present
        assert_eq!(moves[0], "b3a5".parse().unwrap()); // knight takes queen
        assert_eq!(moves[1], "g4h5".parse().unwrap()); // pawn takes rook
        assert_eq!(moves[2], "f5h5".parse().unwrap()); // queen takes rook
        assert_eq!(moves[3], "d5c6".parse().unwrap()); // en passant capture
    }
}
