/// Memory interface helpers.

/// Use this for data which uses an 8-bit base.
/// This has default impls for halfword and word.
/// Ensure that all memory interactions are aligned or there might be issues.
/// 
/// Lower bytes will be read/written first.
pub trait MemInterface8 {
    fn read_byte(&self, addr: u32) -> u8;
    fn write_byte(&mut self, addr: u32, data: u8);

    fn read_halfword(&self, addr: u32) -> u16 {
        use crate::common::bytes::u16;
        let lo = self.read_byte(addr);
        let hi = self.read_byte(addr + 1);
        u16::make(hi, lo)
    }
    fn write_halfword(&mut self, addr: u32, data: u16) {
        use crate::common::bytes::u16;
        self.write_byte(addr, u16::lo(data));
        self.write_byte(addr + 1, u16::hi(data));
    }

    fn read_word(&self, addr: u32) -> u32 {
        use crate::common::bytes::u32;
        let lo = self.read_halfword(addr);
        let hi = self.read_halfword(addr + 2);
        u32::make(hi, lo)
    }
    fn write_word(&mut self, addr: u32, data: u32) {
        use crate::common::bytes::u32;
        self.write_halfword(addr, u32::lo(data));
        self.write_halfword(addr + 2, u32::hi(data));
    }
}

/// Use this for data which uses a 16-bit base.
/// This has default impls for byte and word.
/// Ensure that all memory interactions are aligned or there might be issues.
/// 
/// Lower bytes will be read/written first.
pub trait MemInterface16 {
    fn read_byte(&self, addr: u32) -> u8 {
        use crate::common::bytes::u16;
        let data = self.read_halfword(addr & 0xFFFF_FFFE);
        match addr & 1 {
            0 => u16::lo(data),
            1 => u16::hi(data),
            _ => unreachable!()
        }
    }
    fn write_byte(&mut self, addr: u32, data: u8) {
        use crate::common::bytes::u16;
        let halfword_addr = addr & 0xFFFF_FFFE;
        let halfword_data = self.read_halfword(halfword_addr);
        match addr & 1 {
            0 => self.write_halfword(halfword_addr, u16::set_lo(halfword_data, data)),
            1 => self.write_halfword(halfword_addr, u16::set_hi(halfword_data, data)),
            _ => unreachable!()
        }
    }

    fn read_halfword(&self, addr: u32) -> u16;
    fn write_halfword(&mut self, addr: u32, data: u16);

    fn read_word(&self, addr: u32) -> u32 {
        use crate::common::bytes::u32;
        let lo = self.read_halfword(addr);
        let hi = self.read_halfword(addr + 2);
        u32::make(hi, lo)
    }
    fn write_word(&mut self, addr: u32, data: u32) {
        use crate::common::bytes::u32;
        self.write_halfword(addr, u32::lo(data));
        self.write_halfword(addr + 2, u32::hi(data));
    }
}
