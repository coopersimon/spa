/// Maths hardware functions in DS.

use bitflags::bitflags;
use crate::utils::{
    meminterface::MemInterface32,
    bits::u32,
    bytes::u64,
};

bitflags!{
    #[derive(Default)]
    pub struct DivisionControl: u32 {
        const BUSY          = u32::bit(15);
        const DIV_BY_ZERO   = u32::bit(14);
        const MODE          = u32::bits(0, 1);
    }
}

bitflags!{
    #[derive(Default)]
    pub struct SqrtControl: u32 {
        const BUSY  = u32::bit(15);
        const MODE  = u32::bit(0);
    }
}


pub struct Accelerators {
    div_control:        DivisionControl,
    div_numerator:      i64,
    div_denominator:    i64,
    div_result:         i64,
    mod_result:         i64,

    sqrt_control:   SqrtControl,
    sqrt_param:     u64,
    sqrt_result:    u32,

    //cycle_countdown:    usize,
}

impl Accelerators {
    pub fn new() -> Self {
        Self {
            div_control:        DivisionControl::default(),
            div_numerator:      0,
            div_denominator:    0,
            div_result:         0,
            mod_result:         0,
        
            sqrt_control:   SqrtControl::default(),
            sqrt_param:     0,
            sqrt_result:    0,

            //cycle_countdown:    0,
        }
    }

    /*pub fn clock(&mut self, cycles: usize) {
        // TODO
    }*/

    pub fn start_div(&mut self) {
        if self.div_denominator == 0 {
            self.div_control.insert(DivisionControl::DIV_BY_ZERO);
            return;
        }

        match (self.div_control & DivisionControl::MODE).bits() {
            0 => {
                self.div_result = ((self.div_numerator as i32) / (self.div_denominator as i32)) as i64;
                self.mod_result = ((self.div_numerator as i32) % (self.div_denominator as i32)) as i64;
            },
            1 => {
                self.div_result = self.div_numerator / ((self.div_denominator as i32) as i64);
                self.mod_result = self.div_numerator % ((self.div_denominator as i32) as i64);
            },
            _ => {
                self.div_result = self.div_numerator / self.div_denominator;
                self.mod_result = self.div_numerator % self.div_denominator;
            },
        }
    }

    pub fn start_sqrt(&mut self) {
        if self.sqrt_control.contains(SqrtControl::MODE) {
            let sqrt_in = self.sqrt_param as f64;
            self.sqrt_result = sqrt_in.sqrt() as u32;
        } else {
            let sqrt_in = (self.sqrt_param as u32) as f64;
            self.sqrt_result = sqrt_in.sqrt() as u32;
        }
    }
}

impl MemInterface32 for Accelerators {
    fn read_word(&mut self, addr: u32) -> u32 {
        match addr {
            0x0400_0280 => self.div_control.bits(),
            0x0400_0290 => u64::lo(self.div_numerator as u64),
            0x0400_0294 => u64::hi(self.div_numerator as u64),
            0x0400_0298 => u64::lo(self.div_denominator as u64),
            0x0400_029C => u64::hi(self.div_denominator as u64),
            0x0400_02A0 => u64::lo(self.div_result as u64),
            0x0400_02A4 => u64::hi(self.div_result as u64),
            0x0400_02A8 => u64::lo(self.mod_result as u64),
            0x0400_02AC => u64::hi(self.mod_result as u64),
            0x0400_02B0 => self.sqrt_control.bits(),
            0x0400_02B4 => self.sqrt_result,
            0x0400_02B8 => u64::lo(self.sqrt_param),
            0x0400_02BC => u64::hi(self.sqrt_param),
            _ => 0,
        }
    }

    fn write_word(&mut self, addr: u32, data: u32) {
        match addr {
            0x0400_0280 => {
                self.div_control.remove(DivisionControl::MODE);
                self.div_control.insert(DivisionControl::from_bits_truncate(data & 0x3));
                self.start_div();
            },
            0x0400_0290 => {
                self.div_numerator = u64::set_lo(self.div_numerator as u64, data) as i64;
                self.start_div();
            },
            0x0400_0294 => {
                self.div_numerator = u64::set_hi(self.div_numerator as u64, data) as i64;
                self.start_div();
            },
            0x0400_0298 => {
                self.div_denominator = u64::set_lo(self.div_denominator as u64, data) as i64;
                self.start_div();
            },
            0x0400_029C => {
                self.div_denominator = u64::set_hi(self.div_denominator as u64, data) as i64;
                self.start_div();
            },
            0x0400_02B0 => {
                self.sqrt_control.set(SqrtControl::MODE, u32::test_bit(data, 1));
                self.start_sqrt();
            },
            0x0400_02B8 => {
                self.sqrt_param = u64::set_lo(self.sqrt_param, data);
                self.start_sqrt();
            },
            0x0400_02BC => {
                self.sqrt_param = u64::set_hi(self.sqrt_param, data);
                self.start_sqrt();
            },
            _ => {}
        }
    }
}
