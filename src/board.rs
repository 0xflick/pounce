use std::default;
use std::fmt::Display;
use std::ops::{Add, BitOr};
use std::str::FromStr;

use arrayvec::ArrayVec;

use crate::zobrist::ZOBRIST;

pub const MAX_MOVES: usize = 512;

pub type MoveList = ArrayVec<Move, MAX_MOVES>;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Ord, PartialOrd)]
pub struct Piece(u8);
impl Piece {
    pub const WHITE: u8 = 0b00000000;
    pub const BLACK: u8 = 0b10000000;

    pub const NULL: u8 = 0;
    pub const PAWN: u8 = 1;
    pub const ROOK: u8 = 2;
    pub const KNIGHT: u8 = 3;
    pub const BISHOP: u8 = 4;
    pub const QUEEN: u8 = 5;
    pub const KING: u8 = 6;

    pub const NULL_PIECE: Piece = Piece(Self::NULL);

    pub const WHITE_PAWN: Piece = Piece(Self::PAWN);
    pub const WHITE_ROOK: Piece = Piece(Self::ROOK);
    pub const WHITE_KNIGHT: Piece = Piece(Self::KNIGHT);
    pub const WHITE_BISHOP: Piece = Piece(Self::BISHOP);
    pub const WHITE_QUEEN: Piece = Piece(Self::QUEEN);
    pub const WHITE_KING: Piece = Piece(Self::KING);

    pub const BLACK_PAWN: Piece = Piece(Self::BLACK | Self::PAWN);
    pub const BLACK_ROOK: Piece = Piece(Self::BLACK | Self::ROOK);
    pub const BLACK_KNIGHT: Piece = Piece(Self::BLACK | Self::KNIGHT);
    pub const BLACK_BISHOP: Piece = Piece(Self::BLACK | Self::BISHOP);
    pub const BLACK_QUEEN: Piece = Piece(Self::BLACK | Self::QUEEN);
    pub const BLACK_KING: Piece = Piece(Self::BLACK | Self::KING);

    fn is_null(self) -> bool {
        self.0 == Self::NULL
    }

    pub fn kind(self) -> u8 {
        self.0 & !Self::BLACK
    }

    pub fn side(self) -> u8 {
        self.0 & Self::BLACK
    }

    pub fn zobrist_key(self) -> usize {
        match self {
            Piece::WHITE_PAWN => 0,
            Piece::WHITE_ROOK => 1,
            Piece::WHITE_KNIGHT => 2,
            Piece::WHITE_BISHOP => 3,
            Piece::WHITE_QUEEN => 4,
            Piece::WHITE_KING => 5,
            Piece::BLACK_PAWN => 6,
            Piece::BLACK_ROOK => 7,
            Piece::BLACK_KNIGHT => 8,
            Piece::BLACK_BISHOP => 9,
            Piece::BLACK_QUEEN => 10,
            Piece::BLACK_KING => 11,
            _ => 12,
        }
    }
}

impl Display for Piece {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Piece::WHITE_PAWN => write!(f, "♙")?,
            Piece::WHITE_ROOK => write!(f, "♖")?,
            Piece::WHITE_KNIGHT => write!(f, "♘")?,
            Piece::WHITE_BISHOP => write!(f, "♗")?,
            Piece::WHITE_QUEEN => write!(f, "♕")?,
            Piece::WHITE_KING => write!(f, "♔")?,
            Piece::BLACK_PAWN => write!(f, "♟︎")?,
            Piece::BLACK_ROOK => write!(f, "♜")?,
            Piece::BLACK_KNIGHT => write!(f, "♞")?,
            Piece::BLACK_BISHOP => write!(f, "♝")?,
            Piece::BLACK_QUEEN => write!(f, "♛")?,
            Piece::BLACK_KING => write!(f, "♚")?,
            _ => {}
        }
        Ok(())
    }
}

#[derive(Default, Debug, Clone)]
struct BoardState {
    castle_rights: Castle,
    en_passante_state: Option<Position>,
}

impl BoardState {
    fn zobrist_hash(&self) -> u64 {
        let mut hash = ZOBRIST.castling[self.castle_zobrist_key()];
        if let Some(ep) = self.ep_zobrist_key() {
            hash ^= ZOBRIST.en_passant[ep];
        }
        hash
    }

    fn castle_zobrist_key(&self) -> usize {
        self.castle_rights.zobrist_key()
    }
    fn ep_zobrist_key(&self) -> Option<usize> {
        self.en_passante_state.map(|p| p.col as usize)
    }
}

const MAX_HISTORY: usize = 255;
type History = Vec<BoardState>;

#[derive(Debug, Clone)]
pub struct Board {
    pub board: [[Piece; 8]; 8],
    white_king: Position,
    black_king: Position,
    pub is_white_turn: bool,

    history: History,

    pub z_hash: u64,
    z_hash_history: Vec<u64>,

    irreversible_move: Vec<u8>,
}

impl Board {
    pub fn new() -> Self {
        let mut history = Vec::with_capacity(MAX_HISTORY);
        history.push(BoardState {
            en_passante_state: None,
            castle_rights: Castle::ALL,
        });

        let board = [
            [
                Piece::WHITE_ROOK,
                Piece::WHITE_KNIGHT,
                Piece::WHITE_BISHOP,
                Piece::WHITE_QUEEN,
                Piece::WHITE_KING,
                Piece::WHITE_BISHOP,
                Piece::WHITE_KNIGHT,
                Piece::WHITE_ROOK,
            ],
            [Piece::WHITE_PAWN; 8],
            [Piece(Piece::NULL); 8],
            [Piece(Piece::NULL); 8],
            [Piece(Piece::NULL); 8],
            [Piece(Piece::NULL); 8],
            [Piece::BLACK_PAWN; 8],
            [
                Piece::BLACK_ROOK,
                Piece::BLACK_KNIGHT,
                Piece::BLACK_BISHOP,
                Piece::BLACK_QUEEN,
                Piece::BLACK_KING,
                Piece::BLACK_BISHOP,
                Piece::BLACK_KNIGHT,
                Piece::BLACK_ROOK,
            ],
        ];

        let mut b = Self {
            board,
            white_king: Position { row: 0, col: 4 },
            black_king: Position { row: 7, col: 4 },
            history,
            z_hash: 0,
            z_hash_history: Vec::with_capacity(MAX_HISTORY),
            is_white_turn: true,
            irreversible_move: vec![0],
        };
        b.z_hash = b.zobrist_hash();
        b
    }

    pub fn zobrist_hash(&self) -> u64 {
        let mut z_hash = 0;

        // Piece positions
        for (r_index, row) in self.board.iter().enumerate() {
            for (c_index, cell) in row.iter().enumerate() {
                if cell.is_null() {
                    continue;
                }
                z_hash ^= ZOBRIST.pieces[cell.zobrist_key()][r_index][c_index];
            }
        }

        let history = self.history.last().unwrap();

        z_hash ^= history.zobrist_hash();

        // Side to move
        if !self.is_white_turn {
            z_hash ^= ZOBRIST.black;
        }

        z_hash
    }

    pub fn make_move(&mut self, mv: &Move) {
        self.z_hash_history.push(self.z_hash);
        let mut next_state = if let Some(prev_state) = self.history.last() {
            // update zobrist hash for board state
            self.z_hash ^= prev_state.zobrist_hash();
            (*prev_state).clone()
        } else {
            BoardState::default()
        };

        if self.get(mv.start).kind() == Piece::PAWN || mv.capture.is_some() {
            self.irreversible_move.push(self.history.len() as u8);
        } else {
            self.irreversible_move
                .push(*self.irreversible_move.last().unwrap());
        }
        next_state.en_passante_state = None;

        // update zobrist hash piece positions
        if mv != &Move::NULL_MOVE {
            self.z_hash ^= ZOBRIST.pieces[self.get(mv.start).zobrist_key()][mv.start.row as usize]
                [mv.start.col as usize];
            self.z_hash ^= ZOBRIST.pieces[self.get(mv.start).zobrist_key()][mv.end.row as usize]
                [mv.end.col as usize];

            // update zobrist hash for captured piece
            if let Some(p) = mv.capture {
                self.z_hash ^=
                    ZOBRIST.pieces[p.zobrist_key()][mv.end.row as usize][mv.end.col as usize];
            }

            if mv.capture.is_some() {
                match (self.get(mv.end), mv.end) {
                    (Piece::WHITE_ROOK, Position { row: 0, col: 0 }) => {
                        next_state.castle_rights.remove(Castle::WHITE_QUEEN_SIDE)
                    }
                    (Piece::WHITE_ROOK, Position { row: 0, col: 7 }) => {
                        next_state.castle_rights.remove(Castle::WHITE_KING_SIDE)
                    }
                    (Piece::BLACK_ROOK, Position { row: 7, col: 0 }) => {
                        next_state.castle_rights.remove(Castle::BLACK_QUEEN_SIDE)
                    }
                    (Piece::BLACK_ROOK, Position { row: 7, col: 7 }) => {
                        next_state.castle_rights.remove(Castle::BLACK_KING_SIDE)
                    }
                    _ => {}
                }
            };
            // remove castling rights if we're moving a rook or KING
            if self.history.last().unwrap().castle_rights != Castle::NONE {
                match (self.get(mv.start), mv.start) {
                    (Piece::WHITE_KING, Position { row: 0, col: 4 }) => next_state
                        .castle_rights
                        .remove(Castle::WHITE_KING_SIDE | Castle::WHITE_QUEEN_SIDE),
                    (Piece::BLACK_KING, Position { row: 7, col: 4 }) => next_state
                        .castle_rights
                        .remove(Castle::BLACK_KING_SIDE | Castle::BLACK_QUEEN_SIDE),
                    (Piece::WHITE_ROOK, Position { row: 0, col: 0 }) => {
                        next_state.castle_rights.remove(Castle::WHITE_QUEEN_SIDE)
                    }
                    (Piece::WHITE_ROOK, Position { row: 0, col: 7 }) => {
                        next_state.castle_rights.remove(Castle::WHITE_KING_SIDE)
                    }
                    (Piece::BLACK_ROOK, Position { row: 7, col: 0 }) => {
                        next_state.castle_rights.remove(Castle::BLACK_QUEEN_SIDE)
                    }
                    (Piece::BLACK_ROOK, Position { row: 7, col: 7 }) => {
                        next_state.castle_rights.remove(Castle::BLACK_KING_SIDE)
                    }

                    _ => {}
                }
            }

            *self.get_mut(mv.end) = self.get(mv.start);
            *self.get_mut(mv.start) = Piece::NULL_PIECE;

            if mv.double_pawn_push {
                let offset = if self.is_white_turn { -1 } else { 1 };
                next_state.en_passante_state = Some(Position {
                    row: mv.end.row + offset,
                    col: mv.end.col,
                })
            }

            if mv.en_passante {
                // remove pawn from passed square
                let offset = if self.is_white_turn { -1 } else { 1 };
                *self.get_mut(Position {
                    row: mv.end.row + offset,
                    col: mv.end.col,
                }) = Piece::NULL_PIECE;
            }

            match mv.castle {
                Castle::WHITE_KING_SIDE => {
                    *self.get_mut(Position { row: 0, col: 5 }) = Piece::WHITE_ROOK;
                    *self.get_mut(Position { row: 0, col: 7 }) = Piece::NULL_PIECE;
                }
                Castle::WHITE_QUEEN_SIDE => {
                    *self.get_mut(Position { row: 0, col: 3 }) = Piece::WHITE_ROOK;
                    *self.get_mut(Position { row: 0, col: 0 }) = Piece::NULL_PIECE;
                }
                Castle::BLACK_KING_SIDE => {
                    *self.get_mut(Position { row: 7, col: 5 }) = Piece::BLACK_ROOK;
                    *self.get_mut(Position { row: 7, col: 7 }) = Piece::NULL_PIECE;
                }
                Castle::BLACK_QUEEN_SIDE => {
                    *self.get_mut(Position { row: 7, col: 3 }) = Piece::BLACK_ROOK;
                    *self.get_mut(Position { row: 7, col: 0 }) = Piece::NULL_PIECE;
                }
                _ => {}
            }

            if let Some(prom) = mv.promotion {
                *self.get_mut(mv.end) = prom;
            };

            match self.get(mv.end) {
                Piece::WHITE_KING => self.white_king = mv.end,
                Piece::BLACK_KING => self.black_king = mv.end,
                _ => {}
            }
        }

        self.is_white_turn = !self.is_white_turn;

        // update zobrist hash for board state
        self.z_hash ^= next_state.zobrist_hash();
        self.z_hash ^= ZOBRIST.black;

        self.history.push(next_state);
    }

    pub fn unmake_move(&mut self, mv: &Move) {
        self.z_hash = self.z_hash_history.pop().unwrap();

        if mv != &Move::NULL_MOVE {
            *self.get_mut(mv.start) = self.get(mv.end);
            *self.get_mut(mv.end) = Piece::NULL_PIECE;

            if let Some(p) = mv.capture {
                if mv.en_passante {
                    // remove pawn from passed square
                    let offset = if self.is_white_turn { 1 } else { -1 };
                    *self.get_mut(Position {
                        row: mv.end.row + offset,
                        col: mv.end.col,
                    }) = p;
                } else {
                    *self.get_mut(mv.end) = p;
                }
            };

            match mv.castle {
                Castle::WHITE_KING_SIDE => {
                    *self.get_mut(Position { row: 0, col: 5 }) = Piece::NULL_PIECE;
                    *self.get_mut(Position { row: 0, col: 7 }) = Piece::WHITE_ROOK;
                }
                Castle::WHITE_QUEEN_SIDE => {
                    *self.get_mut(Position { row: 0, col: 3 }) = Piece::NULL_PIECE;
                    *self.get_mut(Position { row: 0, col: 0 }) = Piece::WHITE_ROOK;
                }
                Castle::BLACK_KING_SIDE => {
                    *self.get_mut(Position { row: 7, col: 5 }) = Piece::NULL_PIECE;
                    *self.get_mut(Position { row: 7, col: 7 }) = Piece::BLACK_ROOK;
                }
                Castle::BLACK_QUEEN_SIDE => {
                    *self.get_mut(Position { row: 7, col: 3 }) = Piece::NULL_PIECE;
                    *self.get_mut(Position { row: 7, col: 0 }) = Piece::BLACK_ROOK;
                }
                _ => {}
            }

            if mv.promotion.is_some() {
                let side = self.get(mv.start).side();
                *self.get_mut(mv.start) = if side == Piece::WHITE {
                    Piece::WHITE_PAWN
                } else {
                    Piece::BLACK_PAWN
                };
            };

            match self.get(mv.start) {
                Piece::WHITE_KING => self.white_king = mv.start,
                Piece::BLACK_KING => self.black_king = mv.start,
                _ => {}
            }
        }

        self.is_white_turn = !self.is_white_turn;
        self.history.pop();
        self.irreversible_move.pop();
    }

    pub fn is_legal(&mut self, mv: &Move) -> bool {
        self.make_move(mv);
        self.is_white_turn = !self.is_white_turn;
        let legal = !self.is_check();
        self.is_white_turn = !self.is_white_turn;
        self.unmake_move(mv);

        legal
    }

    pub fn get(&self, pos: Position) -> Piece {
        self.board[pos.row as usize][pos.col as usize]
    }

    fn get_mut(&mut self, pos: Position) -> &mut Piece {
        &mut self.board[pos.row as usize][pos.col as usize]
    }

    pub fn is_stalemate(&self) -> bool {
        // 50 move rule
        if (*self.irreversible_move.last().unwrap() as usize) + 99 < self.history.len() {
            return true;
        }

        if (*self.irreversible_move.last().unwrap() as usize) + 4 < self.history.len() {
            let count = self
                .z_hash_history
                .iter()
                .skip(*self.irreversible_move.last().unwrap() as usize)
                .filter(|z| *z == &self.z_hash)
                .count();

            if count >= 2 {
                return true;
            }
        }
        false
    }

    pub fn gen_moves(&self) -> MoveList {
        let mut res = MoveList::new();
        for (r_index, row) in self.board.iter().enumerate() {
            for (c_index, cell) in row.iter().enumerate() {
                if (cell.side() == Piece::WHITE) != self.is_white_turn {
                    continue;
                }
                match cell.kind() {
                    Piece::PAWN => self.gen_pawn_moves(
                        Position {
                            row: r_index as i8,
                            col: c_index as i8,
                        },
                        &mut res,
                    ),
                    Piece::KNIGHT => self.gen_knight_moves(
                        Position {
                            row: r_index as i8,
                            col: c_index as i8,
                        },
                        &mut res,
                    ),
                    Piece::BISHOP => self.gen_bishop_slide_moves(
                        Position {
                            row: r_index as i8,
                            col: c_index as i8,
                        },
                        &mut res,
                    ),
                    Piece::ROOK => self.gen_rook_slide_moves(
                        Position {
                            row: r_index as i8,
                            col: c_index as i8,
                        },
                        &mut res,
                    ),
                    Piece::QUEEN => {
                        self.gen_rook_slide_moves(
                            Position {
                                row: r_index as i8,
                                col: c_index as i8,
                            },
                            &mut res,
                        );
                        self.gen_bishop_slide_moves(
                            Position {
                                row: r_index as i8,
                                col: c_index as i8,
                            },
                            &mut res,
                        );
                    }
                    Piece::KING => self.gen_king_moves(
                        Position {
                            row: r_index as i8,
                            col: c_index as i8,
                        },
                        &mut res,
                    ),
                    _ => (),
                };
            }
        }

        res
    }

    fn gen_pawn_moves(&self, pos: Position, move_list: &mut MoveList) {
        let offset: i8 = if self.is_white_turn { 1 } else { -1 };
        let side: u8 = if self.is_white_turn {
            Piece::WHITE
        } else {
            Piece::BLACK
        };

        let move_pos = Position {
            row: pos.row + offset,
            col: pos.col,
        };

        let piece = self.get(pos);

        if !move_pos.in_bounds() {
            return;
        }

        if self.get(move_pos).is_null() {
            if move_pos.row == 0 || move_pos.row == 7 {
                let prom_pcs = if side == Piece::WHITE {
                    [
                        Piece::WHITE_QUEEN,
                        Piece::WHITE_BISHOP,
                        Piece::WHITE_ROOK,
                        Piece::WHITE_KNIGHT,
                    ]
                } else {
                    [
                        Piece::BLACK_QUEEN,
                        Piece::BLACK_BISHOP,
                        Piece::BLACK_ROOK,
                        Piece::BLACK_KNIGHT,
                    ]
                };

                for p in prom_pcs {
                    move_list.push(Move {
                        start: pos,
                        end: move_pos,
                        piece,
                        promotion: Some(p),
                        ..Default::default()
                    });
                }
            } else {
                move_list.push(Move {
                    start: pos,
                    end: move_pos,
                    piece,
                    ..Default::default()
                });
            }

            // can only double move if it's possible to single move
            if (pos.row == 1 && side == Piece::WHITE) || (pos.row == 6 && side == Piece::BLACK) {
                let double_move_pos = Position {
                    row: pos.row + offset + offset,
                    col: pos.col,
                };

                if double_move_pos.in_bounds() && self.get(double_move_pos).is_null() {
                    move_list.push(Move {
                        start: pos,
                        end: double_move_pos,
                        piece,
                        double_pawn_push: true,
                        ..Default::default()
                    })
                }
            }
        }

        for attack in [
            Position {
                row: move_pos.row,
                col: move_pos.col + 1,
            },
            Position {
                row: move_pos.row,
                col: move_pos.col - 1,
            },
        ] {
            if !attack.in_bounds() {
                continue;
            }

            if !self.get(attack).is_null() && self.get(attack).side() != side {
                if attack.row == 0 || attack.row == 7 {
                    let prom_pcs = if side == Piece::WHITE {
                        [
                            Piece::WHITE_QUEEN,
                            Piece::WHITE_BISHOP,
                            Piece::WHITE_ROOK,
                            Piece::WHITE_KNIGHT,
                        ]
                    } else {
                        [
                            Piece::BLACK_QUEEN,
                            Piece::BLACK_BISHOP,
                            Piece::BLACK_ROOK,
                            Piece::BLACK_KNIGHT,
                        ]
                    };

                    for p in prom_pcs {
                        move_list.push(Move {
                            start: pos,
                            end: attack,
                            piece,
                            capture: Some(self.get(attack)),
                            promotion: Some(p),
                            ..Default::default()
                        });
                    }
                } else {
                    move_list.push(Move {
                        start: pos,
                        end: attack,
                        piece,
                        capture: Some(self.get(attack)),
                        ..Default::default()
                    })
                }
            } else if [2, 5].contains(&move_pos.row)
                && self
                    .history
                    .last()
                    .unwrap()
                    .en_passante_state
                    .is_some_and(|p| p == attack)
            {
                let offset = if self.is_white_turn {
                    Position { row: -1, col: 0 }
                } else {
                    Position { row: 1, col: 0 }
                };
                move_list.push(Move {
                    start: pos,
                    end: attack,
                    piece,
                    capture: Some(self.get(attack + offset)),
                    en_passante: true,
                    ..Default::default()
                })
            }
        }
    }

    fn gen_knight_moves(&self, pos: Position, move_list: &mut MoveList) {
        let side = if self.is_white_turn {
            Piece::WHITE
        } else {
            Piece::BLACK
        };
        let piece = self.get(pos);
        let offsets = [
            Position { col: 2, row: 1 },
            Position { col: 2, row: -1 },
            Position { col: -2, row: 1 },
            Position { col: -2, row: -1 },
            Position { col: 1, row: 2 },
            Position { col: 1, row: -2 },
            Position { col: -1, row: 2 },
            Position { col: -1, row: -2 },
        ];

        for offset in offsets {
            if (pos + offset).in_bounds() {
                if !self.get(pos + offset).is_null() && self.get(pos + offset).side() == side {
                    continue;
                }
                let capture = match self.get(pos + offset) {
                    Piece::NULL_PIECE => None,
                    p => Some(p),
                };
                move_list.push(Move {
                    start: pos,
                    end: pos + offset,
                    piece,
                    capture,
                    ..Default::default()
                })
            }
        }
    }

    fn gen_slide_moves(
        &self,
        start_pos: Position,
        offsets: [Position; 4],
        move_list: &mut MoveList,
    ) {
        let side = if self.is_white_turn {
            Piece::WHITE
        } else {
            Piece::BLACK
        };
        let piece = self.get(start_pos);

        for offset in offsets {
            let mut cell = start_pos + offset;
            while cell.in_bounds() {
                let blocked = !self.get(cell).is_null();
                let is_capture = blocked && self.get(cell).side() != side;

                if blocked == is_capture {
                    let capture = match self.get(cell) {
                        Piece::NULL_PIECE => None,
                        p => Some(p),
                    };
                    move_list.push(Move {
                        start: start_pos,
                        end: cell,
                        piece,
                        capture,
                        ..Default::default()
                    });
                }
                if blocked {
                    break;
                }
                cell = cell + offset;
            }
        }
    }

    fn gen_bishop_slide_moves(&self, start_pos: Position, move_list: &mut MoveList) {
        self.gen_slide_moves(
            start_pos,
            [
                Position { row: 1, col: 1 },
                Position { row: 1, col: -1 },
                Position { row: -1, col: 1 },
                Position { row: -1, col: -1 },
            ],
            move_list,
        )
    }

    fn gen_rook_slide_moves(&self, start_pos: Position, move_list: &mut MoveList) {
        self.gen_slide_moves(
            start_pos,
            [
                Position { row: 0, col: 1 },
                Position { row: 0, col: -1 },
                Position { row: 1, col: 0 },
                Position { row: -1, col: -0 },
            ],
            move_list,
        )
    }

    fn gen_king_moves(&self, start_pos: Position, move_list: &mut MoveList) {
        let side = if self.is_white_turn {
            Piece::WHITE
        } else {
            Piece::BLACK
        };
        let piece = self.get(start_pos);

        for offset in [
            Position { row: 1, col: 1 },
            Position { row: 1, col: -1 },
            Position { row: -1, col: 1 },
            Position { row: -1, col: -1 },
            Position { row: 0, col: 1 },
            Position { row: 0, col: -1 },
            Position { row: 1, col: 0 },
            Position { row: -1, col: -0 },
        ] {
            if !(start_pos + offset).in_bounds() {
                continue;
            }
            if !self.get(start_pos + offset).is_null()
                && self.get(start_pos + offset).side() == side
            {
                continue;
            }
            let capture = match self.get(start_pos + offset) {
                Piece::NULL_PIECE => None,
                p => Some(p),
            };
            move_list.push(Move {
                start: start_pos,
                end: start_pos + offset,
                piece,
                capture,
                ..Default::default()
            })
        }

        if !self.is_check() {
            for castling in [
                Castle::WHITE_KING_SIDE,
                Castle::WHITE_QUEEN_SIDE,
                Castle::BLACK_KING_SIDE,
                Castle::BLACK_QUEEN_SIDE,
            ]
            .into_iter()
            .filter(|c| self.history.last().unwrap().castle_rights.contains(*c))
            {
                match (side, castling) {
                    (Piece::WHITE, Castle::WHITE_KING_SIDE) => {
                        let mut valid = true;
                        for position in [Position { row: 0, col: 5 }, Position { row: 0, col: 6 }] {
                            if self.is_attacked(position) || !self.get(position).is_null() {
                                valid = false;
                                break;
                            }
                        }
                        if valid {
                            move_list.push(Move {
                                start: start_pos,
                                end: Position { row: 0, col: 6 },
                                piece,
                                castle: Castle::WHITE_KING_SIDE,
                                ..Default::default()
                            })
                        };
                    }
                    (Piece::WHITE, Castle::WHITE_QUEEN_SIDE) => {
                        let mut valid = true;
                        for position in [Position { row: 0, col: 3 }, Position { row: 0, col: 2 }] {
                            if self.is_attacked(position) || !self.get(position).is_null() {
                                valid = false;
                                break;
                            }
                        }
                        // additionally queen side castles must have an empy space next to the rook
                        if valid && self.get(Position { row: 0, col: 1 }).is_null() {
                            move_list.push(Move {
                                start: start_pos,
                                end: Position { row: 0, col: 2 },
                                piece,
                                castle: Castle::WHITE_QUEEN_SIDE,
                                ..Default::default()
                            })
                        }
                    }
                    (Piece::BLACK, Castle::BLACK_KING_SIDE) => {
                        let mut valid = true;
                        for position in [Position { row: 7, col: 5 }, Position { row: 7, col: 6 }] {
                            if self.is_attacked(position) || !self.get(position).is_null() {
                                valid = false;
                                break;
                            }
                        }
                        if valid {
                            move_list.push(Move {
                                start: start_pos,
                                end: Position { row: 7, col: 6 },
                                piece,
                                castle: Castle::BLACK_KING_SIDE,
                                ..Default::default()
                            })
                        }
                    }
                    (Piece::BLACK, Castle::BLACK_QUEEN_SIDE) => {
                        let mut valid = true;
                        for position in [Position { row: 7, col: 3 }, Position { row: 7, col: 2 }] {
                            if self.is_attacked(position) || !self.get(position).is_null() {
                                valid = false;
                                break;
                            }
                        }
                        // additionally queen side castles must have an empy space next to the rook
                        if valid && self.get(Position { row: 7, col: 1 }).is_null() {
                            move_list.push(Move {
                                start: start_pos,
                                end: Position { row: 7, col: 2 },
                                piece,
                                castle: Castle::BLACK_QUEEN_SIDE,
                                ..Default::default()
                            })
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn is_check(&self) -> bool {
        if self.is_white_turn {
            self.is_attacked(self.white_king)
        } else {
            self.is_attacked(self.black_king)
        }
    }

    pub fn is_king_pawn(&self) -> bool {
        for row in self.board.iter() {
            for cell in row.iter() {
                match cell.kind() {
                    Piece::KNIGHT | Piece::BISHOP | Piece::ROOK | Piece::QUEEN => return false,
                    _ => {}
                }
            }
        }
        true
    }

    fn is_attacked(&self, pos: Position) -> bool {
        let helper = |offsets: [Position; 4], slide_attackers: [Piece; 2]| {
            for offset in offsets {
                let mut cell = pos + offset;
                while cell.in_bounds() {
                    let blocked = !self.get(cell).is_null();

                    if blocked && slide_attackers.contains(&self.get(cell)) {
                        return true;
                    }

                    if blocked {
                        break;
                    }
                    cell = cell + offset;
                }
            }

            false
        };

        let knight_helper = |enemy_knight: Piece| {
            let knight_offsets = [
                Position { col: 2, row: 1 },
                Position { col: 2, row: -1 },
                Position { col: -2, row: 1 },
                Position { col: -2, row: -1 },
                Position { col: 1, row: 2 },
                Position { col: 1, row: -2 },
                Position { col: -1, row: 2 },
                Position { col: -1, row: -2 },
            ];

            for offset in knight_offsets {
                let cell = pos + offset;
                if cell.in_bounds() && self.get(cell) == enemy_knight {
                    return true;
                }
            }

            false
        };

        let pawn_helper = |enemy_pawn: Piece, direction: i8| {
            for offset in [
                Position {
                    row: direction,
                    col: 1,
                },
                Position {
                    row: direction,
                    col: -1,
                },
            ] {
                let cell = pos + offset;
                if cell.in_bounds() && self.get(cell) == enemy_pawn {
                    return true;
                }
            }
            false
        };

        let king_helper = |enemy_king: Piece| {
            for offset in [
                Position { row: 1, col: 1 },
                Position { row: 1, col: -1 },
                Position { row: -1, col: 1 },
                Position { row: -1, col: -1 },
                Position { row: 0, col: 1 },
                Position { row: 0, col: -1 },
                Position { row: 1, col: 0 },
                Position { row: -1, col: -0 },
            ] {
                let cell = pos + offset;
                if cell.in_bounds() && self.get(cell) == enemy_king {
                    return true;
                }
            }
            false
        };

        let rook_offsets = [
            Position { row: 0, col: 1 },
            Position { row: 0, col: -1 },
            Position { row: 1, col: 0 },
            Position { row: -1, col: -0 },
        ];

        let bishop_offsets = [
            Position { row: 1, col: 1 },
            Position { row: 1, col: -1 },
            Position { row: -1, col: 1 },
            Position { row: -1, col: -1 },
        ];

        if self.is_white_turn {
            helper(rook_offsets, [Piece::BLACK_ROOK, Piece::BLACK_QUEEN])
                || helper(bishop_offsets, [Piece::BLACK_BISHOP, Piece::BLACK_QUEEN])
                || knight_helper(Piece::BLACK_KNIGHT)
                || pawn_helper(Piece::BLACK_PAWN, 1)
                || king_helper(Piece::BLACK_KING)
        } else {
            helper(rook_offsets, [Piece::WHITE_ROOK, Piece::WHITE_QUEEN])
                || helper(bishop_offsets, [Piece::WHITE_BISHOP, Piece::WHITE_QUEEN])
                || knight_helper(Piece::WHITE_KNIGHT)
                || pawn_helper(Piece::WHITE_PAWN, -1)
                || king_helper(Piece::WHITE_KING)
        }
    }

    pub fn annotate_move(&self, mv: &Move) -> Move {
        let double_pawn_push =
            self.get(mv.start).kind() == Piece::PAWN && (mv.start.row).abs_diff(mv.end.row) == 2;
        let capture = match self.get(mv.end) {
            Piece::NULL_PIECE => None,
            p => Some(p),
        };
        let en_passante = if self.get(mv.start).kind() == Piece::PAWN {
            self.history
                .last()
                .unwrap()
                .en_passante_state
                .is_some_and(|p| p == mv.end)
        } else {
            false
        };

        let castle = match self.get(mv.start) {
            Piece::WHITE_KING => {
                if !(mv.start == Position { row: 0, col: 4 }) {
                    Castle::NONE
                } else {
                    match mv.end.col {
                        2 => Castle::WHITE_QUEEN_SIDE,
                        6 => Castle::WHITE_KING_SIDE,
                        _ => Castle::NONE,
                    }
                }
            }
            Piece::BLACK_KING => {
                if !(mv.start == Position { row: 7, col: 4 }) {
                    Castle::NONE
                } else {
                    match mv.end.col {
                        2 => Castle::BLACK_QUEEN_SIDE,
                        6 => Castle::BLACK_KING_SIDE,
                        _ => Castle::NONE,
                    }
                }
            }
            _ => Castle::NONE,
        };

        Move {
            start: mv.start,
            end: mv.end,
            piece: self.get(mv.start),
            double_pawn_push,
            en_passante,
            capture,
            castle,
            promotion: mv.promotion,
        }
    }

    pub fn perft(&mut self, depth: usize) -> (usize, usize, usize, usize) {
        if depth == 1 {
            let mvs = self.gen_moves();
            let mut mv_count = 0;
            let mut capture_count = 0;
            let mut ep_count = 0;
            let mut castles = 0;
            for mv in mvs.iter().filter(|mv| self.is_legal(mv)) {
                mv_count += 1;
                if mv.capture.is_some() {
                    capture_count += 1;
                }
                if mv.en_passante {
                    ep_count += 1;
                }
                if mv.castle != Castle::NONE {
                    castles += 1;
                }
            }
            (mv_count, capture_count, ep_count, castles)
        } else {
            let mut count = 0;
            let mut capture_count = 0;
            let mut ep_count = 0;
            let mut castles = 0;
            let mvs = self.gen_moves();
            for mv in mvs {
                self.make_move(&mv);
                self.is_white_turn = !self.is_white_turn;
                let legal = !self.is_check();
                self.is_white_turn = !self.is_white_turn;
                if legal {
                    let (tc, tcc, tepc, cc) = self.perft(depth - 1);
                    count += tc;
                    capture_count += tcc;
                    ep_count += tepc;
                    castles += cc;
                }
                self.unmake_move(&mv);
            }

            (count, capture_count, ep_count, castles)
        }
    }

    pub fn moves_played(&self) -> usize {
        self.history.len() - 1
    }
}

impl default::Default for Board {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for Board {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let cur_state = self.history.last().unwrap();
        writeln!(
            f,
            "   ep: {:?}, castle_rights: {:?}",
            cur_state.en_passante_state, cur_state.castle_rights
        )?;
        writeln!(f, "   +---+---+---+---+---+---+---+---+")?;
        let mut write_row = |cells: &[Piece; 8], num: usize| -> std::fmt::Result {
            write!(f, " {} |", num)?;
            for p in cells {
                write!(f, " ")?;
                if p.is_null() {
                    write!(f, " ")?;
                } else {
                    write!(f, "{}", p)?;
                }
                write!(f, " |")?;
            }
            writeln!(f)?;
            writeln!(f, "   +---+---+---+---+---+---+---+---+")?;
            Ok(())
        };
        for (idx, row) in self.board.iter().enumerate().rev() {
            write_row(row, idx + 1)?
        }
        writeln!(f, "     A   B   C   D   E   F   G   H  ")?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct ParseFenError;

impl FromStr for Board {
    type Err = ParseFenError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let pieces: Vec<&str> = s.split(' ').collect();
        if pieces.len() != 6 {
            return Err(ParseFenError);
        }

        let mut board = [[Piece::NULL_PIECE; 8]; 8];
        let mut white_king = Position { row: 0, col: 0 };
        let mut black_king = Position { row: 0, col: 0 };

        for (r_index, row) in pieces[0].split('/').enumerate() {
            let mut c_index = 0;
            for cell in row.chars() {
                if let Some(skip) = cell.to_digit(10) {
                    c_index += skip;
                } else {
                    board[7 - r_index][c_index as usize] = match cell {
                        'r' => Ok(Piece::BLACK_ROOK),
                        'b' => Ok(Piece::BLACK_BISHOP),
                        'n' => Ok(Piece::BLACK_KNIGHT),
                        'q' => Ok(Piece::BLACK_QUEEN),
                        'k' => {
                            black_king = Position {
                                row: 7 - r_index as i8,
                                col: c_index as i8,
                            };
                            Ok(Piece::BLACK_KING)
                        }
                        'p' => Ok(Piece::BLACK_PAWN),
                        'R' => Ok(Piece::WHITE_ROOK),
                        'B' => Ok(Piece::WHITE_BISHOP),
                        'N' => Ok(Piece::WHITE_KNIGHT),
                        'Q' => Ok(Piece::WHITE_QUEEN),
                        'K' => {
                            white_king = Position {
                                row: 7 - r_index as i8,
                                col: c_index as i8,
                            };
                            Ok(Piece::WHITE_KING)
                        }
                        'P' => Ok(Piece::WHITE_PAWN),
                        _ => Err(ParseFenError),
                    }?;
                    c_index += 1;
                }
            }
        }

        let is_white_turn = match pieces[1] {
            "w" => Ok(true),
            "b" => Ok(false),
            _ => Err(ParseFenError),
        }?;

        let mut castle_rights = Castle::NONE;
        for c in pieces[2].chars() {
            let right_or = match c {
                '-' => Ok(None),
                'K' => Ok(Some(Castle::WHITE_KING_SIDE)),
                'Q' => Ok(Some(Castle::WHITE_QUEEN_SIDE)),
                'k' => Ok(Some(Castle::BLACK_KING_SIDE)),
                'q' => Ok(Some(Castle::BLACK_QUEEN_SIDE)),
                _ => Err(ParseFenError),
            }?;
            if let Some(right) = right_or {
                castle_rights.add(right);
            };
        }

        let en_passante_state = match pieces[3] {
            "-" => Ok(None),
            s => s.parse::<Position>().map(Some).map_err(|_| ParseFenError),
        }?;

        let half_move: u8 = pieces[4].parse().map_err(|_| ParseFenError)?;

        let mut history = Vec::with_capacity(MAX_HISTORY);
        history.push(BoardState {
            castle_rights,
            en_passante_state,
        });

        let mut b = Board {
            board,
            history,
            is_white_turn,
            white_king,
            black_king,
            z_hash_history: Vec::with_capacity(MAX_HISTORY),
            z_hash: 0,
            irreversible_move: vec![half_move],
        };
        b.z_hash = b.zobrist_hash();

        Ok(b)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Move {
    start: Position,
    end: Position,
    pub piece: Piece,
    double_pawn_push: bool,
    pub capture: Option<Piece>,
    en_passante: bool,
    pub castle: Castle,
    promotion: Option<Piece>,
}

impl Move {
    pub const NULL_MOVE: Self = Self {
        start: Position { row: 0, col: 0 },
        end: Position { row: 0, col: 0 },
        piece: Piece::NULL_PIECE,
        double_pawn_push: false,
        capture: None,
        en_passante: false,
        castle: Castle::NONE,
        promotion: None,
    };

    pub fn to_algebraic(&self) -> String {
        if self.castle != Castle::NONE {
            return match self.castle {
                Castle::WHITE_KING_SIDE => "O-O".to_string(),
                Castle::WHITE_QUEEN_SIDE => "O-O-O".to_string(),
                Castle::BLACK_KING_SIDE => "O-O".to_string(),
                Castle::BLACK_QUEEN_SIDE => "O-O-O".to_string(),
                _ => "".to_string(),
            };
        }
        let piece = match self.piece.kind() {
            Piece::KING => "K",
            Piece::QUEEN => "Q",
            Piece::BISHOP => "B",
            Piece::ROOK => "R",
            Piece::KNIGHT => "N",
            Piece::PAWN => {
                if self.capture.is_some() {
                    match self.start.col {
                        0 => "a",
                        1 => "b",
                        2 => "c",
                        3 => "d",
                        4 => "e",
                        5 => "f",
                        6 => "g",
                        7 => "h",
                        _ => "",
                    }
                } else {
                    ""
                }
            }
            _ => "",
        };

        let capture = if self.capture.is_some() { "x" } else { "" };
        let to = format!("{}", self.end);
        let promotion = match self.promotion {
            Some(p) => match p.kind() {
                Piece::QUEEN => "=Q",
                Piece::BISHOP => "=B",
                Piece::ROOK => "=R",
                Piece::KNIGHT => "=N",
                _ => "",
            },
            None => "",
        };

        format!("{}{}{}{}{}", piece, capture, to, promotion, "")
    }
}

impl Default for Move {
    fn default() -> Self {
        Move {
            start: Position { row: 0, col: 0 },
            end: Position { row: 0, col: 0 },
            piece: Piece::NULL_PIECE,
            double_pawn_push: false,
            capture: None,
            en_passante: false,
            castle: Castle::NONE,
            promotion: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Castle(u8);

impl Castle {
    pub const NONE: Self = Self(0);
    pub const WHITE_KING_SIDE: Self = Self(1);
    pub const WHITE_QUEEN_SIDE: Self = Self(1 << 1);
    pub const BLACK_KING_SIDE: Self = Self(1 << 2);
    pub const BLACK_QUEEN_SIDE: Self = Self(1 << 3);

    pub const ALL: Self = Self(0b1111);

    pub fn add(&mut self, castle: Castle) {
        self.0 |= castle.0;
    }

    pub fn remove(&mut self, castle: Castle) {
        self.0 &= !castle.0;
    }

    pub fn contains(&self, castle: Castle) -> bool {
        self.0 & castle.0 != 0
    }

    pub fn zobrist_key(&self) -> usize {
        self.0 as usize
    }
}

impl Default for Castle {
    fn default() -> Self {
        Castle::ALL
    }
}

impl BitOr for Castle {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

#[derive(Debug)]
pub struct ParseMoveError;

impl FromStr for Move {
    type Err = ParseMoveError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        {}
        let lower = s.trim().to_lowercase();

        let (start, end, promotion) = match lower.len() {
            4 => {
                let (start, end) = lower.split_at(2);
                (
                    start.parse().map_err(|_| ParseMoveError)?,
                    end.parse().map_err(|_| ParseMoveError)?,
                    None,
                )
            }
            5 => {
                let (start_str, rest) = lower.split_at(2);
                let (end_str, prom_str) = rest.split_at(2);

                let start: Position = start_str.parse().map_err(|_| ParseMoveError)?;
                let end: Position = end_str.parse().map_err(|_| ParseMoveError)?;

                let side = if end.row >= start.row {
                    Piece::WHITE
                } else {
                    Piece::BLACK
                };

                let mut prom = match prom_str.to_lowercase().as_str() {
                    "q" => Some(Piece::WHITE_QUEEN),
                    "b" => Some(Piece::WHITE_BISHOP),
                    "r" => Some(Piece::WHITE_ROOK),
                    "n" => Some(Piece::WHITE_KNIGHT),
                    _ => None,
                };

                if side == Piece::BLACK {
                    prom = prom.map(|p| match p {
                        Piece::WHITE_QUEEN => Piece::BLACK_QUEEN,
                        Piece::WHITE_BISHOP => Piece::BLACK_BISHOP,
                        Piece::WHITE_ROOK => Piece::BLACK_ROOK,
                        Piece::WHITE_KNIGHT => Piece::BLACK_KNIGHT,
                        _ => p,
                    })
                }

                (start, end, prom)
            }
            _ => return Err(ParseMoveError),
        };

        if !(4..6).contains(&lower.len()) {
            return Err(ParseMoveError);
        }

        Ok(Move {
            start,
            end,
            promotion,
            ..Default::default()
        })
    }
}

impl Display for Move {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.start, self.end)?;
        if let Some(prom) = self.promotion {
            match prom.kind() {
                Piece::QUEEN => write!(f, "Q"),
                Piece::BISHOP => write!(f, "B"),
                Piece::ROOK => write!(f, "R"),
                Piece::KNIGHT => write!(f, "N"),
                _ => Err(std::fmt::Error),
            }
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    row: i8,
    col: i8,
}

impl Position {
    fn in_bounds(&self) -> bool {
        (0..8).contains(&self.row) && (0..8).contains(&self.col)
    }
}

impl Add for Position {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            row: self.row + rhs.row,
            col: self.col + rhs.col,
        }
    }
}

impl Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.col {
            0 => write!(f, "a"),
            1 => write!(f, "b"),
            2 => write!(f, "c"),
            3 => write!(f, "d"),
            4 => write!(f, "e"),
            5 => write!(f, "f"),
            6 => write!(f, "g"),
            7 => write!(f, "h"),
            _ => Err(std::fmt::Error),
        }?;
        match self.row {
            0 => write!(f, "1"),
            1 => write!(f, "2"),
            2 => write!(f, "3"),
            3 => write!(f, "4"),
            4 => write!(f, "5"),
            5 => write!(f, "6"),
            6 => write!(f, "7"),
            7 => write!(f, "8"),
            _ => Err(std::fmt::Error),
        }?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct ParsePositionError;

impl FromStr for Position {
    type Err = ParsePositionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let lower = s.trim().to_lowercase();
        if lower.len() != 2 {
            return Err(ParsePositionError);
        }
        let (col, row) = lower.split_at(1);

        let c = match col {
            "a" => Ok(0),
            "b" => Ok(1),
            "c" => Ok(2),
            "d" => Ok(3),
            "e" => Ok(4),
            "f" => Ok(5),
            "g" => Ok(6),
            "h" => Ok(7),
            _ => Err(ParsePositionError),
        }?;

        let mut r = row.parse::<i8>().map_err(|_| ParsePositionError)?;
        r -= 1;

        if !(0..8).contains(&r) {
            return Err(ParsePositionError);
        }

        Ok(Position { row: r, col: c })
    }
}

pub struct ParseMoveListError;
pub fn parse_move_list(list: &str) -> Result<Vec<Move>, ParseMoveListError> {
    let mut res: Vec<Move> = Vec::new();
    for s in list.split(' ') {
        match s.parse::<Move>() {
            Ok(m) => res.push(m),
            _ => return Err(ParseMoveListError),
        }
    }

    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perft_1() {
        let mut board = Board::new();

        let (count, _, _, _) = board.perft(1);
        assert_eq!(20, count);
    }

    #[test]
    fn perft_2() {
        let mut board = Board::new();

        let (count, _, _, _) = board.perft(2);
        assert_eq!(400, count);
    }

    #[test]
    fn perft_3() {
        let mut board = Board::new();

        let (count, _, _, _) = board.perft(3);
        assert_eq!(8902, count);
    }

    #[test]
    fn perft_4() {
        let mut board = Board::new();

        let (count, _, _, _) = board.perft(4);
        assert_eq!(197281, count);
    }

    #[test]
    fn perft_5() {
        let mut board = Board::new();

        let (count, _, _, _) = board.perft(5);
        assert_eq!(4865609, count);
    }

    #[test]
    fn perft_alt_1() {
        let mut board = "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8"
            .parse::<Board>()
            .unwrap();

        let (count, _, _, _) = board.perft(1);
        assert_eq!(44, count);
    }

    #[test]
    fn perft_alt_2() {
        let mut board = "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8"
            .parse::<Board>()
            .unwrap();

        let (count, _, _, _) = board.perft(2);
        assert_eq!(1486, count);
    }

    #[test]
    fn perft_alt_3() {
        let mut board = "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8"
            .parse::<Board>()
            .unwrap();

        let (count, _, _, _) = board.perft(3);
        assert_eq!(62379, count);
    }

    #[test]
    fn perft_alt_4() {
        let mut board = "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8"
            .parse::<Board>()
            .unwrap();

        let (count, _, _, _) = board.perft(4);
        assert_eq!(2103487, count)
    }

    #[test]
    fn threefold_1() {
        let mut board = Board::new();

        let moves = vec![
            "b1c3", "b8c6", "a1b1", "a8b8", "b1a1", "b8a8", "a1b1", "a8b8", "b1a1", "b8a8", "a1b1",
            "a8b8",
        ]
        .into_iter()
        .map(|s| s.parse::<Move>().unwrap());

        for mv in moves {
            let anno = board.annotate_move(&mv);
            board.make_move(&anno);
        }
        assert!(board.is_stalemate());
    }

    #[test]
    fn threefold_2() {
        let mut board = Board::new();
        let moves = vec![
            "b1c3", "b8c6", "c3d5", "g8h6", "g1f3", "h6g4", "d2d4", "g4f6", "d5f6", "e7f6", "d4d5",
            "c6e5", "f3e5", "f6e5", "d1d3", "d8f6", "c1e3", "f8d6", "e1c1", "e8f8", "d3e4", "d6e7",
            "d1d2", "e7d6", "c1b1", "d6e7", "b1a1", "e7d6", "h1g1", "d6e7", "e4c4", "e7d6", "c4e4",
            "d6e7", "e4c4", "e7d6", "c4e4",
        ]
        .into_iter()
        .map(|s| s.parse::<Move>().unwrap());

        for mv in moves {
            let anno = board.annotate_move(&mv);
            board.make_move(&anno);
        }
        assert!(board.is_stalemate());
    }
}
