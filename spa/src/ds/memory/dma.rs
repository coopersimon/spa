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

    /// To be called when card data is ready.
    pub fn on_card(&mut self) {
        for (active, chan) in self.active.iter_mut().zip(&self.channels) {
            *active = *active || chan.should_start_card();
        }
    }
}

impl MemInterface32 for DMA {
    fn read_halfword(&mut self, addr: u32) -> u16 {
        match addr {
            0x0400_00B0..=0x0400_00BB => self.channels[0].read_halfword(addr - 0x0400_00B0),
            0x0400_00BC..=0x0400_00C7 => self.channels[1].read_halfword(addr - 0x0400_00BC),
            0x0400_00C8..=0x0400_00D3 => self.channels[2].read_halfword(addr - 0x0400_00C8),
            0x0400_00D4..=0x0400_00DF => self.channels[3].read_halfword(addr - 0x0400_00D4),
            0x0400_00E0 => bytes::u32::lo(self.fill_data[0]),
            0x0400_00E2 => bytes::u32::hi(self.fill_data[0]),
            0x0400_00E4 => bytes::u32::lo(self.fill_data[1]),
            0x0400_00E6 => bytes::u32::hi(self.fill_data[1]),
            0x0400_00E8 => bytes::u32::lo(self.fill_data[2]),
            0x0400_00EA => bytes::u32::hi(self.fill_data[2]),
            0x0400_00EC => bytes::u32::lo(self.fill_data[3]),
            0x0400_00EE => bytes::u32::hi(self.fill_data[3]),
            _ => unreachable!()
        }
    }
    fn write_halfword(&mut self, addr: u32, data: u16) {
        match addr {
            0x0400_00B0..=0x0400_00B9 => self.channels[0].write_halfword(addr - 0x0400_00B0, data),
            0x0400_00BA => {
                self.channels[0].write_halfword(addr - 0x0400_00B0, data);
                self.active[0] = self.channels[0].should_start_now();
            },
            0x0400_00BC..=0x0400_00C5 => self.channels[1].write_halfword(addr - 0x0400_00BC, data),
            0x0400_00C6 => {
                self.channels[1].write_halfword(addr - 0x0400_00BC, data);
                self.active[1] = self.channels[1].should_start_now();
            },
            0x0400_00C8..=0x0400_00D1 => self.channels[2].write_halfword(addr - 0x0400_00C8, data),
            0x0400_00D2 => {
                self.channels[2].write_halfword(addr - 0x0400_00C8, data);
                self.active[2] = self.channels[2].should_start_now();
            },
            0x0400_00D4..=0x0400_00DD => self.channels[3].write_halfword(addr - 0x0400_00D4, data),
            0x0400_00DE => {
                self.channels[3].write_halfword(addr - 0x0400_00D4, data);
                self.active[3] = self.channels[3].should_start_now();
            },
            0x0400_00E0 => self.fill_data[0] = bytes::u32::set_lo(self.fill_data[0], data),
            0x0400_00E2 => self.fill_data[0] = bytes::u32::set_hi(self.fill_data[0], data),
            0x0400_00E4 => self.fill_data[1] = bytes::u32::set_lo(self.fill_data[1], data),
            0x0400_00E6 => self.fill_data[1] = bytes::u32::set_hi(self.fill_data[1], data),
            0x0400_00E8 => self.fill_data[2] = bytes::u32::set_lo(self.fill_data[2], data),
            0x0400_00EA => self.fill_data[2] = bytes::u32::set_hi(self.fill_data[2], data),
            0x0400_00EC => self.fill_data[3] = bytes::u32::set_lo(self.fill_data[3], data),
            0x0400_00EE => self.fill_data[3] = bytes::u32::set_hi(self.fill_data[3], data),
            _ => unreachable!()
        }
    }

    fn read_word(&mut self, addr: u32) -> u32 {
        match addr {
            0x0400_00B0..=0x0400_00BB => self.channels[0].read_word(addr - 0x0400_00B0),
            0x0400_00BC..=0x0400_00C7 => self.channels[1].read_word(addr - 0x0400_00BC),
            0x0400_00C8..=0x0400_00D3 => self.channels[2].read_word(addr - 0x0400_00C8),
            0x0400_00D4..=0x0400_00DF => self.channels[3].read_word(addr - 0x0400_00D4),
            0x0400_00E0 => self.fill_data[0],
            0x0400_00E4 => self.fill_data[1],
            0x0400_00E8 => self.fill_data[2],
            0x0400_00EC => self.fill_data[3],
            _ => unreachable!()
        }
    }
    fn write_word(&mut self, addr: u32, data: u32) {
        match addr {
            0x0400_00B0..=0x0400_00B7 => self.channels[0].write_word(addr - 0x0400_00B0, data),
            0x0400_00B8 => {
                self.channels[0].write_word(addr - 0x0400_00B0, data);
                self.active[0] = self.channels[0].should_start_now();
            },
            0x0400_00BC..=0x0400_00C3 => self.channels[1].write_word(addr - 0x0400_00BC, data),
            0x0400_00C4 => {
                self.channels[1].write_word(addr - 0x0400_00BC, data);
                self.active[1] = self.channels[1].should_start_now();
            },
            0x0400_00C8..=0x0400_00CF => self.channels[2].write_word(addr - 0x0400_00C8, data),
            0x0400_00D0 => {
                self.channels[2].write_word(addr - 0x0400_00C8, data);
                self.active[2] = self.channels[2].should_start_now();
            },
            0x0400_00D4..=0x0400_00DB => self.channels[3].write_word(addr - 0x0400_00D4, data),
            0x0400_00DC => {
                self.channels[3].write_word(addr - 0x0400_00D4, data);
                self.active[3] = self.channels[3].should_start_now();
            },
            0x0400_00E0 => self.fill_data[0] = data,
            0x0400_00E4 => self.fill_data[1] = data,
            0x0400_00E8 => self.fill_data[2] = data,
            0x0400_00EC => self.fill_data[3] = data,
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

    /// Check to see if dma should start upon screen display.
    pub fn should_start_display(&self) -> bool {
        (self.control & Control::SHOULD_START) == Control::START_DISPLAY
    }

    /// Check to see if dma should start when main mem FIFO requests.
    pub fn should_start_main_mem(&self) -> bool {
        (self.control & Control::SHOULD_START) == Control::START_MAIN_D
    }

    /// Check to see if dma should start upon card ready.
    pub fn should_start_card(&self) -> bool {
        (self.control & Control::SHOULD_START) == Control::START_DS_CART
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
                self.control = Control::from_bits_truncate(bytes::u32::set_lo(self.control.bits(), data));
                //self.set_control(control);
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
        let new_control = Control::from_bits_truncate(data);
        let enabled = new_control.contains(Control::ENABLE);
        self.control = new_control;
        //if enabled != was_enabled {
        //    self.control = new_control;
        //    //println!("SET DMA CTRL: {:X} | len: {:X} | {:X} => {:X}", data, self.control.word_count(), self.src_addr, self.dst_addr);
        //}
        if enabled && !was_enabled {
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
            self.current_count = self.control.word_count();

            if (self.control & Control::DST_ADDR_MODE).bits() == u32::bits(21, 22) {
                if self.transfer_32bit_word() {
                    self.current_dst_addr = self.dst_addr & 0x0FFF_FFFC;
                } else {
                    self.current_dst_addr = self.dst_addr & 0x0FFF_FFFE;
                }
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
}