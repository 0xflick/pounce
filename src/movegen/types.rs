use arrayvec::ArrayVec;

use crate::{
    bitboard::Bitboard,
    chess::{Color, Role, Square},
    movegen::*,
    moves::Move,
    position::Position,
};

pub type MoveList = ArrayVec<FromAndMoves, 18>;

#[derive(Debug, Clone, Copy)]
pub struct FromAndMoves {
    from: Square,
    moves: Bitboard,
    is_promotion: bool,
}

impl FromAndMoves {
    pub fn new(from: Square, moves: Bitboard, is_promotion: bool) -> Self {
        FromAndMoves {
            from,
            moves,
            is_promotion,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PromotionIndex {
    Queen,
    Rook,
    Bishop,
    Knight,
}

impl PromotionIndex {
    fn next(self) -> Self {
        match self {
            PromotionIndex::Queen => PromotionIndex::Rook,
            PromotionIndex::Rook => PromotionIndex::Bishop,
            PromotionIndex::Bishop => PromotionIndex::Knight,
            PromotionIndex::Knight => PromotionIndex::Queen,
        }
    }
}

#[derive(Debug)]
pub struct MoveGen {
    moves: MoveList,
    index: usize,
    promotion_index: PromotionIndex,
    iter_mask: Bitboard,
}

impl MoveGen {
    pub fn new(pos: &Position) -> Self {
        let mut moves = MoveList::new();
        let checkers = pos.checkers;

        if checkers == Bitboard::EMPTY {
            match pos.side {
                Color::White => {
                    PawnType::legal_moves::<NotCheck, WhiteType>(pos, &mut moves);
                    KnightType::legal_moves::<NotCheck, WhiteType>(pos, &mut moves);
                    BishopType::legal_moves::<NotCheck, WhiteType>(pos, &mut moves);
                    RookType::legal_moves::<NotCheck, WhiteType>(pos, &mut moves);
                    QueenType::legal_moves::<NotCheck, WhiteType>(pos, &mut moves);
                    KingType::legal_moves::<NotCheck, WhiteType>(pos, &mut moves);
                }
                Color::Black => {
                    PawnType::legal_moves::<NotCheck, BlackType>(pos, &mut moves);
                    KnightType::legal_moves::<NotCheck, BlackType>(pos, &mut moves);
                    BishopType::legal_moves::<NotCheck, BlackType>(pos, &mut moves);
                    RookType::legal_moves::<NotCheck, BlackType>(pos, &mut moves);
                    QueenType::legal_moves::<NotCheck, BlackType>(pos, &mut moves);
                    KingType::legal_moves::<NotCheck, BlackType>(pos, &mut moves);
                }
            }
        } else if checkers.count() == 1 {
            match pos.side {
                Color::White => {
                    PawnType::legal_moves::<InCheck, WhiteType>(pos, &mut moves);
                    KnightType::legal_moves::<InCheck, WhiteType>(pos, &mut moves);
                    BishopType::legal_moves::<InCheck, WhiteType>(pos, &mut moves);
                    RookType::legal_moves::<InCheck, WhiteType>(pos, &mut moves);
                    QueenType::legal_moves::<InCheck, WhiteType>(pos, &mut moves);
                    KingType::legal_moves::<InCheck, WhiteType>(pos, &mut moves);
                }
                Color::Black => {
                    PawnType::legal_moves::<InCheck, BlackType>(pos, &mut moves);
                    KnightType::legal_moves::<InCheck, BlackType>(pos, &mut moves);
                    BishopType::legal_moves::<InCheck, BlackType>(pos, &mut moves);
                    RookType::legal_moves::<InCheck, BlackType>(pos, &mut moves);
                    QueenType::legal_moves::<InCheck, BlackType>(pos, &mut moves);
                    KingType::legal_moves::<InCheck, BlackType>(pos, &mut moves);
                }
            }
        } else {
            match pos.side {
                Color::White => {
                    KingType::legal_moves::<InCheck, WhiteType>(pos, &mut moves);
                }
                Color::Black => {
                    KingType::legal_moves::<InCheck, BlackType>(pos, &mut moves);
                }
            }
        }

        MoveGen {
            moves,
            index: 0,
            promotion_index: PromotionIndex::Queen,
            iter_mask: Bitboard::FULL,
        }
    }

    pub fn set_mask(&mut self, mask: Bitboard) {
        self.index = 0;
        self.iter_mask = mask;
    }
}

impl ExactSizeIterator for MoveGen {
    fn len(&self) -> usize {
        let mut res = 0;
        for i in self.index..self.moves.len() {
            let move_count = (self.moves[i].moves & self.iter_mask).count();
            if self.moves[i].is_promotion {
                res += move_count * 4;
            } else {
                res += move_count;
            }
        }
        res as usize
    }
}

impl Iterator for MoveGen {
    type Item = Move;

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.moves.len() {
            None
        } else if self.moves[self.index].is_promotion {
            let moves = &mut self.moves[self.index];
            let masked = moves.moves & self.iter_mask;
            if masked == Bitboard::EMPTY {
                self.index += 1;
                return self.next();
            }
            let to = Square::from(masked);

            match self.promotion_index {
                PromotionIndex::Queen => {
                    self.promotion_index = self.promotion_index.next();
                    Some(Move::new(moves.from, to, Some(Role::Queen)))
                }
                PromotionIndex::Rook => {
                    self.promotion_index = self.promotion_index.next();
                    Some(Move::new(moves.from, to, Some(Role::Rook)))
                }
                PromotionIndex::Bishop => {
                    self.promotion_index = self.promotion_index.next();
                    Some(Move::new(moves.from, to, Some(Role::Bishop)))
                }
                PromotionIndex::Knight => {
                    self.promotion_index = self.promotion_index.next();

                    moves.moves ^= Bitboard::from(to);
                    if moves.moves == Bitboard::EMPTY {
                        self.index += 1;
                    }

                    Some(Move::new(moves.from, to, Some(Role::Knight)))
                }
            }
        } else {
            let moves = &mut self.moves[self.index];
            let masked = moves.moves & self.iter_mask;
            if masked == Bitboard::EMPTY {
                self.index += 1;
                return self.next();
            }
            let to = Square::from(masked);

            moves.moves ^= Bitboard::from(to);
            if moves.moves == Bitboard::EMPTY {
                self.index += 1;
            }

            Some(Move::new(moves.from, to, None))
        }
    }
}
pub trait CheckType {
    const IN_CHECK: bool;
}

pub struct InCheck;
pub struct NotCheck;

impl CheckType for InCheck {
    const IN_CHECK: bool = true;
}

impl CheckType for NotCheck {
    const IN_CHECK: bool = false;
}

pub trait ColorType {
    const COLOR: Color;
}

pub struct WhiteType;
pub struct BlackType;

impl ColorType for WhiteType {
    const COLOR: Color = Color::White;
}

impl ColorType for BlackType {
    const COLOR: Color = Color::Black;
}

pub struct PawnType;
pub struct KnightType;
pub struct BishopType;
pub struct RookType;
pub struct QueenType;
pub struct KingType;

pub trait Mover {
    fn into_piece() -> Role;

    fn pseudo_legal_moves<CO: ColorType>(from: Square, pos: &Position) -> Bitboard;

    #[inline]
    fn legal_moves<CH: CheckType, CO: ColorType>(pos: &Position, movelist: &mut MoveList) {
        let side = CO::COLOR;
        let ksq = Square::from(pos.king_of(side));
        let pieces = pos.by_color_role(side, Self::into_piece());
        let pinned = pos.pinned;
        let checkers = pos.checkers;

        let check_mask = if CH::IN_CHECK {
            between(Square::from(checkers), ksq) ^ checkers
        } else {
            Bitboard::FULL
        };

        for sq in pieces & !pinned {
            let moves = Self::pseudo_legal_moves::<CO>(sq, pos) & check_mask;

            if moves != Bitboard::EMPTY {
                unsafe {
                    movelist.push_unchecked(FromAndMoves {
                        from: sq,
                        moves,
                        is_promotion: false,
                    })
                }
            }
        }

        if !CH::IN_CHECK {
            for sq in pieces & pinned {
                let moves = Self::pseudo_legal_moves::<CO>(sq, pos) & line(ksq, sq);
                if moves != Bitboard::EMPTY {
                    unsafe {
                        movelist.push_unchecked(FromAndMoves {
                            from: sq,
                            moves,
                            is_promotion: false,
                        });
                    }
                }
            }
        }
    }
}
