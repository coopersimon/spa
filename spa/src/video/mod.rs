/// GBA video

mod memory;

use crate::common::meminterface::MemInterface16;
use crate::constants::gba::*;
use crate::interrupt::Interrupts;
use memory::VideoMemory;

/// Signal from the video unit.
pub enum Signal {
    None,
    HBlank,
    VBlank
}

/// Renderer trait. The renderer should implement this.
pub trait Renderer {
    /// Render a single line.
    fn render_line(&mut self, mem: &mut VideoMemory, line: u16);
    /// Start rendering the frame.
    fn start_frame(&mut self);
    /// Complete rendering the frame.
    fn finish_frame(&mut self);
}

/// Video rendering.
/// 
/// Consists of three parts:
/// - A unit that manages timing of drawing and blanking periods
/// - The memory (VRAM, OAM, Palette memory, and registers)
/// - The renderer (which converts memory to image)
pub struct GBAVideo {
    state:          VideoState,

    cycle_count:    usize,
    v_count:        u16,

    mem:            VideoMemory,
}

impl GBAVideo {
    pub fn new() -> Self {
        Self {
            state:          VideoState::Init,

            cycle_count:    0,
            v_count:        0,

            mem:            VideoMemory::new(),
        }
    }

    pub fn clock(&mut self, cycles: usize) -> (Signal, Interrupts) {
        use VideoState::*;
        use Transition::*;
        self.cycle_count += cycles;

        match self.state {
            Init                                                => self.transition_state(StartFrame, 0),
            Drawing if self.cycle_count >= H_DRAW_CYCLES        => self.transition_state(EndDrawing, H_DRAW_CYCLES),
            PreHBlank if self.cycle_count >= POST_H_DRAW_CYCLES => self.transition_state(EnterHBlank, POST_H_DRAW_CYCLES),
            HBlank if self.cycle_count >= H_BLANK_CYCLES => if self.v_count < V_MAX {
                self.transition_state(BeginDrawing, H_BLANK_CYCLES)
            } else {
                self.transition_state(EnterVBlank, H_BLANK_CYCLES)
            },
            VHBlank if self.cycle_count >= H_BLANK_CYCLES => if self.v_count < V_MAX2 {
                self.transition_state(ExitVHBlank, H_BLANK_CYCLES)
            } else {
                self.transition_state(StartFrame, H_BLANK_CYCLES)
            },
            VBlank if self.cycle_count >= PRE_H_BLANK_CYCLES    => self.transition_state(EnterVHBlank, PRE_H_BLANK_CYCLES),
            _                                                   => (Signal::None, Interrupts::default()),
        }
    }
}

// Note that IO (register) addresses are from zero -
// this is due to the mem bus interface.
impl MemInterface16 for GBAVideo {
    fn read_halfword(&self, addr: u32) -> u16 {
        match addr {
            0x00..=0x57 => self.mem.registers.read_halfword(addr),
            0x0500_0000..=0x0500_03FF => self.mem.palette.read_halfword(addr - 0x0500_0000),
            0x0600_0000..=0x0601_7FFF => self.mem.vram.read_halfword(addr - 0x0600_0000),
            0x0700_0000..=0x0700_03FF => self.mem.oam.read_halfword(addr - 0x0700_0000),
            _ => panic!(format!("reading invalid video address {:X}", addr))
        }
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {
        match addr {
            0x00..=0x57 => self.mem.registers.write_halfword(addr, data),
            0x0500_0000..=0x0500_03FF => self.mem.palette.write_halfword(addr - 0x0500_0000, data),
            0x0600_0000..=0x0601_7FFF => self.mem.vram.write_halfword(addr - 0x0600_0000, data),
            0x0700_0000..=0x0700_03FF => self.mem.oam.write_halfword(addr - 0x0700_0000, data),
            _ => panic!(format!("writing invalid video address {:X}", addr))
        }
    }
}

// Internal
impl GBAVideo {
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
                self.mem.registers.set_v_count(0);
                (Signal::None, self.mem.registers.v_count_irq())
            },
            BeginDrawing => {
                self.state = Drawing;
                self.v_count += 1;
                self.mem.registers.set_h_blank(false);
                self.mem.registers.set_v_count(self.v_count);
                (Signal::None, self.mem.registers.v_count_irq())
            },
            EndDrawing => {
                self.state = PreHBlank;
                // TODO: wait to finish drawing?
                // TODO: do we need this state?
                (Signal::None, Interrupts::default())
            },
            EnterHBlank => {
                self.state = HBlank;
                self.mem.registers.set_h_blank(true);
                (Signal::HBlank, self.mem.registers.h_blank_irq())
            },
            EnterVBlank => {
                self.state = VBlank;
                self.mem.registers.set_v_blank(true);
                (Signal::VBlank, self.mem.registers.v_blank_irq())
            }
            EnterVHBlank => {
                self.state = VHBlank;
                self.mem.registers.set_h_blank(true);
                (Signal::HBlank, self.mem.registers.h_blank_irq())
            },
            ExitVHBlank => {
                self.state = VBlank;
                self.v_count += 1;
                self.mem.registers.set_h_blank(false);
                self.mem.registers.set_v_count(self.v_count);
                (Signal::None, self.mem.registers.v_count_irq())
            },
        }
    }
}

enum VideoState {
    Init,       // Initial state.
    Drawing,    // Drawing a line.
    PreHBlank,  // Finished drawing, H-blank hasn't triggered yet.
    HBlank,     // Horizontal blanking period.
    VBlank,     // Vertical blanking period.
    VHBlank,    // Horizontal blanking period during v-blank.
}

enum Transition {
    StartFrame,     // Exit V-blank and start drawing a new frame
    BeginDrawing,   // Start drawing a line
    EndDrawing,     // End drawing a line (before H-blank)
    EnterHBlank,    // Enter H-blank
    EnterVBlank,    // Enter V-blank
    EnterVHBlank,   // Enter H-blank while in V-blank
    ExitVHBlank,    // Exit H-blank while in V-blank
}