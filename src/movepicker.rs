use std::cell::RefCell;

use arrayvec::ArrayVec;

use crate::{bitboard::Bitboard, movegen::MoveGen, moves::Move, position::Position};

const CAPTURE_SCORE: u16 = 1000;
const KILLER_1_SCORE: u16 = 900;
const KILLER_2_SCORE: u16 = 800;

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
    score: i32,
}

type MoveList = ArrayVec<MoveWithScore, MAX_MOVES>;

// TODO: killers, history, etc.
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
    occ: Bitboard,
    move_generator: MoveGen,
    stage: MovePickerStage,
    mode: MovePickerMode,
    tt_move: Move,
    killers: [Move; 2],

    scored_moves: MoveList,
    scored_index: usize,

    pos: RefCell<Position>,
}

impl MovePicker {
    pub fn new(
        pos: RefCell<Position>,
        mode: MovePickerMode,
        tt_move: Move,
        killers: [Move; 2],
    ) -> MovePicker {
        let occ = pos.borrow().occupancy;
        let mg = MoveGen::new(&pos.borrow());
        MovePicker {
            occ,
            move_generator: mg,
            stage: MovePickerStage::TT,
            mode,
            tt_move,
            killers,
            scored_moves: ArrayVec::new(),
            scored_index: 0,
            pos,
        }
    }

    pub fn new_quiescence(pos: RefCell<Position>) -> MovePicker {
        MovePicker::new(pos, MovePickerMode::Quiescence, Move::NULL, [Move::NULL; 2])
    }

    pub fn new_ab_search(pos: RefCell<Position>, tt_move: Move, killers: [Move; 2]) -> MovePicker {
        MovePicker::new(pos, MovePickerMode::Normal, tt_move, killers)
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

    fn score_captures(&mut self) {
        for i in 0..self.scored_moves.len() {
            self.scored_moves[i].score = self.mvv_lva(self.scored_moves[i].m) as i32;
        }
    }

    fn score_quiets(&mut self) {
        for i in 0..self.scored_moves.len() {
            let m = self.scored_moves[i].m;
            if m == self.killers[0] {
                self.scored_moves[i].score = KILLER_1_SCORE as i32;
            } else if m == self.killers[1] {
                self.scored_moves[i].score = KILLER_2_SCORE as i32;
            } else {
                self.scored_moves[i].score = 0;
            }
        }
    }

    fn select_sorted(&mut self) -> Option<Move> {
        let mut best_score = -1;
        let mut best_index = 0;

        for i in self.scored_index..self.scored_moves.len() {
            let move_score = &self.scored_moves[i];
            if move_score.score > best_score {
                best_score = move_score.score;
                best_index = i;
            }
        }

        if best_score == -1 {
            return None;
        }

        // swap
        self.scored_moves.swap(self.scored_index, best_index);
        self.scored_index += 1;

        Some(self.scored_moves[self.scored_index - 1].m)
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
                self.scored_moves.clear();

                self.move_generator.set_mask(self.occ);

                for m in self.move_generator.by_ref() {
                    self.scored_moves.push(MoveWithScore { m, score: 0 });
                }

                self.score_captures();
                self.next()
            }
            MovePickerStage::Captures => {
                // Don't need to filter this to enemies, right?
                match self.select_sorted() {
                    Some(m) => {
                        if m == self.tt_move {
                            return self.next();
                        }
                        Some(m)
                    }
                    None => {
                        if self.mode == MovePickerMode::Quiescence {
                            return None;
                        }
                        self.stage = MovePickerStage::ScoreQuiets;
                        self.next()
                    }
                }
            }
            MovePickerStage::ScoreQuiets => {
                self.stage = MovePickerStage::Quiets;
                self.scored_moves.clear();
                self.scored_index = 0;
                self.move_generator.set_mask(Bitboard::FULL);

                for m in self.move_generator.by_ref() {
                    self.scored_moves.push(MoveWithScore { m, score: 0 });
                }

                self.score_quiets();
                self.next()
            }
            MovePickerStage::Quiets => match self.select_sorted() {
                Some(m) => {
                    if m == self.tt_move {
                        return self.next();
                    }
                    Some(m)
                }
                None => None,
            },
        }
    }
}

#[cfg(test)]
mod test {
    use std::cell::RefCell;

    use crate::{fen::Fen, movegen::init_tables, zobrist::init_zobrist};

    #[test]
    fn move_order() {
        init_tables();
        init_zobrist();

        let Fen(pos) = "rnb1kbnr/pppp1ppp/8/3qp3/2PQ4/8/PPP1PPPP/RNB1KBNR w KQkq - 0 1"
            .parse()
            .unwrap();

        let mp = super::MovePicker::new_ab_search(
            RefCell::new(pos),
            "d4e5".parse().unwrap(),
            ["c1e3".parse().unwrap(), "g1f3".parse().unwrap()],
        );

        let moves: Vec<_> = mp.collect();

        assert_eq!(moves.len(), 41);
        // queen takes pawn (tt move)
        assert_eq!(moves[0], "d4e5".parse().unwrap());

        // pawn takes queen
        assert_eq!(moves[1], "c4d5".parse().unwrap());
        // queen takes queen
        assert_eq!(moves[2], "d4d5".parse().unwrap());
        // queen takes pawn
        assert_eq!(moves[3], "d4a7".parse().unwrap());

        // killer 1
        assert_eq!(moves[4], "c1e3".parse().unwrap());
        // killer 2
        assert_eq!(moves[5], "g1f3".parse().unwrap());
    }
}
