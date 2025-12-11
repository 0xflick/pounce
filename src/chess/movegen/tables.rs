use std::sync::OnceLock;

use crate::chess::bitboard::Bitboard;
use crate::chess::{Color, File, Square};

struct Tables {
    pawn_moves: [[Bitboard; 64]; 2],
    pawn_attacks: [[Bitboard; 64]; 2],
    knight_moves: [Bitboard; 64],
    king_moves: [Bitboard; 64],
    kingside_castle: [Bitboard; 2],
    queenside_castle: [Bitboard; 2],
    between: [[Bitboard; 64]; 64],
    line: [[Bitboard; 64]; 64],
    bishop_rays: [Bitboard; 64],
    rook_rays: [Bitboard; 64],
}

static TABLES: OnceLock<Tables> = OnceLock::new();

fn get_tables() -> &'static Tables {
    TABLES.get_or_init(|| Tables {
        pawn_moves: init_pawn_moves(),
        pawn_attacks: init_pawn_attacks(),
        knight_moves: init_knight_moves(),
        king_moves: init_king_moves(),
        kingside_castle: init_kingside_castle(),
        queenside_castle: init_queenside_castle(),
        between: init_between(),
        line: init_line(),
        bishop_rays: init_bishop_rays(),
        rook_rays: init_rook_rays(),
    })
}

pub fn init_tables() {
    // Force initialization
    let _ = get_tables();
}

pub fn get_pawn_moves(color: Color, sq: Square) -> Bitboard {
    get_tables().pawn_moves[color as usize][sq as usize]
}

pub fn get_pawn_attacks(color: Color, sq: Square) -> Bitboard {
    get_tables().pawn_attacks[color as usize][sq as usize]
}

pub fn get_knight_moves(sq: Square) -> Bitboard {
    get_tables().knight_moves[sq as usize]
}

pub fn get_king_moves(sq: Square) -> Bitboard {
    get_tables().king_moves[sq as usize]
}

pub fn get_kingside_castle(color: Color) -> Bitboard {
    get_tables().kingside_castle[color as usize]
}

pub fn get_queenside_castle(color: Color) -> Bitboard {
    get_tables().queenside_castle[color as usize]
}

pub fn get_between(from: Square, to: Square) -> Bitboard {
    get_tables().between[from as usize][to as usize]
}

pub fn get_line(from: Square, to: Square) -> Bitboard {
    get_tables().line[from as usize][to as usize]
}

pub fn get_bishop_rays(sq: Square) -> Bitboard {
    get_tables().bishop_rays[sq as usize]
}

pub fn get_rook_rays(sq: Square) -> Bitboard {
    get_tables().rook_rays[sq as usize]
}

fn init_pawn_moves() -> [[Bitboard; 64]; 2] {
    let mut moves = [[Bitboard::EMPTY; 64]; 2];

    for color in [Color::White, Color::Black].into_iter() {
        for sq in Square::ALL {
            let move_bb = &mut moves[color as usize][sq as usize];
            if let Some(s) = sq.up(color) {
                move_bb.set(s);
            }

            if sq.rank() == color.home_rank()
                && let Some(s) = sq.up(color).and_then(|s| s.up(color))
            {
                move_bb.set(s);
            }
        }
    }

    moves
}

fn init_pawn_attacks() -> [[Bitboard; 64]; 2] {
    let mut attacks = [[Bitboard::EMPTY; 64]; 2];

    for color in [Color::White, Color::Black].into_iter() {
        for sq in Square::ALL {
            let attack_bb = &mut attacks[color as usize][sq as usize];
            if let Some(s) = sq.up(color) {
                if let Some(l) = s.east() {
                    attack_bb.set(l);
                }
                if let Some(r) = s.west() {
                    attack_bb.set(r);
                }
            }
        }
    }

    attacks
}

#[rustfmt::skip]
fn init_knight_moves() -> [Bitboard; 64] {
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
    moves
}

fn init_king_moves() -> [Bitboard; 64] {
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
    moves
}

fn init_kingside_castle() -> [Bitboard; 2] {
    let mut kingside = [Bitboard::EMPTY; 2];
    for color in [Color::White, Color::Black].into_iter() {
        let back_rank = color.back_rank();
        kingside[color as usize].set(Square::make(File::F, back_rank));
        kingside[color as usize].set(Square::make(File::G, back_rank));
    }
    kingside
}

fn init_queenside_castle() -> [Bitboard; 2] {
    let mut queenside = [Bitboard::EMPTY; 2];
    for color in [Color::White, Color::Black].into_iter() {
        let back_rank = color.back_rank();
        queenside[color as usize].set(Square::make(File::B, back_rank));
        queenside[color as usize].set(Square::make(File::C, back_rank));
        queenside[color as usize].set(Square::make(File::D, back_rank));
    }
    queenside
}

fn init_between() -> [[Bitboard; 64]; 64] {
    let mut between = [[Bitboard::EMPTY; 64]; 64];
    for from in Square::ALL {
        for to in Square::ALL {
            between[from as usize][to as usize] = gen_between(from, to)
        }
    }
    between
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

fn init_line() -> [[Bitboard; 64]; 64] {
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
    line
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

fn init_bishop_rays() -> [Bitboard; 64] {
    let mut rays = [Bitboard::EMPTY; 64];
    for sq in Square::ALL {
        rays[sq as usize] = gen_bishop_ray(sq);
    }
    rays
}

fn init_rook_rays() -> [Bitboard; 64] {
    let mut rays = [Bitboard::EMPTY; 64];
    for sq in Square::ALL {
        rays[sq as usize] = gen_rook_ray(sq);
    }
    rays
}
