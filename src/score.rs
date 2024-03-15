use crate::board::{Board, Piece};

const PAWN_SCORE: i32 = 100;
const KNIGHT_SCORE: i32 = 320;
const BISHOP_SCORE: i32 = 330;
const ROOK_SCORE: i32 = 500;
const QUEEN_SCORE: i32 = 900;

pub fn score(board: &Board) -> i32 {
    let mut score = 0;
    for row in board.board.iter() {
        for cell in row.iter() {
            match cell.kind() {
                Piece::PAWN => {
                    score += PAWN_SCORE * if cell.side() == Piece::WHITE { 1 } else { -1 };
                }
                Piece::KNIGHT => {
                    score += KNIGHT_SCORE * if cell.side() == Piece::WHITE { 1 } else { -1 };
                }
                Piece::BISHOP => {
                    score += BISHOP_SCORE * if cell.side() == Piece::WHITE { 1 } else { -1 };
                }
                Piece::ROOK => {
                    score += ROOK_SCORE * if cell.side() == Piece::WHITE { 1 } else { -1 };
                }
                Piece::QUEEN => {
                    score += QUEEN_SCORE * if cell.side() == Piece::WHITE { 1 } else { -1 };
                }
                _ => {}
            }
        }
    }

    if board.is_white_turn {
        score
    } else {
        -score
    }
}
