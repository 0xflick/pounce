use arrayvec::ArrayVec;

use crate::{
    bitboard::Bitboard,
    chess::{Color, File, Role, Square},
    magic::{BISHOP_ATTACKS, ROOK_ATTACKS},
    magic_gen::{BISHOP_MAGICS, ROOK_MAGICS},
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

pub struct MoveGen {
    moves: MoveList,
    index: usize,
    promotion_index: PromotionIndex,
    iter_mask: Bitboard,
}

impl MoveGen {
    pub fn new(pos: &Position) -> Self {
        let mut moves = MoveList::new();
        let checkers = pos.board.checkers();

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
            let move_count = self.moves[i].moves.count();
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

pub static mut PAWN_MOVES: [[Bitboard; 64]; 2] = [[Bitboard::EMPTY; 64]; 2];
static mut PAWN_ATTACKS: [[Bitboard; 64]; 2] = [[Bitboard::EMPTY; 64]; 2];
static mut KNIGHT_MOVES: [Bitboard; 64] = [Bitboard::EMPTY; 64];
static mut KING_MOVES: [Bitboard; 64] = [Bitboard::EMPTY; 64];
static mut KINGSIDE_CASTLE: [Bitboard; 2] = [Bitboard::EMPTY; 2];
static mut QUEENSIDE_CASTLE: [Bitboard; 2] = [Bitboard::EMPTY; 2];
static mut BETWEEN: [[Bitboard; 64]; 64] = [[Bitboard::EMPTY; 64]; 64];
static mut LINE: [[Bitboard; 64]; 64] = [[Bitboard::EMPTY; 64]; 64];
static mut BISHOP_RAYS: [Bitboard; 64] = [Bitboard::EMPTY; 64];
static mut ROOK_RAYS: [Bitboard; 64] = [Bitboard::EMPTY; 64];

fn gen_pawn_move_table() {
    let mut moves = [[Bitboard::EMPTY; 64]; 2];
    let mut attacks = [[Bitboard::EMPTY; 64]; 2];

    for color in [Color::White, Color::Black].into_iter() {
        for sq in Square::ALL {
            let move_bb = &mut moves[color as usize][sq as usize];
            let attack_bb = &mut attacks[color as usize][sq as usize];
            if let Some(s) = sq.up(color) {
                move_bb.set(s);
                if let Some(l) = s.east() {
                    attack_bb.set(l);
                }
                if let Some(r) = s.west() {
                    attack_bb.set(r);
                }
            }

            if sq.rank() == color.home_rank() {
                if let Some(s) = sq.up(color).and_then(|s| s.up(color)) {
                    move_bb.set(s);
                }
            }
        }
    }

    unsafe {
        PAWN_MOVES = moves;
        PAWN_ATTACKS = attacks;
    }
}

#[rustfmt::skip]
fn gen_knight_move_table() {
    let mut moves = [Bitboard::EMPTY; 64];
    for sq in Square::ALL {
        let mut bb = Bitboard::EMPTY;
        // NNE, NEE 
        sq.north().and_then(|s| s.north().and_then(|s| s.east().map(|s| bb.set(s))));
        sq.north().and_then(|s| s.east().and_then(|s| s.east().map(|s| bb.set(s))));

        // NNW, NWW
        sq.north().and_then(|s| s.north().and_then(|s| s.west().map(|s| bb.set(s))));
        sq.north().and_then(|s| s.west().and_then(|s| s.west().map(|s| bb.set(s))));
        
        // SSE, SEE
        sq.south().and_then(|s| s.south().and_then(|s| s.east().map(|s| bb.set(s))));
        sq.south().and_then(|s| s.east().and_then(|s| s.east().map(|s| bb.set(s))));

        // SSW, SWW
        sq.south().and_then(|s| s.south().and_then(|s| s.west().map(|s| bb.set(s))));
        sq.south().and_then(|s| s.west().and_then(|s| s.west().map(|s| bb.set(s))));
        moves[sq as usize] = bb;
    }
    unsafe {
        KNIGHT_MOVES = moves;
    }
}

fn gen_king_move_table() {
    let mut moves = [Bitboard::EMPTY; 64];
    for sq in Square::ALL {
        let mut bb = Bitboard::EMPTY;
        if let Some(s) = sq.north() {
            bb.set(s)
        }
        if let Some(s) = sq.south() {
            bb.set(s)
        }
        if let Some(s) = sq.east() {
            bb.set(s)
        }
        if let Some(s) = sq.west() {
            bb.set(s)
        }
        sq.north().and_then(|s| s.east().map(|s| bb.set(s)));
        sq.north().and_then(|s| s.west().map(|s| bb.set(s)));
        sq.south().and_then(|s| s.east().map(|s| bb.set(s)));
        sq.south().and_then(|s| s.west().map(|s| bb.set(s)));
        moves[sq as usize] = bb;
    }
    unsafe {
        KING_MOVES = moves;
    }
}

fn gen_castle_table() {
    let mut kingside = [Bitboard::EMPTY; 2];
    let mut queenside = [Bitboard::EMPTY; 2];
    for color in [Color::White, Color::Black].into_iter() {
        let back_rank = color.back_rank();
        kingside[color as usize].set(Square::make(File::F, back_rank));
        kingside[color as usize].set(Square::make(File::G, back_rank));

        queenside[color as usize].set(Square::make(File::B, back_rank));
        queenside[color as usize].set(Square::make(File::C, back_rank));
        queenside[color as usize].set(Square::make(File::D, back_rank));
    }
    unsafe {
        KINGSIDE_CASTLE = kingside;
        QUEENSIDE_CASTLE = queenside;
    }
}

fn gen_between_table() {
    let mut between = [[Bitboard::EMPTY; 64]; 64];
    for from in Square::ALL {
        for to in Square::ALL {
            between[from as usize][to as usize] = gen_between(from, to)
        }
    }
    unsafe {
        BETWEEN = between;
    }
}

fn gen_between(from: Square, to: Square) -> Bitboard {
    let mut bb = Bitboard::EMPTY;

    let min_file = from.file().min(to.file());
    let max_file = from.file().max(to.file());

    let min_rank = from.rank().min(to.rank());
    let max_rank = from.rank().max(to.rank());

    for sq in Square::ALL {
        // same rank
        if sq.rank() == from.rank()
            && from.rank() == to.rank()
            && sq.file() > min_file
            && sq.file() < max_file
        {
            bb |= sq;
        }

        // same file
        if sq.file() == from.file()
            && from.file() == to.file()
            && sq.rank() > min_rank
            && sq.rank() < max_rank
        {
            bb |= sq;
        }

        // same diagonal
        if sq.rank().distance(from.rank()) == sq.file().distance(from.file())
            && from.rank().distance(to.rank()) == from.file().distance(to.file())
            && sq.rank() > min_rank
            && sq.rank() < max_rank
            && sq.file() > min_file
            && sq.file() < max_file
        {
            bb |= sq;
        }
    }

    bb
}

fn gen_line_table() {
    let mut line = [[Bitboard::EMPTY; 64]; 64];
    for from in Square::ALL {
        for to in Square::ALL {
            line[from as usize][to as usize] = if from == to {
                Bitboard::EMPTY
            } else {
                gen_line(from, to)
            };
        }
    }
    unsafe {
        LINE = line;
    }
}

fn gen_line(from: Square, to: Square) -> Bitboard {
    let mut bb = Bitboard::EMPTY;

    for sq in Square::ALL {
        // same rank
        if sq.rank() == from.rank() && from.rank() == to.rank() {
            bb |= sq;
        }

        // same file
        if sq.file() == from.file() && from.file() == to.file() {
            bb |= sq;
        }

        // same diagonal
        if (sq.rank().distance(from.rank()) == sq.file().distance(from.file()))
            && (sq.rank().distance(to.rank()) == sq.file().distance(to.file()))
            && (from.rank().distance(to.rank()) == from.file().distance(to.file()))
        {
            bb |= sq;
        }
    }

    bb
}

fn gen_rook_ray(sq: Square) -> Bitboard {
    let mut bb = Bitboard::EMPTY;

    let mut s = sq;
    while let Some(n) = s.north() {
        bb.set(n);
        s = n;
    }

    s = sq;
    while let Some(n) = s.south() {
        bb.set(n);
        s = n;
    }

    s = sq;
    while let Some(n) = s.east() {
        bb.set(n);
        s = n;
    }

    s = sq;
    while let Some(n) = s.west() {
        bb.set(n);
        s = n;
    }
    bb
}

fn gen_bishop_ray(sq: Square) -> Bitboard {
    let mut bb = Bitboard::EMPTY;

    let mut s = sq;
    while let Some(n) = s.north().and_then(|s| s.east()) {
        bb.set(n);
        s = n;
    }

    s = sq;
    while let Some(n) = s.north().and_then(|s| s.west()) {
        bb.set(n);
        s = n;
    }

    s = sq;
    while let Some(n) = s.south().and_then(|s| s.east()) {
        bb.set(n);
        s = n;
    }

    s = sq;
    while let Some(n) = s.south().and_then(|s| s.west()) {
        bb.set(n);
        s = n;
    }
    bb
}

fn gen_bishop_rays() {
    let mut rays = [Bitboard::EMPTY; 64];
    for sq in Square::ALL {
        rays[sq as usize] = gen_bishop_ray(sq);
    }
    unsafe {
        BISHOP_RAYS = rays;
    }
}

fn gen_rook_rays() {
    let mut rays = [Bitboard::EMPTY; 64];
    for sq in Square::ALL {
        rays[sq as usize] = gen_rook_ray(sq);
    }
    unsafe {
        ROOK_RAYS = rays;
    }
}

pub fn gen_all_tables() {
    gen_pawn_move_table();
    gen_knight_move_table();
    gen_king_move_table();
    gen_castle_table();
    gen_between_table();
    gen_line_table();
    gen_bishop_rays();
    gen_rook_rays();
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

    fn legal_moves<CH: CheckType, CO: ColorType>(pos: &Position, movelist: &mut MoveList) {
        let side = CO::COLOR;
        let ksq = Square::from(pos.board.king_of(side));
        let pieces = pos.board.by_color_role(side, Self::into_piece());
        let pinned = pos.board.pinned();
        let checkers = pos.board.checkers();

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

impl Mover for PawnType {
    fn into_piece() -> Role {
        Role::Pawn
    }

    #[inline]
    fn pseudo_legal_moves<CO: ColorType>(from: Square, pos: &Position) -> Bitboard {
        let mut bb = Bitboard::EMPTY;
        let side = CO::COLOR;
        // add single moves
        if from
            .up(side)
            .is_some_and(|s| pos.board.occupancy() & s == Bitboard::EMPTY)
        {
            bb |= get_pawn_moves(from, side);
            bb &= !pos.board.occupancy();
        }

        bb |= get_pawn_attacks(from, side) & pos.board.by_color(side.opponent());
        bb
    }

    #[inline]
    fn legal_moves<CH: CheckType, CO: ColorType>(pos: &Position, movelist: &mut MoveList) {
        let side = CO::COLOR;
        let ksq = Square::from(pos.board.king_of(side));
        let pieces = pos.board.by_color_role(side, Self::into_piece());
        let pinned = pos.board.pinned();
        let checkers = pos.board.checkers();

        let promotion_bb = Bitboard::from(side.opponent().home_rank());

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
                        is_promotion: promotion_bb & Bitboard::from(sq) != Bitboard::EMPTY,
                    });
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
                            is_promotion: promotion_bb & Bitboard::from(sq) != Bitboard::EMPTY,
                        });
                    }
                }
            }
        }

        if let Some(ep) = pos.ep_square {
            // en passant source squares are the same as the squares that any
            // enemy pawn could attack from the en passant square
            let ep_source_squares = get_pawn_attacks(ep, side.opponent()) & pos.our(Role::Pawn);
            for sq in ep_source_squares {
                if Self::legal_ep_move::<CO>(sq, ep, pos) {
                    unsafe {
                        movelist.push_unchecked(FromAndMoves {
                            from: sq,
                            moves: Bitboard::from(ep),
                            is_promotion: false,
                        });
                    }
                }
            }
        }
    }
}

impl PawnType {
    fn legal_ep_move<CO: ColorType>(from: Square, to: Square, pos: &Position) -> bool {
        let side = CO::COLOR;
        let ksq = Square::from(pos.board.king_of(side));
        let mask = pos.board.occupancy()
            ^ Bitboard::from(from) // unset the from square
            ^ Bitboard::from(to) // set the to square
            ^ Bitboard::from(to.down(side).unwrap()); // unset the captured pawn

        let rooks = (pos.board.by_role(Role::Rook) | pos.board.by_role(Role::Queen))
            & pos.board.by_color(side.opponent());
        let bishops = (pos.board.by_role(Role::Bishop) | pos.board.by_role(Role::Queen))
            & pos.board.by_color(side.opponent());

        let mut attackers = Bitboard::EMPTY;
        attackers |= get_rook_moves(ksq, mask) & rooks;
        attackers |= get_bishop_moves(ksq, mask) & bishops;
        attackers == Bitboard::EMPTY
    }
}

impl Mover for KnightType {
    fn into_piece() -> Role {
        Role::Knight
    }

    fn pseudo_legal_moves<CO: ColorType>(from: Square, pos: &Position) -> Bitboard {
        get_knight_moves(from) & !pos.board.by_color(CO::COLOR)
    }
}

impl Mover for BishopType {
    fn into_piece() -> Role {
        Role::Bishop
    }

    fn pseudo_legal_moves<CO: ColorType>(from: Square, pos: &Position) -> Bitboard {
        get_bishop_moves(from, pos.board.occupancy()) & !pos.board.by_color(CO::COLOR)
    }
}

impl Mover for RookType {
    fn into_piece() -> Role {
        Role::Rook
    }

    fn pseudo_legal_moves<CO: ColorType>(from: Square, pos: &Position) -> Bitboard {
        get_rook_moves(from, pos.board.occupancy()) & !pos.board.by_color(CO::COLOR)
    }
}

impl Mover for QueenType {
    fn into_piece() -> Role {
        Role::Queen
    }

    fn pseudo_legal_moves<CO: ColorType>(from: Square, pos: &Position) -> Bitboard {
        let rook_moves = RookType::pseudo_legal_moves::<CO>(from, pos);
        let bishop_moves = BishopType::pseudo_legal_moves::<CO>(from, pos);
        rook_moves | bishop_moves
    }
}

impl Mover for KingType {
    fn into_piece() -> Role {
        Role::King
    }

    fn pseudo_legal_moves<CO: ColorType>(from: Square, pos: &Position) -> Bitboard {
        unsafe { KING_MOVES[from as usize] & !pos.board.by_color(CO::COLOR) }
    }

    fn legal_moves<CH: CheckType, CO: ColorType>(pos: &Position, movelist: &mut MoveList) {
        let side = CO::COLOR;
        let ksq = Square::from(pos.board.king_of(side));

        let mut moves = Self::pseudo_legal_moves::<CO>(ksq, pos);
        for m in moves {
            if !Self::legal_king_move::<CO>(pos, m) {
                moves ^= Bitboard::from(m);
            }
        }

        if !CH::IN_CHECK {
            if pos.castling.can_castle_kingside(side)
                && (get_kingside_castle_through_squares(side) & pos.board.occupancy()).none()
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
                && (get_queenside_castle_throught_squares(side) & pos.board.occupancy()).none()
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
                movelist.push_unchecked(FromAndMoves {
                    from: ksq,
                    moves,
                    is_promotion: false,
                });
            }
        }
    }
}

impl KingType {
    pub fn legal_king_move<CO: ColorType>(pos: &Position, sq: Square) -> bool {
        let side = CO::COLOR;
        let mask = pos.board.occupancy() ^ pos.board.king_of(side);

        let mut attackers = Bitboard::EMPTY;
        let rooks = pos.board.by_color_role(side.opponent(), Role::Rook)
            | pos.board.by_color_role(side.opponent(), Role::Queen);

        attackers |= get_rook_moves(sq, mask) & rooks;
        if attackers != Bitboard::EMPTY {
            return false;
        }

        let bishops =
            pos.their(Role::Bishop) | pos.board.by_color_role(side.opponent(), Role::Queen);
        attackers |= get_bishop_moves(sq, mask) & bishops;
        if attackers != Bitboard::EMPTY {
            return false;
        }

        attackers |= get_knight_moves(sq) & pos.board.by_color_role(side.opponent(), Role::Knight);
        if attackers != Bitboard::EMPTY {
            return false;
        }

        attackers |=
            get_pawn_attacks(sq, side) & pos.board.by_color_role(side.opponent(), Role::Pawn);
        if attackers != Bitboard::EMPTY {
            return false;
        }

        true
    }
}

#[inline]
fn get_pawn_moves(sq: Square, color: Color) -> Bitboard {
    unsafe {
        *PAWN_MOVES
            .get_unchecked(color as usize)
            .get_unchecked(sq as usize)
    }
}

#[inline]
pub fn get_pawn_attacks(sq: Square, color: Color) -> Bitboard {
    unsafe {
        *PAWN_ATTACKS
            .get_unchecked(color as usize)
            .get_unchecked(sq as usize)
    }
}

#[inline]
pub fn get_rook_moves(sq: Square, occ: Bitboard) -> Bitboard {
    unsafe {
        let magic = ROOK_MAGICS.get_unchecked(sq as usize);
        let occ = occ & magic.mask;
        *ROOK_ATTACKS.get_unchecked(magic.index(occ))
    }
}

#[inline]
pub fn get_bishop_moves(sq: Square, occ: Bitboard) -> Bitboard {
    unsafe {
        let magic = BISHOP_MAGICS.get_unchecked(sq as usize);
        let occ = occ & magic.mask;
        *BISHOP_ATTACKS.get_unchecked(magic.index(occ))
    }
}

#[inline]
pub fn get_knight_moves(sq: Square) -> Bitboard {
    unsafe { *KNIGHT_MOVES.get_unchecked(sq as usize) }
}

#[inline]
pub fn between(from: Square, to: Square) -> Bitboard {
    unsafe {
        *BETWEEN
            .get_unchecked(from as usize)
            .get_unchecked(to as usize)
    }
}

#[inline]
pub fn line(from: Square, to: Square) -> Bitboard {
    unsafe { *LINE.get_unchecked(from as usize).get_unchecked(to as usize) }
}

#[inline]
pub fn bishop_rays(sq: Square) -> Bitboard {
    unsafe { *BISHOP_RAYS.get_unchecked(sq as usize) }
}

#[inline]
pub fn rook_rays(sq: Square) -> Bitboard {
    unsafe { *ROOK_RAYS.get_unchecked(sq as usize) }
}

#[inline]
pub fn get_kingside_castle_through_squares(color: Color) -> Bitboard {
    unsafe { *KINGSIDE_CASTLE.get_unchecked(color as usize) }
}

pub fn get_queenside_castle_throught_squares(color: Color) -> Bitboard {
    unsafe { *QUEENSIDE_CASTLE.get_unchecked(color as usize) }
}

pub fn perft(pos: Position, depth: u8) -> u64 {
    let mut total = 0;
    let mut mg = MoveGen::new(&pos);

    if depth == 0 {
        return 1;
    }

    if depth == 1 {
        return mg.len() as u64;
    }

    for m in &mut mg {
        let mut p_new = pos;
        p_new.make_move(m);
        total += perft(p_new, depth - 1);
        // pos.unmake_move(m)
    }
    total
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::fen::Fen;

    const NORMAL_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    const KIWIPETE_FEN: &str =
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq -  0 1";
    const POSITION_5_FEN: &str = "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8";

    #[test]
    fn perft_normal() {
        gen_all_tables();
        let Fen(position) = Fen::parse(NORMAL_FEN).unwrap();
        assert_eq!(perft(position, 2), 400);
        assert_eq!(perft(position, 3), 8902);
        assert_eq!(perft(position, 4), 197_281);
        assert_eq!(perft(position, 5), 4_865_609);
    }

    #[test]
    fn perft_kiwipete() {
        gen_all_tables();
        let Fen(position) = Fen::parse(KIWIPETE_FEN).unwrap();
        assert_eq!(perft(position, 1), 48);
        assert_eq!(perft(position, 2), 2_039);
        assert_eq!(perft(position, 3), 97_862);
        assert_eq!(perft(position, 4), 4_085_603);
    }

    #[test]
    fn perft_pos_5() {
        gen_all_tables();
        let Fen(position) = Fen::parse(POSITION_5_FEN).unwrap();
        assert_eq!(perft(position, 1), 44);
        assert_eq!(perft(position, 2), 1_486);
        assert_eq!(perft(position, 3), 62_379);
        assert_eq!(perft(position, 4), 2_103_487);
    }
}
