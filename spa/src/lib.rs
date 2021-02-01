mod common;
mod memory;
mod joypad;
mod timers;
mod interrupt;
mod constants;
mod video;

use arm::{
    ARM7TDMI, ARMCore
};

use memory::MemoryBus;

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
    cpu: ARM7TDMI<MemoryBus>,

    cycle_count: usize,
}

impl GBA {
    pub fn new(cart_path: &std::path::Path, bios_path: Option<&std::path::Path>) -> Self {
        let bus = MemoryBus::new(cart_path, bios_path).unwrap();
        Self {
            cpu: ARM7TDMI::new(bus, std::collections::HashMap::new()),

            cycle_count: 0,
        }
    }

    // TODO: return some sort of framebuffer
    pub fn frame(&mut self) {
        while self.cycle_count < constants::gba::FRAME_CYCLES {
            let step_cycles = self.cpu.step();
            let mem = self.cpu.ref_mem();
            mem.clock(step_cycles);
            let dma_cycles = mem.do_dma();
            if let Some(exception) = mem.check_exceptions() {
                self.cpu.trigger_exception(exception);
            }
            self.cycle_count += step_cycles + dma_cycles;
        }
        self.cycle_count -= constants::gba::FRAME_CYCLES;
    }

    pub fn set_button(&mut self, button: Button, pressed: bool) {
        self.cpu.ref_mem().set_button(button.into(), pressed);
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
        let (data, _) = self.cpu.ref_mem().load_word(MemCycleType::N, addr);
        data
    }

    /// Read a halfword from memory.
    pub fn get_halfword_at(&mut self, addr: u32) -> u16 {
        use arm::{Mem32, MemCycleType};
        let (data, _) = self.cpu.ref_mem().load_halfword(MemCycleType::N, addr);
        data
    }

    /// Read a byte from memory.
    pub fn get_byte_at(&mut self, addr: u32) -> u8 {
        use arm::{Mem32, MemCycleType};
        let (data, _) = self.cpu.ref_mem().load_byte(MemCycleType::N, addr);
        data
    }

    /// Step the device by one CPU cycle.
    pub fn step(&mut self) {
        let step_cycles = self.cpu.step();
        let mem = self.cpu.ref_mem();
        mem.clock(step_cycles);
        mem.do_dma();
        if let Some(exception) = mem.check_exceptions() {
            self.cpu.trigger_exception(exception);
        }
    }
}
