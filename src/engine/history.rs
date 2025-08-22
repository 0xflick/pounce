use crate::chess::{Color, Square};

const HISTORY_MAX: i32 = i16::MAX as i32;

pub struct History([[[i32; Square::NUM]; Square::NUM]; Color::NUM]);

impl Default for History {
    fn default() -> Self {
        Self([[[0; Square::NUM]; Square::NUM]; Color::NUM])
    }
}

impl History {
    pub fn update(&mut self, side: Color, from: Square, to: Square, bonus: i32) {
        let index = &mut self.0[side][from][to];
        taper_update::<HISTORY_MAX>(index, bonus);
    }

    pub fn score(&self, side: Color, from: Square, to: Square) -> i32 {
        self.0[side][from][to]
    }
}

fn taper_update<const MAX: i32>(index: &mut i32, bonus: i32) {
    *index += bonus - (*index * bonus.abs() / MAX);
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_taper() {
        let mut history = History::default();

        assert_eq!(history.score(Color::White, Square::A1, Square::A1), 0);
        history.update(Color::White, Square::A1, Square::A1, 1000);

        assert_eq!(history.score(Color::White, Square::A1, Square::A1), 1000);

        history.update(Color::White, Square::A1, Square::A1, 1000);
        assert_eq!(history.score(Color::White, Square::A1, Square::A1), 1970);
    }
}
