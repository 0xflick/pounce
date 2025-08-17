use std::fs::File;
use std::io::{Read, Result, Error, ErrorKind};

use crate::chess::{Accumulator, Color, Move, Piece, Position, Square};

pub struct NNUEAccumulator<'a, const HIDDEN_SIZE: usize> {
    white_persp: [f32; HIDDEN_SIZE],
    black_persp: [f32; HIDDEN_SIZE],

    stack: Vec<([f32; HIDDEN_SIZE], [f32; HIDDEN_SIZE])>,

    pub net: &'a PerspectiveNet<HIDDEN_SIZE>,
}

impl<'a, const HIDDEN_SIZE: usize> NNUEAccumulator<'a, HIDDEN_SIZE> {
    pub fn new(net: &'a PerspectiveNet<HIDDEN_SIZE>) -> Self {
        Self {
            white_persp: [0.0; HIDDEN_SIZE],
            black_persp: [0.0; HIDDEN_SIZE],
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

                for i in 0..HIDDEN_SIZE {
                    self.white_persp[i] += self.net.persp_weights[i][white_index];
                    self.black_persp[i] += self.net.persp_weights[i][black_index]
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
        for i in 0..HIDDEN_SIZE {
            self.white_persp[i] += self.net.persp_weights[i][index(&piece, sq)];
            self.black_persp[i] += self.net.persp_weights[i][index(&piece.flip(), sq.flip())];
        }
    }
    fn on_make_move_discard(&mut self, sq: Square, piece: Piece) {
        for i in 0..HIDDEN_SIZE {
            self.white_persp[i] -= self.net.persp_weights[i][index(&piece, sq)];
            self.black_persp[i] -= self.net.persp_weights[i][index(&piece.flip(), sq.flip())];
        }
    }

    fn on_unmake_move_set(&mut self, _: Square, _: Piece) {}
    fn on_unmake_move_discard(&mut self, _: Square, _: Piece) {}
}

pub struct PerspectiveNet<const HIDDEN_SIZE: usize> {
    persp_weights: [[f32; 768]; HIDDEN_SIZE],
    persp_bias: [f32; HIDDEN_SIZE],

    output_weights: [[f32; 2]; HIDDEN_SIZE],
    output_bias: f32,
}

impl<const HIDDEN_SIZE: usize> PerspectiveNet<HIDDEN_SIZE> {
    pub fn new(
        persp_weights: [[f32; 768]; HIDDEN_SIZE],
        persp_bias: [f32; HIDDEN_SIZE],
        output_weights: [[f32; 2]; HIDDEN_SIZE],
        output_bias: f32,
    ) -> Self {
        Self {
            persp_weights,
            persp_bias,
            output_weights,
            output_bias,
        }
    }
}

impl<const HIDDEN_SIZE: usize> PerspectiveNet<HIDDEN_SIZE> {
    pub fn forward(&self, accumulator: &NNUEAccumulator<HIDDEN_SIZE>, stm: Color) -> f32 {
        let mut output = self.output_bias;

        match stm {
            Color::White => {
                for i in 0..HIDDEN_SIZE {
                    output += self.output_weights[i][0] * accumulator.white_persp[i].max(0.0);
                    output += self.output_weights[i][1] * accumulator.black_persp[i].max(0.0);
                }
            }
            Color::Black => {
                for i in 0..HIDDEN_SIZE {
                    output += self.output_weights[i][0] * accumulator.black_persp[i].max(0.0);
                    output += self.output_weights[i][1] * accumulator.white_persp[i].max(0.0);
                }
            }
        }

        output
    }
}

const HEADER_SIZE: usize = 32;
const MAGIC_NUMBER: [u8; 4] = [b'P', b'N', b'C', b'E'];

#[repr(u16)]
#[derive(Debug, Clone, Copy)]
enum ModelType {
    _Unknown = 0,
    Net768,
}
impl<const HIDDEN_SIZE: usize> PerspectiveNet<HIDDEN_SIZE> {
    pub fn load(path: &str) -> Result<Self> {
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        
        // Validate header
        if buffer.len() < HEADER_SIZE {
            return Err(Error::new(ErrorKind::InvalidData, "File too small"));
        }
        
        if buffer[0..4] != MAGIC_NUMBER {
            return Err(Error::new(ErrorKind::InvalidData, "Invalid magic number"));
        }
        
        let version = u16::from_le_bytes([buffer[4], buffer[5]]);
        let model_type = u16::from_le_bytes([buffer[6], buffer[7]]);
        let hidden_size = u32::from_le_bytes([buffer[8], buffer[9], buffer[10], buffer[11]]) as usize;

        if version != 1 {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!("Unsupported version: {}", version)
            ));
        }
        
        if model_type != ModelType::Net768 as u16 {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!("Unexpected model type: {}", model_type)
            ));
        }
        
        if hidden_size != HIDDEN_SIZE {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!("Hidden size mismatch: file has {}, expected {}", hidden_size, HIDDEN_SIZE)
            ));
        }
        
        // Calculate expected file size
        let expected_size = HEADER_SIZE + 
            (768 * HIDDEN_SIZE * 4) +  // persp_weights
            (HIDDEN_SIZE * 4) +         // persp_bias
            (2 * HIDDEN_SIZE * 4) +     // output_weights
            4;                          // output_bias
            
        if buffer.len() != expected_size {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!("File size mismatch: expected {} bytes, got {}", expected_size, buffer.len())
            ));
        }
        
        // Parse weights using chunks
        let data = &buffer[HEADER_SIZE..];
        let mut chunks = data.chunks_exact(4);
        
        // Read persp_weights transposed
        let mut persp_weights = [[0.0f32; 768]; HIDDEN_SIZE];
        for feature_idx in 0..768 {
            for hidden_column in persp_weights.iter_mut() {
                let bytes = chunks.next().unwrap();
                let val = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                hidden_column[feature_idx] = val;
            }
        }
        
        // Read persp_bias
        let mut persp_bias = [0.0f32; HIDDEN_SIZE];
        for val in persp_bias.iter_mut() {
            let bytes = chunks.next().unwrap();
            *val = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        }
        
        // Read output_weights
        let mut output_weights = [[0.0f32; 2]; HIDDEN_SIZE];

        // Read all weights for output 0 first
        for col in output_weights.iter_mut() {
            let bytes = chunks.next().unwrap();
            col[0] = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        }

        // Then read all weights for output 1
        for col in output_weights.iter_mut() {
            let bytes = chunks.next().unwrap();
            col[1] = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        }
        
        // Read output_bias
        let bytes = chunks.next().unwrap();
        let output_bias = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        
        Ok(Self::new(persp_weights, persp_bias, output_weights, output_bias))
    }
}
