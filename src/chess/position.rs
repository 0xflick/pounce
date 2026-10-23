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

pub trait Accumulator {
    fn reset(&mut self, pos: &Position);

    fn on_make_move(&mut self, mv: Move);
    fn on_unmake_move(&mut self, mv: Move);

    fn on_make_move_set(&mut self, sq: Square, piece: Piece);
    fn on_make_move_discard(&mut self, sq: Square, piece: Piece);

    fn on_unmake_move_set(&mut self, sq: Square, piece: Piece);
    fn on_unmake_move_discard(&mut self, sq: Square, piece: Piece);
}

struct NoAccumulator;

impl Accumulator for NoAccumulator {
    fn reset(&mut self, _pos: &Position) {}

    fn on_make_move(&mut self, _mv: Move) {}
    fn on_unmake_move(&mut self, _mv: Move) {}

    fn on_make_move_set(&mut self, _sq: Square, _piece: Piece) {}
    fn on_make_move_discard(&mut self, _sq: Square, _piece: Piece) {}

    fn on_unmake_move_set(&mut self, _sq: Square, _piece: Piece) {}
    fn on_unmake_move_discard(&mut self, _sq: Square, _piece: Piece) {}
}

#[derive(Debug, Clone, Copy)]
pub struct State {
    pub castling: CastleRights,
    pub ep_square: Option<Square>,
    pub halfmove_clock: u8,
    pub captured: Option<Piece>,
    pub key: ZobristHash,
    pub pinned: [Bitboard; Color::NUM],
    pub checkers: [Bitboard; Color::NUM],
}

#[derive(Debug, Clone)]
pub struct Position {
    pub by_color: [Bitboard; Color::NUM],
    pub by_role: [Bitboard; Role::NUM],
    pub occupancy: Bitboard,
    pub checkers: [Bitboard; Color::NUM],
    pub pinned: [Bitboard; Color::NUM],

    pub mailbox: [Option<Piece>; 64],

    pub castling: CastleRights,
    pub ep_square: Option<Square>,

    pub side: Color,

    pub halfmove_clock: u8,
    pub fullmove_number: NonZeroU16,

    pub key: ZobristHash,

    pub history: Vec<State>,
}

impl Position {
    pub fn new() -> Position {
        Position {
            by_color: [Bitboard::EMPTY; Color::NUM],
            by_role: [Bitboard::EMPTY; Role::NUM],
            occupancy: Bitboard::EMPTY,
            checkers: [Bitboard::EMPTY; Color::NUM],
            pinned: [Bitboard::EMPTY; Color::NUM],
            mailbox: [None; 64],
            castling: CastleRights::all(),
            ep_square: None,
            side: Color::White,
            halfmove_clock: 0,
            fullmove_number: NonZeroU16::new(1).unwrap(),
            key: ZobristHash::new(),
            history: Vec::new(),
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
        self.checkers[self.side].any()
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
        self.by_color.iter_mut().for_each(|bb| bb.clear(sq));
        self.by_role.iter_mut().for_each(|bb| bb.clear(sq));
        self.occupancy.clear(sq);
        self.mailbox[sq] = None;
        self.key.toggle_piece(sq, piece);
    }

    #[inline]
    pub fn set(&mut self, sq: Square, piece: Piece) {
        debug_assert!(
            self.piece_at(sq).is_none(),
            "set() called on occupied square - use discard() first"
        );
        self.by_color[piece.color as usize].set(sq);
        self.by_role[piece.role as usize].set(sq);
        self.occupancy.set(sq);
        self.mailbox[sq] = Some(piece);
        self.key.toggle_piece(sq, piece);
    }

    #[inline]
    pub fn make_move(&mut self, mv: Move) {
        self.make_move_with(mv, &mut NoAccumulator);
    }

    #[inline]
    pub fn make_move_with<A: Accumulator>(&mut self, mv: Move, acc: &mut A) {
        acc.on_make_move(mv);

        let from = mv.from();
        let to = mv.to();

        let piece = self.piece_at(from).unwrap();
        debug_assert!(piece.color == self.side);
        let mut state = State {
            castling: self.castling,
            ep_square: self.ep_square,
            halfmove_clock: self.halfmove_clock,
            captured: None,
            key: self.key,
            pinned: self.pinned,
            checkers: self.checkers,
        };

        // reset the en passant square
        let prev_ep_square = self.ep_square;
        self.key.toggle_ep(self.ep_square);
        self.ep_square = None;

        self.halfmove_clock += 1;

        let mut potential_ep_sq = None;
        let mut ep_attackers = Bitboard::EMPTY;

        match mv.move_type(piece.role, prev_ep_square) {
            MoveType::Normal => {
                state.captured = self.piece_at(to);

                // Handle captured piece first
                if let Some(captured) = state.captured {
                    acc.on_make_move_discard(to, captured);
                    self.discard(to, captured);
                }

                // Move our piece
                acc.on_make_move_discard(from, piece);
                self.discard(from, piece);
                acc.on_make_move_set(to, piece);
                self.set(to, piece);
            }
            MoveType::DoublePawnPush => {
                acc.on_make_move_discard(from, piece);
                self.discard(from, piece);
                acc.on_make_move_set(to, piece);
                self.set(to, piece);

                potential_ep_sq = from.up(self.side);

                ep_attackers =
                    get_pawn_attacks(potential_ep_sq.unwrap(), self.side) & self.their(Role::Pawn);

                // we handle setting the ep square below, after we've updated the checkers and pins
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

                acc.on_make_move_discard(from, piece);
                acc.on_make_move_discard(captured_pawn_square, state.captured.unwrap());
                acc.on_make_move_set(to, piece);
            }
            MoveType::Castle => {
                self.halfmove_clock = 0;
                if from.file().direction(to.file()) == 2 {
                    let rook_from = Square::make(File::H, self.side.back_rank());
                    let rook_to = Square::make(File::F, self.side.back_rank());
                    let rook = self.piece_at(rook_from).unwrap();
                    self.discard(rook_from, rook);
                    self.set(rook_to, rook);

                    acc.on_make_move_discard(rook_from, rook);
                    acc.on_make_move_set(rook_to, rook);
                } else {
                    let rook_from = Square::make(File::A, self.side.back_rank());
                    let rook_to = Square::make(File::D, self.side.back_rank());
                    let rook = self.piece_at(rook_from).unwrap();
                    self.discard(rook_from, rook);
                    self.set(rook_to, rook);

                    acc.on_make_move_discard(rook_from, rook);
                    acc.on_make_move_set(rook_to, rook);
                }
                self.discard(from, piece);
                self.set(to, piece);

                acc.on_make_move_discard(from, piece);
                acc.on_make_move_set(to, piece);
            }
            MoveType::Promotion => {
                state.captured = self.piece_at(to);
                let promoted = Piece::new(self.side, mv.promotion().unwrap());

                // Handle captured piece first
                if let Some(captured) = state.captured {
                    acc.on_make_move_discard(to, captured);
                    self.discard(to, captured);
                }

                // Move our piece
                acc.on_make_move_discard(from, piece);
                self.discard(from, piece);
                acc.on_make_move_set(to, promoted);
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
        if let Some(captured) = state.captured
            && captured.role == Role::Rook
        {
            self.key.toggle_castling(self.castling);
            self.castling.discard_square(to);
            self.key.toggle_castling(self.castling);
        }

        self.update_checks_and_pins(mv, Some(mv.promotion().unwrap_or(piece.role)));

        self.history.push(state);
        if self.side == Color::Black {
            self.fullmove_number = self.fullmove_number.saturating_add(1);
        }

        self.side = self.side.opponent();
        self.key.toggle_side();

        // handle ep square here (after updating checks and pins)
        // only set ep square if the attacker if the ep move is legal
        if let Some(ep_sq) = potential_ep_sq
            && (ep_attackers & !self.pinned[self.side]).any()
            && (!self.checkers[self.side].any()
                || self.checkers[self.side] == Bitboard::from(ep_sq.down(self.side).unwrap()))
        {
            self.ep_square = Some(ep_sq);
            self.key.toggle_ep(self.ep_square);
        }
    }

    #[inline]
    pub fn unmake_move(&mut self, mv: Move) {
        self.unmake_move_with(mv, &mut NoAccumulator);
    }

    #[inline]
    pub fn unmake_move_with<A: Accumulator>(&mut self, mv: Move, acc: &mut A) {
        acc.on_unmake_move(mv);

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

                acc.on_unmake_move_discard(to, piece);
                acc.on_unmake_move_set(from, piece);

                if let Some(captured) = past.captured {
                    self.set(to, captured);
                    acc.on_unmake_move_set(to, captured);
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

                acc.on_unmake_move_discard(to, piece);
                acc.on_unmake_move_set(from, piece);
                acc.on_unmake_move_set(captured_pawn_square, captured_pawn);
            }
            MoveType::Castle => {
                if from.file().direction(to.file()) == 2 {
                    let rook_from = Square::make(File::H, self.side.back_rank());
                    let rook_to = Square::make(File::F, self.side.back_rank());
                    let rook = self.piece_at(rook_to).expect("castling always has a rook");
                    self.discard(rook_to, rook);
                    self.set(rook_from, rook);

                    acc.on_unmake_move_discard(rook_to, rook);
                    acc.on_unmake_move_set(rook_from, rook);
                } else {
                    let rook_from = Square::make(File::A, self.side.back_rank());
                    let rook_to = Square::make(File::D, self.side.back_rank());
                    let rook = self.piece_at(rook_to).expect("castling always has a rook");
                    self.discard(rook_to, rook);
                    self.set(rook_from, rook);

                    acc.on_unmake_move_discard(rook_to, rook);
                    acc.on_unmake_move_set(rook_from, rook);
                }
                self.discard(to, piece);
                self.set(from, piece);

                acc.on_unmake_move_discard(to, piece);
                acc.on_unmake_move_set(from, piece);
            }
            MoveType::Promotion => {
                let promoted = Piece::new(self.side, mv.promotion().unwrap());
                self.discard(to, promoted);
                self.set(from, Piece::new(self.side, Role::Pawn));

                acc.on_unmake_move_discard(to, promoted);
                acc.on_unmake_move_set(from, Piece::new(self.side, Role::Pawn));

                if let Some(captured) = past.captured {
                    self.set(to, captured);
                    acc.on_unmake_move_set(to, captured);
                }
            }
        }
    }

    #[inline]
    pub fn make_null_move(&mut self) {
        self.make_null_move_with(&mut NoAccumulator);
    }

    #[inline]
    pub fn make_null_move_with<A: Accumulator>(&mut self, acc: &mut A) {
        acc.on_make_move(Move::NULL);

        let state = State {
            castling: self.castling,
            ep_square: self.ep_square,
            halfmove_clock: self.halfmove_clock,
            captured: None,
            key: self.key,
            pinned: self.pinned,
            checkers: self.checkers,
        };

        debug_assert!(self.checkers[self.side].none());

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

    #[inline]
    pub fn unmake_null_move(&mut self) {
        self.unmake_null_move_with(&mut NoAccumulator);
    }

    #[inline]
    pub fn unmake_null_move_with<A: Accumulator>(&mut self, acc: &mut A) {
        acc.on_unmake_move(Move::NULL);

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
        // Clear all checks and pins - we'll recalculate
        self.checkers = [Bitboard::EMPTY; Color::NUM];
        self.pinned = [Bitboard::EMPTY; Color::NUM];

        // Get both kings' positions
        let our_king_bb = self.our_king();
        let their_king_bb = self.their_king();
        // Mask to 6 bits so a king-less bitboard (pathological / corrupted
        // state) produces Square::A1 instead of an invalid `Square` enum
        // value, which would be UB when used as an array index.
        let our_ksq = Square::new_unchecked((our_king_bb.0.trailing_zeros() & 0x3F) as u8);
        let their_ksq = Square::new_unchecked((their_king_bb.0.trailing_zeros() & 0x3F) as u8);

        // Check if we're directly checking the opponent with the piece we just moved
        let dest_bb = Bitboard::from(mv.to());

        if let Some(piece_role) = piece {
            match piece_role {
                Role::Knight => {
                    self.checkers[self.side.opponent()] |= get_knight_moves(their_ksq) & dest_bb;
                }
                Role::Pawn => {
                    self.checkers[self.side.opponent()] |=
                        get_pawn_attacks(their_ksq, self.side.opponent()) & dest_bb;
                }
                _ => {} // Sliding pieces handled below
            }
        }

        // Update sliding attacks for both kings in a single loop
        for (ksq, king_color) in [(our_ksq, self.side), (their_ksq, self.side.opponent())] {
            let opponent = king_color.opponent();

            let bishop_attackers = (self.by_color_role(opponent, Role::Bishop)
                | self.by_color_role(opponent, Role::Queen))
                & bishop_rays(ksq);
            let rook_attackers = (self.by_color_role(opponent, Role::Rook)
                | self.by_color_role(opponent, Role::Queen))
                & rook_rays(ksq);

            for sq in bishop_attackers | rook_attackers {
                let btw = between(ksq, sq) & self.occupancy;

                if btw == Bitboard::EMPTY {
                    self.checkers[king_color] |= Bitboard::from(sq);
                } else if btw.count() == 1 {
                    self.pinned[king_color] |= btw & self.by_color[king_color];
                }
            }

            // Add non-sliding attacks
            let knight_attackers =
                self.by_color_role(opponent, Role::Knight) & get_knight_moves(ksq);
            let pawn_attackers =
                self.by_color_role(opponent, Role::Pawn) & get_pawn_attacks(ksq, king_color);

            self.checkers[king_color] |= knight_attackers | pawn_attackers;
        }
    }

    pub fn refresh_checks_and_pins(&mut self) {
        // Clear all checks and pins
        self.checkers = [Bitboard::EMPTY; Color::NUM];
        self.pinned = [Bitboard::EMPTY; Color::NUM];

        // Update checks and pins for each color
        for color in [Color::White, Color::Black] {
            let king_bb = self.by_color_role(color, Role::King);
            let ksq = Square::new_unchecked((king_bb.0.trailing_zeros() & 0x3F) as u8);
            let opponent = color.opponent();

            // Direct attacks (knights and pawns)
            let knight_attackers =
                self.by_color_role(opponent, Role::Knight) & get_knight_moves(ksq);
            let pawn_attackers =
                self.by_color_role(opponent, Role::Pawn) & get_pawn_attacks(ksq, color);

            self.checkers[color] = knight_attackers | pawn_attackers;

            // Sliding piece attacks (bishops, rooks, queens)
            let bishop_attackers = (self.by_color_role(opponent, Role::Bishop)
                | self.by_color_role(opponent, Role::Queen))
                & bishop_rays(ksq);
            let rook_attackers = (self.by_color_role(opponent, Role::Rook)
                | self.by_color_role(opponent, Role::Queen))
                & rook_rays(ksq);

            for sq in bishop_attackers | rook_attackers {
                let btw = between(ksq, sq) & self.occupancy;

                if btw == Bitboard::EMPTY {
                    self.checkers[color] |= Bitboard::from(sq);
                } else if btw.count() == 1 {
                    self.pinned[color] |= btw & self.by_color[color];
                }
            }
        }
    }

    pub fn is_quiet(&self) -> bool {
        if self.in_check() {
            return false;
        }

        let mg = MoveGen::new(self);
        for mv in mg {
            if mv.is_capture(self) {
                return false;
            }
        }

        true
    }

    /// Lightweight pseudo-legality check for moves from untrusted sources (e.g., TT).
    /// Returns true if the move looks reasonable, false if obviously corrupt.
    /// This is NOT a full legality check - it's just to catch corrupted data.
    pub fn is_pseudo_legal(&self, mv: Move) -> bool {
        // Reject sentinel values
        if mv == Move::NONE || mv == Move::NULL {
            return false;
        }

        let from = mv.from();
        let to = mv.to();

        // Reject moves where from == to
        if from == to {
            return false;
        }

        // Check that we have a piece of our color at the from square
        let piece = match self.piece_at(from) {
            Some(p) if p.color == self.side => p,
            _ => return false,
        };

        // Don't capture our own pieces
        if let Some(captured) = self.piece_at(to)
            && captured.color == self.side
        {
            return false;
        }

        // Check validity based on move type
        let move_type = mv.move_type(piece.role, self.ep_square);

        match move_type {
            MoveType::Normal => self.is_valid_normal_move(piece, from, to),

            MoveType::EnPassant => {
                piece.role == Role::Pawn
                    && self.ep_square == Some(to)
                    && get_pawn_attacks(from, self.side).contains(to)
            }

            MoveType::DoublePawnPush => {
                piece.role == Role::Pawn
                    && from.file() == to.file()
                    && match self.side {
                        Color::White => {
                            from.rank() == crate::chess::board::Rank::R2
                                && to.rank() == crate::chess::board::Rank::R4
                        }
                        Color::Black => {
                            from.rank() == crate::chess::board::Rank::R7
                                && to.rank() == crate::chess::board::Rank::R5
                        }
                    }
            }

            MoveType::Castle => {
                piece.role == Role::King
                    && from.rank() == self.side.back_rank()
                    && to.rank() == self.side.back_rank()
                    && from.file().distance(to.file()) == 2
            }

            MoveType::Promotion => {
                piece.role == Role::Pawn
                    && to.rank() == self.side.opponent().back_rank()
                    && matches!(
                        mv.promotion(),
                        Some(Role::Queen | Role::Rook | Role::Bishop | Role::Knight)
                    )
                    && from.file().distance(to.file()) <= 1
            }
        }
    }

    fn is_valid_normal_move(&self, piece: Piece, from: Square, to: Square) -> bool {
        match piece.role {
            Role::Pawn => {
                // Check if it's a diagonal attack (must be capturing)
                if get_pawn_attacks(from, self.side).contains(to) {
                    self.piece_at(to).is_some()
                } else {
                    // Must be a forward push (already checked it's not double push)
                    from.up(self.side) == Some(to) && self.piece_at(to).is_none()
                }
            }

            Role::Knight => get_knight_moves(from).contains(to),

            Role::Bishop => {
                bishop_rays(from).contains(to) && (between(from, to) & self.occupancy).none()
            }

            Role::Rook => {
                rook_rays(from).contains(to) && (between(from, to) & self.occupancy).none()
            }

            Role::Queen => {
                (bishop_rays(from) | rook_rays(from)).contains(to)
                    && (between(from, to) & self.occupancy).none()
            }

            Role::King => {
                // One square in any direction (castling handled separately)
                from.rank().distance(to.rank()) <= 1 && from.file().distance(to.file()) <= 1
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::chess::position::fen::Fen;
    use crate::init;

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

    #[test]
    fn test_en_passant_legal() {
        init();

        let Fen(mut position) =
            Fen::parse("rnbq1bnr/pp1ppppp/2k5/8/2p5/8/PP1PPPPP/2Q1KBNR w K - 0 1").unwrap();

        assert_eq!(position.ep_square, None);
        position.make_move("b2b4".parse::<Move>().unwrap());

        assert_eq!(position.ep_square, None);
    }

    #[test]
    fn test_en_passant_not_allowed_while_checked() {
        init();

        let Fen(mut position) =
            Fen::parse("rnbq1bnr/pp1ppppp/8/8/2p5/8/QPkPPPPP/4KBNR w K - 0 1").unwrap();

        assert_eq!(position.ep_square, None);
        position.make_move("b2b4".parse::<Move>().unwrap());

        assert_eq!(position.ep_square, None);
    }

    #[test]
    fn test_en_passant_allowed_while_checked() {
        init();

        let Fen(mut position) =
            Fen::parse("rnbq1bnr/pp1ppppp/8/2k5/2p5/8/QP1PPPPP/4KBNR w K - 0 1").unwrap();

        assert_eq!(position.ep_square, None);
        position.make_move("b2b4".parse::<Move>().unwrap());

        assert_eq!(position.ep_square, Some(Square::B3));
    }

    #[test]
    fn test_pinned() {
        init();

        let Fen(mut pos) =
            Fen::parse("rnbqkbn1/p4ppp/4r3/8/4Q3/8/PPP1RPPP/1NB1KBNR w Kq - 0 1").unwrap();

        assert_eq!(pos.pinned[Color::White], Bitboard::EMPTY);
        assert_eq!(pos.pinned[Color::Black].count(), 1);
        assert!(pos.pinned[Color::Black].contains(Square::E6));

        pos.make_move("e4c6".parse::<Move>().unwrap());
        assert_eq!(pos.pinned[Color::Black].count(), 1);
        assert_eq!(pos.checkers[Color::Black].count(), 1);
        assert!(pos.checkers[Color::Black].contains(Square::C6));

        pos.make_move("d8d7".parse::<Move>().unwrap());
        assert_eq!(pos.pinned[Color::White].count(), 1);
        assert!(pos.pinned[Color::White].contains(Square::E2));

        pos.unmake_move("d8d7".parse::<Move>().unwrap());
        assert_eq!(pos.pinned[Color::Black].count(), 1);
        assert_eq!(pos.checkers[Color::Black].count(), 1);
        assert!(pos.checkers[Color::Black].contains(Square::C6));

        pos.unmake_move("e4c6".parse::<Move>().unwrap());
        assert_eq!(pos.pinned[Color::White], Bitboard::EMPTY);
        assert_eq!(pos.pinned[Color::Black].count(), 1);
        assert!(pos.pinned[Color::Black].contains(Square::E6));
    }
}
