use arrayvec::ArrayVec;

use crate::{
    bitboard::Bitboard,
    chess::{Color, File, Rank, Role, Square},
    magic::{BISHOP_ATTACKS, ROOK_ATTACKS},
    magic_gen::{BISHOP_MAGICS, ROOK_MAGICS},
    moves::Move,
    position::Position,
};

pub type MoveList = ArrayVec<FromAndMoves, 20>;
pub struct FromAndMoves {
    from: Square,
    moves: Bitboard,
}

pub struct MoveGen {
    moves: MoveList,
    index: usize,
    iter_mask: Bitboard,
}

impl MoveGen {
    pub fn new(pos: &Position) -> Self {
        let mut moves = MoveList::new();

        let checkers = pos.board.checkers();

        if checkers == Bitboard::EMPTY {
            PawnType::legal_moves::<NotCheck>(pos, &mut moves);
            KnightType::legal_moves::<NotCheck>(pos, &mut moves);
            BishopType::legal_moves::<NotCheck>(pos, &mut moves);
            RookType::legal_moves::<NotCheck>(pos, &mut moves);
            QueenType::legal_moves::<NotCheck>(pos, &mut moves);
            KingType::legal_moves::<NotCheck>(pos, &mut moves);
        } else if checkers.count() == 1 {
            PawnType::legal_moves::<InCheck>(pos, &mut moves);
            KnightType::legal_moves::<InCheck>(pos, &mut moves);
            BishopType::legal_moves::<InCheck>(pos, &mut moves);
            RookType::legal_moves::<InCheck>(pos, &mut moves);
            QueenType::legal_moves::<InCheck>(pos, &mut moves);
            KingType::legal_moves::<InCheck>(pos, &mut moves);
        } else {
            KingType::legal_moves::<InCheck>(pos, &mut moves);
        }

        MoveGen {
            moves,
            index: 0,
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
            res += self.moves[i].moves.count();
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

pub static mut PAWN_MOVES: [[Bitboard; 64]; 2] = [[Bitboard::EMPTY; 64]; 2];
static mut PAWN_DOUBLES: [[Bitboard; 64]; 2] = [[Bitboard::EMPTY; 64]; 2];
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
    let mut doubles = [[Bitboard::EMPTY; 64]; 2];
    let mut attacks = [[Bitboard::EMPTY; 64]; 2];

    for color in [Color::White, Color::Black].into_iter() {
        for sq in Square::ALL {
            if sq.rank() == color.back_rank() {
                continue;
            }
            let move_bb = &mut moves[color as usize][sq as usize];
            let double_bb = &mut doubles[color as usize][sq as usize];
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
                    double_bb.set(s);
                }
            }
        }
    }

    unsafe {
        PAWN_MOVES = moves;
        PAWN_DOUBLES = doubles;
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
        let home_rank = color.home_rank();
        kingside[color as usize].set(Square::make(File::F, home_rank));
        kingside[color as usize].set(Square::make(File::G, home_rank));

        queenside[color as usize].set(Square::make(File::B, home_rank));
        queenside[color as usize].set(Square::make(File::C, home_rank));
        queenside[color as usize].set(Square::make(File::D, home_rank));
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
        if sq.rank().distance(from.rank()) == sq.file().distance(from.file())
            && from.rank().distance(to.rank()) == from.file().distance(to.file())
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

struct PawnType;
struct KnightType;
struct BishopType;
struct RookType;
struct QueenType;
struct KingType;

pub trait Mover {
    fn into_piece() -> Role;

    fn pseudo_legal_moves(from: Square, pos: &Position) -> Bitboard;

    fn legal_moves<T: CheckType>(pos: &Position, movelist: &mut MoveList) {
        if pos.our_king() == Bitboard::EMPTY {
            panic!("No king");
        }
        let ksq = Square::from(pos.our_king());
        let pinned = pos.board.pinned();
        let checkers = pos.board.checkers();

        let check_mask = if T::IN_CHECK {
            between(Square::from(checkers), ksq) ^ checkers
        } else {
            Bitboard::FULL
        };

        for sq in pos.our(Self::into_piece()) & !pinned {
            let moves = Self::pseudo_legal_moves(sq, pos) & check_mask;

            if moves != Bitboard::EMPTY {
                unsafe {
                    movelist.push_unchecked(FromAndMoves { from: sq, moves });
                }
            }
        }

        if !T::IN_CHECK {
            for sq in pos.our(Self::into_piece()) & pinned {
                let moves = Self::pseudo_legal_moves(sq, pos) & line(ksq, sq);
                if moves != Bitboard::EMPTY {
                    unsafe {
                        movelist.push_unchecked(FromAndMoves { from: sq, moves });
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
    fn pseudo_legal_moves(from: Square, pos: &Position) -> Bitboard {
        let mut bb = Bitboard::EMPTY;
        let single = get_pawn_single_moves(from, pos);
        let double = get_pawn_double_moves(from, pos);

        bb |= single & !pos.board.occupancy();
        bb |= double & !pos.board.occupancy() & !pos.board.occupancy().down(pos.side);

        bb |= get_pawn_attacks(from, pos.side, pos.them());
        bb
    }

    fn legal_moves<T: CheckType>(pos: &Position, movelist: &mut MoveList) {
        if pos.our_king() == Bitboard::EMPTY {
            println!("{:?}", pos);
            panic!("No king");
        };
        let ksq = Square::from(pos.our_king());
        let pinned = pos.board.pinned();
        let checkers = pos.board.checkers();

        let check_mask = if T::IN_CHECK {
            between(Square::from(checkers), ksq) ^ checkers
        } else {
            Bitboard::FULL
        };

        for sq in pos.our(Self::into_piece()) & !pinned {
            let moves = Self::pseudo_legal_moves(sq, pos) & check_mask;

            if moves != Bitboard::EMPTY {
                unsafe {
                    movelist.push_unchecked(FromAndMoves { from: sq, moves });
                }
            }
        }

        if !T::IN_CHECK {
            for sq in pos.our(Self::into_piece()) & pinned {
                let moves = Self::pseudo_legal_moves(sq, pos) & line(ksq, sq);
                if moves != Bitboard::EMPTY {
                    unsafe {
                        movelist.push_unchecked(FromAndMoves { from: sq, moves });
                    }
                }
            }
        }

        if let Some(ep) = pos.ep_square {
            for sq in get_ep_srcs(ep, pos) {
                if Self::legal_ep_move(sq, ep, pos) {
                    unsafe {
                        movelist.push_unchecked(FromAndMoves {
                            from: sq,
                            moves: Bitboard::from(ep),
                        });
                    }
                }
            }
        }
    }
}

impl PawnType {
    fn legal_ep_move(from: Square, to: Square, pos: &Position) -> bool {
        let color = pos.side;
        let ksq = Square::from(pos.our_king());
        let mask = pos.board.occupancy()
            ^ Bitboard::from(from) // unset the from square
            ^ Bitboard::from(to) // set the to square
            ^ Bitboard::from(to.down(color).unwrap()); // unset the captured pawn

        let rooks = pos.their(Role::Rook) | pos.their(Role::Queen);
        let bishops = pos.their(Role::Bishop) | pos.their(Role::Queen);

        let mut attackers = Bitboard::EMPTY;
        attackers |= get_rook_moves(ksq, pos, mask) & rooks;
        attackers |= get_bishop_moves(ksq, pos, mask) & bishops;
        attackers == Bitboard::EMPTY
    }
}

impl Mover for KnightType {
    fn into_piece() -> Role {
        Role::Knight
    }

    fn pseudo_legal_moves(from: Square, pos: &Position) -> Bitboard {
        get_knight_moves(from) & !pos.us()
    }
}

impl Mover for BishopType {
    fn into_piece() -> Role {
        Role::Bishop
    }

    fn pseudo_legal_moves(from: Square, pos: &Position) -> Bitboard {
        get_bishop_moves(from, pos, pos.board.occupancy())
    }
}

impl Mover for RookType {
    fn into_piece() -> Role {
        Role::Rook
    }

    fn pseudo_legal_moves(from: Square, pos: &Position) -> Bitboard {
        get_rook_moves(from, pos, pos.board.occupancy())
    }
}

impl Mover for QueenType {
    fn into_piece() -> Role {
        Role::Queen
    }

    fn pseudo_legal_moves(from: Square, pos: &Position) -> Bitboard {
        let rook_moves = RookType::pseudo_legal_moves(from, pos);
        let bishop_moves = BishopType::pseudo_legal_moves(from, pos);
        rook_moves | bishop_moves
    }
}

impl Mover for KingType {
    fn into_piece() -> Role {
        Role::King
    }

    fn pseudo_legal_moves(from: Square, pos: &Position) -> Bitboard {
        unsafe { KING_MOVES[from as usize] & !pos.us() }
    }

    fn legal_moves<T: CheckType>(pos: &Position, movelist: &mut MoveList) {
        if pos.our_king() == Bitboard::EMPTY {
            panic!("No king");
        }
        let ksq = Square::from(pos.our_king());

        let mut moves = Self::pseudo_legal_moves(ksq, pos);
        for m in moves {
            if !Self::legal_king_move(pos, m) {
                moves ^= Bitboard::from(m);
            }
        }

        if moves != Bitboard::EMPTY {
            unsafe {
                movelist.push_unchecked(FromAndMoves { from: ksq, moves });
            }
        }
    }
}

impl KingType {
    pub fn legal_king_move(pos: &Position, sq: Square) -> bool {
        let mask = pos.board.occupancy() ^ pos.our_king();

        let mut attackers = Bitboard::EMPTY;
        let rooks = pos.their(Role::Rook) | pos.their(Role::Queen);

        attackers |= get_rook_moves(sq, pos, mask) & rooks;
        if attackers != Bitboard::EMPTY {
            return false;
        }

        let bishops = pos.their(Role::Bishop) | pos.their(Role::Queen);
        attackers |= get_bishop_moves(sq, pos, mask) & bishops;
        if attackers != Bitboard::EMPTY {
            return false;
        }

        attackers |= KnightType::pseudo_legal_moves(sq, pos) & pos.their(Role::Knight);
        if attackers != Bitboard::EMPTY {
            return false;
        }

        attackers |= PawnType::pseudo_legal_moves(sq, pos) & pos.their(Role::Pawn);
        if attackers != Bitboard::EMPTY {
            return false;
        }

        true
    }
}

#[inline]
fn get_pawn_single_moves(sq: Square, pos: &Position) -> Bitboard {
    unsafe { PAWN_MOVES[pos.side as usize][sq as usize] & !pos.board.occupancy() }
}

#[inline]
fn get_pawn_double_moves(sq: Square, pos: &Position) -> Bitboard {
    unsafe { PAWN_DOUBLES[pos.side as usize][sq as usize] & !pos.board.occupancy() }
}

#[inline]
pub fn get_ep_srcs(sq: Square, pos: &Position) -> Bitboard {
    let color = pos.side.opponent();
    unsafe { PAWN_ATTACKS[color as usize][sq as usize] & pos.our(Role::Pawn) }
}

#[inline]
pub fn get_pawn_attacks(sq: Square, color: Color, them: Bitboard) -> Bitboard {
    unsafe { PAWN_ATTACKS[color as usize][sq as usize] & them }
}

#[inline]
pub fn get_rook_moves(sq: Square, pos: &Position, occ: Bitboard) -> Bitboard {
    let magic = ROOK_MAGICS[sq as usize];
    let occ = occ & magic.mask;
    ROOK_ATTACKS[magic.index(occ)] & !pos.us()
}

#[inline]
pub fn get_bishop_moves(sq: Square, pos: &Position, occ: Bitboard) -> Bitboard {
    let magic = BISHOP_MAGICS[sq as usize];
    let occ = occ & magic.mask;
    BISHOP_ATTACKS[magic.index(occ)] & !pos.us()
}

#[inline]
pub fn get_knight_moves(sq: Square) -> Bitboard {
    unsafe { KNIGHT_MOVES[sq as usize] }
}

#[inline]
pub fn between(from: Square, to: Square) -> Bitboard {
    unsafe { BETWEEN[from as usize][to as usize] }
}

#[inline]
pub fn line(from: Square, to: Square) -> Bitboard {
    unsafe { LINE[from as usize][to as usize] }
}

#[inline]
pub fn bishop_rays(sq: Square) -> Bitboard {
    unsafe { BISHOP_RAYS[sq as usize] }
}

#[inline]
pub fn rook_rays(sq: Square) -> Bitboard {
    unsafe { ROOK_RAYS[sq as usize] }
}

pub fn perft(pos: &mut Position, depth: u8) -> u64 {
    let mut total = 0;
    let mut mg = MoveGen::new(pos);

    if depth == 1 {
        return mg.len() as u64;
    }

    for m in &mut mg {
        pos.make_move(m);
        total += perft(pos, depth - 1);
        pos.unmake_move(m)
    }
    total
}

#[cfg(test)]
mod test {
    use crate::fen::Fen;

    use super::*;

    #[test]
    fn pawns() {
        gen_all_tables();
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let Fen(position) = Fen::parse(fen).unwrap();

        let mut mg = MoveGen::new(&position);
        assert_eq!(mg.len(), 20);
    }

    #[test]
    fn perft_2() {
        gen_all_tables();
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let Fen(mut position) = Fen::parse(fen).unwrap();

        let nodes = perft(&mut position, 4);
        assert_eq!(nodes, (400, 0));
    }

    #[test]
    fn perft_3() {
        gen_all_tables();
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let Fen(mut position) = Fen::parse(fen).unwrap();
        position.make_move(Move::new(Square::B2, Square::B4, None));

        let nodes = perft(&mut position, 2);
        assert_eq!(nodes, (400, 0));
    }
}
