/// Interrupt controller.

use bitflags::bitflags;
use crate::common::{
    bits::u16,
    meminterface::MemInterface16,
};

bitflags!{
    #[derive(Default)]
    pub struct Interrupts: u16 {
        const GAME_PAK  = u16::bit(13);
        const KEYPAD    = u16::bit(12);
        const DMA_3     = u16::bit(11);
        const DMA_2     = u16::bit(10);
        const DMA_1     = u16::bit(9);
        const DMA_0     = u16::bit(8);
        const SERIAL    = u16::bit(7);
        const TIMER_3   = u16::bit(6);
        const TIMER_2   = u16::bit(5);
        const TIMER_1   = u16::bit(4);
        const TIMER_0   = u16::bit(3);
        const V_COUNTER = u16::bit(2);
        const H_BLANK   = u16::bit(1);
        const V_BLANK   = u16::bit(0);
    }
}

pub struct InterruptControl {
    interrupt_enable:   Interrupts,
    interrupt_req:      Interrupts,
    interrupt_master:   bool,
}

impl InterruptControl {
    pub fn new() -> Self {
        Self {
            interrupt_enable:   Interrupts::default(),
            interrupt_req:      Interrupts::default(),
            interrupt_master:   false,
        }
    }

    /// Set from other devices when an interrupt should happen.
    pub fn interrupt_request(&mut self, interrupts: Interrupts) {
        self.interrupt_req.insert(interrupts);
    }

    /// Check if an IRQ should be sent to the CPU.
    pub fn irq(&self) -> bool {
        self.interrupt_master && self.interrupt_enable.intersects(self.interrupt_req)
    }
}

impl MemInterface16 for InterruptControl {
    fn read_halfword(&self, addr: u32) -> u16 {
        match addr {
            0x0 => self.interrupt_enable.bits(),
            0x2 => self.interrupt_req.bits(),
            0x8 => if self.interrupt_master {1} else {0},
            0xA => 0,
            _ => panic!("interrupt controller: read unreachable address {:X}", addr)
        }
    }
    fn write_halfword(&mut self, addr: u32, data: u16) {
        match addr {
            0x0 => self.interrupt_enable = Interrupts::from_bits_truncate(data),
            0x2 => self.interrupt_req.remove(Interrupts::from_bits_truncate(data)),
            0x8 => self.interrupt_master = u16::test_bit(data, 0),
            0xA => {},
            _ => panic!("interrupt controller: write unreachable address {:X}", addr)
        }
    }
}
