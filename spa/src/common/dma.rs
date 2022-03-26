/// Direct memory access controller

use bitflags::bitflags;

use crate::utils::{
    bits::u16,
    bytes::u32,
    meminterface::MemInterface16
};

bitflags!{
    #[derive(Default)]
    struct Control: u16 {
        const ENABLE        = u16::bit(15);
        const END_IRQ       = u16::bit(14);
        const START_TIMING  = u16::bits(12, 13);
        const GAME_PAK_DRQ  = u16::bit(11);
        const WORD_TYPE     = u16::bit(10);
        const REPEAT        = u16::bit(9);
        const SRC_ADDR_MODE = u16::bits(7, 8);
        const DST_ADDR_MODE = u16::bits(5, 6);

        // ENABLE | START_TIMING
        const SHOULD_START  = u16::bit(15) | u16::bits(12, 13);
        const START_IMM     = u16::bit(15);
        const START_VBLANK  = u16::bit(15) | u16::bit(12);
        const START_HBLANK  = u16::bit(15) | u16::bit(13);
        const START_SPECIAL = u16::bit(15) | u16::bits(12, 13);
    }
}

// Interrupt bits.
const DMA_0: u16 = u16::bit(8);
const DMA_1: u16 = u16::bit(9);
const DMA_2: u16 = u16::bit(10);
const DMA_3: u16 = u16::bit(11);

/// The DMA Channels.
pub struct DMA {
    pub channels:   [DMAChannel; 4],
    /// Whether each channel is active or not.
    active:     [bool; 4],
}

impl DMA {
    pub fn new() -> Self {
        Self {
            channels: [
                DMAChannel::new(DMA_0, 0x3FFF, false),
                DMAChannel::new(DMA_1, 0x3FFF, true),
                DMAChannel::new(DMA_2, 0x3FFF, true),
                DMAChannel::new(DMA_3, 0xFFFF, false),
            ],
            active: [
                false, false, false, false
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

    /// To be called when requested by sound FIFO.
    pub fn on_sound_fifo_1(&mut self) {
        self.active[1] = self.active[1] || self.channels[1].should_start_special();
    }

    /// To be called when requested by sound FIFO.
    pub fn on_sound_fifo_2(&mut self) {
        self.active[2] = self.active[2] || self.channels[2].should_start_special();
    }

    /// To be called on each video line between 2 and 162.
    pub fn on_video_capture(&mut self) {
        self.active[3] = self.active[3] || self.channels[3].should_start_special();
    }
}

impl MemInterface16 for DMA {
    fn read_halfword(&mut self, addr: u32) -> u16 {
        match addr {
            0x00..=0x0B => self.channels[0].read_halfword(addr),
            0x0C..=0x17 => self.channels[1].read_halfword(addr - 0x0C),
            0x18..=0x23 => self.channels[2].read_halfword(addr - 0x18),
            0x24..=0x2F => self.channels[3].read_halfword(addr - 0x24),
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
            _ => unreachable!()
        }
    }
}

/// Returned by `DMAChannel::next_addrs`.
pub enum DMAAddress {
    /// A pair of addresses.
    Addr{
        source: u32,
        dest: u32,
    },
    /// Returned when the DMA is complete after this transfer.
    Done{
        source: u32,
        dest: u32,
        irq: u16,
    }
}

/// A single DMA channel.
pub struct DMAChannel {
    // External control registers
    src_addr:           u32,
    dst_addr:           u32,
    word_count:         u16,
    control:            Control,

    // Internal registers
    /// Word size in bytes. Can be 2 or 4.
    word_size:          u32,
    current_src_addr:   u32,
    current_dst_addr:   u32,
    current_count:      u16,

    // Channel-specific data (will remain const)
    fifo_special:       bool,
    word_count_mask:    u16,
    interrupt:          u16,
}

impl DMAChannel {
    pub fn new(interrupt: u16, word_count_mask: u16, fifo: bool) -> Self {
        Self {
            src_addr:           0,
            dst_addr:           0,
            word_count:         0,
            control:            Control::default(),

            word_size:          2,
            current_src_addr:   0,
            current_dst_addr:   0,
            current_count:      0,

            fifo_special:       fifo,
            word_count_mask:    word_count_mask,
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

    /// Check to see if dma should start upon special conditions.
    pub fn should_start_special(&self) -> bool {
        (self.control & Control::SHOULD_START) == Control::START_SPECIAL
    }

    /// Check to see if a 32-bit word should be transferred.
    pub fn transfer_32bit_word(&self) -> bool {
        self.fifo_mode() || self.control.contains(Control::WORD_TYPE)
    }

    /// Get next pair of addresses.
    /// If `Done` is returned, the addresses inside are the final ones, and this transfer is complete.
    pub fn next_addrs(&mut self) -> DMAAddress {
        let src_addr = self.current_src_addr;
        let dst_addr = self.current_dst_addr;
        self.current_src_addr = match (self.control & Control::SRC_ADDR_MODE).bits() >> 7 {
            0b00 => self.current_src_addr.wrapping_add(self.word_size),
            0b01 => self.current_src_addr.wrapping_sub(self.word_size),
            0b10 => self.current_src_addr,
            0b11 => panic!("invalid src addr mode 3"),
            _ => unreachable!()
        };
        self.current_dst_addr = if self.fifo_mode() {
            self.current_dst_addr
        } else {
            match (self.control & Control::DST_ADDR_MODE).bits() >> 5 {
                0b00 | 0b11 => self.current_dst_addr.wrapping_add(self.word_size),
                0b01 => self.current_dst_addr.wrapping_sub(self.word_size),
                0b10 => self.current_dst_addr,
                _ => unreachable!()
            }
        };

        self.current_count = self.current_count.wrapping_sub(1) & self.word_count_mask;
        if self.current_count == 0 {
            DMAAddress::Done{
                source: src_addr,
                dest: dst_addr,
                irq: self.reset(),
            }
        } else {
            DMAAddress::Addr{
                source: src_addr,
                dest: dst_addr,
            }
        }
    }
}

impl MemInterface16 for DMAChannel {
    fn read_halfword(&mut self, addr: u32) -> u16 {
        match addr {
            0x0 => 0,
            0x2 => 0,
            0x4 => 0,
            0x6 => 0,
            0x8 => 0,
            0xA => self.control.bits(),
            _ => unreachable!()
        }
    }
    fn write_halfword(&mut self, addr: u32, data: u16) {
        match addr {
            0x0 => self.src_addr = u32::set_lo(self.src_addr, data),
            0x2 => self.src_addr = u32::set_hi(self.src_addr, data),
            0x4 => self.dst_addr = u32::set_lo(self.dst_addr, data),
            0x6 => self.dst_addr = u32::set_hi(self.dst_addr, data),
            0x8 => self.word_count = data & self.word_count_mask,
            0xA => self.set_control(data),
            _ => unreachable!()
        }
    }

    fn write_word(&mut self, addr: u32, data: u32) {
        match addr {
            0x0 => self.src_addr = data,
            0x4 => self.dst_addr = data,
            0x8 => {
                self.word_count = u32::lo(data) & self.word_count_mask;
                self.set_control(u32::hi(data));
            },
            _ => unreachable!()
        }
    }
}

// Internal
impl DMAChannel {
    fn set_control(&mut self, data: u16) {
        let was_enabled = self.control.contains(Control::ENABLE);
        self.control = Control::from_bits_truncate(data);
        let enabled = self.control.contains(Control::ENABLE);
        if enabled && !was_enabled {
            self.current_count = if self.fifo_mode() {
                4
            } else {
                self.word_count
            };

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
    fn reset(&mut self) -> u16 {
        if self.control.contains(Control::REPEAT) {
            let fifo_mode = self.fifo_mode();
            self.current_count = if fifo_mode {
                4
            } else {
                self.word_count
            };
            if (self.control & Control::DST_ADDR_MODE).bits() == u16::bits(5, 6) {
                self.current_dst_addr = self.dst_addr;
            }
        } else {
            self.control.remove(Control::ENABLE);
        }

        if self.control.contains(Control::END_IRQ) {
            self.interrupt
        } else {
            0
        }
    }

    fn fifo_mode(&self) -> bool {
        self.fifo_special && self.should_start_special()
    }
}