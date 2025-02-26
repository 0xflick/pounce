use types::{FromAndMoves, PawnType};

use crate::{
    bitboard::Bitboard,
    chess::{Color, Role, Square},
    movegen::*,
    position::Position,
};

impl Mover for PawnType {
    #[inline]
    fn into_piece() -> Role {
        Role::Pawn
    }

    #[inline]
    fn pseudo_legal_moves<const BLACK: bool>(from: Square, pos: &Position) -> Bitboard {
        let mut bb = Bitboard::EMPTY;
        let side = match BLACK {
            true => Color::Black,
            false => Color::White,
        };
        // add single moves
        if from
            .up(side)
            .is_some_and(|s| pos.occupancy & s == Bitboard::EMPTY)
        {
            bb |= get_pawn_moves(from, side);
            bb &= !pos.occupancy;
        }

        bb |= get_pawn_attacks(from, side) & pos.by_color[side.opponent()];
        bb
    }

    #[inline]
    fn legal_moves<const CHECK: bool, const BLACK: bool>(pos: &Position, movelist: &mut MoveList) {
        let side = match BLACK {
            true => Color::Black,
            false => Color::White,
        };
        let ksq = Square::from(pos.king_of(side));
        let pieces = pos.by_color_role(side, Self::into_piece());
        let pinned = pos.pinned;
        let checkers = pos.checkers;

        let promotion_bb = Bitboard::from(side.opponent().home_rank());

        let check_mask = if CHECK {
            between(Square::from(checkers), ksq) ^ checkers
        } else {
            Bitboard::FULL
        };

        for sq in pieces & !pinned {
            let moves = Self::pseudo_legal_moves::<BLACK>(sq, pos) & check_mask;
            if moves != Bitboard::EMPTY {
                unsafe {
                    movelist.push_unchecked(FromAndMoves::new(
                        sq,
                        moves,
                        promotion_bb & Bitboard::from(sq) != Bitboard::EMPTY,
                    ));
                }
            }
        }

        if !CHECK {
            for sq in pieces & pinned {
                let moves = Self::pseudo_legal_moves::<BLACK>(sq, pos) & line(ksq, sq);
                if moves != Bitboard::EMPTY {
                    unsafe {
                        movelist.push_unchecked(FromAndMoves::new(
                            sq,
                            moves,
                            promotion_bb & Bitboard::from(sq) != Bitboard::EMPTY,
                        ));
                    }
                }
            }
        }

        if let Some(ep) = pos.ep_square {
            // en passant source squares are the same as the squares that any
            // enemy pawn could attack from the en passant square
            let ep_source_squares = get_pawn_attacks(ep, side.opponent()) & pos.our(Role::Pawn);
            for sq in ep_source_squares {
                if Self::legal_ep_move::<BLACK>(sq, ep, pos) {
                    unsafe {
                        movelist.push_unchecked(FromAndMoves::new(sq, Bitboard::from(ep), false));
                    }
                }
            }
        }
    }
}

impl PawnType {
    #[inline]
    fn legal_ep_move<const BLACK: bool>(from: Square, to: Square, pos: &Position) -> bool {
        let side = match BLACK {
            true => Color::Black,
            false => Color::White,
        };
        let ksq = Square::from(pos.king_of(side));
        let mask = pos.occupancy
            ^ Bitboard::from(from) // unset the from square
            ^ Bitboard::from(to) // set the to square
            ^ Bitboard::from(to.down(side).unwrap()); // unset the captured pawn

        // be careful about the parentheses here, the & operator has higher
        // precedence than the | operator
        let rooks =
            (pos.by_role[Role::Rook] | pos.by_role[Role::Queen]) & pos.by_color[side.opponent()];
        let bishops =
            (pos.by_role[Role::Bishop] | pos.by_role[Role::Queen]) & pos.by_color[side.opponent()];

        let mut attackers = Bitboard::EMPTY;
        attackers |= get_rook_moves(ksq, mask) & rooks;
        attackers |= get_bishop_moves(ksq, mask) & bishops;
        attackers == Bitboard::EMPTY
    }
}
