use std::io::Result;

use crate::chess::{Accumulator, Color, Move, Piece, Position, Square};

pub type HiddenSize = usize;
pub const NNUE_HIDDEN_SIZE: HiddenSize = 128;

const QA: i32 = 255;
const QB: i32 = 64;
const SCALE: i32 = 216;

#[repr(C, align(16))]
struct Align16<T>(pub T);

impl<T> std::ops::Deref for Align16<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> std::ops::DerefMut for Align16<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[repr(C)]
struct FileHeader {
    magic: [u8; 4],
    version: u16,
    model_type: u16,
    hidden_size: u32,
    extra: [u8; 20],
}

#[repr(C)]
struct NetworkData<const HIDDEN_SIZE: usize> {
    header: FileHeader,
    persp_weights: [[i16; HIDDEN_SIZE]; 768], // Feature-major: for each of 768 features, HIDDEN_SIZE weights
    persp_bias: [i16; HIDDEN_SIZE],
    output_weights: [[i16; HIDDEN_SIZE]; 2], // Output-major: weights for output 0, then weights for output 1
    output_bias: i16,
}

// Static network loaded at compile time via transmute
static NETWORK: NetworkData<NNUE_HIDDEN_SIZE> =
    unsafe { std::mem::transmute(*include_bytes!("../../../nets/net.pnn")) };

pub struct NNUEAccumulator<'a, const HIDDEN_SIZE: usize> {
    white_persp: [i16; HIDDEN_SIZE],
    black_persp: [i16; HIDDEN_SIZE],

    stack: Vec<([i16; HIDDEN_SIZE], [i16; HIDDEN_SIZE])>,

    pub net: &'a PerspectiveNet<HIDDEN_SIZE>,
}

impl<'a, const HIDDEN_SIZE: usize> NNUEAccumulator<'a, HIDDEN_SIZE> {
    pub fn new(net: &'a PerspectiveNet<HIDDEN_SIZE>) -> Self {
        Self {
            white_persp: [0; HIDDEN_SIZE],
            black_persp: [0; HIDDEN_SIZE],
            stack: Vec::new(),
            net,
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
        for i in 0..HIDDEN_SIZE {
            self.white_persp[i] = self.net.persp_bias[i];
            self.black_persp[i] = self.net.persp_bias[i];
        }

        for (sq, maybe_piece) in pos.mailbox.iter().enumerate() {
            if let Some(piece) = maybe_piece {
                let sq = Square::new(sq as u8);

                let white_index = index(piece, sq);
                let black_index = index(&piece.flip(), sq.flip());

                let white_weights = &self.net.persp_weights.0[white_index];
                let black_weights = &self.net.persp_weights.0[black_index];

                for i in 0..HIDDEN_SIZE {
                    self.white_persp[i] += white_weights[i];
                    self.black_persp[i] += black_weights[i];
                }
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
        let white_idx = index(&piece, sq);
        let black_idx = index(&piece.flip(), sq.flip());

        let white_weights = &self.net.persp_weights.0[white_idx];
        let black_weights = &self.net.persp_weights.0[black_idx];

        for i in 0..HIDDEN_SIZE {
            self.white_persp[i] += white_weights[i];
            self.black_persp[i] += black_weights[i];
        }
    }
    fn on_make_move_discard(&mut self, sq: Square, piece: Piece) {
        let white_idx = index(&piece, sq);
        let black_idx = index(&piece.flip(), sq.flip());

        let white_weights = &self.net.persp_weights.0[white_idx];
        let black_weights = &self.net.persp_weights.0[black_idx];

        for i in 0..HIDDEN_SIZE {
            self.white_persp[i] -= white_weights[i];
            self.black_persp[i] -= black_weights[i];
        }
    }

    fn on_unmake_move_set(&mut self, _: Square, _: Piece) {}
    fn on_unmake_move_discard(&mut self, _: Square, _: Piece) {}
}

pub struct PerspectiveNet<const HIDDEN_SIZE: usize> {
    persp_weights: Box<Align16<[[i16; HIDDEN_SIZE]; 768]>>,
    persp_bias: Align16<[i16; HIDDEN_SIZE]>,

    output_weights: Align16<[[i16; HIDDEN_SIZE]; 2]>,
    output_bias: i16,
}

impl<const HIDDEN_SIZE: usize> PerspectiveNet<HIDDEN_SIZE> {
    pub fn new(
        persp_weights: [[i16; HIDDEN_SIZE]; 768],
        persp_bias: [i16; HIDDEN_SIZE],
        output_weights: [[i16; HIDDEN_SIZE]; 2],
        output_bias: i16,
    ) -> Self {
        Self {
            persp_weights: Box::new(Align16(persp_weights)),
            persp_bias: Align16(persp_bias),
            output_weights: Align16(output_weights),
            output_bias,
        }
    }
}

fn screlu(v: i16) -> i32 {
    let clamped = v.clamp(0, QA as i16) as i32;
    clamped * clamped
}

impl<const HIDDEN_SIZE: usize> PerspectiveNet<HIDDEN_SIZE> {
    pub fn forward(&self, accumulator: &NNUEAccumulator<HIDDEN_SIZE>, stm: Color) -> i32 {
        let (us, them) = match stm {
            Color::White => (&accumulator.white_persp, &accumulator.black_persp),
            Color::Black => (&accumulator.black_persp, &accumulator.white_persp),
        };

        let mut output = 0;

        // Process our perspective
        for (i, (&us_accum, &them_accum)) in us.iter().zip(them.iter()).enumerate() {
            output += screlu(us_accum) * self.output_weights[0][i] as i32;
            output += screlu(them_accum) * self.output_weights[1][i] as i32;
        }

        output /= QA;
        output += self.output_bias as i32;

        output *= SCALE;
        output /= QA * QB;

        output
    }
}

impl PerspectiveNet<NNUE_HIDDEN_SIZE> {
    pub fn load() -> Result<Self> {
        // // Convert output weights from [2][HIDDEN_SIZE] to [HIDDEN_SIZE][2]
        // let output_weights =
        //     std::array::from_fn(|i| [NETWORK.output_weights[0][i], NETWORK.output_weights[1][i]]);

        Ok(Self::new(
            NETWORK.persp_weights,
            NETWORK.persp_bias,
            NETWORK.output_weights,
            NETWORK.output_bias,
        ))
    }
}
