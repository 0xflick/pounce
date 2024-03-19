use rand::Rng;

use lazy_static::lazy_static;

lazy_static! {
    pub static ref ZOBRIST: Zobrist = Zobrist::new();
}

#[derive(Clone, Debug)]
pub struct Zobrist {
    pub pieces: [[[u64; 8]; 8]; 12],
    pub castling: [u64; 16],
    pub en_passant: [u64; 8],
    pub black: u64,
}

#[allow(clippy::needless_range_loop)]
impl Zobrist {
    fn new() -> Zobrist {
        let mut rng = rand::thread_rng();
        let mut pieces = [[[0; 8]; 8]; 12];
        let mut castling = [0; 16];
        let mut en_passant = [0; 8];
        let black = rng.gen();

        for i in 0..12 {
            for j in 0..8 {
                for k in 0..8 {
                    pieces[i][j][k] = rng.gen();
                }
            }
        }
        for i in 0..16 {
            castling[i] = rng.gen();
        }
        for i in 0..8 {
            en_passant[i] = rng.gen();
        }

        Zobrist {
            pieces,
            castling,
            en_passant,
            black,
        }
    }
}
