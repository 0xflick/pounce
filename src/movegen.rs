use magic::Magic;
pub use tables::{
    between, bishop_rays, gen_all_tables, get_bishop_moves, get_king_moves,
    get_kingside_castle_through_squares, get_knight_moves, get_pawn_attacks, get_pawn_moves,
    get_queenside_castle_throught_squares, get_rook_moves, line, rook_rays,
};
pub use types::{
    BishopType, BlackType, InCheck, KingType, KnightType, MoveGen, MoveList, Mover, NotCheck,
    PawnType, QueenType, RookType, WhiteType,
};

use crate::position::Position;

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
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
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
