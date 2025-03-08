use std::fmt::{self, Debug, Formatter};

#[derive(Copy, Clone, Default, PartialEq, Eq)]
#[repr(transparent)]
pub struct U4Array32([u8; 16]);

impl U4Array32 {
    pub const fn get(&self, i: usize) -> u8 {
        (self.0[i / 2] >> (4 * (i % 2))) & 0b1111
    }

    pub const fn set(&mut self, i: usize, val: u8) {
        debug_assert!(val < 0x10);
        let shift = 4 * (i % 2);
        let idx = i / 2;
        self.0[idx] &= !(0b1111 << shift);
        self.0[idx] |= val << shift;
    }
}

impl Debug for U4Array32 {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let mut dbg_list = f.debug_list();
        for i in 0..32 {
            dbg_list.entry(&format!("{:#02x}", self.get(i)));
        }
        dbg_list.finish()
    }
}
