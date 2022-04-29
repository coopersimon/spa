/// NDS video

mod constants;
mod render;
mod memory;

use bitflags::bitflags;
use crate::utils::{
    meminterface::MemInterface16,
    bits::u16,
    bytes
};
use crate::ds::interrupt::Interrupts;
pub use render::*;
use memory::DSVideoMemory;
pub use memory::{ARM7VRAM, VRAMRegion};

use constants::*;

/// Signal from the video unit.
pub enum Signal {
    None,
    HBlank,
    VBlank
}

/// Video rendering.
/// 
/// Consists of three parts:
/// - This object, which acts as the interface from CPU to memory,
///   and manages timing of the drawing and blanking periods
/// - The memory (VRAM, OAM, Palette memory, and registers)
/// - The renderer (which converts memory to images)
pub struct DSVideo<R: Renderer> {
    state:          VideoState,
    cycle_count:    usize,

    v_count:        u16,
    lcd_status:     LCDStatus,

    pub mem:        DSVideoMemory,

    renderer:       R,
}

impl<R: Renderer> DSVideo<R> {
    pub fn new(renderer: R) -> (Self, ARM7VRAM) {
        let (arm9_mem, arm7_vram) = DSVideoMemory::new();
        (Self {
            state:          VideoState::Init,
            cycle_count:    0,

            v_count:        0,
            lcd_status:     LCDStatus::default(),

            mem:            arm9_mem,

            renderer:       renderer,
        }, arm7_vram)
    }

    pub fn clock(&mut self, cycles: usize) -> (Signal, Interrupts) {
        use VideoState::*;
        use Transition::*;
        self.cycle_count += cycles;

        match self.state {
            Init                                                => self.transition_state(StartFrame, 0),
            Drawing if self.cycle_count >= H_CYCLES             => self.transition_state(EnterHBlank, H_CYCLES),
            HBlank if self.cycle_count >= H_BLANK_CYCLES => if self.v_count < V_MAX {
                self.transition_state(BeginDrawing, H_BLANK_CYCLES)
            } else {
                self.transition_state(EnterVBlank, H_BLANK_CYCLES)
            },
            VHBlank if self.cycle_count >= H_BLANK_CYCLES => if self.v_count < VBLANK_MAX {
                self.transition_state(ExitVHBlank, H_BLANK_CYCLES)
            } else {
                self.transition_state(StartFrame, H_BLANK_CYCLES)
            },
            VBlank if self.cycle_count >= H_CYCLES              => self.transition_state(EnterVHBlank, H_CYCLES),
            _                                                   => (Signal::None, Interrupts::default()),
        }
    }
}

// Interface refers to engine A registers + LCD status
impl<R: Renderer> MemInterface16 for DSVideo<R> {
    fn read_halfword(&mut self, addr: u32) -> u16 {
        match addr {
            0x4 => self.lcd_status.bits(),
            0x6 => self.v_count as u16,
            0x00..=0x6F => self.mem.mut_engine_a().registers.read_halfword(addr),
            _ => panic!("reading invalid video address {:X}", addr)
        }
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {
        match addr {
            0x04 => self.set_lcd_status(data),
            0x06 => {},
            0x00..=0x6F => self.mem.mut_engine_a().registers.write_halfword(addr, data),
            _ => panic!("writing invalid video address {:X}", addr)
        }
    }

    fn read_word(&mut self, addr: u32) -> u32 {
        match addr {
            0x04 => bytes::u32::make(self.v_count, self.lcd_status.bits()),
            0x00..=0x6F => self.mem.mut_engine_a().registers.read_word(addr),
            _ => panic!("reading invalid video address {:X}", addr)
        }
    }

    fn write_word(&mut self, addr: u32, data: u32) {
        match addr {
            0x04 => {
                self.set_lcd_status(bytes::u32::lo(data));
                self.v_count = bytes::u32::hi(data);
            },
            0x00..=0x6F => self.mem.mut_engine_a().registers.write_word(addr, data),
            _ => panic!("writing invalid video address {:X}", addr)
        }
    }
}

bitflags! {
    #[derive(Default)]
    struct LCDStatus: u16 {
        const VCOUNT        = u16::bits(8, 15);
        const VCOUNT_HI     = u16::bit(7);
        const VCOUNT_IRQ    = u16::bit(5);
        const HBLANK_IRQ    = u16::bit(4);
        const VBLANK_IRQ    = u16::bit(3);
        const VCOUNT_FLAG   = u16::bit(2);
        const HBLANK_FLAG   = u16::bit(1);
        const VBLANK_FLAG   = u16::bit(0);
    }
}

impl LCDStatus {
    fn get_flags(self) -> LCDStatus {
        self & (LCDStatus::VBLANK_FLAG | LCDStatus::HBLANK_FLAG | LCDStatus::VCOUNT_FLAG)
    }

    fn v_count(self) -> u16 {
        ((self & LCDStatus::VCOUNT).bits() >> 8) | ((self & LCDStatus::VCOUNT_HI).bits() << 8)
    }
}

// Internal
impl<R: Renderer> DSVideo<R> {
    fn transition_state(&mut self, transition: Transition, threshold: usize) -> (Signal, Interrupts) {
        use VideoState::*;
        use Transition::*;
        self.cycle_count -= threshold;

        match transition {
            StartFrame => {
                self.state = Drawing;
                self.v_count = 0;
                self.lcd_status.remove(LCDStatus::VBLANK_FLAG | LCDStatus::HBLANK_FLAG);
                self.lcd_status.set(LCDStatus::VCOUNT_FLAG, self.v_count == self.lcd_status.v_count());
                self.renderer.start_frame();
                self.renderer.render_line(&mut self.mem, 0);
                (Signal::None, self.v_count_irq())
            },
            BeginDrawing => {
                self.state = Drawing;
                self.v_count += 1;
                self.lcd_status.remove(LCDStatus::HBLANK_FLAG);
                self.lcd_status.set(LCDStatus::VCOUNT_FLAG, self.v_count == self.lcd_status.v_count());
                self.renderer.render_line(&mut self.mem, self.v_count);
                (Signal::None, self.v_count_irq())
            },
            EnterHBlank => {
                self.state = HBlank;
                self.lcd_status.insert(LCDStatus::HBLANK_FLAG);
                (Signal::HBlank, self.h_blank_irq())
            },
            EnterVBlank => {
                self.state = VBlank;
                self.v_count += 1;
                self.lcd_status.insert(LCDStatus::VBLANK_FLAG);
                self.lcd_status.set(LCDStatus::VCOUNT_FLAG, self.v_count == self.lcd_status.v_count());
                self.renderer.finish_frame();
                (Signal::VBlank, self.v_blank_irq())
            }
            EnterVHBlank => {
                self.state = VHBlank;
                self.lcd_status.insert(LCDStatus::HBLANK_FLAG);
                (Signal::None, Interrupts::empty())
            },
            ExitVHBlank => {
                self.state = VBlank;
                self.v_count += 1;
                self.lcd_status.remove(LCDStatus::HBLANK_FLAG);
                self.lcd_status.set(LCDStatus::VCOUNT_FLAG, self.v_count == self.lcd_status.v_count());
                (Signal::None, self.v_count_irq())
            },
        }
    }

    #[inline]
    fn v_count_irq(&self) -> Interrupts {
        if self.lcd_status.contains(LCDStatus::VCOUNT_IRQ | LCDStatus::VCOUNT_FLAG) {
            Interrupts::V_COUNTER
        } else {
            Interrupts::empty()
        }
    }

    #[inline]
    fn h_blank_irq(&self) -> Interrupts {
        if self.lcd_status.contains(LCDStatus::HBLANK_IRQ | LCDStatus::HBLANK_FLAG) {
            Interrupts::H_BLANK
        } else {
            Interrupts::empty()
        }
    }

    #[inline]
    fn v_blank_irq(&self) -> Interrupts {
        if self.lcd_status.contains(LCDStatus::VBLANK_IRQ | LCDStatus::VBLANK_FLAG) {
            Interrupts::V_BLANK
        } else {
            Interrupts::empty()
        }
    }

    fn set_lcd_status(&mut self, data: u16) {
        let old_flags = self.lcd_status.get_flags();
        let lcd_status = LCDStatus::from_bits_truncate(data & 0xFFF8);
        self.lcd_status = lcd_status | old_flags;
    }
}

enum VideoState {
    Init,       // Initial state.
    Drawing,    // Drawing a line.
    HBlank,     // Horizontal blanking period.
    VBlank,     // Vertical blanking period.
    VHBlank,    // Horizontal blanking period during v-blank.
}

enum Transition {
    StartFrame,     // Exit V-blank and start drawing a new frame
    BeginDrawing,   // Start drawing a line
    EnterHBlank,    // Enter H-blank
    EnterVBlank,    // Enter V-blank
    EnterVHBlank,   // Enter H-blank while in V-blank
    ExitVHBlank,    // Exit H-blank while in V-blank
}