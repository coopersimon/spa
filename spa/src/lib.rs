mod common;
mod memory;
mod timers;

use arm::ARM7TDMI;

use memory::MemoryBus;

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
}