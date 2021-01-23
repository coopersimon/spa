mod common;
mod memory;
mod joypad;
mod timers;
mod interrupt;

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
    cpu: ARM7TDMI<MemoryBus>
}

impl GBA {
    pub fn new() -> Self {
        let bus = MemoryBus::new();
        Self {
            cpu: ARM7TDMI::new(bus, std::collections::HashMap::new())
        }
    }

    pub fn set_button(&mut self, button: Button, pressed: bool) {
        self.cpu.ref_mem().set_button(button.into(), pressed);
    }
}