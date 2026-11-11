mod simd;

use std::io::Result;

use simd::SIMD_WEIGHT_LIMIT;
pub use simd::Simd;

use crate::chess::{Accumulator, Color, Move, Piece, Position, Square};

pub type HiddenSize = usize;
pub const NNUE_HIDDEN_SIZE: HiddenSize = 384;
const BUCKETS: usize = 8;

const QA: i32 = 255;
const QB: i32 = 64;
const SCALE: i32 = 216;

/// Aligned to a cache line, so that the SIMD loads never straddle two.
#[derive(Clone, Copy)]
#[repr(C, align(64))]
struct Align64<T>(pub T);

impl<T> std::ops::Deref for Align64<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> std::ops::DerefMut for Align64<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// The net's header. The trainer records what the net needs here, and nets from before it did have
/// zeros in those fields.
#[repr(C)]
struct FileHeader {
    magic: [u8; 4],
    version: u16,
    model_type: u16,
    hidden_size: u32,
    buckets: u32,
    qa: u16,
    qb: u16,
    scale: u32,
    reserved: [u8; 8],
}

#[repr(C)]
struct NetworkData<const HIDDEN_SIZE: usize> {
    header: FileHeader,
    persp_weights: [[i16; HIDDEN_SIZE]; 768], // Feature-major: for each of 768 features, HIDDEN_SIZE weights
    persp_bias: [i16; HIDDEN_SIZE],
    output_weights: [[[i16; HIDDEN_SIZE]; 2]; BUCKETS],
    output_bias: [i16; BUCKETS],
}

// Static network loaded at compile time via transmute
static NETWORK: NetworkData<NNUE_HIDDEN_SIZE> =
    unsafe { std::mem::transmute(*include_bytes!("../../../nets/net.pnn")) };

/// Nothing checks the net at runtime, and a mismatch only shows up as bad evals, so the build fails
/// instead. A size mismatch already fails at the transmute above, but two different nets can be the
/// same size. Fix these with the trainer's `tools/export_weights.py install`.
const _: () = {
    let header = &NETWORK.header;
    let magic = &header.magic;
    assert!(
        magic[0] == b'P' && magic[1] == b'N' && magic[2] == b'C' && magic[3] == b'E',
        "nets/net.pnn is not a net"
    );
    assert!(
        header.hidden_size as usize == NNUE_HIDDEN_SIZE,
        "NNUE_HIDDEN_SIZE doesn't match nets/net.pnn"
    );
    assert!(
        header.buckets == 0 || header.buckets as usize == BUCKETS,
        "BUCKETS doesn't match nets/net.pnn"
    );
    assert!(
        header.qa == 0 || header.qa as i32 == QA,
        "QA doesn't match nets/net.pnn"
    );
    assert!(
        header.qb == 0 || header.qb as i32 == QB,
        "QB doesn't match nets/net.pnn"
    );
    assert!(
        header.scale == 0 || header.scale as i32 == SCALE,
        "SCALE doesn't match nets/net.pnn"
    );
};

/// One perspective's hidden layer, before the activation.
type Persp<const HIDDEN_SIZE: usize> = Align64<[i16; HIDDEN_SIZE]>;

pub struct NNUEAccumulator<'a, const HIDDEN_SIZE: usize> {
    white_persp: Persp<HIDDEN_SIZE>,
    black_persp: Persp<HIDDEN_SIZE>,

    stack: Vec<(Persp<HIDDEN_SIZE>, Persp<HIDDEN_SIZE>)>,

    pub net: &'a PerspectiveNet<HIDDEN_SIZE>,
}

impl<'a, const HIDDEN_SIZE: usize> NNUEAccumulator<'a, HIDDEN_SIZE> {
    pub fn new(net: &'a PerspectiveNet<HIDDEN_SIZE>) -> Self {
        Self {
            white_persp: Align64([0; HIDDEN_SIZE]),
            black_persp: Align64([0; HIDDEN_SIZE]),
            stack: Vec::new(),
            net,
        }
    }

    /// Adds (or with `ADD` false, subtracts) a piece on a square to both perspectives.
    #[inline]
    fn update<const ADD: bool>(&mut self, sq: Square, piece: Piece) {
        let white_weights = &self.net.persp_weights[index(&piece, sq)];
        let black_weights = &self.net.persp_weights[index(&piece.flip(), sq.flip())];
        let (white, black) = (&mut self.white_persp.0, &mut self.black_persp.0);

        match self.net.simd {
            Simd::Scalar => {
                simd::scalar::update::<HIDDEN_SIZE, ADD>(white, black, white_weights, black_weights)
            }
            #[cfg(target_arch = "x86_64")]
            // SAFETY: the net only picks AVX2 on a CPU that has it.
            Simd::Avx2 => unsafe {
                simd::avx2::update::<HIDDEN_SIZE, ADD>(white, black, white_weights, black_weights)
            },
            #[cfg(target_arch = "aarch64")]
            Simd::Neon => {
                simd::neon::update::<HIDDEN_SIZE, ADD>(white, black, white_weights, black_weights)
            }
        }
    }
}

fn index(piece: &Piece, sq: Square) -> usize {
    let color_offset = match piece.color {
        Color::White => 0,
        Color::Black => 384,
    };

    let piece_offset = (piece.role as usize) * 64;

    color_offset + piece_offset + sq as usize
}

impl<const HIDDEN_SIZE: usize> Accumulator for NNUEAccumulator<'_, HIDDEN_SIZE> {
    fn reset(&mut self, pos: &Position) {
        self.white_persp = self.net.persp_bias;
        self.black_persp = self.net.persp_bias;

        for (sq, maybe_piece) in pos.mailbox.iter().enumerate() {
            if let Some(piece) = maybe_piece {
                self.update::<true>(Square::new(sq as u8), *piece);
            }
        }
    }

    fn on_make_move(&mut self, _: Move) {
        self.stack.push((self.white_persp, self.black_persp));
    }
    fn on_unmake_move(&mut self, _: Move) {
        let (white_persp, black_persp) = self.stack.pop().expect("Stack underflow on unmake move");
        self.white_persp = white_persp;
        self.black_persp = black_persp;
    }

    fn on_make_move_set(&mut self, sq: Square, piece: Piece) {
        self.update::<true>(sq, piece);
    }
    fn on_make_move_discard(&mut self, sq: Square, piece: Piece) {
        self.update::<false>(sq, piece);
    }

    fn on_unmake_move_set(&mut self, _: Square, _: Piece) {}
    fn on_unmake_move_discard(&mut self, _: Square, _: Piece) {}
}

/// An output weight the SIMD paths' clamp took something off: `weight` is what it took off, from
/// the weight for `index` in perspective `persp` (0 for the side to move).
#[derive(Clone, Copy, Debug)]
struct Remainder {
    persp: usize,
    index: usize,
    weight: i32,
}

pub struct PerspectiveNet<const HIDDEN_SIZE: usize> {
    persp_weights: Box<Align64<[[i16; HIDDEN_SIZE]; 768]>>,
    persp_bias: Align64<[i16; HIDDEN_SIZE]>,

    output_weights: Align64<[[[i16; HIDDEN_SIZE]; 2]; BUCKETS]>,
    output_bias: Align64<[i16; BUCKETS]>,

    /// `output_weights` clamped to `±SIMD_WEIGHT_LIMIT`, for the SIMD paths. The trainer doesn't
    /// clip output weights, so a few can be outside it.
    clamped_output_weights: Align64<[[[i16; HIDDEN_SIZE]; 2]; BUCKETS]>,
    /// For each bucket, what the clamp took off.
    output_remainders: [Vec<Remainder>; BUCKETS],

    simd: Simd,
}

impl<const HIDDEN_SIZE: usize> PerspectiveNet<HIDDEN_SIZE> {
    pub fn new(
        persp_weights: [[i16; HIDDEN_SIZE]; 768],
        persp_bias: [i16; HIDDEN_SIZE],
        output_weights: [[[i16; HIDDEN_SIZE]; 2]; BUCKETS],
        output_bias: [i16; BUCKETS],
    ) -> Self {
        const {
            assert!(
                HIDDEN_SIZE.is_multiple_of(simd::CHUNK),
                "the SIMD paths need HIDDEN_SIZE to be a multiple of 32"
            )
        };

        let mut clamped_output_weights = output_weights;
        let output_remainders = std::array::from_fn(|bucket| {
            let mut remainders = Vec::new();
            let bucket: &mut [[i16; HIDDEN_SIZE]; 2] = &mut clamped_output_weights[bucket];
            for (persp, weights) in bucket.iter_mut().enumerate() {
                for (index, weight) in weights.iter_mut().enumerate() {
                    let clamped = (*weight).clamp(-SIMD_WEIGHT_LIMIT, SIMD_WEIGHT_LIMIT);
                    if clamped != *weight {
                        remainders.push(Remainder {
                            persp,
                            index,
                            weight: *weight as i32 - clamped as i32,
                        });
                        *weight = clamped;
                    }
                }
            }
            remainders
        });

        Self {
            persp_weights: Box::new(Align64(persp_weights)),
            persp_bias: Align64(persp_bias),
            output_weights: Align64(output_weights),
            output_bias: Align64(output_bias),
            clamped_output_weights: Align64(clamped_output_weights),
            output_remainders,
            simd: Simd::detect(),
        }
    }

    /// Uses `simd` for the net's loops, instead of the fastest this CPU has. They all compute the
    /// same thing, so this is for comparing them.
    ///
    /// # Panics
    ///
    /// If this CPU can't run `simd`.
    pub fn with_simd(mut self, simd: Simd) -> Self {
        assert!(
            Simd::available().contains(&simd),
            "this CPU can't run {simd:?}"
        );
        self.simd = simd;
        self
    }
}

#[inline(always)]
fn screlu(v: i16) -> i32 {
    let clamped = v.clamp(0, QA as i16) as i32;
    clamped * clamped
}

impl<const HIDDEN_SIZE: usize> PerspectiveNet<HIDDEN_SIZE> {
    pub fn forward(&self, accumulator: &NNUEAccumulator<HIDDEN_SIZE>, pos: &Position) -> i32 {
        let (us, them) = match pos.side {
            Color::White => (&accumulator.white_persp, &accumulator.black_persp),
            Color::Black => (&accumulator.black_persp, &accumulator.white_persp),
        };

        let bucket = ((pos.occupancy.count() as usize - 2) * BUCKETS / (32 - 2)).min(BUCKETS - 1);
        let mut output = self.screlu_dot(us, them, bucket);

        output /= QA;
        output += self.output_bias[bucket] as i32 * QA;

        output *= SCALE;
        output /= QA * QB;

        output
    }

    /// `Σ screlu(us) * w_us + Σ screlu(them) * w_them` with the bucket's output weights, wrapping
    /// on overflow.
    #[inline]
    fn screlu_dot(&self, us: &[i16; HIDDEN_SIZE], them: &[i16; HIDDEN_SIZE], bucket: usize) -> i32 {
        let clamped = &self.clamped_output_weights[bucket];
        let sum = match self.simd {
            Simd::Scalar => {
                return simd::scalar::screlu_dot(us, them, &self.output_weights[bucket]);
            }
            #[cfg(target_arch = "x86_64")]
            // SAFETY: the net only picks AVX2 on a CPU that has it.
            Simd::Avx2 => unsafe { simd::avx2::screlu_dot(us, them, clamped) },
            #[cfg(target_arch = "aarch64")]
            Simd::Neon => simd::neon::screlu_dot(us, them, clamped),
        };

        // Adding back what the clamp took off makes it exact, since wrapping addition doesn't
        // depend on the order.
        let accs = [us, them];
        self.output_remainders[bucket].iter().fold(sum, |sum, r| {
            sum.wrapping_add(screlu(accs[r.persp][r.index]).wrapping_mul(r.weight))
        })
    }
}

impl PerspectiveNet<NNUE_HIDDEN_SIZE> {
    pub fn load() -> Result<Self> {
        Ok(Self::new(
            NETWORK.persp_weights,
            NETWORK.persp_bias,
            NETWORK.output_weights,
            NETWORK.output_bias,
        ))
    }
}

#[cfg(test)]
mod tests {
    use rand::prelude::IndexedRandom;
    use rand::rngs::SmallRng;
    use rand::{Rng, SeedableRng};

    use super::*;
    use crate::chess::movegen::MoveGen;
    use crate::chess::position::fen::Fen;
    use crate::chess::{Color, Role};
    use crate::engine::bench::BENCHMARK_FENS;
    use crate::init;

    /// Passes every update to two accumulators.
    struct Both<'a, 'n, const H: usize>(
        &'a mut NNUEAccumulator<'n, H>,
        &'a mut NNUEAccumulator<'n, H>,
    );

    impl<const H: usize> Accumulator for Both<'_, '_, H> {
        fn reset(&mut self, pos: &Position) {
            self.0.reset(pos);
            self.1.reset(pos);
        }
        fn on_make_move(&mut self, mv: Move) {
            self.0.on_make_move(mv);
            self.1.on_make_move(mv);
        }
        fn on_unmake_move(&mut self, mv: Move) {
            self.0.on_unmake_move(mv);
            self.1.on_unmake_move(mv);
        }
        fn on_make_move_set(&mut self, sq: Square, piece: Piece) {
            self.0.on_make_move_set(sq, piece);
            self.1.on_make_move_set(sq, piece);
        }
        fn on_make_move_discard(&mut self, sq: Square, piece: Piece) {
            self.0.on_make_move_discard(sq, piece);
            self.1.on_make_move_discard(sq, piece);
        }
        fn on_unmake_move_set(&mut self, sq: Square, piece: Piece) {
            self.0.on_unmake_move_set(sq, piece);
            self.1.on_unmake_move_set(sq, piece);
        }
        fn on_unmake_move_discard(&mut self, sq: Square, piece: Piece) {
            self.0.on_unmake_move_discard(sq, piece);
            self.1.on_unmake_move_discard(sq, piece);
        }
    }

    fn assert_same_accumulators<const H: usize>(
        a: &NNUEAccumulator<H>,
        b: &NNUEAccumulator<H>,
        what: &str,
    ) {
        assert_eq!(
            a.white_persp.0, b.white_persp.0,
            "white accumulator, {what}"
        );
        assert_eq!(
            a.black_persp.0, b.black_persp.0,
            "black accumulator, {what}"
        );
    }

    /// Plays random games from the bench positions, updating a scalar and a SIMD accumulator
    /// together, then takes the moves back. After every move and every take-back, the two
    /// accumulators, a SIMD accumulator reset from scratch and the two evals must all agree.
    #[test]
    fn simd_matches_scalar_through_make_and_unmake() {
        init();
        let scalar_net = PerspectiveNet::load().unwrap().with_simd(Simd::Scalar);

        for simd in Simd::available() {
            let net = PerspectiveNet::load().unwrap().with_simd(simd);
            let mut rng = SmallRng::seed_from_u64(0x5ca1a7);
            let mut positions = 0;

            for (n, fen) in BENCHMARK_FENS.iter().enumerate() {
                let Fen(mut pos) = fen.parse().unwrap();
                let mut scalar = NNUEAccumulator::new(&scalar_net);
                let mut fast = NNUEAccumulator::new(&net);
                let mut fresh = NNUEAccumulator::new(&net);
                Both(&mut scalar, &mut fast).reset(&pos);

                let mut check = |scalar: &NNUEAccumulator<NNUE_HIDDEN_SIZE>,
                                 fast: &NNUEAccumulator<NNUE_HIDDEN_SIZE>,
                                 pos: &Position,
                                 what: &str| {
                    let what = format!("{simd:?}, bench position {n}, {what}");
                    assert_same_accumulators(scalar, fast, &what);
                    fresh.reset(pos);
                    assert_same_accumulators(&fresh, fast, &format!("{what}, after reset"));
                    assert_eq!(
                        scalar_net.forward(scalar, pos),
                        net.forward(fast, pos),
                        "eval, {what}"
                    );
                    positions += 1;
                };

                check(&scalar, &fast, &pos, "start");
                let mut moves = Vec::new();
                for ply in 0..rng.random_range(8..24) {
                    let legal: Vec<Move> = MoveGen::new(&pos).collect();
                    let Some(&mv) = legal.choose(&mut rng) else {
                        break;
                    };
                    pos.make_move_with(mv, &mut Both(&mut scalar, &mut fast));
                    moves.push(mv);
                    check(&scalar, &fast, &pos, &format!("after {} moves", ply + 1));
                }
                while let Some(mv) = moves.pop() {
                    pos.unmake_move_with(mv, &mut Both(&mut scalar, &mut fast));
                    check(
                        &scalar,
                        &fast,
                        &pos,
                        &format!("back to {} moves", moves.len()),
                    );
                }
            }
            assert!(positions > 500, "only checked {positions} positions");
        }
    }

    /// Weights and accumulators anywhere in the i16 range, including the edges, where the SIMD
    /// paths' 16-bit products would overflow without the clamp, must still give exactly what the
    /// scalar path gives.
    #[test]
    fn simd_matches_scalar_on_extreme_values() {
        const H: usize = 64;
        let mut rng = SmallRng::seed_from_u64(0xed6e);
        let value = |rng: &mut SmallRng| -> i16 {
            const EDGES: [i16; 12] = [
                i16::MIN,
                i16::MIN + 1,
                -SIMD_WEIGHT_LIMIT - 1,
                -SIMD_WEIGHT_LIMIT,
                -1,
                0,
                1,
                SIMD_WEIGHT_LIMIT,
                SIMD_WEIGHT_LIMIT + 1,
                QA as i16,
                QA as i16 + 1,
                i16::MAX,
            ];
            match rng.random_range(0..4) {
                0 => *EDGES.choose(rng).unwrap(),
                1 => rng.random(),
                _ => rng.random_range(-400..400),
            }
        };

        for simd in Simd::available() {
            let persp_weights: [[i16; H]; 768] =
                std::array::from_fn(|_| std::array::from_fn(|_| value(&mut rng)));
            let persp_bias: [i16; H] = std::array::from_fn(|_| value(&mut rng));
            let output_weights: [[[i16; H]; 2]; BUCKETS] = std::array::from_fn(|_| {
                std::array::from_fn(|_| std::array::from_fn(|_| value(&mut rng)))
            });
            let output_bias: [i16; BUCKETS] = std::array::from_fn(|_| value(&mut rng));
            let scalar_net =
                PerspectiveNet::new(persp_weights, persp_bias, output_weights, output_bias)
                    .with_simd(Simd::Scalar);
            let net = PerspectiveNet::new(persp_weights, persp_bias, output_weights, output_bias)
                .with_simd(simd);
            assert!(
                net.output_remainders.iter().any(|r| !r.is_empty()),
                "nothing to add back"
            );

            let mut scalar = NNUEAccumulator::new(&scalar_net);
            let mut fast = NNUEAccumulator::new(&net);
            for round in 0..2000 {
                if round % 50 == 0 {
                    let start: [[i16; H]; 2] =
                        std::array::from_fn(|_| std::array::from_fn(|_| value(&mut rng)));
                    (scalar.white_persp.0, scalar.black_persp.0) = (start[0], start[1]);
                    (fast.white_persp.0, fast.black_persp.0) = (start[0], start[1]);
                }
                let sq = Square::new(rng.random_range(0..64));
                let piece = Piece {
                    color: if rng.random() {
                        Color::White
                    } else {
                        Color::Black
                    },
                    role: *Role::ALL.choose(&mut rng).unwrap(),
                };
                if rng.random() {
                    scalar.update::<true>(sq, piece);
                    fast.update::<true>(sq, piece);
                } else {
                    scalar.update::<false>(sq, piece);
                    fast.update::<false>(sq, piece);
                }
                assert_same_accumulators(&scalar, &fast, &format!("{simd:?}, round {round}"));

                for (bucket, weights) in output_weights.iter().enumerate() {
                    let (us, them) = (&fast.white_persp.0, &fast.black_persp.0);
                    let expected: i64 = (0..H)
                        .map(|i| {
                            screlu(us[i]) as i64 * weights[0][i] as i64
                                + screlu(them[i]) as i64 * weights[1][i] as i64
                        })
                        .sum();
                    for (net, persp) in [(&scalar_net, &scalar), (&net, &fast)] {
                        assert_eq!(
                            net.screlu_dot(&persp.white_persp, &persp.black_persp, bucket),
                            expected as i32,
                            "{:?}, round {round}, bucket {bucket}",
                            net.simd
                        );
                    }
                }
            }
        }
    }
}
