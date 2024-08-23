use crate::{
    chess::{CastleRights, Color, File, Piece, Role, Square},
    position::Position,
};

use rand::{rngs::SmallRng, Rng};
use rand_core::SeedableRng;

// One entry for each piece on each square, 1 for the side to move,
// 8 for the en passant file, 16 for castling rights (don't
// need that many, but it's easier to just index that way).
const ZOBRIST_LEN: usize = Square::NUM * Color::NUM * Role::NUM + 1 + File::NUM + 16;
static mut ZOBRIST_KEYS: [u64; ZOBRIST_LEN] = [0; ZOBRIST_LEN];

fn zobrist_init() {
    let mut rng = SmallRng::seed_from_u64(0xcafe);
    unsafe {
        rng.fill(ZOBRIST_KEYS.as_mut());
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZobristHash(u64);

impl ZobristHash {
    pub fn new() -> Self {
        Self(0)
    }

    pub fn toggle_piece(&mut self, square: Square, Piece { color, role }: Piece) {
        let piece_idx = role as usize + Role::NUM * color as usize;
        self.0 ^= unsafe {
            ZOBRIST_KEYS.get_unchecked(square as usize * Color::NUM * Role::NUM + piece_idx)
        };
    }

    pub fn toggle_side(&mut self) {
        self.0 ^= unsafe { ZOBRIST_KEYS.get_unchecked(Square::NUM * Color::NUM * Role::NUM) };
    }

    pub fn toggle_ep(&mut self, ep_square: Option<Square>) {
        if let Some(ep_square) = ep_square {
            let file = ep_square.file();
            self.0 ^= unsafe {
                ZOBRIST_KEYS.get_unchecked(Square::NUM * Color::NUM * Role::NUM + 1 + file as usize)
            };
        }
    }

    pub fn toggle_castling(&mut self, castling: CastleRights) {
        self.0 ^= unsafe {
            ZOBRIST_KEYS.get_unchecked(
                Square::NUM * Color::NUM * Role::NUM + 1 + File::NUM + castling.bits() as usize,
            )
        };
    }
}

impl Default for ZobristHash {
    fn default() -> Self {
        Self::new()
    }
}

impl Position {
    pub fn zobrist_hash(&self) -> ZobristHash {
        let mut hash = ZobristHash::new();
        for square in Square::ALL {
            if let Some(piece) = self.piece_at(square) {
                hash.toggle_piece(square, piece);
            }
        }

        if self.side == Color::Black {
            hash.toggle_side();
        }

        hash.toggle_ep(self.ep_square);
        hash.toggle_castling(self.castling);

        hash
    }
}

#[cfg(test)]
fn perft_zobrist(pos: &mut Position, depth: u8) {
    use crate::{fen::Fen, movegen::MoveGen};

    if depth == 0 {
        return;
    }

    let before = pos.zobrist_hash();
    assert_eq!(before, pos.key, "hash mismatch");

    let mg = MoveGen::new(pos);
    for m in mg {
        pos.make_move(m);

        let after = pos.zobrist_hash();
        assert_eq!(
            after,
            pos.key,
            "hash mismatch after make move {} to fen {}",
            m,
            Fen(pos.clone())
        );

        perft_zobrist(pos, depth - 1);
        pos.unmake_move(m);

        let after = pos.zobrist_hash();
        assert_eq!(
            after,
            pos.key,
            "hash mismatch after unmake move {} to fen {}",
            m,
            Fen(pos.clone())
        );
    }
}

#[cfg(test)]
mod test {
    use crate::{fen::Fen, movegen::init_tables, zobrist::perft_zobrist};

    use super::zobrist_init;

    const STARTPOS: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    const KIWIPETE_FEN: &str =
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
    const POSITTION_3_FEN: &str = "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1";
    const POSITION_4_FEN: &str = "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1";
    const POSITION_5_FEN: &str = "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8";
    const POSITION_6_FEN: &str =
        "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10";

    #[test]
    fn test_zobrist() {
        init_tables();
        zobrist_init();

        let Fen(mut position) = STARTPOS.parse().unwrap();

        let hash = position.zobrist_hash();
        assert_eq!(hash, position.key);

        perft_zobrist(&mut position, 5);
        assert_eq!(hash, position.zobrist_hash());
        assert_eq!(hash, position.key);
    }

    #[test]
    fn test_zobrist_kiwipete() {
        init_tables();
        zobrist_init();
        let Fen(mut position) = KIWIPETE_FEN.parse().unwrap();
        let hash = position.zobrist_hash();
        assert_eq!(hash, position.key);
        perft_zobrist(&mut position, 4);
        assert_eq!(hash, position.zobrist_hash());
        assert_eq!(hash, position.key);
    }

    #[test]
    fn test_zobrist_position_3() {
        init_tables();
        zobrist_init();
        let Fen(mut position) = POSITTION_3_FEN.parse().unwrap();
        let hash = position.zobrist_hash();
        assert_eq!(hash, position.key);
        perft_zobrist(&mut position, 4);
        assert_eq!(hash, position.zobrist_hash());
        assert_eq!(hash, position.key);
    }

    #[test]
    fn test_zobrist_position_4() {
        init_tables();
        zobrist_init();
        let Fen(mut position) = POSITION_4_FEN.parse().unwrap();
        let hash = position.zobrist_hash();
        assert_eq!(hash, position.key);
        perft_zobrist(&mut position, 4);
        assert_eq!(hash, position.zobrist_hash());
        assert_eq!(hash, position.key);
    }

    #[test]
    fn test_zobrist_position_5() {
        init_tables();
        zobrist_init();
        let Fen(mut position) = POSITION_5_FEN.parse().unwrap();
        let hash = position.zobrist_hash();
        assert_eq!(hash, position.key);
        perft_zobrist(&mut position, 4);
        assert_eq!(hash, position.zobrist_hash());
        assert_eq!(hash, position.key);
    }

    #[test]
    fn test_zobrist_position_6() {
        init_tables();
        zobrist_init();
        let Fen(mut position) = POSITION_6_FEN.parse().unwrap();
        let hash = position.zobrist_hash();
        assert_eq!(hash, position.key);
        perft_zobrist(&mut position, 4);
        assert_eq!(hash, position.zobrist_hash());
        assert_eq!(hash, position.key);
    }
}
