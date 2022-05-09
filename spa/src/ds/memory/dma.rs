/// Direct memory access controller

use bitflags::bitflags;

use crate::utils::{
    bits::u32,
    bytes,
    meminterface::MemInterface32
};
use crate::common::dma::DMAAddress;
use crate::ds::interrupt::Interrupts;

bitflags!{
    #[derive(Default)]
    struct Control: u32 {
        const ENABLE        = u32::bit(31);
        const END_IRQ       = u32::bit(30);
        const START_TIMING  = u32::bits(27, 29);
        const WORD_TYPE     = u32::bit(26);
        const REPEAT        = u32::bit(25);
        const SRC_ADDR_MODE = u32::bits(23, 24);
        const DST_ADDR_MODE = u32::bits(21, 22);
        const WORD_COUNT    = u32::bits(0, 20);

        // ENABLE | START_TIMING
        const SHOULD_START  = u32::bit(31) | u32::bits(27, 29);
        const START_IMM     = u32::bit(31);
        const START_VBLANK  = u32::bit(31) | u32::bit(27);
        const START_HBLANK  = u32::bit(31) | u32::bit(28);
        const START_DISPLAY = u32::bit(31) | u32::bits(27, 28);
        const START_MAIN_D  = u32::bit(31) | u32::bit(29);
        const START_DS_CART = u32::bit(31) | u32::bit(27) | u32::bit(29);
        const START_GB_CART = u32::bit(31) | u32::bits(28, 29);
        const START_G_FIFO  = u32::bit(31) | u32::bits(27, 29);
    }
}

impl Control {
    fn word_count(self) -> u32 {
        (self & Control::WORD_COUNT).bits()
    }
}

const WORD_COUNT_MASK: u32 = 0x1F_FFFF;

/// The DMA Channels.
pub struct DMA {
    pub channels:   [DMAChannel; 4],
    /// Whether each channel is active or not.
    pub active:     [bool; 4],
    /// Fill data.
    fill_data:      [u32; 4],
}

impl DMA {
    pub fn new() -> Self {
        Self {
            channels: [
                DMAChannel::new(Interrupts::DMA_0),
                DMAChannel::new(Interrupts::DMA_1),
                DMAChannel::new(Interrupts::DMA_2),
                DMAChannel::new(Interrupts::DMA_3),
            ],
            active: [
                false, false, false, false
            ],
            fill_data: [
                0, 0, 0, 0
            ]
        }
    }

    /// Get the index of the highest-priority active channel.
    pub fn get_active(&self) -> Option<usize> {
        for c in 0..4 {
            if self.active[c] {
                return Some(c);
            }
        }
        None
    }

    /// Mark the channel as complete.
    pub fn set_inactive(&mut self, chan: usize) {
        self.active[chan] = false;
    }

    /// To be called when v-blank occurs.
    pub fn on_vblank(&mut self) {
        for (active, chan) in self.active.iter_mut().zip(&self.channels) {
            *active = *active || chan.should_start_vblank();
        }
    }

    /// To be called when h-blank occurs.
    pub fn on_hblank(&mut self) {
        for (active, chan) in self.active.iter_mut().zip(&self.channels) {
            *active = *active || chan.should_start_hblank();
        }
    }

    // To be called when display start occurs.
    /*pub fn on_display_start(&mut self) {
        for (active, chan) in self.active.iter_mut().zip(&self.channels) {
            *active = *active || chan.should_start_display();
        }
    }*/
}

impl MemInterface32 for DMA {
    fn read_halfword(&mut self, addr: u32) -> u16 {
        match addr {
            0x00..=0x0B => self.channels[0].read_halfword(addr),
            0x0C..=0x17 => self.channels[1].read_halfword(addr - 0x0C),
            0x18..=0x23 => self.channels[2].read_halfword(addr - 0x18),
            0x24..=0x2F => self.channels[3].read_halfword(addr - 0x24),
            0x30 => bytes::u32::lo(self.fill_data[0]),
            0x32 => bytes::u32::hi(self.fill_data[0]),
            0x34 => bytes::u32::lo(self.fill_data[1]),
            0x36 => bytes::u32::hi(self.fill_data[1]),
            0x38 => bytes::u32::lo(self.fill_data[2]),
            0x3A => bytes::u32::hi(self.fill_data[2]),
            0x3C => bytes::u32::lo(self.fill_data[3]),
            0x3E => bytes::u32::hi(self.fill_data[3]),
            _ => unreachable!()
        }
    }
    fn write_halfword(&mut self, addr: u32, data: u16) {
        match addr {
            0x00..=0x09 => self.channels[0].write_halfword(addr, data),
            0x0A        => {
                self.channels[0].write_halfword(addr, data);
                self.active[0] = self.channels[0].should_start_now();
            },
            0x0C..=0x15 => self.channels[1].write_halfword(addr - 0x0C, data),
            0x16        => {
                self.channels[1].write_halfword(addr - 0x0C, data);
                self.active[1] = self.channels[1].should_start_now();
            },
            0x18..=0x21 => self.channels[2].write_halfword(addr - 0x18, data),
            0x22        => {
                self.channels[2].write_halfword(addr - 0x18, data);
                self.active[2] = self.channels[2].should_start_now();
            },
            0x24..=0x2D => self.channels[3].write_halfword(addr - 0x24, data),
            0x2E        => {
                self.channels[3].write_halfword(addr - 0x24, data);
                self.active[3] = self.channels[3].should_start_now();
            },
            0x30 => self.fill_data[0] = bytes::u32::set_lo(self.fill_data[0], data),
            0x32 => self.fill_data[0] = bytes::u32::set_hi(self.fill_data[0], data),
            0x34 => self.fill_data[1] = bytes::u32::set_lo(self.fill_data[1], data),
            0x36 => self.fill_data[1] = bytes::u32::set_hi(self.fill_data[1], data),
            0x38 => self.fill_data[2] = bytes::u32::set_lo(self.fill_data[2], data),
            0x3A => self.fill_data[2] = bytes::u32::set_hi(self.fill_data[2], data),
            0x3C => self.fill_data[3] = bytes::u32::set_lo(self.fill_data[3], data),
            0x3E => self.fill_data[3] = bytes::u32::set_hi(self.fill_data[3], data),
            _ => unreachable!()
        }
    }

    fn read_word(&mut self, addr: u32) -> u32 {
        match addr {
            0x00..=0x0B => self.channels[0].read_word(addr),
            0x0C..=0x17 => self.channels[1].read_word(addr - 0x0C),
            0x18..=0x23 => self.channels[2].read_word(addr - 0x18),
            0x24..=0x2F => self.channels[3].read_word(addr - 0x24),
            0x30 => self.fill_data[0],
            0x34 => self.fill_data[1],
            0x38 => self.fill_data[2],
            0x3C => self.fill_data[3],
            _ => unreachable!()
        }
    }
    fn write_word(&mut self, addr: u32, data: u32) {
        match addr {
            0x00..=0x07 => self.channels[0].write_word(addr, data),
            0x08        => {
                self.channels[0].write_word(addr, data);
                self.active[0] = self.channels[0].should_start_now();
            },
            0x0C..=0x13 => self.channels[1].write_word(addr - 0x0C, data),
            0x14        => {
                self.channels[1].write_word(addr - 0x0C, data);
                self.active[1] = self.channels[1].should_start_now();
            },
            0x18..=0x1F => self.channels[2].write_word(addr - 0x18, data),
            0x20        => {
                self.channels[2].write_word(addr - 0x18, data);
                self.active[2] = self.channels[2].should_start_now();
            },
            0x24..=0x2B => self.channels[3].write_word(addr - 0x24, data),
            0x2C        => {
                self.channels[3].write_word(addr - 0x24, data);
                self.active[3] = self.channels[3].should_start_now();
            },
            0x30 => self.fill_data[0] = data,
            0x34 => self.fill_data[1] = data,
            0x38 => self.fill_data[2] = data,
            0x3C => self.fill_data[3] = data,
            _ => unreachable!()
        }
    }
}

/// A single DMA channel.
pub struct DMAChannel {
    // External control registers
    src_addr:           u32,
    dst_addr:           u32,
    control:            Control,
    //word_count:         u32,

    // Internal registers
    /// Word size in bytes. Can be 2 or 4.
    word_size:          u32,
    current_src_addr:   u32,
    current_dst_addr:   u32,
    current_count:      u32,

    // Channel-specific data (will remain const)
    interrupt:          Interrupts,
}

impl DMAChannel {
    pub fn new(interrupt: Interrupts) -> Self {
        Self {
            src_addr:           0,
            dst_addr:           0,
            //word_count:         0,
            control:            Control::default(),

            word_size:          2,
            current_src_addr:   0,
            current_dst_addr:   0,
            current_count:      0,

            interrupt:          interrupt,
        }
    }

    /// Check to see if dma should start immediately.
    pub fn should_start_now(&self) -> bool {
        (self.control & Control::SHOULD_START) == Control::START_IMM
    }

    /// Check to see if dma should start upon vblank.
    pub fn should_start_vblank(&self) -> bool {
        (self.control & Control::SHOULD_START) == Control::START_VBLANK
    }

    /// Check to see if dma should start upon hblank.
    pub fn should_start_hblank(&self) -> bool {
        (self.control & Control::SHOULD_START) == Control::START_HBLANK
    }

    /// Check to see if a 32-bit word should be transferred.
    pub fn transfer_32bit_word(&self) -> bool {
        self.control.contains(Control::WORD_TYPE)
    }

    /// Get next pair of addresses.
    /// If `Done` is returned, the addresses inside are the final ones, and this transfer is complete.
    pub fn next_addrs(&mut self) -> DMAAddress {
        let src_addr = self.current_src_addr;
        let dst_addr = self.current_dst_addr;
        self.current_src_addr = match (self.control & Control::SRC_ADDR_MODE).bits() >> 23 {
            0b00 => self.current_src_addr.wrapping_add(self.word_size),
            0b01 => self.current_src_addr.wrapping_sub(self.word_size),
            0b10 => self.current_src_addr,
            0b11 => panic!("invalid src addr mode 3"),
            _ => unreachable!()
        };
        /*self.current_dst_addr = if self.fifo_mode() {
            self.current_dst_addr
        } else {
            match (self.control & Control::DST_ADDR_MODE).bits() >> 5 {
                0b00 | 0b11 => self.current_dst_addr.wrapping_add(self.word_size),
                0b01 => self.current_dst_addr.wrapping_sub(self.word_size),
                0b10 => self.current_dst_addr,
                _ => unreachable!()
            }
        };*/
        self.current_dst_addr = match (self.control & Control::DST_ADDR_MODE).bits() >> 21 {
            0b00 | 0b11 => self.current_dst_addr.wrapping_add(self.word_size),
            0b01 => self.current_dst_addr.wrapping_sub(self.word_size),
            0b10 => self.current_dst_addr,
            _ => unreachable!()
        };

        self.current_count = self.current_count.wrapping_sub(1) & WORD_COUNT_MASK;
        if self.current_count == 0 {
            DMAAddress::Done{
                source: src_addr,
                dest: dst_addr,
                irq: self.reset().bits() as u16,
            }
        } else {
            DMAAddress::Addr{
                source: src_addr,
                dest: dst_addr,
            }
        }
    }
}

impl MemInterface32 for DMAChannel {
    fn read_halfword(&mut self, addr: u32) -> u16 {
        match addr {
            0x0 => 0,
            0x2 => 0,
            0x4 => 0,
            0x6 => 0,
            0x8 => bytes::u32::lo(self.control.bits()),
            0xA => bytes::u32::hi(self.control.bits()),
            _ => unreachable!()
        }
    }
    fn write_halfword(&mut self, addr: u32, data: u16) {
        match addr {
            0x0 => self.src_addr = bytes::u32::set_lo(self.src_addr, data),
            0x2 => self.src_addr = bytes::u32::set_hi(self.src_addr, data),
            0x4 => self.dst_addr = bytes::u32::set_lo(self.dst_addr, data),
            0x6 => self.dst_addr = bytes::u32::set_hi(self.dst_addr, data),
            0x8 => {
                let control = bytes::u32::set_lo(self.control.bits(), data);
                self.set_control(control);
            },
            0xA => {
                let control = bytes::u32::set_hi(self.control.bits(), data);
                self.set_control(control);
            },
            _ => unreachable!()
        }
    }

    fn read_word(&mut self, addr: u32) -> u32 {
        match addr {
            0x0 => 0,
            0x4 => 0,
            0x8 => self.control.bits(),
            _ => unreachable!()
        }
    }
    fn write_word(&mut self, addr: u32, data: u32) {
        match addr {
            0x0 => self.src_addr = data,
            0x4 => self.dst_addr = data,
            0x8 => self.set_control(data),
            _ => unreachable!()
        }
    }
}

// Internal
impl DMAChannel {
    fn set_control(&mut self, data: u32) {
        let was_enabled = self.control.contains(Control::ENABLE);
        self.control = Control::from_bits_truncate(data);
        let enabled = self.control.contains(Control::ENABLE);
        if enabled && !was_enabled {
            /*self.current_count = if self.fifo_mode() {
                4
            } else {
                self.word_count
            };*/
            self.current_count = self.control.word_count();

            if self.transfer_32bit_word() {
                self.word_size = 4;
                self.current_src_addr = self.src_addr & 0x0FFF_FFFC;
                self.current_dst_addr = self.dst_addr & 0x0FFF_FFFC;
            } else {
                self.word_size = 2;
                self.current_src_addr = self.src_addr & 0x0FFF_FFFE;
                self.current_dst_addr = self.dst_addr & 0x0FFF_FFFE;
            }
        }
    }

    /// Call on completion of DMA transfer.
    fn reset(&mut self) -> Interrupts {
        if self.control.contains(Control::REPEAT) {
            /*let fifo_mode = self.fifo_mode();
            self.current_count = if fifo_mode {
                4
            } else {
                self.word_count
            };*/
            self.current_count = self.control.word_count();
            if (self.control & Control::DST_ADDR_MODE).bits() == u32::bits(21, 22) {
                self.current_dst_addr = self.dst_addr;
            }
        } else {
            self.control.remove(Control::ENABLE);
        }

        if self.control.contains(Control::END_IRQ) {
            self.interrupt
        } else {
            Interrupts::empty()
        }
    }

    /*fn fifo_mode(&self) -> bool {
        self.fifo_special && self.should_start_special()
    }*/
}