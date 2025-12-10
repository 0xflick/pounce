use crate::chess::bitboard::Bitboard;
use crate::chess::{Rank, Square};

// All tables computed at compile time - no runtime initialization needed
pub static PAWN_MOVES: [[Bitboard; 64]; 2] = compute_pawn_moves();
pub static PAWN_ATTACKS: [[Bitboard; 64]; 2] = compute_pawn_attacks();
pub static KNIGHT_MOVES: [Bitboard; 64] = compute_knight_moves();
pub static KING_MOVES: [Bitboard; 64] = compute_king_moves();
pub static KINGSIDE_CASTLE: [Bitboard; 2] = compute_kingside_castle();
pub static QUEENSIDE_CASTLE: [Bitboard; 2] = compute_queenside_castle();
#[allow(long_running_const_eval)]
pub static BETWEEN: [[Bitboard; 64]; 64] = compute_between();
#[allow(long_running_const_eval)]
pub static LINE: [[Bitboard; 64]; 64] = compute_line();
pub static BISHOP_RAYS: [Bitboard; 64] = compute_bishop_rays();
pub static ROOK_RAYS: [Bitboard; 64] = compute_rook_rays();

// No-op for backwards compatibility - tables are now compile-time constants
pub fn init_tables() {}

const fn compute_pawn_moves() -> [[Bitboard; 64]; 2] {
    let mut moves = [[Bitboard::EMPTY; 64]; 2];

    // White pawns
    let mut sq = 0u8;
    while sq < 64 {
        let square = Square::new_unchecked(sq);
        let mut bb = Bitboard::EMPTY;

        if let Some(s) = square.north() {
            bb = bb.with_square(s);
        }

        // Double push from rank 2
        if square.rank() as u8 == Rank::R2 as u8 {
            if let Some(s1) = square.north() {
                if let Some(s2) = s1.north() {
                    bb = bb.with_square(s2);
                }
            }
        }

        moves[0][sq as usize] = bb;
        sq += 1;
    }

    // Black pawns
    sq = 0;
    while sq < 64 {
        let square = Square::new_unchecked(sq);
        let mut bb = Bitboard::EMPTY;

        if let Some(s) = square.south() {
            bb = bb.with_square(s);
        }

        // Double push from rank 7
        if square.rank() as u8 == Rank::R7 as u8 {
            if let Some(s1) = square.south() {
                if let Some(s2) = s1.south() {
                    bb = bb.with_square(s2);
                }
            }
        }

        moves[1][sq as usize] = bb;
        sq += 1;
    }

    moves
}

const fn compute_pawn_attacks() -> [[Bitboard; 64]; 2] {
    let mut attacks = [[Bitboard::EMPTY; 64]; 2];

    // White pawn attacks
    let mut sq = 0u8;
    while sq < 64 {
        let square = Square::new_unchecked(sq);
        let mut bb = Bitboard::EMPTY;

        if let Some(s) = square.north() {
            if let Some(l) = s.east() {
                bb = bb.with_square(l);
            }
            if let Some(r) = s.west() {
                bb = bb.with_square(r);
            }
        }

        attacks[0][sq as usize] = bb;
        sq += 1;
    }

    // Black pawn attacks
    sq = 0;
    while sq < 64 {
        let square = Square::new_unchecked(sq);
        let mut bb = Bitboard::EMPTY;

        if let Some(s) = square.south() {
            if let Some(l) = s.east() {
                bb = bb.with_square(l);
            }
            if let Some(r) = s.west() {
                bb = bb.with_square(r);
            }
        }

        attacks[1][sq as usize] = bb;
        sq += 1;
    }

    attacks
}

const fn compute_knight_moves() -> [Bitboard; 64] {
    let mut moves = [Bitboard::EMPTY; 64];
    let mut sq = 0u8;

    while sq < 64 {
        let square = Square::new_unchecked(sq);
        let mut bb = Bitboard::EMPTY;

        // NNE
        if let Some(n1) = square.north() {
            if let Some(n2) = n1.north() {
                if let Some(e) = n2.east() {
                    bb = bb.with_square(e);
                }
            }
        }

        // NEE
        if let Some(n) = square.north() {
            if let Some(e1) = n.east() {
                if let Some(e2) = e1.east() {
                    bb = bb.with_square(e2);
                }
            }
        }

        // NNW
        if let Some(n1) = square.north() {
            if let Some(n2) = n1.north() {
                if let Some(w) = n2.west() {
                    bb = bb.with_square(w);
                }
            }
        }

        // NWW
        if let Some(n) = square.north() {
            if let Some(w1) = n.west() {
                if let Some(w2) = w1.west() {
                    bb = bb.with_square(w2);
                }
            }
        }

        // SSE
        if let Some(s1) = square.south() {
            if let Some(s2) = s1.south() {
                if let Some(e) = s2.east() {
                    bb = bb.with_square(e);
                }
            }
        }

        // SEE
        if let Some(s) = square.south() {
            if let Some(e1) = s.east() {
                if let Some(e2) = e1.east() {
                    bb = bb.with_square(e2);
                }
            }
        }

        // SSW
        if let Some(s1) = square.south() {
            if let Some(s2) = s1.south() {
                if let Some(w) = s2.west() {
                    bb = bb.with_square(w);
                }
            }
        }

        // SWW
        if let Some(s) = square.south() {
            if let Some(w1) = s.west() {
                if let Some(w2) = w1.west() {
                    bb = bb.with_square(w2);
                }
            }
        }

        moves[sq as usize] = bb;
        sq += 1;
    }

    moves
}

const fn compute_king_moves() -> [Bitboard; 64] {
    let mut moves = [Bitboard::EMPTY; 64];
    let mut sq = 0u8;

    while sq < 64 {
        let square = Square::new_unchecked(sq);
        let mut bb = Bitboard::EMPTY;

        if let Some(s) = square.north() {
            bb = bb.with_square(s);
        }
        if let Some(s) = square.south() {
            bb = bb.with_square(s);
        }
        if let Some(s) = square.east() {
            bb = bb.with_square(s);
        }
        if let Some(s) = square.west() {
            bb = bb.with_square(s);
        }

        // Diagonals
        if let Some(n) = square.north() {
            if let Some(e) = n.east() {
                bb = bb.with_square(e);
            }
            if let Some(w) = n.west() {
                bb = bb.with_square(w);
            }
        }
        if let Some(s) = square.south() {
            if let Some(e) = s.east() {
                bb = bb.with_square(e);
            }
            if let Some(w) = s.west() {
                bb = bb.with_square(w);
            }
        }

        moves[sq as usize] = bb;
        sq += 1;
    }

    moves
}

const fn compute_kingside_castle() -> [Bitboard; 2] {
    let mut kingside = [Bitboard::EMPTY; 2];

    // White: F1, G1
    kingside[0] = Bitboard::EMPTY
        .with_square(Square::F1)
        .with_square(Square::G1);

    // Black: F8, G8
    kingside[1] = Bitboard::EMPTY
        .with_square(Square::F8)
        .with_square(Square::G8);

    kingside
}

const fn compute_queenside_castle() -> [Bitboard; 2] {
    let mut queenside = [Bitboard::EMPTY; 2];

    // White: B1, C1, D1
    queenside[0] = Bitboard::EMPTY
        .with_square(Square::B1)
        .with_square(Square::C1)
        .with_square(Square::D1);

    // Black: B8, C8, D8
    queenside[1] = Bitboard::EMPTY
        .with_square(Square::B8)
        .with_square(Square::C8)
        .with_square(Square::D8);

    queenside
}

const fn compute_between() -> [[Bitboard; 64]; 64] {
    let mut between = [[Bitboard::EMPTY; 64]; 64];
    let mut from = 0u8;

    while from < 64 {
        let mut to = 0u8;
        while to < 64 {
            between[from as usize][to as usize] =
                gen_between(Square::new_unchecked(from), Square::new_unchecked(to));
            to += 1;
        }
        from += 1;
    }

    between
}

const fn gen_between(from: Square, to: Square) -> Bitboard {
    let mut bb = Bitboard::EMPTY;

    let from_file = from.file() as u8;
    let to_file = to.file() as u8;
    let from_rank = from.rank() as u8;
    let to_rank = to.rank() as u8;

    let min_file = if from_file < to_file {
        from_file
    } else {
        to_file
    };
    let max_file = if from_file > to_file {
        from_file
    } else {
        to_file
    };
    let min_rank = if from_rank < to_rank {
        from_rank
    } else {
        to_rank
    };
    let max_rank = if from_rank > to_rank {
        from_rank
    } else {
        to_rank
    };

    let mut sq = 0u8;
    while sq < 64 {
        let square = Square::new_unchecked(sq);
        let sq_file = square.file() as u8;
        let sq_rank = square.rank() as u8;

        // same rank
        if sq_rank == from_rank && from_rank == to_rank && sq_file > min_file && sq_file < max_file
        {
            bb = bb.with_square(square);
        }

        // same file
        if sq_file == from_file && from_file == to_file && sq_rank > min_rank && sq_rank < max_rank
        {
            bb = bb.with_square(square);
        }

        // same diagonal
        let sq_rank_dist_from = if sq_rank > from_rank {
            sq_rank - from_rank
        } else {
            from_rank - sq_rank
        };
        let sq_file_dist_from = if sq_file > from_file {
            sq_file - from_file
        } else {
            from_file - sq_file
        };
        let from_rank_dist_to = if from_rank > to_rank {
            from_rank - to_rank
        } else {
            to_rank - from_rank
        };
        let from_file_dist_to = if from_file > to_file {
            from_file - to_file
        } else {
            to_file - from_file
        };

        if sq_rank_dist_from == sq_file_dist_from
            && from_rank_dist_to == from_file_dist_to
            && sq_rank > min_rank
            && sq_rank < max_rank
            && sq_file > min_file
            && sq_file < max_file
        {
            bb = bb.with_square(square);
        }

        sq += 1;
    }

    bb
}

const fn compute_line() -> [[Bitboard; 64]; 64] {
    let mut line = [[Bitboard::EMPTY; 64]; 64];
    let mut from = 0u8;

    while from < 64 {
        let mut to = 0u8;
        while to < 64 {
            line[from as usize][to as usize] = if from == to {
                Bitboard::EMPTY
            } else {
                gen_line(Square::new_unchecked(from), Square::new_unchecked(to))
            };
            to += 1;
        }
        from += 1;
    }

    line
}

const fn gen_line(from: Square, to: Square) -> Bitboard {
    let mut bb = Bitboard::EMPTY;

    let from_file = from.file() as u8;
    let to_file = to.file() as u8;
    let from_rank = from.rank() as u8;
    let to_rank = to.rank() as u8;

    let mut sq = 0u8;
    while sq < 64 {
        let square = Square::new_unchecked(sq);
        let sq_file = square.file() as u8;
        let sq_rank = square.rank() as u8;

        // same rank
        if sq_rank == from_rank && from_rank == to_rank {
            bb = bb.with_square(square);
        }

        // same file
        if sq_file == from_file && from_file == to_file {
            bb = bb.with_square(square);
        }

        // same diagonal
        let sq_rank_dist_from = if sq_rank > from_rank {
            sq_rank - from_rank
        } else {
            from_rank - sq_rank
        };
        let sq_file_dist_from = if sq_file > from_file {
            sq_file - from_file
        } else {
            from_file - sq_file
        };
        let sq_rank_dist_to = if sq_rank > to_rank {
            sq_rank - to_rank
        } else {
            to_rank - sq_rank
        };
        let sq_file_dist_to = if sq_file > to_file {
            sq_file - to_file
        } else {
            to_file - sq_file
        };
        let from_rank_dist_to = if from_rank > to_rank {
            from_rank - to_rank
        } else {
            to_rank - from_rank
        };
        let from_file_dist_to = if from_file > to_file {
            from_file - to_file
        } else {
            to_file - from_file
        };

        if sq_rank_dist_from == sq_file_dist_from
            && sq_rank_dist_to == sq_file_dist_to
            && from_rank_dist_to == from_file_dist_to
        {
            bb = bb.with_square(square);
        }

        sq += 1;
    }

    bb
}

const fn compute_bishop_rays() -> [Bitboard; 64] {
    let mut rays = [Bitboard::EMPTY; 64];
    let mut sq = 0u8;

    while sq < 64 {
        rays[sq as usize] = gen_bishop_ray(Square::new_unchecked(sq));
        sq += 1;
    }

    rays
}

const fn gen_bishop_ray(sq: Square) -> Bitboard {
    let mut bb = Bitboard::EMPTY;

    // NE
    let mut s = sq;
    loop {
        match s.north() {
            Some(n) => match n.east() {
                Some(ne) => {
                    bb = bb.with_square(ne);
                    s = ne;
                }
                None => break,
            },
            None => break,
        }
    }

    // NW
    s = sq;
    loop {
        match s.north() {
            Some(n) => match n.west() {
                Some(nw) => {
                    bb = bb.with_square(nw);
                    s = nw;
                }
                None => break,
            },
            None => break,
        }
    }

    // SE
    s = sq;
    loop {
        match s.south() {
            Some(south) => match south.east() {
                Some(se) => {
                    bb = bb.with_square(se);
                    s = se;
                }
                None => break,
            },
            None => break,
        }
    }

    // SW
    s = sq;
    loop {
        match s.south() {
            Some(south) => match south.west() {
                Some(sw) => {
                    bb = bb.with_square(sw);
                    s = sw;
                }
                None => break,
            },
            None => break,
        }
    }

    bb
}

const fn compute_rook_rays() -> [Bitboard; 64] {
    let mut rays = [Bitboard::EMPTY; 64];
    let mut sq = 0u8;

    while sq < 64 {
        rays[sq as usize] = gen_rook_ray(Square::new_unchecked(sq));
        sq += 1;
    }

    rays
}

const fn gen_rook_ray(sq: Square) -> Bitboard {
    let mut bb = Bitboard::EMPTY;

    // North
    let mut s = sq;
    while let Some(n) = s.north() {
        bb = bb.with_square(n);
        s = n;
    }

    // South
    s = sq;
    while let Some(south) = s.south() {
        bb = bb.with_square(south);
        s = south;
    }

    // East
    s = sq;
    while let Some(e) = s.east() {
        bb = bb.with_square(e);
        s = e;
    }

    // West
    s = sq;
    while let Some(w) = s.west() {
        bb = bb.with_square(w);
        s = w;
    }

    bb
}
