mod common;
mod memory;
mod joypad;
mod timers;
mod interrupt;
mod constants;
mod video;
mod audio;

use arm::{
    ARM7TDMI, ARMCore
};
use std::path::Path;
use memory::MemoryBus;
use audio::Resampler;

pub enum Button {
    A,
    B,
    Start,
    Select,
    Left,
    Right,
    Up,
    Down,
    L,
    R
}

pub struct GBA {
    cpu: ARM7TDMI<MemoryBus<crate::video::ProceduralRenderer>>,

    cycle_count: usize,
}

impl GBA {
    pub fn new(rom_path: &Path, save_path: Option<&Path>, bios_path: Option<&Path>) -> Self {
        let bus = MemoryBus::new(rom_path, save_path, bios_path).unwrap();
        Self {
            cpu: ARM7TDMI::new(bus, std::collections::HashMap::new(), None),

            cycle_count: 0,
        }
    }

    /// Drives the emulator and returns a frame.
    /// 
    /// This should be called at 60fps.
    /// The frame is in the format R8G8B8A8.
    pub fn frame(&mut self, frame: &mut [u8]) {
        while self.cycle_count < constants::gba::FRAME_CYCLES {
            let step_cycles = if !self.cpu.ref_mem().is_halted() {
                self.cpu.step()
            } else {
                1
            };
            let mem = self.cpu.ref_mem_mut();
            mem.clock(step_cycles);
            let dma_cycles = mem.do_dma();
            if mem.check_irq() {
                mem.unhalt();
                self.cpu.interrupt();
            }
            self.cycle_count += step_cycles + dma_cycles;
        }
        self.cycle_count -= constants::gba::FRAME_CYCLES;
        self.cpu.ref_mem().get_frame_data(frame);
        self.cpu.ref_mem_mut().flush_save();
    }

    pub fn render_size(&mut self) -> (usize, usize) {
        self.cpu.ref_mem().render_size()
    }

    /// Call this at the start to enable audio.
    /// It creates a GBAAudioHandler that can be sent to the audio thread.
    pub fn enable_audio(&mut self, sample_rate: f64) -> GBAAudioHandler {
        let (sample_rx, rate_rx) = self.cpu.ref_mem_mut().enable_audio();

        GBAAudioHandler {
            resampler: Resampler::new(sample_rx, rate_rx, sample_rate),
        }
    }

    pub fn set_button(&mut self, button: Button, pressed: bool) {
        self.cpu.ref_mem_mut().set_button(button.into(), pressed);
    }
}

/// Created by a GBA.
pub struct GBAAudioHandler {
    resampler:    Resampler,
}

impl GBAAudioHandler {
    /// Fill the provided buffer with samples.
    /// The format is PCM interleaved stereo.
    pub fn get_audio_packet(&mut self, buffer: &mut [f32]) {
        for (o_frame, i_frame) in buffer.chunks_exact_mut(2).zip(&mut self.resampler) {
            for (o, i) in o_frame.iter_mut().zip(i_frame.iter()) {
                *o = *i;
            }
        }
    }
}

// Debug
//#[cfg(feature = "debug")]
impl GBA {
    /// Capture the state of the internal registers.
    pub fn get_state(&mut self) -> arm::CPUState {
        use arm::Debugger;
        self.cpu.inspect_state()
    }

    /// Read a word from memory.
    pub fn get_word_at(&mut self, addr: u32) -> u32 {
        use arm::{Mem32, MemCycleType};
        let (data, _) = self.cpu.ref_mem_mut().load_word(MemCycleType::N, addr);
        data
    }

    /// Read a halfword from memory.
    pub fn get_halfword_at(&mut self, addr: u32) -> u16 {
        use arm::{Mem32, MemCycleType};
        let (data, _) = self.cpu.ref_mem_mut().load_halfword(MemCycleType::N, addr);
        data
    }

    /// Read a byte from memory.
    pub fn get_byte_at(&mut self, addr: u32) -> u8 {
        use arm::{Mem32, MemCycleType};
        let (data, _) = self.cpu.ref_mem_mut().load_byte(MemCycleType::N, addr);
        data
    }

    /// Step the device by one CPU cycle.
    pub fn step(&mut self) {
        let step_cycles = if !self.cpu.ref_mem().is_halted() {
            self.cpu.step()
        } else {
            1
        };
        let mem = self.cpu.ref_mem_mut();
        mem.clock(step_cycles);
        mem.do_dma();
        if mem.check_irq() {
            mem.unhalt();
            self.cpu.interrupt();
        }
    }
}
