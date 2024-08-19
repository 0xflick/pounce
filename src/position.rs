use std::num::NonZeroU32;

use crate::{
    bitboard::Bitboard,
    board::Board,
    chess::{CastleRights, Color, File, Piece, Role, Square},
    movegen::{between, bishop_rays, get_knight_moves, get_pawn_attacks, rook_rays},
    moves::Move,
};

#[derive(Debug, Clone, Copy)]
pub struct State {
    pub castling: CastleRights,
    pub ep_square: Option<Square>,
    pub halfmove_clock: u16,
    pub captured: Option<Piece>,
}

#[derive(Debug, Clone, Copy)]
pub struct Position {
    pub board: Board,
    pub side: Color,
    pub castling: CastleRights,
    pub ep_square: Option<Square>,
    pub halfmove_clock: u16,
    pub fullmove_number: NonZeroU32,
    // pub history: Vec<State>,
}

impl Position {
    pub fn new() -> Position {
        Position {
            board: Board::new(),
            side: Color::White,
            castling: CastleRights::all(),
            ep_square: None,
            halfmove_clock: 0,
            fullmove_number: NonZeroU32::new(1).unwrap(),
            // history: Vec::new(),
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
    pub fn us(&self) -> Bitboard {
        self.board.by_color(self.side)
    }

    #[inline]
    pub fn them(&self) -> Bitboard {
        self.board.by_color(self.side.opponent())
    }

    #[inline]
    pub fn our(&self, role: Role) -> Bitboard {
        self.board.by_color_role(self.side, role)
    }

    #[inline]
    pub fn their(&self, role: Role) -> Bitboard {
        self.board.by_color_role(self.side.opponent(), role)
    }

    #[inline]
    pub fn our_king(&self) -> Bitboard {
        self.board.king_of(self.side)
    }

    #[inline]
    pub fn their_king(&self) -> Bitboard {
        self.board.king_of(self.side.opponent())
    }

    #[inline]
    pub fn make_move(&mut self, mv: Move) {
        let from = mv.from();
        let to = mv.to();

        let piece = self.board.piece_at(from).unwrap();

        // do move and capture
        let captured = if piece.role == Role::Pawn && self.ep_square == Some(to) {
            // unwrapping is safe here because we know ep_square is never at the edge of the board
            let captured_pawn_square = to.down(self.side).unwrap();
            let captured = self.board.piece_at(captured_pawn_square);
            self.board.discard(from);
            self.board.discard(captured_pawn_square);
            self.board.set(to, piece);
            captured
        } else {
            let captured = self.board.piece_at(to);
            self.board.discard(from);
            self.board.set(to, piece);
            captured
        };

        if let Some(promotion) = mv.promotion() {
            let promoted = Piece::new(self.side, promotion);
            self.board.set(to, promoted);
        }

        // if a castling move, move the rook
        if piece.role == Role::King && from.file().direction(to.file()) == 2 {
            let rook_from = Square::make(File::H, self.side.back_rank());
            let rook_to = Square::make(File::F, self.side.back_rank());
            let rook = self.board.piece_at(rook_from).unwrap();
            self.board.discard(rook_from);
            self.board.set(rook_to, rook);
        } else if piece.role == Role::King && from.file().direction(to.file()) == -2 {
            let rook_from = Square::make(File::A, self.side.back_rank());
            let rook_to = Square::make(File::D, self.side.back_rank());
            let rook = self.board.piece_at(rook_from).unwrap();
            self.board.discard(rook_from);
            self.board.set(rook_to, rook);
        }

        // self.history.push(State {
        //     castling: self.castling,
        //     ep_square: self.ep_square,
        //     halfmove_clock: self.halfmove_clock,
        //     captured,
        // });

        self.update_checks_and_pins(mv, mv.promotion().unwrap_or(piece.role));

        // update our castling rights
        if piece.role == Role::King {
            self.castling.discard_color(self.side);
        } else if piece.role == Role::Rook {
            self.castling.discard_square(from);
        }

        // update their castling rights
        if let Some(captured) = captured {
            if captured.role == Role::Rook {
                self.castling.discard_square(to);
            }
        }

        // set ep
        self.ep_square = if piece.role == Role::Pawn
            && from.rank() == self.side.home_rank()
            && to.rank() == self.side.double_pawn_rank()
        {
            from.up(self.side)
        } else {
            None
        };

        self.side = self.side.opponent();
        self.fullmove_number = NonZeroU32::new(self.fullmove_number.get() + 1).unwrap();
        self.halfmove_clock += 1;
    }

    // pub fn unmake_move(&mut self, mv: Move) {
    //     self.side = self.side.opponent();
    //
    //     let past = self.history.pop().unwrap();
    //     self.castling = past.castling;
    //     self.ep_square = past.ep_square;
    //     self.halfmove_clock = past.halfmove_clock;
    //     self.fullmove_number = NonZeroU32::new(self.fullmove_number.get() - 1).unwrap();
    //
    //     let from = mv.from();
    //     let to = mv.to();
    //     let piece = self.board.piece_at(to).unwrap();
    //
    //     self.board.discard(to);
    //     self.board.set(from, piece);
    //     if let Some(captured) = past.captured {
    //         self.board.set(to, captured);
    //     }
    //
    //     self.update_checks_and_pins(mv.reverse(), piece.role);
    // }

    #[inline]
    fn update_checks_and_pins(&mut self, mv: Move, piece: Role) {
        // we update side at the very end of make move,
        // so we're checking agaings the opponent king

        *self.board.checkers_mut() = Bitboard::EMPTY;
        *self.board.pinned_mut() = Bitboard::EMPTY;

        let dest_bb = Bitboard::from(mv.to());

        let ksq = Square::new_unchecked(self.their_king().0.trailing_zeros() as u8);

        if piece == Role::Knight {
            *self.board.checkers_mut() |= get_knight_moves(ksq) & dest_bb;
        } else if piece == Role::Pawn {
            *self.board.checkers_mut() |= get_pawn_attacks(ksq, self.side.opponent()) & dest_bb;
        }

        let bishop_attackers = (self.our(Role::Bishop) | self.our(Role::Queen)) & bishop_rays(ksq);
        let rook_attackers = (self.our(Role::Rook) | self.our(Role::Queen)) & rook_rays(ksq);
        let attackers = bishop_attackers | rook_attackers;

        for sq in attackers {
            let btw = between(ksq, sq) & self.board.occupancy();

            if btw == Bitboard::EMPTY {
                *self.board.checkers_mut() |= Bitboard::from(sq);
            } else if btw.count() == 1 {
                let them = self.them();
                *self.board.pinned_mut() |= btw & them
            }
        }
    }

    pub fn refresh_checks_and_pins(&mut self) {
        // fully refresh checks and pins for the current side
        *self.board.checkers_mut() = Bitboard::EMPTY;
        *self.board.pinned_mut() = Bitboard::EMPTY;

        let ksq = Square::new_unchecked(self.our_king().0.trailing_zeros() as u8);

        let knight_attackers = self.their(Role::Knight) & get_knight_moves(ksq);
        let pawn_attackers = self.their(Role::Pawn) & get_pawn_attacks(ksq, self.side.opponent());

        *self.board.checkers_mut() |= knight_attackers | pawn_attackers;

        let bishop_attackers =
            (self.their(Role::Bishop) | self.their(Role::Queen)) & bishop_rays(ksq);
        let rook_attackers = (self.their(Role::Rook) | self.their(Role::Queen)) & rook_rays(ksq);

        let attackers = bishop_attackers | rook_attackers;
        for sq in attackers {
            let btw = between(ksq, sq) & self.board.occupancy();
            if btw == Bitboard::EMPTY {
                *self.board.checkers_mut() |= Bitboard::from(sq);
            } else if btw.count() == 1 {
                let us = self.us();
                *self.board.pinned_mut() |= btw & us;
            }
        }
    }
}
