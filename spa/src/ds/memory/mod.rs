mod dma;

use crate::common::{
    dma::DMA as ds7DMA,
    timers::Timers,
    wram::WRAM
};
use super::{
    maths::Accelerators,
    ipc::IPC,
    joypad::DSJoypad,
    interrupt::InterruptControl
};
use dma::DMA;

/// Memory bus for DS ARM9 processor.
pub struct DS9MemoryBus {

    //itcm:       WRAM,
    //dtcm:       WRAM,

    wram:   WRAM,

    ipc:    IPC,

    timers:             Timers,
    joypad:             DSJoypad,
    accelerators:       Accelerators,

    dma:                DMA,
    interrupt_control:  InterruptControl,
}

impl DS9MemoryBus {
    pub fn new() -> (Self, DS7MemoryBus) {
        let (ds9_ipc, ds7_ipc) = IPC::new();

        (Self{
            wram:           WRAM::new(4 * 1024 * 1024),
            ipc:            ds9_ipc,
            timers:         Timers::new(),
            joypad:         DSJoypad::new(),
            accelerators:   Accelerators::new(),
            dma:            DMA::new(),
            interrupt_control:  InterruptControl::new(),
        }, DS7MemoryBus{
            wram:   WRAM::new(64 * 1024),
            ipc:    ds7_ipc,
            dma:    ds7DMA::new(),
        })
    }
}

/// Memory bus for DS ARM7 processor.
pub struct DS7MemoryBus {
    wram:   WRAM,

    ipc:    IPC,

    dma:    ds7DMA,
}
