use magic::Magic;
pub use tables::{
    between, bishop_rays, get_bishop_moves, get_king_moves, get_kingside_castle_through_squares,
    get_knight_moves, get_pawn_attacks, get_pawn_moves, get_queenside_castle_throught_squares,
    get_rook_moves, init_tables, line, rook_rays,
};
pub use types::{
    BishopType, BlackType, InCheck, KingType, KnightType, MoveGen, MoveList, Mover, NotCheck,
    PawnType, QueenType, RookType, WhiteType,
};

use crate::{bitboard::Bitboard, position::Position};

mod magic;
mod magic_gen;
mod tables;
mod types;

mod bishop;
mod king;
mod knight;
mod pawn;
mod queen;
mod rook;

pub mod magic_finder;

#[inline]
pub fn perft(pos: &mut Position, depth: u8) -> usize {
    let mut total = 0;
    let mut mg = MoveGen::new(pos);

    if depth == 0 {
        return 1;
    }

    if depth == 1 {
        return mg.len();
    }

    for m in &mut mg {
        pos.make_move(m);
        total += perft(pos, depth - 1);
        pos.unmake_move(m)
    }
    total
}

#[cfg(test)]
fn masked_perft(pos: &mut Position, depth: u8) -> usize {
    if depth == 0 {
        return 1;
    }

    let mut total = 0;
    let mask = pos.occupancy;

    let mut mg = MoveGen::new(pos);
    mg.set_mask(mask);

    for m in &mut mg {
        pos.make_move(m);
        total += masked_perft(pos, depth - 1);
        pos.unmake_move(m)
    }

    mg.set_mask(Bitboard::FULL);
    for m in &mut mg {
        pos.make_move(m);
        total += masked_perft(pos, depth - 1);
        pos.unmake_move(m)
    }

    total
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::fen::Fen;

    const STARTPOS: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    const KIWIPETE_FEN: &str =
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
    const POSITTION_3_FEN: &str = "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1";
    const POSITION_4_FEN: &str = "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1";
    const POSITION_5_FEN: &str = "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8";
    const POSITION_6_FEN: &str =
        "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10";

    #[test]
    fn perft_normal() {
        init_tables();
        let Fen(position) = Fen::parse(STARTPOS).unwrap();
        assert_eq!(perft(&mut position.clone(), 2), 400);
        assert_eq!(perft(&mut position.clone(), 3), 8902);
        assert_eq!(perft(&mut position.clone(), 4), 197_281);
        assert_eq!(perft(&mut position.clone(), 5), 4_865_609);
        assert_eq!(perft(&mut position.clone(), 6), 119_060_324);
    }

    #[test]
    fn masked_perft_normal() {
        init_tables();
        let Fen(position) = Fen::parse(STARTPOS).unwrap();
        assert_eq!(masked_perft(&mut position.clone(), 2), 400);
        assert_eq!(masked_perft(&mut position.clone(), 3), 8902);
        assert_eq!(masked_perft(&mut position.clone(), 4), 197_281);
        assert_eq!(masked_perft(&mut position.clone(), 5), 4_865_609);
    }

    #[test]
    fn perft_kiwipete() {
        init_tables();
        let Fen(position) = Fen::parse(KIWIPETE_FEN).unwrap();
        assert_eq!(perft(&mut position.clone(), 1), 48);
        assert_eq!(perft(&mut position.clone(), 2), 2_039);
        assert_eq!(perft(&mut position.clone(), 3), 97_862);
        assert_eq!(perft(&mut position.clone(), 4), 4_085_603);
        assert_eq!(perft(&mut position.clone(), 5), 193_690_690);
    }

    #[test]
    fn perft_pos_3() {
        init_tables();
        let Fen(position) = Fen::parse(POSITTION_3_FEN).unwrap();
        assert_eq!(perft(&mut position.clone(), 1), 14);
        assert_eq!(perft(&mut position.clone(), 2), 191);
        assert_eq!(perft(&mut position.clone(), 3), 2_812);
        assert_eq!(perft(&mut position.clone(), 4), 43_238);
        assert_eq!(perft(&mut position.clone(), 5), 674_624);
        assert_eq!(perft(&mut position.clone(), 6), 11_030_083);
        assert_eq!(perft(&mut position.clone(), 7), 178_633_661);
    }

    #[test]
    fn perft_pos_4() {
        init_tables();
        let Fen(position) = Fen::parse(POSITION_4_FEN).unwrap();
        assert_eq!(perft(&mut position.clone(), 1), 6);
        assert_eq!(perft(&mut position.clone(), 2), 264);
        assert_eq!(perft(&mut position.clone(), 3), 9_467);
        assert_eq!(perft(&mut position.clone(), 4), 422_333);
        assert_eq!(perft(&mut position.clone(), 5), 15_833_292);
    }

    #[test]
    fn perft_pos_5() {
        init_tables();
        let Fen(position) = Fen::parse(POSITION_5_FEN).unwrap();
        assert_eq!(perft(&mut position.clone(), 1), 44);
        assert_eq!(perft(&mut position.clone(), 2), 1_486);
        assert_eq!(perft(&mut position.clone(), 3), 62_379);
        assert_eq!(perft(&mut position.clone(), 4), 2_103_487);
        assert_eq!(perft(&mut position.clone(), 5), 89_941_194);
    }

    #[test]
    fn masked_perft_pos_5() {
        init_tables();
        let Fen(position) = Fen::parse(POSITION_5_FEN).unwrap();
        assert_eq!(masked_perft(&mut position.clone(), 1), 44);
        assert_eq!(masked_perft(&mut position.clone(), 2), 1_486);
        assert_eq!(masked_perft(&mut position.clone(), 3), 62_379);
        assert_eq!(masked_perft(&mut position.clone(), 4), 2_103_487);
    }

    #[test]
    fn perft_pos_6() {
        init_tables();
        let Fen(position) = Fen::parse(POSITION_6_FEN).unwrap();
        assert_eq!(perft(&mut position.clone(), 1), 46);
        assert_eq!(perft(&mut position.clone(), 2), 2_079);
        assert_eq!(perft(&mut position.clone(), 3), 89_890);
        assert_eq!(perft(&mut position.clone(), 4), 3_894_594);
        assert_eq!(perft(&mut position.clone(), 5), 164_075_551);
    }
}
