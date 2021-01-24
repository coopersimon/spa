mod common;
mod memory;
mod joypad;
mod timers;
mod interrupt;
mod constants;

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
    pub fn new() -> Self {
        let bus = MemoryBus::new();
        Self {
            cpu: ARM7TDMI::new(bus, std::collections::HashMap::new()),

            cycle_count: 0,
        }
    }

    // TODO: return some sort of framebuffer
    pub fn frame(&mut self) {
        while self.cycle_count < constants::GBA::FRAME_CYCLES {
            let step_cycles = self.cpu.step();
            let mem = self.cpu.ref_mem();
            mem.clock(step_cycles);
            let dma_cycles = mem.do_dma();
            if let Some(exception) = mem.check_exceptions() {
                self.cpu.trigger_exception(exception);
            }
            self.cycle_count += step_cycles + dma_cycles;
        }
        self.cycle_count -= constants::GBA::FRAME_CYCLES;
    }

    pub fn set_button(&mut self, button: Button, pressed: bool) {
        self.cpu.ref_mem().set_button(button.into(), pressed);
    }
}