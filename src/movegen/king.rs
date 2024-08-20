use types::{CheckType, ColorType, FromAndMoves, KingType};

use crate::{
    bitboard::Bitboard,
    chess::{Role, Square},
    movegen::*,
    position::Position,
};

impl Mover for KingType {
    #[inline]
    fn into_piece() -> Role {
        Role::King
    }

    #[inline]
    fn pseudo_legal_moves<CO: ColorType>(from: Square, pos: &Position) -> Bitboard {
        get_king_moves(from) & !pos.by_color[CO::COLOR as usize]
    }

    #[inline]
    fn legal_moves<CH: CheckType, CO: ColorType>(pos: &Position, movelist: &mut MoveList) {
        let side = CO::COLOR;
        let ksq = Square::from(pos.king_of(side));

        let mut moves = Self::pseudo_legal_moves::<CO>(ksq, pos);
        for m in moves {
            if !Self::legal_king_move::<CO>(pos, m) {
                moves ^= Bitboard::from(m);
            }
        }

        if !CH::IN_CHECK {
            if pos.castling.can_castle_kingside(side)
                && (get_kingside_castle_through_squares(side) & pos.occupancy).none()
            {
                let middle = ksq.east().unwrap();
                let end = middle.east().unwrap();

                if KingType::legal_king_move::<CO>(pos, middle)
                    && KingType::legal_king_move::<CO>(pos, end)
                {
                    moves ^= Bitboard::from(end);
                }
            }

            if pos.castling.can_castle_queenside(side)
                && (get_queenside_castle_throught_squares(side) & pos.occupancy).none()
            {
                let middle = ksq.west().unwrap();
                let end = middle.west().unwrap();
                if KingType::legal_king_move::<CO>(pos, middle)
                    && KingType::legal_king_move::<CO>(pos, middle)
                    && KingType::legal_king_move::<CO>(pos, end)
                {
                    moves ^= Bitboard::from(end);
                }
            }
        }

        if moves != Bitboard::EMPTY {
            unsafe {
                movelist.push_unchecked(FromAndMoves::new(ksq, moves, false));
            }
        }
    }
}

impl KingType {
    #[inline]
    pub fn legal_king_move<CO: ColorType>(pos: &Position, sq: Square) -> bool {
        let side = CO::COLOR;
        let mask = pos.occupancy ^ pos.king_of(side);

        let mut attackers = Bitboard::EMPTY;
        let rooks = pos.by_color_role(side.opponent(), Role::Rook)
            | pos.by_color_role(side.opponent(), Role::Queen);

        attackers |= get_rook_moves(sq, mask) & rooks;
        if attackers != Bitboard::EMPTY {
            return false;
        }

        let bishops = pos.their(Role::Bishop) | pos.by_color_role(side.opponent(), Role::Queen);
        attackers |= get_bishop_moves(sq, mask) & bishops;
        if attackers != Bitboard::EMPTY {
            return false;
        }

        attackers |= get_knight_moves(sq) & pos.by_color_role(side.opponent(), Role::Knight);
        if attackers != Bitboard::EMPTY {
            return false;
        }

        attackers |= get_pawn_attacks(sq, side) & pos.by_color_role(side.opponent(), Role::Pawn);
        if attackers != Bitboard::EMPTY {
            return false;
        }

        true
    }
}
