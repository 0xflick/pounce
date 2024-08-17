use crate::{
    bitboard::Bitboard,
    chess::{Color, Piece, Role, Square},
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct ByColor<T> {
    white: T,
    black: T,
}

impl<T> ByColor<T> {
    #[inline]
    pub fn get(&self, color: Color) -> &T {
        match color {
            Color::White => &self.white,
            Color::Black => &self.black,
        }
    }

    #[inline]
    pub fn get_mut(&mut self, color: Color) -> &mut T {
        match color {
            Color::White => &mut self.white,
            Color::Black => &mut self.black,
        }
    }

    #[inline]
    pub fn as_mut(&mut self) -> ByColor<&mut T> {
        ByColor {
            white: &mut self.white,
            black: &mut self.black,
        }
    }

    #[inline]
    pub fn for_each<F: FnMut(T)>(self, mut f: F) {
        f(self.white);
        f(self.black);
    }

    #[inline]
    pub fn find<F>(self, mut f: F) -> Option<Color>
    where
        F: FnMut(T) -> bool,
    {
        if f(self.white) {
            return Some(Color::White);
        }
        if f(self.black) {
            return Some(Color::Black);
        }
        None
    }
}

#[derive(Debug, Clone, Copy)]
struct ByRole<T> {
    pawn: T,
    knight: T,
    bishop: T,
    rook: T,
    queen: T,
    king: T,
}

impl<T> ByRole<T> {
    #[inline]
    pub fn get(&self, role: Role) -> &T {
        match role {
            Role::Pawn => &self.pawn,
            Role::Knight => &self.knight,
            Role::Bishop => &self.bishop,
            Role::Rook => &self.rook,
            Role::Queen => &self.queen,
            Role::King => &self.king,
        }
    }

    #[inline]
    pub fn get_mut(&mut self, role: Role) -> &mut T {
        match role {
            Role::Pawn => &mut self.pawn,
            Role::Knight => &mut self.knight,
            Role::Bishop => &mut self.bishop,
            Role::Rook => &mut self.rook,
            Role::Queen => &mut self.queen,
            Role::King => &mut self.king,
        }
    }

    #[inline]
    pub fn as_mut(&mut self) -> ByRole<&mut T> {
        ByRole {
            pawn: &mut self.pawn,
            knight: &mut self.knight,
            bishop: &mut self.bishop,
            rook: &mut self.rook,
            queen: &mut self.queen,
            king: &mut self.king,
        }
    }

    #[inline]
    pub fn for_each<F: FnMut(T)>(self, mut f: F) {
        f(self.pawn);
        f(self.knight);
        f(self.bishop);
        f(self.rook);
        f(self.queen);
        f(self.king);
    }

    #[inline]
    pub fn find<F>(self, mut f: F) -> Option<Role>
    where
        F: FnMut(T) -> bool,
    {
        if f(self.pawn) {
            return Some(Role::Pawn);
        }
        if f(self.knight) {
            return Some(Role::Knight);
        }
        if f(self.bishop) {
            return Some(Role::Bishop);
        }
        if f(self.rook) {
            return Some(Role::Rook);
        }
        if f(self.queen) {
            return Some(Role::Queen);
        }
        if f(self.king) {
            return Some(Role::King);
        }
        None
    }
}

#[derive(Clone, Copy)]
pub struct Board {
    by_color: ByColor<Bitboard>,
    by_role: ByRole<Bitboard>,
    occupancy: Bitboard,

    checkers: Bitboard,
    pinned: Bitboard,
}

impl Board {
    pub const fn new() -> Self {
        Self {
            by_color: ByColor {
                white: Bitboard::EMPTY,
                black: Bitboard::EMPTY,
            },
            by_role: ByRole {
                pawn: Bitboard::EMPTY,
                knight: Bitboard::EMPTY,
                bishop: Bitboard::EMPTY,
                rook: Bitboard::EMPTY,
                queen: Bitboard::EMPTY,
                king: Bitboard::EMPTY,
            },

            occupancy: Bitboard::EMPTY,
            checkers: Bitboard::EMPTY,
            pinned: Bitboard::EMPTY,
        }
    }
}

impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}

impl Board {
    #[inline]
    pub fn pawns(&self) -> Bitboard {
        self.by_role.pawn
    }

    #[inline]
    pub fn knights(&self) -> Bitboard {
        self.by_role.knight
    }

    #[inline]
    pub fn bishops(&self) -> Bitboard {
        self.by_role.bishop
    }

    #[inline]
    pub fn rooks(&self) -> Bitboard {
        self.by_role.rook
    }

    #[inline]
    pub fn queens(&self) -> Bitboard {
        self.by_role.queen
    }

    #[inline]
    pub fn kings(&self) -> Bitboard {
        self.by_role.king
    }

    #[inline]
    pub fn white(&self) -> Bitboard {
        self.by_color.white
    }

    #[inline]
    pub fn black(&self) -> Bitboard {
        self.by_color.black
    }

    #[inline]
    pub fn occupancy(&self) -> Bitboard {
        self.occupancy
    }

    #[inline]
    pub fn checkers(&self) -> Bitboard {
        self.checkers
    }

    #[inline]
    pub fn checkers_mut(&mut self) -> &mut Bitboard {
        &mut self.checkers
    }

    #[inline]
    pub fn pinned(&self) -> Bitboard {
        self.pinned
    }

    #[inline]
    pub fn pinned_mut(&mut self) -> &mut Bitboard {
        &mut self.pinned
    }

    #[inline]
    pub fn steppers(&self) -> Bitboard {
        self.pawns() | self.knights() | self.kings()
    }

    #[inline]
    pub fn sliders(&self) -> Bitboard {
        self.bishops() | self.rooks() | self.queens()
    }

    #[inline]
    pub fn by_color(&self, color: Color) -> Bitboard {
        *self.by_color.get(color)
    }

    #[inline]
    pub fn by_role(&self, role: Role) -> Bitboard {
        *self.by_role.get(role)
    }

    #[inline]
    pub fn by_color_role(&self, color: Color, role: Role) -> Bitboard {
        self.by_color(color) & self.by_role(role)
    }

    #[inline]
    pub fn king_of(&self, color: Color) -> Bitboard {
        self.by_role.king & self.by_color(color)
    }

    #[inline]
    pub fn discard(&mut self, sq: Square) {
        self.by_color.as_mut().for_each(|bb| bb.clear(sq));
        self.by_role.as_mut().for_each(|bb| bb.clear(sq));
        self.occupancy.clear(sq);
    }

    #[inline]
    pub fn set(&mut self, sq: Square, Piece { color, role }: Piece) {
        self.discard(sq);
        self.by_color.get_mut(color).set(sq);
        self.by_role.get_mut(role).set(sq);
        self.occupancy.set(sq);
    }

    #[inline]
    pub fn color_at(&self, sq: Square) -> Option<Color> {
        self.by_color.find(|bb| bb.contains(sq))
    }

    #[inline]
    pub fn role_at(&self, sq: Square) -> Option<Role> {
        if self.occupancy.contains(sq) {
            self.by_role.find(|bb| bb.contains(sq))
        } else {
            None
        }
    }

    #[inline]
    pub fn piece_at(&self, sq: Square) -> Option<Piece> {
        self.role_at(sq).map(|role| Piece {
            color: self.color_at(sq).unwrap(),
            role,
        })
    }
}

impl std::fmt::Debug for Board {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let spacing = 16;
        writeln!(f, "Board:")?;

        write!(f, "{:width$}", "White:", width = spacing)?;
        write!(f, "{:width$}", "Black:", width = spacing)?;
        write!(f, "{:width$}", "Occupancy:", width = spacing)?;
        writeln!(f)?;

        fmt_bbs(
            vec![self.by_color.white, self.by_color.black, self.occupancy],
            spacing,
            f,
        )?;
        writeln!(f)?;

        write!(f, "{:width$}", "Pawns:", width = spacing)?;
        write!(f, "{:width$}", "Knights:", width = spacing)?;
        write!(f, "{:width$}", "Bishops:", width = spacing)?;
        writeln!(f)?;

        fmt_bbs(
            vec![self.by_role.pawn, self.by_role.knight, self.by_role.bishop],
            spacing,
            f,
        )?;
        writeln!(f)?;

        write!(f, "{:width$}", "Rooks:", width = spacing)?;
        write!(f, "{:width$}", "Queens:", width = spacing)?;
        write!(f, "{:width$}", "Kings:", width = spacing)?;
        writeln!(f)?;

        fmt_bbs(
            vec![self.by_role.rook, self.by_role.queen, self.by_role.king],
            spacing,
            f,
        )?;
        writeln!(f)?;

        write!(f, "{:width$}", "Checkers:", width = spacing)?;
        write!(f, "{:width$}", "Pinned:", width = spacing)?;
        writeln!(f)?;

        fmt_bbs(vec![self.checkers, self.pinned], spacing, f)?;
        writeln!(f)?;

        Ok(())
    }
}

fn fmt_bbs(bbs: Vec<Bitboard>, width: usize, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let bbs_strs = bbs
        .iter()
        .map(|bb| {
            format!("{:?}", bb)
                .lines()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    for i in 0..8 {
        for bb in &bbs_strs {
            write!(f, "{:width$}", bb[i], width = width)?;
        }
        writeln!(f)?;
    }

    Ok(())
}
