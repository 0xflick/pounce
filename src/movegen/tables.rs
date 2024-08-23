use crate::{
    bitboard::Bitboard,
    chess::{Color, File, Square},
    movegen::{
        magic::{BISHOP_ATTACKS, ROOK_ATTACKS},
        magic_gen::{BISHOP_MAGICS, ROOK_MAGICS},
    },
};

static mut PAWN_MOVES: [[Bitboard; 64]; 2] = [[Bitboard::EMPTY; 64]; 2];
static mut PAWN_ATTACKS: [[Bitboard; 64]; 2] = [[Bitboard::EMPTY; 64]; 2];
static mut KNIGHT_MOVES: [Bitboard; 64] = [Bitboard::EMPTY; 64];
static mut KING_MOVES: [Bitboard; 64] = [Bitboard::EMPTY; 64];
static mut KINGSIDE_CASTLE: [Bitboard; 2] = [Bitboard::EMPTY; 2];
static mut QUEENSIDE_CASTLE: [Bitboard; 2] = [Bitboard::EMPTY; 2];
static mut BETWEEN: [[Bitboard; 64]; 64] = [[Bitboard::EMPTY; 64]; 64];
static mut LINE: [[Bitboard; 64]; 64] = [[Bitboard::EMPTY; 64]; 64];
static mut BISHOP_RAYS: [Bitboard; 64] = [Bitboard::EMPTY; 64];
static mut ROOK_RAYS: [Bitboard; 64] = [Bitboard::EMPTY; 64];

pub fn init_tables() {
    init_pawn_move_table();
    init_knight_move_table();
    init_king_move_table();
    init_castle_table();
    init_between_table();
    init_line_table();
    init_bishop_rays();
    init_rook_rays();
}

#[inline(always)]
pub fn get_pawn_moves(sq: Square, color: Color) -> Bitboard {
    unsafe {
        *PAWN_MOVES
            .get_unchecked(color as usize)
            .get_unchecked(sq as usize)
    }
}

#[inline(always)]
pub fn get_pawn_attacks(sq: Square, color: Color) -> Bitboard {
    unsafe {
        *PAWN_ATTACKS
            .get_unchecked(color as usize)
            .get_unchecked(sq as usize)
    }
}

#[inline(always)]
pub fn get_rook_moves(sq: Square, occ: Bitboard) -> Bitboard {
    unsafe {
        let magic = ROOK_MAGICS.get_unchecked(sq as usize);
        let occ = occ & magic.mask;
        *ROOK_ATTACKS.get_unchecked(magic.index(occ))
    }
}

#[inline(always)]
pub fn get_bishop_moves(sq: Square, occ: Bitboard) -> Bitboard {
    unsafe {
        let magic = BISHOP_MAGICS.get_unchecked(sq as usize);
        let occ = occ & magic.mask;
        *BISHOP_ATTACKS.get_unchecked(magic.index(occ))
    }
}

#[inline(always)]
pub fn get_knight_moves(sq: Square) -> Bitboard {
    unsafe { *KNIGHT_MOVES.get_unchecked(sq as usize) }
}

#[inline(always)]
pub fn get_king_moves(sq: Square) -> Bitboard {
    unsafe { *KING_MOVES.get_unchecked(sq as usize) }
}

#[inline(always)]
pub fn between(from: Square, to: Square) -> Bitboard {
    unsafe {
        *BETWEEN
            .get_unchecked(from as usize)
            .get_unchecked(to as usize)
    }
}

#[inline(always)]
pub fn line(from: Square, to: Square) -> Bitboard {
    unsafe { *LINE.get_unchecked(from as usize).get_unchecked(to as usize) }
}

#[inline(always)]
pub fn bishop_rays(sq: Square) -> Bitboard {
    unsafe { *BISHOP_RAYS.get_unchecked(sq as usize) }
}

#[inline(always)]
pub fn rook_rays(sq: Square) -> Bitboard {
    unsafe { *ROOK_RAYS.get_unchecked(sq as usize) }
}

#[inline(always)]
pub fn get_kingside_castle_through_squares(color: Color) -> Bitboard {
    unsafe { *KINGSIDE_CASTLE.get_unchecked(color as usize) }
}

#[inline(always)]
pub fn get_queenside_castle_throught_squares(color: Color) -> Bitboard {
    unsafe { *QUEENSIDE_CASTLE.get_unchecked(color as usize) }
}

fn init_pawn_move_table() {
    let mut moves = [[Bitboard::EMPTY; 64]; 2];
    let mut attacks = [[Bitboard::EMPTY; 64]; 2];

    for color in [Color::White, Color::Black].into_iter() {
        for sq in Square::ALL {
            let move_bb = &mut moves[color][sq];
            let attack_bb = &mut attacks[color][sq];
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
fn init_knight_move_table() {
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
        moves[sq] = bb;
    }
    unsafe {
        KNIGHT_MOVES = moves;
    }
}

fn init_king_move_table() {
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
        moves[sq] = bb;
    }
    unsafe {
        KING_MOVES = moves;
    }
}

fn init_castle_table() {
    let mut kingside = [Bitboard::EMPTY; 2];
    let mut queenside = [Bitboard::EMPTY; 2];
    for color in [Color::White, Color::Black].into_iter() {
        let back_rank = color.back_rank();
        kingside[color].set(Square::make(File::F, back_rank));
        kingside[color].set(Square::make(File::G, back_rank));

        queenside[color].set(Square::make(File::B, back_rank));
        queenside[color].set(Square::make(File::C, back_rank));
        queenside[color].set(Square::make(File::D, back_rank));
    }
    unsafe {
        KINGSIDE_CASTLE = kingside;
        QUEENSIDE_CASTLE = queenside;
    }
}

fn init_between_table() {
    let mut between = [[Bitboard::EMPTY; 64]; 64];
    for from in Square::ALL {
        for to in Square::ALL {
            between[from][to] = gen_between(from, to)
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

fn init_line_table() {
    let mut line = [[Bitboard::EMPTY; 64]; 64];
    for from in Square::ALL {
        for to in Square::ALL {
            line[from][to] = if from == to {
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

fn init_bishop_rays() {
    let mut rays = [Bitboard::EMPTY; 64];
    for sq in Square::ALL {
        rays[sq] = gen_bishop_ray(sq);
    }
    unsafe {
        BISHOP_RAYS = rays;
    }
}

fn init_rook_rays() {
    let mut rays = [Bitboard::EMPTY; 64];
    for sq in Square::ALL {
        rays[sq] = gen_rook_ray(sq);
    }
    unsafe {
        ROOK_RAYS = rays;
    }
}
