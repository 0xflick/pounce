use std::num::NonZeroU32;

use crate::{
    bitboard::Bitboard,
    chess::{CastleRights, Color, File, GameResult, Piece, Role, Square},
    eval::{PSQT_EG, PSQT_MG},
    movegen::{between, bishop_rays, get_knight_moves, get_pawn_attacks, rook_rays, MoveGen},
    moves::{Move, MoveType},
    zobrist::ZobristHash,
};

#[derive(Debug, Clone, Copy)]
pub struct State {
    pub castling: CastleRights,
    pub ep_square: Option<Square>,
    pub halfmove_clock: u16,
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

    pub castling: CastleRights,
    pub ep_square: Option<Square>,

    pub side: Color,

    pub halfmove_clock: u16,
    pub fullmove_number: NonZeroU32,

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
            castling: CastleRights::all(),
            ep_square: None,
            side: Color::White,
            halfmove_clock: 0,
            fullmove_number: NonZeroU32::new(1).unwrap(),
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
        self.by_color
            .iter()
            .position(|bb| bb.contains(sq))
            .map(|idx| Color::new(idx as u8))
    }

    #[inline]
    pub fn role_at(&self, sq: Square) -> Option<Role> {
        if self.occupancy.contains(sq) {
            self.by_role
                .iter()
                .position(|bb| bb.contains(sq))
                .map(|idx| Role::new(idx as u8))
        } else {
            None
        }
    }

    #[inline]
    pub fn piece_at(&self, sq: Square) -> Option<Piece> {
        self.role_at(sq).map(|role| Piece {
            color: self.color_at(sq).unwrap(),
            role,
        })
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
        self.key.toggle_piece(sq, piece);
    }

    #[inline]
    pub fn make_move(&mut self, mv: Move) {
        let from = mv.from();
        let to = mv.to();

        let piece = self.piece_at(from).unwrap();
        let mut state = State {
            castling: self.castling,
            ep_square: self.ep_square,
            halfmove_clock: self.halfmove_clock + 1,
            captured: None,
            checkers: self.checkers,
            pinned: self.pinned,
            key: self.key,
        };

        let prev_ep_square = self.ep_square;

        self.key.toggle_ep(self.ep_square);
        self.ep_square = None;

        // reset the en passant square

        match mv.move_type(piece.role, prev_ep_square) {
            MoveType::Normal => {
                state.captured = self.piece_at(to);
                self.discard(from, piece);
                self.set(to, piece);
            }
            MoveType::DoublePawnPush => {
                self.discard(from, piece);
                self.set(to, piece);

                self.ep_square = Some(from.up(self.side).unwrap());
                self.key.toggle_ep(self.ep_square);
            }
            MoveType::EnPassant => {
                // unwrapping is safe here because we know ep_square is never at the edge of the board
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
                state.halfmove_clock = 0;
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
            state.halfmove_clock = 0;
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

        self.update_checks_and_pins(mv, mv.promotion().unwrap_or(piece.role));

        self.history.push(state);
        self.fullmove_number = NonZeroU32::new(self.fullmove_number.get() + 1).unwrap();
        self.halfmove_clock += 1;

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

        self.key.toggle_castling(self.castling);
        self.castling = past.castling;
        self.key.toggle_castling(self.castling);

        self.key.toggle_ep(self.ep_square);
        self.ep_square = past.ep_square;
        self.key.toggle_ep(self.ep_square);

        self.halfmove_clock = past.halfmove_clock;
        self.fullmove_number = NonZeroU32::new(self.fullmove_number.get() - 1).unwrap();
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

    #[inline]
    fn update_checks_and_pins(&mut self, mv: Move, piece: Role) {
        // we update side at the very end of make move, so we're looking for checks
        // we make against the opponent
        self.checkers = Bitboard::EMPTY;
        self.pinned = Bitboard::EMPTY;

        let dest_bb = Bitboard::from(mv.to());

        let ksq = Square::new_unchecked(self.their_king().0.trailing_zeros() as u8);

        if piece == Role::Knight {
            self.checkers |= get_knight_moves(ksq) & dest_bb;
        } else if piece == Role::Pawn {
            self.checkers |= get_pawn_attacks(ksq, self.side.opponent()) & dest_bb;
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
