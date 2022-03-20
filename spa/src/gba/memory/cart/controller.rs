/// Module which deals with the on-device control of the game pak.

use arm::MemCycleType;
use bitflags::bitflags;
use crate::utils::{
    bits::u16,
    bytes::u32,
    meminterface::MemInterface16
};

bitflags!{
    #[derive(Default)]
    struct Control: u16 {
        const TYPE_FLAG = u16::bit(15);
        const PREFETCH  = u16::bit(14);
        const WAIT_2_S  = u16::bit(10);
        const WAIT_2_N  = u16::bits(8, 9);
        const WAIT_1_S  = u16::bit(7);
        const WAIT_1_N  = u16::bits(5, 6);
        const WAIT_0_S  = u16::bit(4);
        const WAIT_0_N  = u16::bits(2, 3);
        const SRAM_WAIT = u16::bits(0, 1);
    }
}

/// The controller for the game pak, which controls wait states for memory accesses,
/// and the pre-fetch buffer.
pub struct GamePakController {
    control:    Control,

    sram_wait:  usize,
    wait_0_n:   usize,
    wait_0_s:   usize,
    wait_1_n:   usize,
    wait_1_s:   usize,
    wait_2_n:   usize,
    wait_2_s:   usize,

    // TODO: prefetch buffer
}

impl GamePakController {
    pub fn new() -> Self {
        Self {
            control: Control::default(),

            sram_wait:  5,
            wait_0_n:   5,
            wait_0_s:   3,
            wait_1_n:   5,
            wait_1_s:   5,
            wait_2_n:   5,
            wait_2_s:   9,
        }
    }

    pub fn sram_wait_cycles(&self) -> usize {
        self.sram_wait
    }

    pub fn wait_cycles_0(&self, cycle_type: MemCycleType) -> usize {
        match cycle_type {
            MemCycleType::N => self.wait_0_n,
            MemCycleType::S => self.wait_0_s,
        }
    }

    pub fn wait_cycles_1(&self, cycle_type: MemCycleType) -> usize {
        match cycle_type {
            MemCycleType::N => self.wait_1_n,
            MemCycleType::S => self.wait_1_s,
        }
    }

    pub fn wait_cycles_2(&self, cycle_type: MemCycleType) -> usize {
        match cycle_type {
            MemCycleType::N => self.wait_2_n,
            MemCycleType::S => self.wait_2_s,
        }
    }
}

impl MemInterface16 for GamePakController {
    fn read_halfword(&mut self, addr: u32) -> u16 {
        match addr {
            0 => self.control.bits(),
            2 => 0,
            _ => unreachable!()
        }
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {
        match addr {
            0 => {
                self.control = Control::from_bits_truncate(data);
                self.sram_wait = transfer_cycles((self.control & Control::SRAM_WAIT).bits());
                self.wait_0_n = transfer_cycles((self.control & Control::WAIT_0_N).bits() >> 2);
                self.wait_0_s = if self.control.contains(Control::WAIT_0_S) {2} else {3};
                self.wait_1_n = transfer_cycles((self.control & Control::WAIT_1_N).bits() >> 5);
                self.wait_1_s = if self.control.contains(Control::WAIT_0_S) {2} else {5};
                self.wait_2_n = transfer_cycles((self.control & Control::WAIT_2_N).bits() >> 8);
                self.wait_2_s = if self.control.contains(Control::WAIT_0_S) {2} else {9};
                // TODO: prefetch buffer
            },
            2 => {},
            _ => unreachable!()
        }
    }
}

/// When setting the wait state control, decode the desired transfer cycles
/// from the bits.
/// 
/// `from` must be in the range 0..4
fn transfer_cycles(from: u16) -> usize {
    match from {
        0b00 => 5,
        0b01 => 4,
        0b10 => 3,
        0b11 => 9,
        _ => unreachable!()
    }
}