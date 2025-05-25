pub mod fen;
pub mod san;
pub mod zobrist;

use std::num::NonZeroU16;

use bitflags::bitflags;

use crate::chess::bitboard::Bitboard;
use crate::chess::movegen::MoveGen;
use crate::chess::movegen::utils::{
    between, bishop_rays, get_knight_moves, get_pawn_attacks, rook_rays,
};
use crate::chess::position::zobrist::ZobristHash;
use crate::chess::{Color, File, GameResult, Move, MoveType, Piece, Role, Square};
use crate::engine::eval::{PSQT_EG, PSQT_MG};

bitflags! {
    #[derive(Debug, Copy, Clone, Eq, PartialEq)]
    pub struct CastleRights: u8 {
        const WHITE_KING_SIDE = 0b0001;
        const WHITE_QUEEN_SIDE = 0b0010;
        const BLACK_KING_SIDE = 0b0100;
        const BLACK_QUEEN_SIDE = 0b1000;
    }
}

impl CastleRights {
    pub fn new() -> CastleRights {
        CastleRights::all()
    }
}

impl Default for CastleRights {
    fn default() -> CastleRights {
        CastleRights::new()
    }
}

impl CastleRights {
    pub fn discard_color(&mut self, color: Color) {
        match color {
            Color::White => {
                self.remove(CastleRights::WHITE_KING_SIDE | CastleRights::WHITE_QUEEN_SIDE);
            }
            Color::Black => {
                self.remove(CastleRights::BLACK_KING_SIDE | CastleRights::BLACK_QUEEN_SIDE);
            }
        }
    }

    pub fn discard_square(&mut self, square: Square) {
        match square {
            Square::A1 => self.remove(CastleRights::WHITE_QUEEN_SIDE),
            Square::H1 => self.remove(CastleRights::WHITE_KING_SIDE),
            Square::A8 => self.remove(CastleRights::BLACK_QUEEN_SIDE),
            Square::H8 => self.remove(CastleRights::BLACK_KING_SIDE),
            _ => {}
        }
    }

    pub fn can_castle_kingside(&self, color: Color) -> bool {
        match color {
            Color::White => self.contains(CastleRights::WHITE_KING_SIDE),
            Color::Black => self.contains(CastleRights::BLACK_KING_SIDE),
        }
    }

    pub fn can_castle_queenside(&self, color: Color) -> bool {
        match color {
            Color::White => self.contains(CastleRights::WHITE_QUEEN_SIDE),
            Color::Black => self.contains(CastleRights::BLACK_QUEEN_SIDE),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct State {
    pub castling: CastleRights,
    pub ep_square: Option<Square>,
    pub halfmove_clock: u8,
    pub captured: Option<Piece>,
    pub checkers: Bitboard,
    pub pinned: Bitboard,
    pub key: ZobristHash,
}

#[derive(Debug, Clone)]
pub struct Position {
    pub by_color: [Bitboard; Color::NUM],
    pub by_role: [Bitboard; Role::NUM],
    pub occupancy: Bitboard,
    pub checkers: Bitboard,
    pub pinned: Bitboard,

    pub mailbox: [Option<Piece>; 64],

    pub castling: CastleRights,
    pub ep_square: Option<Square>,

    pub side: Color,

    pub halfmove_clock: u8,
    pub fullmove_number: NonZeroU16,

    pub key: ZobristHash,

    pub history: Vec<State>,

    pub psqt_mg: i32,
    pub psqt_eg: i32,
}

impl Position {
    pub fn new() -> Position {
        Position {
            by_color: [Bitboard::EMPTY; Color::NUM],
            by_role: [Bitboard::EMPTY; Role::NUM],
            occupancy: Bitboard::EMPTY,
            checkers: Bitboard::EMPTY,
            pinned: Bitboard::EMPTY,
            mailbox: [None; 64],
            castling: CastleRights::all(),
            ep_square: None,
            side: Color::White,
            halfmove_clock: 0,
            fullmove_number: NonZeroU16::new(1).unwrap(),
            key: ZobristHash::new(),
            history: Vec::new(),
            psqt_mg: 0,
            psqt_eg: 0,
        }
    }
}

impl Default for Position {
    fn default() -> Position {
        Position::new()
    }
}

impl Position {
    #[inline]
    pub fn color_at(&self, sq: Square) -> Option<Color> {
        self.mailbox[sq].map(|piece| piece.color)
    }

    #[inline]
    pub fn role_at(&self, sq: Square) -> Option<Role> {
        self.mailbox[sq].map(|piece| piece.role)
    }

    #[inline]
    pub fn piece_at(&self, sq: Square) -> Option<Piece> {
        self.mailbox[sq]
    }

    #[inline]
    pub fn by_color_role(&self, color: Color, role: Role) -> Bitboard {
        self.by_color[color as usize] & self.by_role[role as usize]
    }

    #[inline]
    pub fn king_of(&self, color: Color) -> Bitboard {
        self.by_color_role(color, Role::King)
    }

    #[inline]
    pub fn us(&self) -> Bitboard {
        self.by_color[self.side as usize]
    }

    #[inline]
    pub fn them(&self) -> Bitboard {
        self.by_color[self.side.opponent() as usize]
    }

    #[inline]
    pub fn our(&self, role: Role) -> Bitboard {
        self.by_color_role(self.side, role)
    }

    #[inline]
    pub fn their(&self, role: Role) -> Bitboard {
        self.by_color_role(self.side.opponent(), role)
    }

    #[inline]
    pub fn our_king(&self) -> Bitboard {
        self.by_color_role(self.side, Role::King)
    }

    #[inline]
    pub fn their_king(&self) -> Bitboard {
        self.by_color_role(self.side.opponent(), Role::King)
    }

    #[inline]
    pub fn in_check(&self) -> bool {
        !self.checkers.none()
    }

    #[inline]
    pub fn is_draw(&self) -> Option<GameResult> {
        if self.halfmove_clock >= 100 {
            let num_moves = MoveGen::new(self).len();
            if num_moves > 0 && self.in_check() {
                return Some(GameResult::Loss);
            } else {
                return Some(GameResult::Draw);
            }
        }

        let num_pieces = self.occupancy.count();
        if num_pieces == 2 {
            return Some(GameResult::Draw);
        }

        if num_pieces == 3
            && (self.by_role[Role::Bishop].count() > 0 || self.by_role[Role::Knight].count() > 0)
        {
            return Some(GameResult::Draw);
        }

        let wbishops = self.by_color_role(Color::White, Role::Bishop);
        let bbishops = self.by_color_role(Color::Black, Role::Bishop);

        if num_pieces == 4
            && wbishops.count() == 1
            && bbishops.count() == 1
            && Square::from(wbishops).same_color(Square::from(bbishops))
        {
            return Some(GameResult::Draw);
        }

        None
    }

    pub fn is_repetition(&self, count: u32) -> bool {
        let mut found = 0;
        let mut idx = self.history.len() as i32 - 2;

        while idx >= 0 && idx >= self.history.len() as i32 - self.halfmove_clock as i32 - 1 {
            if self.history[idx as usize].key == self.key {
                found += 1;
            }
            if found == count {
                return true;
            }
            idx -= 2;
        }

        false
    }

    pub fn non_pawn_material(&self, color: Color) -> bool {
        if self.by_color[color].count() == 1 {
            return false;
        }

        if self.by_color_role(color, Role::Knight).any() {
            return true;
        }

        if self.by_color_role(color, Role::Bishop).any() {
            return true;
        }

        if self.by_color_role(color, Role::Rook).any() {
            return true;
        }

        if self.by_color_role(color, Role::Queen).any() {
            return true;
        }

        false
    }

    #[inline]
    pub fn discard(&mut self, sq: Square, piece: Piece) {
        match piece.color {
            Color::White => {
                self.psqt_mg -= PSQT_MG[piece.role][sq as usize ^ 56];
                self.psqt_eg -= PSQT_EG[piece.role][sq as usize ^ 56];
            }
            Color::Black => {
                self.psqt_mg += PSQT_MG[piece.role][sq as usize];
                self.psqt_eg += PSQT_EG[piece.role][sq as usize];
            }
        };

        self.by_color.iter_mut().for_each(|bb| bb.clear(sq));
        self.by_role.iter_mut().for_each(|bb| bb.clear(sq));
        self.occupancy.clear(sq);
        self.mailbox[sq] = None;
        self.key.toggle_piece(sq, piece);
    }

    #[inline]
    pub fn set(&mut self, sq: Square, piece: Piece) {
        if let Some(prev) = self.piece_at(sq) {
            self.discard(sq, prev);
        }
        match piece.color {
            Color::White => {
                self.psqt_mg += PSQT_MG[piece.role][sq as usize ^ 56];
                self.psqt_eg += PSQT_EG[piece.role][sq as usize ^ 56];
            }
            Color::Black => {
                self.psqt_mg -= PSQT_MG[piece.role][sq as usize];
                self.psqt_eg -= PSQT_EG[piece.role][sq as usize];
            }
        };
        self.by_color[piece.color as usize].set(sq);
        self.by_role[piece.role as usize].set(sq);
        self.occupancy.set(sq);
        self.mailbox[sq] = Some(piece);
        self.key.toggle_piece(sq, piece);
    }

    #[inline]
    pub fn make_move(&mut self, mv: Move) {
        let from = mv.from();
        let to = mv.to();

        let piece = self.piece_at(from).unwrap();
        debug_assert!(piece.color == self.side);
        let mut state = State {
            castling: self.castling,
            ep_square: self.ep_square,
            halfmove_clock: self.halfmove_clock,
            captured: None,
            checkers: self.checkers,
            pinned: self.pinned,
            key: self.key,
        };

        // reset the en passant square
        let prev_ep_square = self.ep_square;
        self.key.toggle_ep(self.ep_square);
        self.ep_square = None;

        self.halfmove_clock += 1;

        match mv.move_type(piece.role, prev_ep_square) {
            MoveType::Normal => {
                state.captured = self.piece_at(to);
                self.discard(from, piece);
                self.set(to, piece);
            }
            MoveType::DoublePawnPush => {
                self.discard(from, piece);
                self.set(to, piece);

                let potential_ep_sq = from.up(self.side).unwrap();
                if get_pawn_attacks(potential_ep_sq, self.side) & self.their(Role::Pawn)
                    != Bitboard::EMPTY
                {
                    self.ep_square = Some(potential_ep_sq);
                    self.key.toggle_ep(self.ep_square);
                }
            }
            MoveType::EnPassant => {
                // unwrapping is safe here because we know ep_square is never at the edge of the
                // board
                let captured_pawn_square = to
                    .down(self.side)
                    .expect("en passant moves are never at the edge of the board");
                state.captured = Some(
                    self.piece_at(captured_pawn_square)
                        .expect("en passant moves always have a capture"),
                );
                self.discard(from, piece);
                self.discard(captured_pawn_square, state.captured.unwrap());
                self.set(to, piece);
            }
            MoveType::Castle => {
                self.halfmove_clock = 0;
                if from.file().direction(to.file()) == 2 {
                    let rook_from = Square::make(File::H, self.side.back_rank());
                    let rook_to = Square::make(File::F, self.side.back_rank());
                    let rook = self.piece_at(rook_from).unwrap();
                    self.discard(rook_from, rook);
                    self.set(rook_to, rook);
                } else {
                    let rook_from = Square::make(File::A, self.side.back_rank());
                    let rook_to = Square::make(File::D, self.side.back_rank());
                    let rook = self.piece_at(rook_from).unwrap();
                    self.discard(rook_from, rook);
                    self.set(rook_to, rook);
                }
                self.discard(from, piece);
                self.set(to, piece);
            }
            MoveType::Promotion => {
                state.captured = self.piece_at(to);
                let promoted = Piece::new(self.side, mv.promotion().unwrap());
                self.discard(from, piece);
                self.set(to, promoted);
            }
        }

        // update halfmove clock
        // castling was handled above
        if state.captured.is_some() || piece.role == Role::Pawn {
            self.halfmove_clock = 0;
        }

        // update our castling rights
        if piece.role == Role::King {
            self.key.toggle_castling(self.castling);
            self.castling.discard_color(self.side);
            self.key.toggle_castling(self.castling);
        } else if piece.role == Role::Rook {
            self.key.toggle_castling(self.castling);
            self.castling.discard_square(from);
            self.key.toggle_castling(self.castling);
        }

        // update their castling rights
        if let Some(captured) = state.captured {
            if captured.role == Role::Rook {
                self.key.toggle_castling(self.castling);
                self.castling.discard_square(to);
                self.key.toggle_castling(self.castling);
            }
        }

        self.update_checks_and_pins(mv, Some(mv.promotion().unwrap_or(piece.role)));

        self.history.push(state);
        if self.side == Color::Black {
            self.fullmove_number = self.fullmove_number.saturating_add(1);
        }

        self.side = self.side.opponent();
        self.key.toggle_side();
    }

    pub fn unmake_move(&mut self, mv: Move) {
        self.side = self.side.opponent();
        self.key.toggle_side();

        let past = self
            .history
            .pop()
            .expect("unmake called without a past state");

        if self.castling != past.castling {
            self.key.toggle_castling(self.castling);
            self.castling = past.castling;
            self.key.toggle_castling(self.castling);
        }

        if self.ep_square != past.ep_square {
            self.key.toggle_ep(self.ep_square);
            self.ep_square = past.ep_square;
            self.key.toggle_ep(self.ep_square);
        }

        self.halfmove_clock = past.halfmove_clock;
        if self.side == Color::Black {
            self.fullmove_number = NonZeroU16::new(self.fullmove_number.get() - 1).unwrap();
        }
        self.pinned = past.pinned;
        self.checkers = past.checkers;

        let from = mv.from();
        let to = mv.to();
        let piece = self
            .piece_at(to)
            .expect("unmake called without a piece at destination");

        match mv.move_type(piece.role, self.ep_square) {
            MoveType::Normal | MoveType::DoublePawnPush => {
                self.discard(to, piece);
                self.set(from, piece);
                if let Some(captured) = past.captured {
                    self.set(to, captured);
                }
            }
            MoveType::EnPassant => {
                let captured_pawn_square = to
                    .down(self.side)
                    .expect("en passant moves are never at the edge of the board");
                let captured_pawn = past
                    .captured
                    .expect("en passant moves always have a capture");
                self.discard(to, piece);
                self.set(from, piece);
                self.set(captured_pawn_square, captured_pawn);
            }
            MoveType::Castle => {
                if from.file().direction(to.file()) == 2 {
                    let rook_from = Square::make(File::H, self.side.back_rank());
                    let rook_to = Square::make(File::F, self.side.back_rank());
                    let rook = self.piece_at(rook_to).expect("castling always has a rook");
                    self.discard(rook_to, rook);
                    self.set(rook_from, rook);
                } else {
                    let rook_from = Square::make(File::A, self.side.back_rank());
                    let rook_to = Square::make(File::D, self.side.back_rank());
                    let rook = self.piece_at(rook_to).expect("castling always has a rook");
                    self.discard(rook_to, rook);
                    self.set(rook_from, rook);
                }
                self.discard(to, piece);
                self.set(from, piece);
            }
            MoveType::Promotion => {
                let promoted = Piece::new(self.side, mv.promotion().unwrap());
                self.discard(to, promoted);
                self.set(from, Piece::new(self.side, Role::Pawn));
                if let Some(captured) = past.captured {
                    self.set(to, captured);
                }
            }
        }
    }

    pub fn make_null_move(&mut self) {
        let state = State {
            castling: self.castling,
            ep_square: self.ep_square,
            halfmove_clock: self.halfmove_clock,
            captured: None,
            checkers: self.checkers,
            pinned: self.pinned,
            key: self.key,
        };

        debug_assert!(self.checkers.none());

        self.checkers = Bitboard::EMPTY;
        self.update_checks_and_pins(Move::NULL, None);

        self.key.toggle_ep(self.ep_square);
        self.ep_square = None;
        self.key.toggle_ep(self.ep_square);

        self.history.push(state);
        if self.side == Color::Black {
            self.fullmove_number = self.fullmove_number.saturating_add(1);
        }

        self.key.toggle_side();
        self.side = self.side.opponent();
    }

    pub fn unmake_null_move(&mut self) {
        self.side = self.side.opponent();
        self.key.toggle_side();

        let past = self
            .history
            .pop()
            .expect("unmake_null_move called without a past state");

        self.key.toggle_ep(self.ep_square);
        self.ep_square = past.ep_square;
        self.key.toggle_ep(self.ep_square);

        self.halfmove_clock = past.halfmove_clock;
        if self.side == Color::Black {
            self.fullmove_number = NonZeroU16::new(self.fullmove_number.get() - 1).unwrap();
        }
        self.pinned = past.pinned;
        self.checkers = past.checkers;
    }

    #[inline]
    fn update_checks_and_pins(&mut self, mv: Move, piece: Option<Role>) {
        // we update side at the very end of make move, so we're looking for checks
        // we make against the opponent
        self.checkers = Bitboard::EMPTY;
        self.pinned = Bitboard::EMPTY;

        let dest_bb = Bitboard::from(mv.to());

        let ksq = Square::new_unchecked(self.their_king().0.trailing_zeros() as u8);

        if let Some(piece) = piece {
            if piece == Role::Knight {
                self.checkers |= get_knight_moves(ksq) & dest_bb;
            } else if piece == Role::Pawn {
                self.checkers |= get_pawn_attacks(ksq, self.side.opponent()) & dest_bb;
            }
        }

        let bishop_attackers = (self.our(Role::Bishop) | self.our(Role::Queen)) & bishop_rays(ksq);
        let rook_attackers = (self.our(Role::Rook) | self.our(Role::Queen)) & rook_rays(ksq);
        let attackers = bishop_attackers | rook_attackers;

        for sq in attackers {
            let btw = between(ksq, sq) & self.occupancy;

            if btw == Bitboard::EMPTY {
                self.checkers |= Bitboard::from(sq);
            } else if btw.count() == 1 {
                let them = self.them();
                self.pinned |= btw & them
            }
        }
    }

    pub fn refresh_checks_and_pins(&mut self) {
        // fully refresh checks and pins for the current side
        self.checkers = Bitboard::EMPTY;
        self.pinned = Bitboard::EMPTY;

        let ksq = Square::new_unchecked(self.our_king().0.trailing_zeros() as u8);

        let knight_attackers = self.their(Role::Knight) & get_knight_moves(ksq);
        let pawn_attackers = self.their(Role::Pawn) & get_pawn_attacks(ksq, self.side.opponent());

        self.checkers |= knight_attackers | pawn_attackers;

        let bishop_attackers =
            (self.their(Role::Bishop) | self.their(Role::Queen)) & bishop_rays(ksq);
        let rook_attackers = (self.their(Role::Rook) | self.their(Role::Queen)) & rook_rays(ksq);

        let attackers = bishop_attackers | rook_attackers;
        for sq in attackers {
            let btw = between(ksq, sq) & self.occupancy;
            if btw == Bitboard::EMPTY {
                self.checkers |= Bitboard::from(sq);
            } else if btw.count() == 1 {
                let us = self.us();
                self.pinned |= btw & us;
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{chess::position::fen::Fen, init};

    use super::*;

    #[test]
    fn test_repetitions() {
        init();

        let Fen(mut position) =
            Fen::parse("rnb1kbnr/1pqppppp/p7/2p5/4P3/2N2Q2/PPPP1PPP/R1B1KBNR w KQkq - 0 1")
                .unwrap();
        let move_str = "f3g3 d7d6 c3d5 c7d8 g1f3 e7e6 d5e3 g8f6 d2d3 b8c6 c1d2 g7g6 e1c1 f8g7 f1e2 e8g8 h2h3 d6d5 e4d5 e6d5 e3g4 f6g4 h3g4 f8e8 h1e1 h7h6 c1b1 c8e6 g4g5 d8b6 b2b3 h6h5 d2f4 a8c8 a2a4";

        let zero_repetition = "g7c3 f4d2 c3g7";
        let one_repetition = "d2f4 g7c3 f4d2 c3g7";
        let two_repetition = "d2f4";

        for mv in move_str.split_whitespace() {
            position.make_move(mv.parse::<Move>().unwrap());
            assert!(!position.is_repetition(1));
            assert!(!position.is_repetition(2));
        }

        for mv in zero_repetition.split_whitespace() {
            position.make_move(mv.parse::<Move>().unwrap());
            assert!(!position.is_repetition(1));
        }

        for mv in one_repetition.split_whitespace() {
            position.make_move(mv.parse::<Move>().unwrap());
            assert!(position.is_repetition(1));
        }

        for mv in two_repetition.split_whitespace() {
            position.make_move(mv.parse::<Move>().unwrap());
            assert!(position.is_repetition(2));
        }
    }
}
