/// NDS video

mod constants;
mod render;

use crate::utils::meminterface::MemInterface16;
use crate::ds::interrupt::Interrupts;
pub use render::*;

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

    mem:            VideoMemory,

    renderer:       R,
}

impl<R: Renderer> DSVideo<R> {
    pub fn new(renderer: R) -> Self {
        Self {
            state:          VideoState::Init,

            cycle_count:    0,
            v_count:        0,

            mem:            VideoMemory::new(),

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

// Note that IO (register) addresses are from zero -
// this is due to the mem bus interface.
impl<R: Renderer> MemInterface16 for DSVideo<R> {
    fn read_halfword(&mut self, addr: u32) -> u16 {
        match addr {
            0x00..=0x57 => self.mem.registers.read_halfword(addr),
            0x0500_0000..=0x05FF_FFFF => self.mem.palette.read_halfword(addr & 0x7FF),
            0x0600_0000..=0x06FF_FFFF => self.mem.vram.read_halfword(addr & 0x1_FFFF),
            0x0700_0000..=0x07FF_FFFF => self.mem.oam.read_halfword(addr & 0x7FF),
            _ => panic!("reading invalid video address {:X}", addr)
        }
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {
        match addr {
            0x00..=0x57 => self.mem.registers.write_halfword(addr, data),
            0x0500_0000..=0x05FF_FFFF => self.mem.palette.write_halfword(addr & 0x7FF, data),
            0x0600_0000..=0x06FF_FFFF => self.mem.vram.write_halfword(addr & 0x1_FFFF, data),
            0x0700_0000..=0x07FF_FFFF => self.mem.oam.write_halfword(addr & 0x7FF, data),
            _ => panic!("writing invalid video address {:X}", addr)
        }
    }

    // TODO: 32-bit interfaec
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
                self.mem.registers.set_h_blank(false);
                self.mem.registers.set_v_blank(false);
                self.mem.registers.reset_v_count();
                self.renderer.start_frame();
                self.renderer.render_line(&mut self.mem, 0);
                (Signal::None, self.mem.registers.v_count_irq())
            },
            BeginDrawing => {
                self.state = Drawing;
                self.v_count += 1;
                self.mem.registers.set_h_blank(false);
                self.mem.registers.inc_v_count();
                self.renderer.render_line(&mut self.mem, self.v_count);
                (Signal::None, self.mem.registers.v_count_irq())
            },
            EnterHBlank => {
                self.state = HBlank;
                self.mem.registers.set_h_blank(true);
                (Signal::HBlank, self.mem.registers.h_blank_irq())
            },
            EnterVBlank => {
                self.state = VBlank;
                self.v_count += 1;
                self.mem.registers.set_v_blank(true);
                self.mem.registers.inc_v_count();
                self.renderer.finish_frame();
                (Signal::VBlank, self.mem.registers.v_blank_irq())
            }
            EnterVHBlank => {
                self.state = VHBlank;
                self.mem.registers.set_h_blank(true);
                (Signal::None, Interrupts::default())
            },
            ExitVHBlank => {
                self.state = VBlank;
                self.v_count += 1;
                self.mem.registers.set_h_blank(false);
                self.mem.registers.inc_v_count();
                (Signal::None, self.mem.registers.v_count_irq())
            },
        }
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