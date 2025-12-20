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

    div_cycle_countdown: usize,
    sqrt_cycle_countdown: usize,
    div_latch: bool,
    sqrt_latch: bool,
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

            div_cycle_countdown: 0,
            sqrt_cycle_countdown: 0,
            div_latch: false,
            sqrt_latch: false,
        }
    }

    pub fn clock(&mut self, cycles: usize) {
        // TODO: unlatch?
        if self.div_cycle_countdown > 0 {
            if self.div_cycle_countdown <= cycles {
                self.div_cycle_countdown = 0;
                self.div_control.set(DivisionControl::DIV_BY_ZERO, self.div_denominator == 0);
                self.div_control.remove(DivisionControl::BUSY);
            } else {
                self.div_cycle_countdown -= cycles;
            }
        }
        if self.sqrt_cycle_countdown > 0 {
            if self.sqrt_cycle_countdown <= cycles {
                self.sqrt_cycle_countdown = 0;
                self.sqrt_control.remove(SqrtControl::BUSY);
            } else {
                self.sqrt_cycle_countdown -= cycles;
            }
        }
    }

    pub fn start_div(&mut self) {
        match (self.div_control & DivisionControl::MODE).bits() {
            0 => {
                let numerator = (self.div_numerator & 0xFFFF_FFFF) as i32;
                let denominator = (self.div_denominator & 0xFFFF_FFFF) as i32;
                if denominator == 0 {
                    self.mod_result = numerator as i64;
                    self.div_result = self.mod_result >> 63;
                    self.div_result = ((self.div_result as u64) ^ 0xFFFF_FFFF_0000_0000) as i64;
                    return;
                }
                self.div_result = (numerator / denominator) as i64;
                self.mod_result = (numerator % denominator) as i64;
                //println!("Div32 {:X} / {:X} => {:X}", self.div_numerator, self.div_denominator, self.div_result);
                self.div_cycle_countdown = 18;
            },
            1 => {
                let denominator = (self.div_denominator & 0xFFFF_FFFF) as i32 as i64;
                if denominator == 0 {
                    self.mod_result = self.div_numerator;
                    self.div_result = self.mod_result >> 63;
                    self.div_result = ((self.div_result as u64) ^ 0xFFFF_FFFF_0000_0000) as i64;
                    return;
                }
                self.div_result = self.div_numerator / denominator;
                self.mod_result = self.div_numerator % denominator;
                //println!("Div48 {:X} / {:X} => {:X}", self.div_numerator, self.div_denominator, self.div_result);
                self.div_cycle_countdown = 34;
            },
            _ => {
                if self.div_denominator == 0 {
                    self.mod_result = self.div_numerator;
                    self.div_result = self.mod_result >> 63;
                    self.div_result = ((self.div_result as u64) ^ 0xFFFF_FFFF_0000_0000) as i64;
                    return;
                }
                self.div_result = self.div_numerator / self.div_denominator;
                self.mod_result = self.div_numerator % self.div_denominator;
                //println!("Div64 {:X} / {:X} => {:X}", self.div_numerator, self.div_denominator, self.div_result);
                self.div_cycle_countdown = 34;
            },
        }
        self.div_latch = true;
        self.div_control.insert(DivisionControl::BUSY);
    }

    pub fn start_sqrt(&mut self) {
        if self.sqrt_control.contains(SqrtControl::MODE) {
            let sqrt_in = self.sqrt_param as f64;
            self.sqrt_result = sqrt_in.sqrt() as u32;
        } else {
            let sqrt_in = (self.sqrt_param as u32) as f64;
            self.sqrt_result = sqrt_in.sqrt() as u32;
        }
        self.sqrt_cycle_countdown = 13;
        self.sqrt_latch = true;
        self.sqrt_control.insert(SqrtControl::BUSY);
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
                self.sqrt_control.set(SqrtControl::MODE, u32::test_bit(data, 0));
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
