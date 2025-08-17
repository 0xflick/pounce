use std::fmt::{self, Debug, Display, Formatter};
use std::num::NonZeroU16;

use anyhow::Context;

use crate::chess::bitboard::Bitboard;
use crate::chess::position::CastleRights;
use crate::chess::{self, Color, Move, Piece, Position, Role, Square};
use crate::datagen::utils::U4Array32;

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum Wdl {
    BlackWin,
    Draw,
    WhiteWin,
    #[default]
    Unknown,
}

impl Wdl {
    pub fn flip(&self) -> Self {
        match self {
            Wdl::BlackWin => Wdl::WhiteWin,
            Wdl::Draw => Wdl::Draw,
            Wdl::WhiteWin => Wdl::BlackWin,
            Wdl::Unknown => Wdl::Unknown,
        }
    }
}

impl Display for Wdl {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Wdl::Unknown => write!(f, "Unknown"),
            Wdl::BlackWin => write!(f, "0-1"),
            Wdl::Draw => write!(f, "1/2-1/2"),
            Wdl::WhiteWin => write!(f, "1-0"),
        }
    }
}

impl From<u8> for Wdl {
    fn from(val: u8) -> Self {
        match val {
            0 => Wdl::BlackWin,
            1 => Wdl::Draw,
            2 => Wdl::WhiteWin,
            _ => Wdl::Unknown,
        }
    }
}

// 32 bytes (needs to be a multiple of 8 because that's the alignment of Bitboard)
#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq, Default)]
pub struct CompressedPosition {
    occ: Bitboard,      // 8 bytes
    pieces: U4Array32,  // 16 bytes
    score: i16,         // 2 bytes
    halfmove_clock: u8, // 1 byte
    pub wdl: Wdl,       // 1 byte
    ply: u16,           // 2 byte
    stm_ep_square: u8,  // 1 byte
    castling: u8,       // 1 bytes
}

impl CompressedPosition {
    pub fn new(pos: &Position, score: i16, wdl: Wdl) -> Self {
        let mut pieces = U4Array32::default();
        for (idx, sq) in pos.occupancy.enumerate() {
            let pc = pos.mailbox[sq].unwrap();
            let bit_pc = ((pc.color as u8) << 3) | (pc.role as u8);

            pieces.set(idx, bit_pc);
        }

        Self {
            occ: pos.occupancy,
            pieces,
            score,
            wdl,
            stm_ep_square: pos.ep_square.map_or(u8::MAX, |sq| sq as u8),
            halfmove_clock: pos.halfmove_clock,
            ply: (pos.fullmove_number.get() - 1) * 2 + pos.side as u16,
            castling: pos.castling.bits(),
        }
    }

    pub fn flip(&self) -> Self {
        let mut mailbox: [Option<Piece>; 64] = [None; 64];

        for (idx, sq) in self.occ.enumerate() {
            let pc = self.pieces.get(idx);

            let color = if pc >> 3 == 0 {
                Color::White
            } else {
                Color::Black
            };

            mailbox[(sq) as usize] = Some(Piece {
                color,
                role: unsafe { std::mem::transmute::<u8, Role>(pc & 0b111) },
            });
        }

        let occ = self.occ.flip();
        let mut pieces = U4Array32::default();
        for (idx, sq) in occ.enumerate() {
            let pc = mailbox[sq ^ 56].unwrap();
            let bit_pc = ((1 - pc.color as u8) << 3) | (pc.role as u8);

            pieces.set(idx, bit_pc);
        }

        let ep_square = if self.stm_ep_square == u8::MAX {
            u8::MAX
        } else {
            self.stm_ep_square ^ 56
        };

        let new_ply = if self.ply % 2 == 0 {
            self.ply + 1
        } else {
            self.ply - 1
        };

        Self {
            occ,
            pieces,
            score: -self.score,
            wdl: self.wdl.flip(),
            stm_ep_square: ep_square,
            halfmove_clock: self.halfmove_clock,
            ply: new_ply,
            castling: self.castling,
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(self as *const _ as *const u8, std::mem::size_of::<Self>())
        }
    }

    pub fn as_mut_bytes(&mut self) -> &mut [u8] {
        unsafe {
            std::slice::from_raw_parts_mut(self as *mut _ as *mut u8, std::mem::size_of::<Self>())
        }
    }
}

impl Debug for CompressedPosition {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("CompressedPosition")
            .field("occ", &self.occ)
            .field("pieces", &self.pieces)
            .field("score", &self.score)
            .field("wdl", &self.wdl)
            .field("halfmove_clock", &self.halfmove_clock)
            .field("ply", &self.ply)
            .field(
                "stm_ep_square",
                &if self.stm_ep_square == u8::MAX {
                    "None".to_string()
                } else {
                    format!("{}", Square::from(self.stm_ep_square))
                },
            )
            .field("castling", &format!("{:04b}", self.castling))
            .finish()
    }
}

// 4 bytes
#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq, Default)]
pub struct CompressedMove {
    mv: Move,   // 2 bytes
    score: i16, // 2 bytes
}

impl CompressedMove {
    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(self as *const _ as *const u8, std::mem::size_of::<Self>())
        }
    }

    pub fn as_mut_bytes(&mut self) -> &mut [u8] {
        unsafe {
            std::slice::from_raw_parts_mut(self as *mut _ as *mut u8, std::mem::size_of::<Self>())
        }
    }
}

const NULL_TERMINATOR: [u8; 4] = [0; 4];

pub struct CompressedGame {
    pub initial: CompressedPosition,
    pub moves: Vec<CompressedMove>,
}

impl CompressedGame {
    pub fn new(initial: &Position) -> Self {
        Self {
            initial: CompressedPosition::new(initial, 0, Wdl::Unknown),
            moves: Vec::new(),
        }
    }

    pub fn push_move(&mut self, mv: Move, score: i16) {
        self.moves.push(CompressedMove { mv, score });
    }

    pub fn set_win(&mut self, wdl: Wdl) {
        self.initial.wdl = wdl;
    }

    pub fn serialize_into(&self, file: &mut impl std::io::Write) -> std::io::Result<()> {
        file.write_all(self.initial.as_bytes())?;
        for m in &self.moves {
            file.write_all(m.as_bytes())?;
        }
        file.write_all(&NULL_TERMINATOR)?;
        Ok(())
    }

    pub fn deserialize_from(file: &mut impl std::io::Read) -> std::io::Result<Self> {
        let mut initial = CompressedPosition::default();
        file.read_exact(initial.as_mut_bytes())?;

        let mut moves = Vec::new();
        loop {
            let mut unparsed_move = [0; std::mem::size_of::<CompressedMove>()];
            file.read_exact(&mut unparsed_move)?;
            if unparsed_move == NULL_TERMINATOR {
                break;
            }
            let mut m = CompressedMove::default();
            m.as_mut_bytes().copy_from_slice(&unparsed_move);
            moves.push(m);
        }

        Ok(Self { initial, moves })
    }
}

impl TryFrom<CompressedPosition> for Position {
    type Error = anyhow::Error;
    fn try_from(value: CompressedPosition) -> Result<Self, Self::Error> {
        let mut pos = Self::default();

        for (idx, sq) in value.occ.enumerate() {
            let pc = value.pieces.get(idx);

            let color = if pc >> 3 == 0 {
                Color::White
            } else {
                Color::Black
            };

            let role = unsafe { std::mem::transmute::<u8, Role>(pc & 0b111) };

            pos.set(sq, Piece { color, role });
        }

        pos.fullmove_number = NonZeroU16::new(value.ply / 2 + 1).context("Invalid ply")?;
        pos.ep_square = match value.stm_ep_square {
            u8::MAX => None,
            sq => Some(Square::from(sq)),
        };
        pos.halfmove_clock = value.halfmove_clock;
        pos.side = if value.ply % 2 == 0 {
            Color::White
        } else {
            Color::Black
        };
        pos.refresh_checks_and_pins();
        pos.key = pos.zobrist_hash();
        pos.castling = CastleRights::from_bits_retain(value.castling);

        Ok(pos)
    }
}

impl Display for CompressedGame {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let mut pos: Position = self.initial.try_into().unwrap();
        let header = format!(
            r#"[Event "game"]
[Site "NA"]
[Date "NA"]
[White "pounce"]
[Black "pounce"]
[Result "{}"]
[FEN "{}"]"#,
            self.initial.wdl,
            pos.to_fen(),
        );
        writeln!(f, "{header}")?;

        let mut move_count = 0;
        for m in &self.moves {
            if move_count % 12 == 0 && pos.side == Color::White {
                writeln!(f)?;
            }
            if pos.side == Color::White {
                write!(f, "{}. ", pos.fullmove_number.get())?;
            } else {
                move_count += 1;
            }

            let san = pos.san(m.mv).unwrap();
            write!(f, "{} {{{:+.2}}} ", san, m.score as f64 / 100.0)?;
            pos.make_move(m.mv);
        }
        write!(f, "{}\n\n", self.initial.wdl)?;
        Ok(())
    }
}

impl IntoIterator for CompressedGame {
    type Item = ScoredPosition;
    type IntoIter = PositionIterator;

    fn into_iter(self) -> Self::IntoIter {
        PositionIterator {
            game: self,
            position: None,
            index: 0,
        }
    }
}

pub struct ScoredPosition {
    pub position: chess::Position,
    pub score: i16,
    pub wdl: Wdl,
    pub mv: Move, // the next move to be played from this position
}

pub struct PositionIterator {
    game: CompressedGame,
    position: Option<chess::Position>,
    index: usize,
}

impl Iterator for PositionIterator {
    type Item = ScoredPosition;

    fn next(&mut self) -> Option<Self::Item> {
        // we skip the last move because it is the final position
        if self.index >= self.game.moves.len() - 1 {
            return None;
        }

        if self.position.is_none() {
            self.position = Some(self.game.initial.try_into().unwrap());
        }

        let current_position = self.position.clone().unwrap();

        // the score for the current position is the score of the next move
        let score = self.game.moves[self.index].score;
        let mv = self.game.moves[self.index].mv;

        // now make the move
        self.position.as_mut().unwrap().make_move(mv);
        self.index += 1;

        Some(ScoredPosition {
            position: current_position,
            score,
            wdl: self.game.initial.wdl,
            mv,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chess::position::fen::{Fen, STARTPOS};

    #[test]
    fn test_size() {
        assert_eq!(std::mem::size_of::<CompressedPosition>(), 32);
        assert_eq!(std::mem::size_of::<CompressedMove>(), 4);
    }

    #[test]
    fn test_startpos() {
        let Fen(pos) = Fen::parse(STARTPOS).unwrap();

        let comp = CompressedPosition::new(&pos, 0, Wdl::BlackWin);

        assert_eq!(comp, comp.flip().flip());

        assert_eq!(comp.occ, comp.flip().occ);
        assert_eq!(comp.pieces, comp.flip().pieces);
        assert_eq!(comp.score, -comp.flip().score);
    }

    const POS_1: &str = "b3r1k1/5pbp/6p1/1NP5/8/5N2/2Q3PP/3q2K1 b - - 0 31";
    const POS_1_FLIPPED: &str = "3Q2k1/2q3pp/5n2/8/1np5/6P1/5PBP/B3R1K1 w - - 0 31";

    #[test]
    fn test_flip() {
        let Fen(pos) = Fen::parse(POS_1).unwrap();
        let comp = CompressedPosition::new(&pos, 600, Wdl::BlackWin);

        let Fen(flip) = POS_1_FLIPPED.parse().unwrap();
        let comp_flip = CompressedPosition::new(&flip, -600, Wdl::WhiteWin);

        assert_eq!(comp_flip, comp.flip());
    }
}
