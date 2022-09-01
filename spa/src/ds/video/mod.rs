/// NDS video

mod constants;
mod render;
mod memory;
mod video3d;

use bitflags::bitflags;
use parking_lot::Mutex;
use std::sync::{
    Arc, atomic::{AtomicU16, Ordering}
};
use crate::FrameBuffer;
use crate::utils::{
    meminterface::{MemInterface16, MemInterface32},
    bits::u16,
    bytes
};
use crate::ds::interrupt::Interrupts;
pub use render::*;
use memory::DSVideoMemory;
pub use memory::{ARM7VRAM, VRAMRegion};

use constants::*;

use self::video3d::Video3D;

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
    v_count_out:    Arc<AtomicU16>,
    lcd_status:     LCDStatus,

    pub mem:        DSVideoMemory,

    video_3d:       Video3D,

    renderer:       R,
}

impl<R: Renderer> DSVideo<R> {
    pub fn new(upper: Arc<Mutex<FrameBuffer>>, lower: Arc<Mutex<FrameBuffer>>) -> (Self, ARM7Video, ARM7VRAM) {
        let video_3d = Video3D::new();
        let (arm9_mem, arm7_vram, renderer_vram) = DSVideoMemory::new(video_3d.rendering_engine.clone());
        let renderer = R::new(upper, lower, renderer_vram);
        let v_count = Arc::new(AtomicU16::new(0));
        (Self {
            state:          VideoState::Init,
            cycle_count:    0,

            v_count:        0,
            v_count_out:    v_count.clone(),
            lcd_status:     LCDStatus::default(),

            mem:            arm9_mem,

            video_3d:       video_3d,

            renderer:       renderer,
        }, ARM7Video {
            v_count:    v_count,
            lcd_status: LCDStatus::default()
        }, arm7_vram)
    }

    /// Clock the video state machine.
    /// 
    /// This returns a signal indicating if any blanking state has been entered,
    /// interrupts that have triggered,
    /// and a boolean indicating whether the geometry FIFO is under half-full
    /// (and DMA should trigger accordingly).
    pub fn clock(&mut self, cycles: usize) -> (Signal, Interrupts, bool) {
        use VideoState::*;
        use Transition::*;
        self.cycle_count += cycles;

        let (irq_3d, geom_fifo_dma) = self.video_3d.clock(cycles);

        let (signal, irq) = match self.state {
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
        };

        (signal, irq | irq_3d, geom_fifo_dma)
    }
}

// Interface refers to engine A registers + LCD status + 3D regs
impl<R: Renderer> MemInterface16 for DSVideo<R> {
    fn read_halfword(&mut self, addr: u32) -> u16 {
        match addr {
            0x0400_0004 => self.lcd_status.bits(),
            0x0400_0006 => self.v_count as u16,
            0x0400_0060 => self.video_3d.read_halfword(addr),
            0x0400_0000..=0x0400_006F => self.mem.mut_engine_a().registers.read_halfword(addr & 0xFF),
            0x0400_0304 => self.mem.power_cnt.load(Ordering::Acquire),
            0x0400_0320..=0x0400_06FF => self.video_3d.read_halfword(addr),
            _ => panic!("reading invalid video address {:X}", addr)
        }
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {
        match addr {
            0x0400_0004 => self.set_lcd_status(data),
            0x0400_0006 => self.v_count = data,
            0x0400_0060 => self.video_3d.write_halfword(addr, data),
            0x0400_0000..=0x0400_006F => self.mem.mut_engine_a().registers.write_halfword(addr & 0xFF, data),
            0x0400_0304 => self.mem.power_cnt.store(data, Ordering::Release),
            0x0400_0320..=0x0400_06FF => self.video_3d.write_halfword(addr, data),
            _ => panic!("writing invalid video address {:X}", addr)
        }
    }

    fn read_word(&mut self, addr: u32) -> u32 {
        match addr {
            0x0400_0004 => bytes::u32::make(self.v_count, self.lcd_status.bits()),
            0x0400_0060 => self.video_3d.read_word(addr),
            0x0400_0000..=0x0400_006F => self.mem.mut_engine_a().registers.read_word(addr & 0xFF),
            0x0400_0304 => bytes::u32::make(0, self.mem.power_cnt.load(Ordering::Acquire)),
            0x0400_0320..=0x0400_06FF => self.video_3d.read_word(addr),
            _ => panic!("reading invalid video address {:X}", addr)
        }
    }

    fn write_word(&mut self, addr: u32, data: u32) {
        match addr {
            0x0400_0004 => {
                self.set_lcd_status(bytes::u32::lo(data));
                self.v_count = bytes::u32::hi(data);
            },
            0x0400_0060 => self.video_3d.write_word(addr, data),
            0x0400_0000..=0x0400_006F => self.mem.mut_engine_a().registers.write_word(addr & 0xFF, data),
            0x0400_0304 => self.mem.power_cnt.store(bytes::u32::lo(data), Ordering::Release),
            0x0400_0320..=0x0400_06FF => self.video_3d.write_word(addr, data),
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
        ((self & LCDStatus::VCOUNT).bits() >> 8) | ((self & LCDStatus::VCOUNT_HI).bits() << 1)
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
                self.v_count_out.store(0, Ordering::Release);
                self.lcd_status.remove(LCDStatus::VBLANK_FLAG | LCDStatus::HBLANK_FLAG);
                self.lcd_status.set(LCDStatus::VCOUNT_FLAG, self.v_count == self.lcd_status.v_count());
                self.renderer.start_frame();
                self.renderer.render_line(0);
                (Signal::None, self.v_count_irq())
            },
            BeginDrawing => {
                self.state = Drawing;
                self.v_count += 1;
                self.v_count_out.fetch_add(1, Ordering::AcqRel);
                self.lcd_status.remove(LCDStatus::HBLANK_FLAG);
                self.lcd_status.set(LCDStatus::VCOUNT_FLAG, self.v_count == self.lcd_status.v_count());
                self.renderer.render_line(self.v_count);
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
                self.v_count_out.fetch_add(1, Ordering::AcqRel);
                self.lcd_status.insert(LCDStatus::VBLANK_FLAG);
                self.lcd_status.set(LCDStatus::VCOUNT_FLAG, self.v_count == self.lcd_status.v_count());
                self.renderer.finish_frame();
                self.video_3d.on_vblank();
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
                self.v_count_out.fetch_add(1, Ordering::AcqRel);
                self.lcd_status.remove(LCDStatus::HBLANK_FLAG);
                self.lcd_status.set(LCDStatus::VCOUNT_FLAG, self.v_count == self.lcd_status.v_count());
                if self.v_count == 214 {
                    self.renderer.render_3d();
                }
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

/// ARM7 video display status access.
pub struct ARM7Video {
    v_count:        Arc<AtomicU16>,
    lcd_status:     LCDStatus,
}

impl ARM7Video {
    pub fn v_blank_enabled(&self) -> bool {
        self.lcd_status.contains(LCDStatus::VBLANK_IRQ)
    }

    pub fn v_count_irq(&self) -> bool {
        self.lcd_status.contains(LCDStatus::VCOUNT_IRQ) && self.lcd_status.v_count() == self.v_count.load(Ordering::Acquire)
    }
}

impl MemInterface16 for ARM7Video {
    fn read_halfword(&mut self, addr: u32) -> u16 {
        match addr {
            0x0400_0004 => self.lcd_status.bits(),
            0x0400_0006 => self.v_count.load(Ordering::Acquire),
            _ => panic!("reading invalid arm7 video address {:X}", addr)
        }
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {
        match addr {
            0x0400_0004 => self.set_lcd_status(data),
            0x0400_0006 => self.v_count.store(data, Ordering::Release),
            _ => panic!("writing invalid arm7 video address {:X}", addr)
        }
    }
}

impl ARM7Video {
    /*#[inline]
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
    }*/

    fn set_lcd_status(&mut self, data: u16) {
        let old_flags = self.lcd_status.get_flags();
        let lcd_status = LCDStatus::from_bits_truncate(data & 0xFFF8);
        self.lcd_status = lcd_status | old_flags;
    }
}
