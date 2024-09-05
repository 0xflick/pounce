use std::borrow::Borrow;

use thiserror::Error;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub depth: Option<u8>,
    pub nodes: Option<u64>,
    pub wtime: Option<i32>,
    pub btime: Option<i32>,
    pub winc: Option<u32>,
    pub binc: Option<u32>,
    pub movestogo: Option<u32>,
    pub movetime: Option<i32>,
    pub infinite: bool,
}

#[derive(Debug, Error)]
pub enum LimitsParseError {
    #[error("Invalid limit: {0}")]
    InvalidTime(#[from] std::num::ParseIntError),
}

impl Limits {
    pub fn new() -> Self {
        Limits {
            depth: None,
            nodes: None,
            wtime: None,
            btime: None,
            winc: None,
            binc: None,
            movestogo: None,
            movetime: None,
            infinite: false,
        }
    }

    pub fn from_tokens<T>(tokens: &[T]) -> Result<Self, LimitsParseError>
    where
        T: AsRef<str> + Borrow<str>,
    {
        enum ParseStage {
            Pre,
            Depth,
            Nodes,
            WTime,
            BTime,
            WInc,
            BInc,
            Movestogo,
            Movetime,
        }

        let mut limits = Limits::new();
        let mut parse_stage = ParseStage::Pre;

        for token in tokens {
            match token.as_ref() {
                "depth" => {
                    parse_stage = ParseStage::Depth;
                }
                "nodes" => {
                    parse_stage = ParseStage::Nodes;
                }
                "wtime" => {
                    parse_stage = ParseStage::WTime;
                }
                "btime" => {
                    parse_stage = ParseStage::BTime;
                }
                "winc" => {
                    parse_stage = ParseStage::WInc;
                }
                "binc" => {
                    parse_stage = ParseStage::BInc;
                }
                "movestogo" => {
                    parse_stage = ParseStage::Movestogo;
                }
                "movetime" => {
                    parse_stage = ParseStage::Movetime;
                }
                "infinite" => {
                    limits.infinite = true;
                }
                _ => match parse_stage {
                    ParseStage::Depth => {
                        limits.depth = Some(token.as_ref().parse()?);
                    }
                    ParseStage::Nodes => {
                        limits.nodes = Some(token.as_ref().parse()?);
                    }
                    ParseStage::WTime => {
                        limits.wtime = Some(token.as_ref().parse()?);
                    }
                    ParseStage::BTime => {
                        limits.btime = Some(token.as_ref().parse()?);
                    }
                    ParseStage::WInc => {
                        limits.winc = Some(token.as_ref().parse()?);
                    }
                    ParseStage::BInc => {
                        limits.binc = Some(token.as_ref().parse()?);
                    }
                    ParseStage::Movestogo => {
                        limits.movestogo = Some(token.as_ref().parse()?);
                    }
                    ParseStage::Movetime => {
                        limits.movetime = Some(token.as_ref().parse()?);
                    }
                    _ => {}
                },
            }
        }

        Ok(limits)
    }
}

impl Default for Limits {
    fn default() -> Self {
        Limits::new()
    }
}
