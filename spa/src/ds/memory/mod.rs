mod dma;
mod shared;

use arm::{Mem32, MemCycleType};

use crate::common::{
    dma::DMA as ds7DMA,
    timers::Timers,
    wram::WRAM
};
use crate::utils::{
    meminterface::{MemInterface16, MemInterface32}
};
use super::{
    maths::Accelerators,
    ipc::IPC,
    joypad::DSJoypad,
    interrupt::InterruptControl
};
use dma::DMA;
use shared::*;

/// Memory bus for DS ARM9 processor.
pub struct DS9MemoryBus {

    //itcm:       WRAM,
    //dtcm:       WRAM,

    wram:           WRAM,
    shared_wram:    ARM9SharedRAM,

    ipc:    IPC,

    timers:             Timers,
    joypad:             DSJoypad,
    accelerators:       Accelerators,

    dma:                DMA,
    interrupt_control:  InterruptControl,
}

impl DS9MemoryBus {
    pub fn new() -> (Self, Box<DS7MemoryBus>) {
        let (arm9_ram, arm7_ram) = ARM9SharedRAM::new();
        let (ds9_ipc, ds7_ipc) = IPC::new();

        (Self{
            wram:               WRAM::new(4 * 1024 * 1024),
            shared_wram:        arm9_ram,
            ipc:                ds9_ipc,
            timers:             Timers::new(),
            joypad:             DSJoypad::new(),
            accelerators:       Accelerators::new(),
            dma:                DMA::new(),
            interrupt_control:  InterruptControl::new(),
        }, Box::new(DS7MemoryBus{
            wram:               WRAM::new(64 * 1024),
            shared_wram:        arm7_ram,
            ipc:                ds7_ipc,
            timers:             Timers::new(),
            joypad:             DSJoypad::new(),
            dma:                ds7DMA::new(),
            interrupt_control:  InterruptControl::new(),
        }))
    }
}

/*impl DS9MemoryBus {
    fn read_mem_control_abcd(&self) -> u32 {
        0 // TODO
    }
    fn read_mem_control_efg(&self) -> u32 {
        u32::from_le_bytes([
            0,
            0,
            0,
            self.shared_wram.get_bank_control()
        ])
    }
    fn read_mem_control_hi(&self) -> u32 {
        0 // TODO
    }
    fn write_mem_control_abcd(&mut self, data: u32) {
        // TODO
    }
    fn write_mem_control_efg(&mut self, data: u32) {
        let bytes = u32::to_le_bytes(data);
        self.shared_wram.set_bank_control(bytes[3]);
    }
    fn write_mem_control_hi(&mut self, data: u32) {
        // TODO
    }
}*/

impl Mem32 for DS9MemoryBus {
    type Addr = u32;

    fn clock(&mut self, cycles: usize) -> Option<arm::ExternalException> {
        /*if self.do_clock(cycles) {
            self.frame_end();
        }
        self.do_dma();

        // Check if CPU is halted.
        if self.internal.halt {
            loop {
                if self.do_clock(1) {
                    self.frame_end();
                }
                self.do_dma();
                if self.check_irq() {
                    self.internal.halt = false;
                    return Some(arm::ExternalException::IRQ);
                }
            }
        }

        if self.check_irq() {
            self.internal.halt = false;
            Some(arm::ExternalException::IRQ)
        } else {
            None
        }*/
        None
    }

    fn load_byte(&mut self, cycle: MemCycleType, addr: Self::Addr) -> (u8, usize) {
        match addr {
            0x0200_0000..=0x02FF_FFFF => (self.wram.read_byte(addr & 0x3F_FFFF), 3),    // WRAM
            0x0300_0000..=0x03FF_FFFF => (self.shared_wram.read_byte(addr), 1),         // Shared RAM

            // I/O
            0x0400_0247 => (self.shared_wram.get_bank_control(), 1),
            0x0400_0000..=0x04FF_FFFF => (self.io_read_byte(addr), 1),

            // TODO: VRAM
            //0x0500_0000..=0x07FF_FFFF => (self.video.read_byte(addr), 1),

            // TODO: GBA slot
            //0x0800_0000..=0x09FF_FFFF => (self.game_pak.read_byte(addr), self.game_pak_control.wait_cycles_0(cycle)),
            //0x0A00_0000..=0x0BFF_FFFF => (self.game_pak.read_byte(addr), self.game_pak_control.wait_cycles_1(cycle)),
            //0x0C00_0000..=0x0DFF_FFFF => (self.game_pak.read_byte(addr), self.game_pak_control.wait_cycles_2(cycle)),
            //0x0E00_0000..=0x0EFF_FFFF => (self.game_pak.read_byte(addr), self.game_pak_control.sram_wait_cycles()),

            _ => (0, 1) // Unused
        }
    }
    fn store_byte(&mut self, cycle: MemCycleType, addr: Self::Addr, data: u8) -> usize {
        match addr {
            0x0000_0000..=0x0000_3FFF => 1, // BIOS
            0x0200_0000..=0x02FF_FFFF => {  // WRAM
                self.wram.write_byte(addr & 0x3F_FFFF, data);
                3
            },
            0x0300_0000..=0x03FF_FFFF => {  // Shared RAM
                self.shared_wram.write_byte(addr, data);
                1
            },
            // I/O
            0x0400_0247 => {
                self.shared_wram.set_bank_control(data);
                1
            },
            0x0400_0000..=0x04FF_FFFF => {  // I/O
                self.io_write_byte(addr, data);
                1
            },

            // VRAM
            //0x0500_0000..=0x07FF_FFFF => {
            //    self.video.write_byte(addr, data);
            //    1
            //},

            // TODO: GBA slot
            //0x0800_0000..=0x09FF_FFFF => self.game_pak_control.wait_cycles_0(cycle),
            //0x0A00_0000..=0x0BFF_FFFF => self.game_pak_control.wait_cycles_1(cycle),
            //0x0C00_0000..=0x0CFF_FFFF => self.game_pak_control.wait_cycles_2(cycle),
            //0x0D00_0000..=0x0EFF_FFFF => {
            //    self.game_pak.write_byte(addr, data);
            //    self.game_pak_control.sram_wait_cycles()
            //},

            _ => 1 // Unused
        }
    }

    fn load_halfword(&mut self, cycle: MemCycleType, addr: Self::Addr) -> (u16, usize) {
        match addr {
            //0x0000_0000..=0x0000_3FFF => (self.bios.read_halfword(addr), 1),                // BIOS
            0x0200_0000..=0x02FF_FFFF => (self.wram.read_halfword(addr & 0x3_FFFF), 3), // WRAM
            0x0300_0000..=0x03FF_FFFF => (self.shared_wram.read_halfword(addr), 1),     // Shared WRAM

            // I/O
            // TODO: mem ctl
            0x0400_0000..=0x04FF_FFFF => (self.io_read_halfword(addr), 1),

            // VRAM
            //0x0500_0000..=0x07FF_FFFF => (self.video.read_halfword(addr), 1),

            // Cart
            //0x0800_0000..=0x09FF_FFFF => (self.game_pak.read_halfword(addr), self.game_pak_control.wait_cycles_0(cycle)),
            //0x0A00_0000..=0x0BFF_FFFF => (self.game_pak.read_halfword(addr), self.game_pak_control.wait_cycles_1(cycle)),
            //0x0C00_0000..=0x0DFF_FFFF => (self.game_pak.read_halfword(addr), self.game_pak_control.wait_cycles_2(cycle)),
            //0x0E00_0000..=0x0EFF_FFFF => (self.game_pak.read_halfword(addr), self.game_pak_control.sram_wait_cycles()),

            _ => (0, 1) // Unused
        }
    }
    fn store_halfword(&mut self, cycle: MemCycleType, addr: Self::Addr, data: u16) -> usize {
        match addr {
            0x0000_0000..=0x0000_3FFF => 1, // BIOS
            0x0200_0000..=0x02FF_FFFF => {  // WRAM
                self.wram.write_halfword(addr & 0x3_FFFF, data);
                3
            },
            0x0300_0000..=0x03FF_FFFF => {  // Shared WRAM
                self.shared_wram.write_halfword(addr, data);
                1
            },

            // I/O
            0x0400_0000..=0x04FF_FFFF => {
                self.io_write_halfword(addr, data);
                1
            },

            // VRAM
            //0x0500_0000..=0x07FF_FFFF => {
            //    self.video.write_halfword(addr, data);
            //    1
            //},

            // Cart
            //0x0800_0000..=0x09FF_FFFF => self.game_pak_control.wait_cycles_0(cycle),
            //0x0A00_0000..=0x0BFF_FFFF => self.game_pak_control.wait_cycles_1(cycle),
            //0x0C00_0000..=0x0CFF_FFFF => self.game_pak_control.wait_cycles_2(cycle),
            //0x0D00_0000..=0x0EFF_FFFF => {
            //    self.game_pak.write_halfword(addr, data);
            //    self.game_pak_control.sram_wait_cycles()
            //},

            _ => 1 // Unused
        }
    }

    fn load_word(&mut self, cycle: MemCycleType, addr: Self::Addr) -> (u32, usize) {
        match addr {
            //0x0000_0000..=0x0000_3FFF => (self.bios.read_word(addr), 1),                // BIOS
            0x0200_0000..=0x02FF_FFFF => (self.wram.read_word(addr & 0x3_FFFF), 6), // WRAM
            0x0300_0000..=0x03FF_FFFF => (self.shared_wram.read_word(addr), 1),     // Shared WRAM

            // I/O
            0x0400_0000..=0x04FF_FFFF => (self.io_read_word(addr), 1),

            // VRAM
            //0x0500_0000..=0x06FF_FFFF => (self.video.read_word(addr), 2),   // VRAM & Palette
            //0x0700_0000..=0x0700_03FF => (self.video.read_word(addr), 1),   // OAM

            // Cart
            //0x0800_0000..=0x09FF_FFFF => (self.game_pak.read_word(addr), self.game_pak_control.wait_cycles_0(cycle) << 1),
            //0x0A00_0000..=0x0BFF_FFFF => (self.game_pak.read_word(addr), self.game_pak_control.wait_cycles_1(cycle) << 1),
            //0x0C00_0000..=0x0DFF_FFFF => (self.game_pak.read_word(addr), self.game_pak_control.wait_cycles_2(cycle) << 1),
            //0x0E00_0000..=0x0EFF_FFFF => (self.game_pak.read_word(addr), self.game_pak_control.sram_wait_cycles() << 1),

            _ => (0, 1) // Unused
        }
    }
    fn store_word(&mut self, cycle: MemCycleType, addr: Self::Addr, data: u32) -> usize {
        match addr {
            0x0000_0000..=0x0000_3FFF => 1, // BIOS
            0x0200_0000..=0x02FF_FFFF => {  // WRAM
                self.wram.write_word(addr & 0x3_FFFF, data);
                6
            },
            0x0300_0000..=0x03FF_FFFF => {  // Shared WRAM
                self.shared_wram.write_word(addr, data);
                1
            },

            // I/O
            0x0400_0000..=0x04FF_FFFF => {
                self.io_write_word(addr, data);
                1
            },

            // VRAM & Palette
            //0x0500_0000..=0x06FF_FFFF => {
            //    self.video.write_word(addr, data);
            //    2
            //},
            //// OAM
            //0x0700_0000..=0x0700_03FF => {
            //    self.video.write_word(addr, data);
            //    1
            //},

            // Cart
            //0x0800_0000..=0x09FF_FFFF => self.game_pak_control.wait_cycles_0(cycle) << 1,
            //0x0A00_0000..=0x0BFF_FFFF => self.game_pak_control.wait_cycles_1(cycle) << 1,
            //0x0C00_0000..=0x0CFF_FFFF => self.game_pak_control.wait_cycles_2(cycle) << 1,
            //0x0D00_0000..=0x0EFF_FFFF => {
            //    self.game_pak.write_word(addr, data);
            //    self.game_pak_control.sram_wait_cycles() << 1
            //},

            _ => 1 // Unused
        }
    }
}

impl DS9MemoryBus {
    MemoryBusIO!{
        (0x0400_00B0, 0x0400_00EF, dma),
        (0x0400_0100, 0x0400_010F, timers),
        (0x0400_0130, 0x0400_0133, joypad),
        (0x0400_0180, 0x0400_018F, ipc),
        (0x0400_0208, 0x0400_0217, interrupt_control),
        (0x0400_0280, 0x0400_02BF, accelerators),
        (0x0410_0000, 0x0410_0003, ipc)
    }
}

/// Memory bus for DS ARM7 processor.
pub struct DS7MemoryBus {
    wram:           WRAM,
    shared_wram:    ARM7SharedRAM,

    ipc:    IPC,

    timers: Timers,
    joypad: DSJoypad,

    dma:    ds7DMA,
    interrupt_control:  InterruptControl,
}

impl Mem32 for DS7MemoryBus {
    type Addr = u32;

    fn clock(&mut self, cycles: usize) -> Option<arm::ExternalException> {
        /*if self.do_clock(cycles) {
            self.frame_end();
        }
        self.do_dma();

        // Check if CPU is halted.
        if self.internal.halt {
            loop {
                if self.do_clock(1) {
                    self.frame_end();
                }
                self.do_dma();
                if self.check_irq() {
                    self.internal.halt = false;
                    return Some(arm::ExternalException::IRQ);
                }
            }
        }

        if self.check_irq() {
            self.internal.halt = false;
            Some(arm::ExternalException::IRQ)
        } else {
            None
        }*/
        None
    }

    fn load_byte(&mut self, cycle: MemCycleType, addr: Self::Addr) -> (u8, usize) {
        match addr {
            //0x0200_0000..=0x02FF_FFFF => (self.wram.read_byte(addr & 0x3F_FFFF), 3),    // WRAM
            0x0300_0000..=0x037F_FFFF => (self.shared_wram.read_byte(addr), 1),         // Shared RAM
            0x0380_0000..=0x03FF_FFFF => (self.wram.read_byte(addr & 0xFFFF), 1),    // ARM7 WRAM
            0x0400_0000..=0x04FF_FFFF => (self.io_read_byte(addr), 1),                  // I/O

            // TODO: VRAM
            //0x0500_0000..=0x07FF_FFFF => (self.video.read_byte(addr), 1),

            // TODO: GBA slot
            //0x0800_0000..=0x09FF_FFFF => (self.game_pak.read_byte(addr), self.game_pak_control.wait_cycles_0(cycle)),
            //0x0A00_0000..=0x0BFF_FFFF => (self.game_pak.read_byte(addr), self.game_pak_control.wait_cycles_1(cycle)),
            //0x0C00_0000..=0x0DFF_FFFF => (self.game_pak.read_byte(addr), self.game_pak_control.wait_cycles_2(cycle)),
            //0x0E00_0000..=0x0EFF_FFFF => (self.game_pak.read_byte(addr), self.game_pak_control.sram_wait_cycles()),

            _ => (0, 1) // Unused
        }
    }
    fn store_byte(&mut self, cycle: MemCycleType, addr: Self::Addr, data: u8) -> usize {
        match addr {
            0x0000_0000..=0x0000_3FFF => 1, // BIOS
            //0x0200_0000..=0x02FF_FFFF => {  // WRAM
            //    self.wram.write_byte(addr & 0x3F_FFFF, data);
            //    3
            //},
            0x0300_0000..=0x037F_FFFF => {  // Shared RAM
                self.shared_wram.write_byte(addr, data);
                1
            },
            0x0380_0000..=0x03FF_FFFF => {  // ARM7 WRAM
                self.wram.write_byte(addr & 0xFFFF, data);
                1
            },
            0x0400_0000..=0x04FF_FFFF => {  // I/O
                self.io_write_byte(addr, data);
                1
            },

            // VRAM
            //0x0500_0000..=0x07FF_FFFF => {
            //    self.video.write_byte(addr, data);
            //    1
            //},

            // TODO: GBA slot
            //0x0800_0000..=0x09FF_FFFF => self.game_pak_control.wait_cycles_0(cycle),
            //0x0A00_0000..=0x0BFF_FFFF => self.game_pak_control.wait_cycles_1(cycle),
            //0x0C00_0000..=0x0CFF_FFFF => self.game_pak_control.wait_cycles_2(cycle),
            //0x0D00_0000..=0x0EFF_FFFF => {
            //    self.game_pak.write_byte(addr, data);
            //    self.game_pak_control.sram_wait_cycles()
            //},

            _ => 1 // Unused
        }
    }

    fn load_halfword(&mut self, cycle: MemCycleType, addr: Self::Addr) -> (u16, usize) {
        match addr {
            //0x0000_0000..=0x0000_3FFF => (self.bios.read_halfword(addr), 1),                // BIOS
            //0x0200_0000..=0x02FF_FFFF => (self.wram.read_halfword(addr & 0x3_FFFF), 3),     // WRAM
            0x0300_0000..=0x037F_FFFF => (self.shared_wram.read_halfword(addr), 1),         // Shared RAM
            0x0380_0000..=0x03FF_FFFF => (self.wram.read_halfword(addr & 0xFFFF), 1),    // ARM7 WRAM
            0x0400_0000..=0x04FF_FFFF => (self.io_read_halfword(addr), 1),                  // I/O

            // VRAM
            //0x0500_0000..=0x07FF_FFFF => (self.video.read_halfword(addr), 1),

            // Cart
            //0x0800_0000..=0x09FF_FFFF => (self.game_pak.read_halfword(addr), self.game_pak_control.wait_cycles_0(cycle)),
            //0x0A00_0000..=0x0BFF_FFFF => (self.game_pak.read_halfword(addr), self.game_pak_control.wait_cycles_1(cycle)),
            //0x0C00_0000..=0x0DFF_FFFF => (self.game_pak.read_halfword(addr), self.game_pak_control.wait_cycles_2(cycle)),
            //0x0E00_0000..=0x0EFF_FFFF => (self.game_pak.read_halfword(addr), self.game_pak_control.sram_wait_cycles()),

            _ => (0, 1) // Unused
        }
    }
    fn store_halfword(&mut self, cycle: MemCycleType, addr: Self::Addr, data: u16) -> usize {
        match addr {
            0x0000_0000..=0x0000_3FFF => 1, // BIOS
            //0x0200_0000..=0x02FF_FFFF => {  // WRAM
            //    self.wram.write_halfword(addr & 0x3_FFFF, data);
            //    3
            //},
            0x0300_0000..=0x037F_FFFF => {  // Shared RAM
                self.shared_wram.write_halfword(addr, data);
                1
            },
            0x0380_0000..=0x03FF_FFFF => {  // ARM7 WRAM
                self.wram.write_halfword(addr & 0xFFFF, data);
                1
            },
            0x0400_0000..=0x04FF_FFFF => {  // I/O
                self.io_write_halfword(addr, data);
                1
            },

            // VRAM
            //0x0500_0000..=0x07FF_FFFF => {
            //    self.video.write_halfword(addr, data);
            //    1
            //},

            // Cart
            //0x0800_0000..=0x09FF_FFFF => self.game_pak_control.wait_cycles_0(cycle),
            //0x0A00_0000..=0x0BFF_FFFF => self.game_pak_control.wait_cycles_1(cycle),
            //0x0C00_0000..=0x0CFF_FFFF => self.game_pak_control.wait_cycles_2(cycle),
            //0x0D00_0000..=0x0EFF_FFFF => {
            //    self.game_pak.write_halfword(addr, data);
            //    self.game_pak_control.sram_wait_cycles()
            //},

            _ => 1 // Unused
        }
    }

    fn load_word(&mut self, cycle: MemCycleType, addr: Self::Addr) -> (u32, usize) {
        match addr {
            //0x0000_0000..=0x0000_3FFF => (self.bios.read_word(addr), 1),                // BIOS
            //0x0200_0000..=0x02FF_FFFF => (self.wram.read_word(addr & 0x3_FFFF), 6),     // WRAM
            0x0300_0000..=0x037F_FFFF => (self.shared_wram.read_word(addr), 1),         // Shared RAM
            0x0380_0000..=0x03FF_FFFF => (self.wram.read_word(addr & 0xFFFF), 1),    // ARM7 WRAM
            0x0400_0000..=0x04FF_FFFF => (self.io_read_word(addr), 1),                  // I/O

            // VRAM
            //0x0500_0000..=0x06FF_FFFF => (self.video.read_word(addr), 2),   // VRAM & Palette
            //0x0700_0000..=0x0700_03FF => (self.video.read_word(addr), 1),   // OAM

            // Cart
            //0x0800_0000..=0x09FF_FFFF => (self.game_pak.read_word(addr), self.game_pak_control.wait_cycles_0(cycle) << 1),
            //0x0A00_0000..=0x0BFF_FFFF => (self.game_pak.read_word(addr), self.game_pak_control.wait_cycles_1(cycle) << 1),
            //0x0C00_0000..=0x0DFF_FFFF => (self.game_pak.read_word(addr), self.game_pak_control.wait_cycles_2(cycle) << 1),
            //0x0E00_0000..=0x0EFF_FFFF => (self.game_pak.read_word(addr), self.game_pak_control.sram_wait_cycles() << 1),

            _ => (0, 1) // Unused
        }
    }
    fn store_word(&mut self, cycle: MemCycleType, addr: Self::Addr, data: u32) -> usize {
        match addr {
            0x0000_0000..=0x0000_3FFF => 1, // BIOS
            //0x0200_0000..=0x02FF_FFFF => {  // WRAM
            //    self.wram.write_word(addr & 0x3_FFFF, data);
            //    6
            //},
            0x0300_0000..=0x037F_FFFF => {  // Shared RAM
                self.shared_wram.write_word(addr, data);
                1
            },
            0x0380_0000..=0x03FF_FFFF => {  // ARM7 WRAM
                self.wram.write_word(addr & 0xFFFF, data);
                1
            },
            0x0400_0000..=0x04FF_FFFF => {  // I/O
                self.io_write_word(addr, data);
                1
            },

            // VRAM & Palette
            //0x0500_0000..=0x06FF_FFFF => {
            //    self.video.write_word(addr, data);
            //    2
            //},
            //// OAM
            //0x0700_0000..=0x0700_03FF => {
            //    self.video.write_word(addr, data);
            //    1
            //},

            // Cart
            //0x0800_0000..=0x09FF_FFFF => self.game_pak_control.wait_cycles_0(cycle) << 1,
            //0x0A00_0000..=0x0BFF_FFFF => self.game_pak_control.wait_cycles_1(cycle) << 1,
            //0x0C00_0000..=0x0CFF_FFFF => self.game_pak_control.wait_cycles_2(cycle) << 1,
            //0x0D00_0000..=0x0EFF_FFFF => {
            //    self.game_pak.write_word(addr, data);
            //    self.game_pak_control.sram_wait_cycles() << 1
            //},

            _ => 1 // Unused
        }
    }
}

impl DS7MemoryBus {
    MemoryBusIO!{
        (0x0400_00B0, 0x0400_00DF, dma),
        (0x0400_0100, 0x0400_010F, timers),
        (0x0400_0130, 0x0400_0137, joypad),
        (0x0400_0180, 0x0400_018F, ipc),
        (0x0400_0208, 0x0400_0217, interrupt_control),
        (0x0410_0000, 0x0410_0003, ipc)
    }
}
