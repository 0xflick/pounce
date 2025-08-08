use arrayvec::ArrayVec;

use crate::chess::bitboard::Bitboard;
use crate::chess::movegen::utils::{between, line};
use crate::chess::{Color, Move, Position, Role, Square};

pub type MoveList = ArrayVec<FromAndMoves, 18>;

#[derive(Debug, Clone, Copy)]
pub struct FromAndMoves {
    from: Square,
    moves: Bitboard,
    is_promotion: bool,
    is_ep: bool,
}

impl FromAndMoves {
    pub fn new(from: Square, moves: Bitboard, is_promotion: bool, is_ep: bool) -> Self {
        FromAndMoves {
            from,
            moves,
            is_promotion,
            is_ep,
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
    captures_only: bool,
}

impl MoveGen {
    pub fn new(pos: &Position) -> Self {
        let mut moves = MoveList::new();
        let checkers = pos.checkers[pos.side];

        if checkers == Bitboard::EMPTY {
            match pos.side {
                Color::White => {
                    PawnType::legal_moves::<false, false>(pos, &mut moves);
                    KnightType::legal_moves::<false, false>(pos, &mut moves);
                    BishopType::legal_moves::<false, false>(pos, &mut moves);
                    RookType::legal_moves::<false, false>(pos, &mut moves);
                    QueenType::legal_moves::<false, false>(pos, &mut moves);
                    KingType::legal_moves::<false, false>(pos, &mut moves);
                }
                Color::Black => {
                    PawnType::legal_moves::<false, true>(pos, &mut moves);
                    KnightType::legal_moves::<false, true>(pos, &mut moves);
                    BishopType::legal_moves::<false, true>(pos, &mut moves);
                    RookType::legal_moves::<false, true>(pos, &mut moves);
                    QueenType::legal_moves::<false, true>(pos, &mut moves);
                    KingType::legal_moves::<false, true>(pos, &mut moves);
                }
            }
        } else if checkers.count() == 1 {
            match pos.side {
                Color::White => {
                    PawnType::legal_moves::<true, false>(pos, &mut moves);
                    KnightType::legal_moves::<true, false>(pos, &mut moves);
                    BishopType::legal_moves::<true, false>(pos, &mut moves);
                    RookType::legal_moves::<true, false>(pos, &mut moves);
                    QueenType::legal_moves::<true, false>(pos, &mut moves);
                    KingType::legal_moves::<true, false>(pos, &mut moves);
                }
                Color::Black => {
                    PawnType::legal_moves::<true, true>(pos, &mut moves);
                    KnightType::legal_moves::<true, true>(pos, &mut moves);
                    BishopType::legal_moves::<true, true>(pos, &mut moves);
                    RookType::legal_moves::<true, true>(pos, &mut moves);
                    QueenType::legal_moves::<true, true>(pos, &mut moves);
                    KingType::legal_moves::<true, true>(pos, &mut moves);
                }
            }
        } else {
            match pos.side {
                Color::White => {
                    KingType::legal_moves::<true, false>(pos, &mut moves);
                }
                Color::Black => {
                    KingType::legal_moves::<true, true>(pos, &mut moves);
                }
            }
        }

        MoveGen {
            moves,
            index: 0,
            promotion_index: PromotionIndex::Queen,
            iter_mask: Bitboard::FULL,
            captures_only: false,
        }
    }

    pub fn set_mask(&mut self, mask: Bitboard) {
        self.index = 0;
        self.iter_mask = mask;
    }

    pub fn set_captures_only(&mut self, occupancy: Bitboard) {
        self.index = 0;
        self.iter_mask = occupancy;
        self.captures_only = true;
    }

    pub fn disable_captures_only(&mut self) {
        self.index = 0;
        self.iter_mask = Bitboard::FULL;
        self.captures_only = false;
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

            // skip moves that are not in the mask,
            // but make sure that if we are generating captures only,
            // we do not skip en passant moves (which are not in the mask)
            let masked = if self.captures_only && moves.is_ep {
                moves.moves // Use the original moves (the EP target square)
            } else {
                moves.moves & self.iter_mask
            };

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

pub struct PawnType;
pub struct KnightType;
pub struct BishopType;
pub struct RookType;
pub struct QueenType;
pub struct KingType;

pub trait Mover {
    fn into_piece() -> Role;

    fn pseudo_legal_moves<const BLACK: bool>(from: Square, pos: &Position) -> Bitboard;

    #[inline]
    fn legal_moves<const CHECK: bool, const BLACK: bool>(pos: &Position, movelist: &mut MoveList) {
        let side = match BLACK {
            true => Color::Black,
            false => Color::White,
        };
        let ksq = Square::from(pos.king_of(side));
        let pieces = pos.by_color_role(side, Self::into_piece());
        let pinned = pos.pinned[side];
        let checkers = pos.checkers[side];

        let check_mask = if CHECK {
            between(Square::from(checkers), ksq) ^ checkers
        } else {
            Bitboard::FULL
        };

        for sq in pieces & !pinned {
            let moves = Self::pseudo_legal_moves::<BLACK>(sq, pos) & check_mask;

            if moves != Bitboard::EMPTY {
                unsafe {
                    movelist.push_unchecked(FromAndMoves {
                        from: sq,
                        moves,
                        is_promotion: false,
                        is_ep: false,
                    })
                }
            }
        }

        if !CHECK {
            for sq in pieces & pinned {
                let moves = Self::pseudo_legal_moves::<BLACK>(sq, pos) & line(ksq, sq);
                if moves != Bitboard::EMPTY {
                    unsafe {
                        movelist.push_unchecked(FromAndMoves {
                            from: sq,
                            moves,
                            is_promotion: false,
                            is_ep: false,
                        });
                    }
                }
            }
        }
    }
}
