/// Byte manipulation.

pub mod u16 {
    pub const fn lo(val: u16) -> u8 {
        val as u8
    }

    pub const fn hi(val: u16) -> u8 {
        (val >> 8) as u8
    }

    pub const fn make(hi: u8, lo: u8) -> u16 {
        ((hi as u16) << 8) | (lo as u16)
    }

    pub const fn set_lo(val: u16, lo: u8) -> u16 {
        (val & 0xFF00) | (lo as u16)
    }

    pub const fn set_hi(val: u16, hi: u8) -> u16 {
        (val & 0xFF) | ((hi as u16) << 8)
    }
}

pub mod u32 {
    pub const fn lo(val: u32) -> u16 {
        val as u16
    }

    pub const fn hi(val: u32) -> u16 {
        (val >> 16) as u16
    }

    pub const fn make(hi: u16, lo: u16) -> u32 {
        ((hi as u32) << 16) | (lo as u32)
    }

    pub const fn set_lo(val: u32, lo: u16) -> u32 {
        (val & 0xFFFF_0000) | (lo as u32)
    }

    pub const fn set_hi(val: u32, hi: u16) -> u32 {
        (val & 0xFFFF) | ((hi as u32) << 16)
    }
}
