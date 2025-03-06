use crate::bitboard::Bitboard;
use crate::chess::movegen::MoveGen;
use crate::chess::{Move, MoveType, Position, Role};

enum CheckType {
    None,
    Check,
    Checkmate,
}

impl Position {
    fn gives_check(&mut self, mv: Move) -> CheckType {
        self.make_move(mv);
        let check = if self.in_check() {
            if MoveGen::new(self).len() == 0 {
                CheckType::Checkmate
            } else {
                CheckType::Check
            }
        } else {
            CheckType::None
        };
        self.unmake_move(mv);
        check
    }

    fn check_char(&mut self, mv: Move) -> String {
        match self.gives_check(mv) {
            CheckType::Check => "+".to_string(),
            CheckType::Checkmate => "#".to_string(),
            CheckType::None => "".to_string(),
        }
    }

    fn is_capture(&self, mv: Move) -> anyhow::Result<bool> {
        Ok(self.piece_at(mv.to()).is_some()
            || mv.move_type(
                self.role_at(mv.from()).expect("No piece at from square"),
                self.ep_square,
            ) == MoveType::EnPassant)
    }

    fn prefix_char(&self, mv: Move) -> anyhow::Result<String> {
        match self.role_at(mv.from()) {
            Some(Role::Pawn) => {
                if self.is_capture(mv)? {
                    Ok(mv.from().file().char().to_string())
                } else {
                    Ok("".to_string())
                }
            }
            Some(Role::King) => Ok("K".to_string()),
            Some(Role::Queen) => Ok("Q".to_string()),
            Some(Role::Rook) => Ok("R".to_string()),
            Some(Role::Bishop) => Ok("B".to_string()),
            Some(Role::Knight) => Ok("N".to_string()),
            None => Err(anyhow::anyhow!("No piece at from square")),
        }
    }

    fn disambiguation(&self, mv: Move) -> anyhow::Result<(String, String)> {
        let from_role = self.role_at(mv.from()).expect("No piece at from square");
        if from_role == Role::Pawn {
            return Ok(("".to_string(), "".to_string()));
        }

        let mut mg = MoveGen::new(self);
        mg.set_mask(Bitboard::from(mv.to()));

        let mut file_count = 0;
        let mut rank_count = 0;
        let mut needs_disambiguation = false;

        for m in mg {
            if m == mv {
                continue;
            }
            if self.role_at(m.from()).unwrap() != from_role {
                continue;
            }

            needs_disambiguation = true;

            if m.from().file() == mv.from().file() {
                file_count += 1;
            }

            if m.from().rank() == mv.from().rank() {
                rank_count += 1;
            }
        }

        let can_be_file_disambiguated = file_count == 0;
        let can_be_rank_disambiguated = rank_count == 0;
        let needs_both = !can_be_file_disambiguated && !can_be_rank_disambiguated;

        let needs_file_disambiguation =
            needs_disambiguation && (needs_both || can_be_file_disambiguated);
        let needs_rank_disambiguation = needs_disambiguation
            && (needs_both || (can_be_rank_disambiguated && !can_be_file_disambiguated));

        let file_disambiguation = if needs_file_disambiguation {
            mv.from().file().char().to_string()
        } else {
            "".to_string()
        };

        let rank_disambiguation = if needs_rank_disambiguation {
            mv.from().rank().char().to_string()
        } else {
            "".to_string()
        };

        Ok((file_disambiguation, rank_disambiguation))
    }

    fn capture_char(&self, mv: Move) -> String {
        if self.is_capture(mv).unwrap() {
            "x".to_string()
        } else {
            "".to_string()
        }
    }

    fn promotion_char(&self, mv: Move) -> &str {
        match mv.promotion() {
            Some(Role::Queen) => "Q",
            Some(Role::Rook) => "R",
            Some(Role::Bishop) => "B",
            Some(Role::Knight) => "N",
            _ => "",
        }
    }

    pub fn san(&mut self, mv: Move) -> anyhow::Result<String> {
        let check_char = self.check_char(mv);
        let from_role = self.role_at(mv.from()).expect("No piece at from square");

        if mv.move_type(from_role, self.ep_square) == MoveType::Castle {
            let castle_string = if mv.to() > mv.from() { "O-O" } else { "O-O-O" };
            return Ok(format!("{}{}", castle_string, check_char));
        }

        let prefix_char = self.prefix_char(mv)?;
        let (file_disambiguation, rank_disambiguation) = self.disambiguation(mv)?;
        let capture_char = self.capture_char(mv);
        let promotion_char = self.promotion_char(mv);

        Ok(format!(
            "{}{}{}{}{}{}{}",
            prefix_char,
            file_disambiguation,
            rank_disambiguation,
            capture_char,
            mv.to(),
            promotion_char,
            check_char,
        ))
    }
}

#[cfg(test)]
mod test {
    use crate::chess::movegen::init_tables;
    use crate::chess::{Fen, Move, Square};
    use crate::zobrist::init_zobrist;

    #[test]
    fn test_san() {
        init_tables();
        init_zobrist();
        let fen = "r2qkr2/pp2p1bp/2pnbp2/3p2nP/3PP3/P1N2P2/1PP2RB1/R1BQK1N1 w Qq - 1 18";
        let Fen(mut pos) = Fen::parse(fen).unwrap();

        let mv = Move::new(Square::C3, Square::E2, None);
        assert_eq!(pos.san(mv).unwrap(), "Nce2");
    }
}
