/// GBA video

mod constants;
mod memory;
mod render;

use bitflags::bitflags;
use crate::utils::{
    meminterface::MemInterface16,
    bits::u16,
    bytes
};
use crate::common::video::mem::VideoMemory;
use crate::gba::interrupt::Interrupts;
pub use render::*;
use memory::{VRAM, VRAMRenderRef};
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
/// - A unit that manages timing of drawing and blanking periods
/// - The memory (VRAM, OAM, Palette memory, and registers)
/// - The renderer (which converts memory to image)
pub struct GBAVideo<R: Renderer> {
    state:          VideoState,
    cycle_count:    usize,

    lcd_status:     LCDStatus,
    v_count:        u8,

    vram:           VRAM,
    mem:            VideoMemory<VRAMRenderRef>,

    renderer:       R,
}

impl<R: Renderer> GBAVideo<R> {
    pub fn new(renderer: R) -> Self {
        let (vram, render_ref) = VRAM::new();
        Self {
            state:          VideoState::Init,
            cycle_count:    0,

            lcd_status:     LCDStatus::default(),
            v_count:        0,

            vram:           vram,
            mem:            VideoMemory::new(render_ref),

            renderer:       renderer,
        }
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

impl<R: Renderer> MemInterface16 for GBAVideo<R> {
    fn read_halfword(&mut self, addr: u32) -> u16 {
        match addr {
            0x0400_0004 => self.lcd_status.bits(),
            0x0400_0006 => self.v_count as u16,
            0x0400_0000..=0x0400_0057 => self.mem.registers.read_halfword(addr & 0xFF),
            0x0500_0000..=0x05FF_FFFF => self.mem.palette.read_halfword(addr & 0x3FF),
            0x0600_0000..=0x06FF_FFFF => self.vram.read_halfword(addr & 0x1_FFFF),
            0x0700_0000..=0x07FF_FFFF => self.mem.oam.read_halfword(addr & 0x3FF),
            _ => panic!("reading invalid video address {:X}", addr)
        }
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {
        match addr {
            0x0400_0004 => self.set_lcd_status(data),
            0x0400_0006 => {},
            0x0400_0000..=0x0400_0057 => self.mem.registers.write_halfword(addr & 0xFF, data),
            0x0500_0000..=0x05FF_FFFF => self.mem.palette.write_halfword(addr & 0x3FF, data),
            0x0600_0000..=0x06FF_FFFF => self.vram.write_halfword(addr & 0x1_FFFF, data),
            0x0700_0000..=0x07FF_FFFF => self.mem.oam.write_halfword(addr & 0x3FF, data),
            _ => panic!("writing invalid video address {:X}", addr)
        }
    }
}

bitflags! {
    #[derive(Default)]
    struct LCDStatus: u16 {
        const VCOUNT        = u16::bits(8, 15);
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
}

// Internal
impl<R: Renderer> GBAVideo<R> {
    fn transition_state(&mut self, transition: Transition, threshold: usize) -> (Signal, Interrupts) {
        use VideoState::*;
        use Transition::*;
        self.cycle_count -= threshold;

        match transition {
            StartFrame => {
                self.state = Drawing;
                self.v_count = 0;
                self.lcd_status.remove(LCDStatus::VBLANK_FLAG | LCDStatus::HBLANK_FLAG);
                self.lcd_status.set(LCDStatus::VCOUNT_FLAG, self.v_count == bytes::u16::hi(self.lcd_status.bits()));
                self.renderer.start_frame(&mut self.mem);
                self.renderer.render_line(&mut self.mem, 0);
                (Signal::None, self.v_count_irq())
            },
            BeginDrawing => {
                self.state = Drawing;
                self.v_count += 1;
                self.lcd_status.remove(LCDStatus::HBLANK_FLAG);
                self.lcd_status.set(LCDStatus::VCOUNT_FLAG, self.v_count == bytes::u16::hi(self.lcd_status.bits()));
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
                self.lcd_status.set(LCDStatus::VCOUNT_FLAG, self.v_count == bytes::u16::hi(self.lcd_status.bits()));
                self.renderer.finish_frame();
                (Signal::VBlank, self.v_blank_irq())
            }
            EnterVHBlank => {
                self.state = VHBlank;
                self.lcd_status.insert(LCDStatus::HBLANK_FLAG);
                (Signal::None, Interrupts::default())
            },
            ExitVHBlank => {
                self.state = VBlank;
                self.v_count += 1;
                self.lcd_status.remove(LCDStatus::HBLANK_FLAG);
                self.lcd_status.set(LCDStatus::VCOUNT_FLAG, self.v_count == bytes::u16::hi(self.lcd_status.bits()));
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