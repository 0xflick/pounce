use crate::board::{Board, Piece};

const PAWN_SCORE: i32 = 100;
const KNIGHT_SCORE: i32 = 320;
const BISHOP_SCORE: i32 = 330;
const ROOK_SCORE: i32 = 500;
const QUEEN_SCORE: i32 = 900;

const PAWN_TABLE: [[i32; 8]; 8] = [
    [0, 0, 0, 0, 0, 0, 0, 0],
    [50, 50, 50, 50, 50, 50, 50, 50],
    [10, 10, 20, 30, 30, 20, 10, 10],
    [5, 5, 10, 25, 25, 10, 5, 5],
    [0, 0, 0, 20, 20, 0, 0, 0],
    [5, -5, -10, 0, 0, -10, -5, 5],
    [5, 10, 10, -20, -20, 10, 10, 5],
    [0, 0, 0, 0, 0, 0, 0, 0],
];

const PAWN_TABLE_ENDGAME: [[i32; 8]; 8] = [
    [0, 0, 0, 0, 0, 0, 0, 0],
    [80, 80, 80, 80, 80, 80, 80, 80],
    [50, 50, 50, 50, 50, 50, 50, 50],
    [30, 30, 30, 30, 30, 30, 30, 30],
    [20, 20, 20, 20, 20, 20, 20, 20],
    [10, 10, 10, 10, 10, 10, 10, 10],
    [10, 10, 10, 10, 10, 10, 10, 10],
    [0, 0, 0, 0, 0, 0, 0, 0],
];

const KNIGHT_TABLE: [[i32; 8]; 8] = [
    [-50, -40, -30, -30, -30, -30, -40, -50],
    [-40, -20, 0, 0, 0, 0, -20, -40],
    [-30, 0, 10, 15, 15, 10, 0, -30],
    [-30, 5, 15, 20, 20, 15, 5, -30],
    [-30, 0, 15, 20, 20, 15, 0, -30],
    [-30, 5, 10, 15, 15, 10, 5, -30],
    [-40, -20, 0, 5, 5, 0, -20, -40],
    [-50, -40, -30, -30, -30, -30, -40, -50],
];

const BISHOP_TABLE: [[i32; 8]; 8] = [
    [-20, -10, -10, -10, -10, -10, -10, -20],
    [-10, 0, 0, 0, 0, 0, 0, -10],
    [-10, 0, 5, 10, 10, 5, 0, -10],
    [-10, 5, 5, 10, 10, 5, 5, -10],
    [-10, 0, 10, 10, 10, 10, 0, -10],
    [-10, 10, 10, 10, 10, 10, 10, -10],
    [-10, 5, 0, 0, 0, 0, 5, -10],
    [-20, -10, -10, -10, -10, -10, -10, -20],
];

const ROOK_TABLE: [[i32; 8]; 8] = [
    [0, 0, 0, 0, 0, 0, 0, 0],
    [5, 10, 10, 10, 10, 10, 10, 5],
    [-5, 0, 0, 0, 0, 0, 0, -5],
    [-5, 0, 0, 0, 0, 0, 0, -5],
    [-5, 0, 0, 0, 0, 0, 0, -5],
    [-5, 0, 0, 0, 0, 0, 0, -5],
    [-5, 0, 0, 0, 0, 0, 0, -5],
    [0, 0, 0, 5, 5, 0, 0, 0],
];
const QUEEN_TABLE: [[i32; 8]; 8] = [
    [-20, -10, -10, -5, -5, -10, -10, -20],
    [-10, 0, 0, 0, 0, 0, 0, -10],
    [-10, 0, 5, 5, 5, 5, 0, -10],
    [-5, 0, 5, 5, 5, 5, 0, -5],
    [0, 0, 5, 5, 5, 5, 0, -5],
    [-10, 5, 5, 5, 5, 5, 0, -10],
    [-10, 0, 5, 0, 0, 0, 0, -10],
    [-20, -10, -10, -5, -5, -10, -10, -20],
];

const KING_TABLE: [[i32; 8]; 8] = [
    [-30, -40, -40, -50, -50, -40, -40, -30],
    [-30, -40, -40, -50, -50, -40, -40, -30],
    [-30, -40, -40, -50, -50, -40, -40, -30],
    [-30, -40, -40, -50, -50, -40, -40, -30],
    [-20, -30, -30, -40, -40, -30, -30, -20],
    [-10, -20, -20, -20, -20, -20, -20, -10],
    [20, 20, 0, 0, 0, 0, 20, 20],
    [20, 30, 10, 0, 0, 10, 30, 20],
];

const KING_TABLE_ENDGAME: [[i32; 8]; 8] = [
    [-50, -40, -30, -20, -20, -30, -40, -50],
    [-30, -20, -10, 0, 0, -10, -20, -30],
    [-30, -10, 20, 30, 30, 20, -10, -30],
    [-30, -10, 30, 40, 40, 30, -10, -30],
    [-30, -10, 30, 40, 40, 30, -10, -30],
    [-30, -10, 20, 30, 30, 20, -10, -30],
    [-30, -30, 0, 0, 0, 0, -30, -30],
    [-50, -30, -30, -30, -30, -30, -30, -50],
];

pub const MATE: i32 = 1_000_000;

const ENDGAME_MATERIAL: i32 = 2 * KNIGHT_SCORE + 2 * BISHOP_SCORE + 2 * ROOK_SCORE + QUEEN_SCORE;

fn interpolate(t1: [[i32; 8]; 8], t2: [[i32; 8]; 8], f: f32) -> [[i32; 8]; 8] {
    let mut result = [[0; 8]; 8];
    for i in 0..8 {
        for j in 0..8 {
            result[i][j] = (t1[i][j] as f32 * (1.0 - f) + t2[i][j] as f32 * f) as i32;
        }
    }
    result
}

pub fn score(board: &Board) -> i32 {
    let mut score = 0;
    let mut white_endgame_score = 0;
    let mut black_endgame_score = 0;

    for row in board.board.iter() {
        for cell in row.iter() {
            match cell.kind() {
                Piece::PAWN => {
                    score += PAWN_SCORE * if cell.side() == Piece::WHITE { 1 } else { -1 };
                }
                Piece::KNIGHT => {
                    score += KNIGHT_SCORE * if cell.side() == Piece::WHITE { 1 } else { -1 };
                    match cell.side() {
                        Piece::WHITE => {
                            white_endgame_score += KNIGHT_SCORE;
                        }
                        Piece::BLACK => {
                            black_endgame_score += KNIGHT_SCORE;
                        }
                        _ => {}
                    }
                }
                Piece::BISHOP => {
                    score += BISHOP_SCORE * if cell.side() == Piece::WHITE { 1 } else { -1 };
                    match cell.side() {
                        Piece::WHITE => {
                            white_endgame_score += BISHOP_SCORE;
                        }
                        Piece::BLACK => {
                            black_endgame_score += BISHOP_SCORE;
                        }
                        _ => {}
                    }
                }
                Piece::ROOK => {
                    score += ROOK_SCORE * if cell.side() == Piece::WHITE { 1 } else { -1 };
                    match cell.side() {
                        Piece::WHITE => {
                            white_endgame_score += ROOK_SCORE;
                        }
                        Piece::BLACK => {
                            black_endgame_score += ROOK_SCORE;
                        }
                        _ => {}
                    }
                }
                Piece::QUEEN => {
                    score += QUEEN_SCORE * if cell.side() == Piece::WHITE { 1 } else { -1 };
                    match cell.side() {
                        Piece::WHITE => {
                            white_endgame_score += QUEEN_SCORE;
                        }
                        Piece::BLACK => {
                            black_endgame_score += QUEEN_SCORE;
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    let white_endgame_weight =
        1. - f32::min(1., white_endgame_score as f32 / ENDGAME_MATERIAL as f32);

    let black_endgame_weight =
        1. - f32::min(1., black_endgame_score as f32 / ENDGAME_MATERIAL as f32);

    for (r_idx, row) in board.board.iter().enumerate() {
        for (c_idx, cell) in row.iter().enumerate() {
            let r_corr = if cell.side() == Piece::WHITE {
                r_idx
            } else {
                7 - r_idx
            };
            match cell.kind() {
                Piece::PAWN => match cell.side() {
                    Piece::WHITE => {
                        score += interpolate(PAWN_TABLE, PAWN_TABLE_ENDGAME, white_endgame_weight)
                            [r_corr][c_idx]
                            * if cell.side() == Piece::WHITE { 1 } else { -1 }
                    }
                    Piece::BLACK => {
                        score += interpolate(PAWN_TABLE, PAWN_TABLE_ENDGAME, black_endgame_weight)
                            [r_corr][c_idx]
                            * if cell.side() == Piece::WHITE { 1 } else { -1 }
                    }
                    _ => {}
                },
                Piece::KNIGHT => {
                    score += KNIGHT_TABLE[r_corr][c_idx]
                        * if cell.side() == Piece::WHITE { 1 } else { -1 };
                }
                Piece::BISHOP => {
                    score += BISHOP_TABLE[r_corr][c_idx]
                        * if cell.side() == Piece::WHITE { 1 } else { -1 };
                }
                Piece::ROOK => {
                    score += ROOK_TABLE[r_corr][c_idx]
                        * if cell.side() == Piece::WHITE { 1 } else { -1 };
                }
                Piece::QUEEN => {
                    score += QUEEN_TABLE[r_corr][c_idx]
                        * if cell.side() == Piece::WHITE { 1 } else { -1 };
                }
                Piece::KING => match cell.side() {
                    Piece::WHITE => {
                        score += interpolate(KING_TABLE, KING_TABLE_ENDGAME, white_endgame_weight)
                            [r_corr][c_idx]
                            * if cell.side() == Piece::WHITE { 1 } else { -1 };
                    }
                    Piece::BLACK => {
                        score += interpolate(KING_TABLE, KING_TABLE_ENDGAME, black_endgame_weight)
                            [r_corr][c_idx]
                            * if cell.side() == Piece::WHITE { 1 } else { -1 };
                    }
                    _ => {}
                },
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
