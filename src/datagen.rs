use anyhow::Ok;

use crate::{
    bitboard::Bitboard,
    chess::{Color, GameResult},
    limits::Limits,
    movegen::MoveGen,
    position::Position,
    search::Search,
    tt::Table,
};
use std::{
    fmt::{self, Debug, Formatter},
    sync::{atomic::AtomicBool, Arc},
};

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Wdl {
    BlackWin,
    Draw,
    WhiteWin,
}

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct CompressedPosition {
    occ: Bitboard,
    pieces: [u8; 16],
    score: i16,
    pub wdl: u8,
}

impl CompressedPosition {
    pub fn new(pos: &Position, score: i16, wdl: Wdl) -> Self {
        let mut occ = pos.occupancy;
        let mailbox = pos.mailbox;
        let mut wdl = wdl as u8;

        // if side to move is black, we want to flip the orientation of the board
        if pos.side == Color::Black {
            occ = occ.flip();
            wdl = 2 - wdl;
        }

        let mut pieces = [0; 16];
        for (idx, mut sq) in occ.enumerate() {
            if pos.side == Color::Black {
                sq = sq ^ 56;
            };

            let pc = mailbox[sq].unwrap();
            let bit_pc = (((pos.side != pc.color) as u8) << 3) | (pc.role as u8);

            let shift = 4 * (idx % 2);
            let idx = idx / 2;

            pieces[idx] |= bit_pc << shift;
        }

        Self {
            occ,
            pieces,
            score,
            wdl,
        }
    }
}

impl Debug for CompressedPosition {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        writeln!(f, "occ:")?;
        writeln!(f, "{:?}", self.occ)?;
        writeln!(f, "pieces:")?;
        for p in self.pieces {
            writeln!(f, "{:#010b}", p)?;
        }
        writeln!(f, "score: {}, wdl: {}", self.score, self.wdl)
    }
}

pub struct DatagenConfig<'a> {
    limits: Limits,
    tt_size_mb: u32,
    concurrency: u32,
    out_path: &'a str,
}

pub fn datagen(
    DatagenConfig {
        limits,
        tt_size_mb,
        concurrency,
        out_path,
    }: DatagenConfig,
) -> anyhow::Result<()> {
    // start playout threads, share global state, print results
    Ok(())
}

pub fn playout(
    startpos: &Position,
    limits: Limits,
    tt: Arc<Table>,
) -> anyhow::Result<Vec<CompressedPosition>> {
    let mut pos = startpos.clone();

    let stop = Arc::new(AtomicBool::new(false));

    let mut positions = Vec::new();
    // make random moves

    let result = loop {
        let num_moves = MoveGen::new(&pos).len();
        if num_moves == 0 {
            if pos.in_check() {
                match pos.side {
                    Color::Black => break Wdl::WhiteWin,
                    Color::White => break Wdl::BlackWin,
                }
            }
            break Wdl::Draw;
        }

        match pos.is_draw() {
            Some(GameResult::Loss) => match pos.side {
                Color::Black => break Wdl::WhiteWin,
                Color::White => break Wdl::BlackWin,
            },
            Some(GameResult::Draw) => break Wdl::Draw,
            Some(GameResult::Win) => unreachable!(),
            None => {}
        }

        let mut search = Search::new(pos.clone(), limits, tt.clone(), stop.clone());
        search.set_silent(true);
        let res = search.think();
        pos.make_move(res.bestmove);
        positions.push(res);
    };

    let mut compressed_positions = Vec::with_capacity(positions.len());
    let mut pos = startpos.clone();
    // build up CompressedPositions
    for p in positions {
        compressed_positions.push(CompressedPosition::new(&pos, p.score, result));
        pos.make_move(p.bestmove);
    }

    Ok(compressed_positions)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::fen::Fen;

    const STARTPOS: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    const STARTPOS_FLIPPED: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

    #[test]
    fn test_startpos() {
        let Fen(pos) = Fen::parse(STARTPOS).unwrap();
        let Fen(flipped_pos) = Fen::parse(STARTPOS_FLIPPED).unwrap();

        let comp = CompressedPosition::new(&pos, 0, Wdl::BlackWin);
        let comp_flipped = CompressedPosition::new(&flipped_pos, 0, Wdl::BlackWin);

        assert_eq!(comp, comp_flipped);
    }
}
