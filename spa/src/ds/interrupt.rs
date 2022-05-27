/// Interrupt controller.

use bitflags::bitflags;
use crate::utils::{
    bits::u32,
    meminterface::MemInterface32,
};

bitflags!{
    #[derive(Default)]
    pub struct Interrupts: u32 {
        const WIFI              = u32::bit(24);
        const SPI               = u32::bit(23);
        const SCREEN_UNFOLD     = u32::bit(22);
        const GEOM_FIFO         = u32::bit(21);
        const CARD_IRQ          = u32::bit(20);
        const CARD_COMPLETE     = u32::bit(19);
        const IPC_RECV_NEMPTY   = u32::bit(18);
        const IPC_SEND_EMPTY    = u32::bit(17);
        const IPC_SYNC          = u32::bit(16);

        const GAME_PAK  = u32::bit(13);
        const KEYPAD    = u32::bit(12);
        const DMA_3     = u32::bit(11);
        const DMA_2     = u32::bit(10);
        const DMA_1     = u32::bit(9);
        const DMA_0     = u32::bit(8);
        const RTC       = u32::bit(7);
        const TIMER_3   = u32::bit(6);
        const TIMER_2   = u32::bit(5);
        const TIMER_1   = u32::bit(4);
        const TIMER_0   = u32::bit(3);
        const V_COUNTER = u32::bit(2);
        const H_BLANK   = u32::bit(1);
        const V_BLANK   = u32::bit(0);
    }
}

pub struct InterruptControl {
    interrupt_enable:   Interrupts,
    interrupt_req:      Interrupts,
    interrupt_master:   bool,
    name: String
}

impl InterruptControl {
    pub fn new(name: &str) -> Self {
        Self {
            interrupt_enable:   Interrupts::default(),
            interrupt_req:      Interrupts::default(),
            interrupt_master:   false,
            name: name.to_string()
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

impl MemInterface32 for InterruptControl {
    fn read_word(&mut self, addr: u32) -> u32 {
        match addr {
            0x0400_0208 => if self.interrupt_master {1} else {0},
            0x0400_020C => 0,
            0x0400_0210 => self.interrupt_enable.bits(),
            0x0400_0214 => self.interrupt_req.bits(),
            _ => panic!("interrupt controller: read unreachable address {:X}", addr)
        }
    }
    fn write_word(&mut self, addr: u32, data: u32) {
        match addr {
            0x0400_0208 => self.interrupt_master = u32::test_bit(data, 0),
            0x0400_020C => {},
            0x0400_0210 => {
                //println!("{} enable: {:X}", self.name, data);
                self.interrupt_enable = Interrupts::from_bits_truncate(data)
            },
            0x0400_0214 => self.interrupt_req.remove(Interrupts::from_bits_truncate(data)),
            _ => panic!("interrupt controller: write unreachable address {:X}", addr)
        }
    }
}
