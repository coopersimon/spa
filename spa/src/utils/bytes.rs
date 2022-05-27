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

    pub const fn byte(val: u32, n: usize) -> u8 {
        val.to_le_bytes()[n]
    }

    pub const fn set_byte(val: u32, byte: u8, n: usize) -> u32 {
        let mut bytes = val.to_le_bytes();
        bytes[n] = byte;
        u32::from_le_bytes(bytes)
    }
}

pub mod u64 {
    pub const fn lo(val: u64) -> u32 {
        val as u32
    }

    pub const fn hi(val: u64) -> u32 {
        (val >> 32) as u32
    }

    pub const fn make(hi: u32, lo: u32) -> u64 {
        ((hi as u64) << 32) | (lo as u64)
    }

    pub const fn set_lo(val: u64, lo: u32) -> u64 {
        (val & 0xFFFF_FFFF_0000_0000) | (lo as u64)
    }

    pub const fn set_hi(val: u64, hi: u32) -> u64 {
        (val & 0xFFFF_FFFF) | ((hi as u64) << 32)
    }
}
