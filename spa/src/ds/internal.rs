/// Cache and TCM for the ARM9 processor.
/// Provides a memory interface,
/// and an interface for Coprocessor 15

use arm::{
    armv4::CoprocV4,
    armv5::CoprocV5,
    Mem32, MemCycleType,
    ExternalException,
    ARM9Mem
};
use bitflags::bitflags;

use crate::{
    utils::bits::{u8, u32},
    common::wram::WRAM,
};
use super::{
    memory::DS9MemoryBus,
    video::Renderer,
    cache::*
};

const INSTR_TCM_SIZE: u32 = 32 * 1024;
const ITCM_MASK: u32 = INSTR_TCM_SIZE - 1;
const DATA_TCM_SIZE: u32 = 16 * 1024;
const DTCM_MASK: u32 = u32::MAX - (DATA_TCM_SIZE - 1);

/// ARM9 on-chip memory.
/// Includes Cache and TCM.
pub struct DS9InternalMem<R: Renderer> {

    instr_tcm:  WRAM,
    data_tcm:   WRAM,

    instr_cache:    Cache,
    data_cache:     Cache,

    instr_cache_mask:   u32,
    instr_cache_base:   u32,
    data_cache_mask:    u32,
    data_cache_base:    u32,

    mem_bus: DS9MemoryBus<R>,

    control_reg: CP15Control,

    data_cache_bits:            u8,
    instr_cache_bits:           u8,
    cache_write_buffer_bits:    u8,

    data_access_perm_bits:      u16,
    instr_access_perm_bits:     u16,

    protection_unit_regions: [MemRegion; 8],

    data_tcm_region:    MemRegion,
    instr_tcm_region:   MemRegion,

    data_tcm_base: u32,
}

impl<R: Renderer> DS9InternalMem<R> {
    pub fn new(mem_bus: DS9MemoryBus<R>) -> Self {
        Self {
            instr_tcm:  WRAM::new(INSTR_TCM_SIZE as usize),
            data_tcm:   WRAM::new(DATA_TCM_SIZE as usize),

            instr_cache:    Cache::new(64),
            data_cache:     Cache::new(32),

            instr_cache_mask:   u32::MAX,
            instr_cache_base:   0,
            data_cache_mask:    u32::MAX,
            data_cache_base:    0,
        
            mem_bus: mem_bus,

            control_reg: CP15Control::PRESET,
        
            data_cache_bits: 0,
            instr_cache_bits: 0,
            cache_write_buffer_bits: 0,
        
            data_access_perm_bits: 0,
            instr_access_perm_bits: 0,
        
            protection_unit_regions: [MemRegion::default(); 8],
        
            data_tcm_region: MemRegion::default(),
            instr_tcm_region: MemRegion::default(),

            data_tcm_base: 0,
        }
    }

    /// When booting without BIOS, CP15 needs to be setup correctly.
    pub fn setup_init(&mut self) {
        self.write_tcm_settings(0x0080_000A, 0);
        self.write_control_reg(0x0001_2078);
    }
}

impl<R: Renderer> Mem32 for DS9InternalMem<R> {
    type Addr = u32;

    fn clock(&mut self, cycles: usize) -> Option<ExternalException> {
        self.mem_bus.clock(cycles)
    }

    fn fetch_instr_halfword(&mut self, cycle: MemCycleType, addr: Self::Addr) -> (u16, usize) {
        match addr {
            0x0000_0000..=0x01FF_FFFF => (self.instr_tcm.read_halfword(addr & ITCM_MASK), 1),
            _ => {
                if self.instr_cache_mask & addr == self.instr_cache_base {
                    if let Some(data) = self.instr_cache.read_halfword(addr) {
                        (data, 1)
                    } else {
                        self.fill_i_cache_line(addr);
                        (self.instr_cache.read_halfword(addr).unwrap(), 1)
                    }
                } else {
                    self.mem_bus.fetch_instr_halfword(cycle, addr)
                }
            }
        }
    }

    fn fetch_instr_word(&mut self, cycle: MemCycleType, addr: Self::Addr) -> (u32, usize) {
        match addr {
            0x0000_0000..=0x01FF_FFFF => (self.instr_tcm.read_word(addr & ITCM_MASK), 1),
            _ => {
                if self.instr_cache_mask & addr == self.instr_cache_base {
                    if let Some(data) = self.instr_cache.read_word(addr) {
                        (data, 1)
                    } else {
                        self.fill_i_cache_line(addr);
                        (self.instr_cache.read_word(addr).unwrap(), 1)
                    }
                } else {
                    self.mem_bus.fetch_instr_word(cycle, addr)
                }
            }
        }
    }

    fn load_byte(&mut self, cycle: MemCycleType, addr: Self::Addr) -> (u8, usize) {
        match addr {
            0x0000_0000..=0x01FF_FFFF => (self.instr_tcm.read_byte(addr & ITCM_MASK), 1),
            _ => if addr & DTCM_MASK == self.data_tcm_base {
                (self.data_tcm.read_byte(addr - self.data_tcm_base), 1)
            } else if self.data_cache_mask & addr == self.data_cache_base {
                if let Some(data) = self.data_cache.read_byte(addr) {
                    (data, 1)
                } else {
                    self.fill_d_cache_line(addr);
                    (self.data_cache.read_byte(addr).unwrap(), 1)
                }
            } else {
                self.mem_bus.load_byte(cycle, addr)
            }
        }
    }
    fn store_byte(&mut self, cycle: MemCycleType, addr: Self::Addr, data: u8) -> usize {
        match addr {
            0x0000_0000..=0x01FF_FFFF => {
                self.instr_tcm.write_byte(addr & ITCM_MASK, data);
                1
            },
            _ => if addr & DTCM_MASK == self.data_tcm_base {
                self.data_tcm.write_byte(addr - self.data_tcm_base, data);
                1
            } else if self.data_cache_mask & addr == self.data_cache_base {
                if self.data_cache.write_byte(addr, data) {
                    1
                } else {
                    self.fill_d_cache_line(addr);
                    self.data_cache.write_byte(addr, data);
                    1
                }
            } else {
                self.mem_bus.store_byte(cycle, addr, data)
            }
        }
    }

    fn load_halfword(&mut self, cycle: MemCycleType, addr: Self::Addr) -> (u16, usize) {
        match addr {
            0x0000_0000..=0x01FF_FFFF => (self.instr_tcm.read_halfword(addr & ITCM_MASK), 1),
            _ => if addr & DTCM_MASK == self.data_tcm_base {
                (self.data_tcm.read_halfword(addr - self.data_tcm_base), 1)
            } else if self.data_cache_mask & addr == self.data_cache_base {
                if let Some(data) = self.data_cache.read_halfword(addr) {
                    (data, 1)
                } else {
                    self.fill_d_cache_line(addr);
                    (self.data_cache.read_halfword(addr).unwrap(), 1)
                }
            } else {
                self.mem_bus.load_halfword(cycle, addr)
            }
        }
    }
    fn store_halfword(&mut self, cycle: MemCycleType, addr: Self::Addr, data: u16) -> usize {
        match addr {
            0x0000_0000..=0x01FF_FFFF => {
                self.instr_tcm.write_halfword(addr & ITCM_MASK, data);
                1
            },
            _ => if addr & DTCM_MASK == self.data_tcm_base {
                self.data_tcm.write_halfword(addr - self.data_tcm_base, data);
                1
            } else if self.data_cache_mask & addr == self.data_cache_base {
                if self.data_cache.write_halfword(addr, data) {
                    1
                } else {
                    self.fill_d_cache_line(addr);
                    self.data_cache.write_halfword(addr, data);
                    1
                }
            } else {
                self.mem_bus.store_halfword(cycle, addr, data)
            }
        }
    }

    fn load_word(&mut self, cycle: MemCycleType, addr: Self::Addr) -> (u32, usize) {
        match addr {
            0x0000_0000..=0x01FF_FFFF => (self.instr_tcm.read_word(addr & ITCM_MASK), 1),
            _ => if addr & DTCM_MASK == self.data_tcm_base {
                (self.data_tcm.read_word(addr - self.data_tcm_base), 1)
            } else if self.data_cache_mask & addr == self.data_cache_base {
                if let Some(data) = self.data_cache.read_word(addr) {
                    (data, 1)
                } else {
                    self.fill_d_cache_line(addr);
                    (self.data_cache.read_word(addr).unwrap(), 1)
                }
            } else {
                self.mem_bus.load_word(cycle, addr)
            }
        }
    }
    fn store_word(&mut self, cycle: MemCycleType, addr: Self::Addr, data: u32) -> usize {
        match addr {
            0x0000_0000..=0x01FF_FFFF => {
                self.instr_tcm.write_word(addr & ITCM_MASK, data);
                1
            },
            _ => if addr & DTCM_MASK == self.data_tcm_base {
                self.data_tcm.write_word(addr - self.data_tcm_base, data);
                1
            } else if self.data_cache_mask & addr == self.data_cache_base {
                if self.data_cache.write_word(addr, data) {
                    1
                } else {
                    self.fill_d_cache_line(addr);
                    self.data_cache.write_word(addr, data);
                    1
                }
            } else {
                self.mem_bus.store_word(cycle, addr, data)
            }
        }
    }
}

// CP15 register functions
impl<R: Renderer> DS9InternalMem<R> {
    fn read_id_code(&self, info: u32) -> u32 {
        /// 41=ARM
        /// 05=ARMv5TE
        /// 946=ARM946
        /// 1=Post-ARM7
        const MAIN_ID_REG: u32 = 0x41_05_946_1;
        /// F=Write-back,Reg7Clean,Lock-downB + separate caches
        /// 0D2=(data) 16KB 4-way 32B lines
        /// 112=(instr) 32KB 4-way 32B lines
        const CACHE_TYPE: u32 = 0x0_F_0D2_112;
        /// 14=(data) 16KB
        /// 18=(instr) 32KB
        const TCM_SIZE: u32 = 0x00_14_0_18_0;
        
        match info {
            1 => CACHE_TYPE,
            2 => TCM_SIZE,
            _ => MAIN_ID_REG
        }
    }

    fn read_control_reg(&self) -> u32 {
        self.control_reg.bits()
    }
    fn write_control_reg(&mut self, data: u32) {
        self.control_reg = CP15Control::from_bits_truncate(data) | CP15Control::PRESET;
        if self.control_reg.contains(CP15Control::ENDIANNESS) {
            panic!("Big endian mode unsupported.");
        }
        if !self.control_reg.contains(CP15Control::HI_VECTORS) {
            panic!("Low interrupt vectors unsupported.");
        }
        if self.control_reg.contains(CP15Control::PRE_ARM5_RET) {
            panic!("ARMv4 return mode unsupported.");
        }
        // TODO: cache enable?
        self.set_cache_masks();
    }

    fn read_cache_bits(&self, info: u32) -> u32 {
        if info == 1 {
            self.instr_cache_bits as u32
        } else {
            self.data_cache_bits as u32
        }
    }
    fn write_cache_bits(&mut self, data: u32, info: u32) {
        if info == 1 {
            self.instr_cache_bits = data as u8
        } else {
            self.data_cache_bits = data as u8
        }
        self.set_cache_masks();
    }

    fn read_access_permission_bits(&self, info: u32) -> u32 {
        if info == 1 {
            self.instr_access_perm_bits as u32
        } else {
            self.data_access_perm_bits as u32
        }
    }
    fn write_access_permission_bits(&mut self, data: u32, info: u32) {
        if info == 1 {
            self.instr_access_perm_bits = data as u16
        } else {
            self.data_access_perm_bits = data as u16
        }
    }

    fn read_tcm_settings(&self, info: u32) -> u32 {
        if info == 1 {
            self.instr_tcm_region.bits()
        } else {
            self.data_tcm_region.bits()
        }
    }
    fn write_tcm_settings(&mut self, data: u32, info: u32) {
        if info == 1 {
            self.instr_tcm_region = MemRegion::from_bits_truncate(data);
        } else {
            self.data_tcm_region = MemRegion::from_bits_truncate(data);
            self.data_tcm_base = (self.data_tcm_region & MemRegion::BASE_ADDR).bits();
            //println!("SET data TCM: {:X} => {:X}", self.data_tcm_base, self.data_tcm_base + DATA_TCM_SIZE);
        }
    }

    /// Cache and wait commands. CP15 register 7.
    fn cache_command(&mut self, op_reg: usize, data: u32, info: u32) -> usize {
        match (op_reg, info) {
            (0, 4) => self.wait_for_interrupt(),
            (5, 0) => self.instr_cache.invalidate_all(),
            (5, 1) => self.instr_cache.invalidate_line(data),
            (5, 2) => panic!("inv i S/I"),
            (5, 4) => panic!("prefetch"),
            (6, 0) => self.data_cache.invalidate_all(),
            (6, 1) => self.data_cache.invalidate_line(data),
            (6, 2) => panic!("inv d S/I"),
            (7, _) => panic!("unified"),
            (8, 2) => self.wait_for_interrupt(),
            (10, 1) => return self.clean_line(data),
            (10, 2) => return self.clean_set_line(SetLine::from_bits_truncate(data)),
            (10, 4) => {},  // TODO: Drain write buffer
            (13, 1) => return self.prefetch_icache_line(data),
            (14, 1) => return self.clean_and_invalidate_line(data),
            (14, 2) => return self.clean_and_invalidate_set_line(SetLine::from_bits_truncate(data)),

            _ => panic!("unknown cache command {} | {}", op_reg, info)
        }

        0
    }

    fn wait_for_interrupt(&mut self) {
        self.mem_bus.halt = true;
    }
}

// Cache commands.
impl<R: Renderer> DS9InternalMem<R> {
    /// Write data back to memory if necessary.
    /// 
    /// Argument specifies the address of the data.
    fn clean_line(&mut self, addr: u32) -> usize {
        let mut cache_line = [0_u32; 8];
        let mut cycles = 0;
        //println!("Try clean {:X}", addr);
        if self.data_cache.clean_line(addr, &mut cache_line) {
            //println!("Clean {:X}", addr);
            cycles += self.mem_bus.store_word(MemCycleType::N, addr, cache_line[0]);
            for (data, offset) in cache_line[1..8].iter().zip((4..32).step_by(4)) {
                cycles += self.mem_bus.store_word(MemCycleType::S, addr + offset, *data);
            }
        }
        cycles
    }

    /// Write data back to memory if necessary.
    /// 
    /// Argument specifies the exact set and line to replace.
    fn clean_set_line(&mut self, set_line: SetLine) -> usize {
        let mut cache_line = [0_u32; 8];
        let mut cycles = 0;
        //println!("Try clean bits {:X}", set_line.bits());
        if let Some(tag) = self.data_cache.clean_set_line(set_line.set_idx(), set_line.data_index(), &mut cache_line) {
            //println!("Clean {:X} | {:X}", tag, set_line.data_offset());
            let addr = tag + set_line.data_offset();
            cycles += self.mem_bus.store_word(MemCycleType::N, addr, cache_line[0]);
            for (data, offset) in cache_line[1..8].iter().zip((4..32).step_by(4)) {
                cycles += self.mem_bus.store_word(MemCycleType::S, addr + offset, *data);
            }
        }
        cycles
    }

    /// Write data back to memory if necessary, and mark the line as invalid.
    /// 
    /// Argument specifies the address of the data.
    fn clean_and_invalidate_line(&mut self, addr: u32) -> usize {
        let mut cache_line = [0_u32; 8];
        let mut cycles = 0;
        if self.data_cache.clean_and_invalidate_line(addr, &mut cache_line) {
            cycles += self.mem_bus.store_word(MemCycleType::N, addr, cache_line[0]);
            for (data, offset) in cache_line[1..8].iter().zip((4..32).step_by(4)) {
                cycles += self.mem_bus.store_word(MemCycleType::S, addr + offset, *data);
            }
        }
        cycles
    }

    /// Write data back to memory if necessary, and mark the line as invalid.
    /// 
    /// Argument specifies the exact set and line to replace.
    fn clean_and_invalidate_set_line(&mut self, set_line: SetLine) -> usize {
        let mut cache_line = [0_u32; 8];
        let mut cycles = 0;
        if let Some(tag) = self.data_cache.clean_and_invalidate_set_line(set_line.set_idx(), set_line.data_index(), &mut cache_line) {
            let addr = tag + set_line.data_offset();
            cycles += self.mem_bus.store_word(MemCycleType::N, addr, cache_line[0]);
            for (data, offset) in cache_line[1..8].iter().zip((4..32).step_by(4)) {
                cycles += self.mem_bus.store_word(MemCycleType::S, addr + offset, *data);
            }
        }
        cycles
    }

    fn set_cache_masks(&mut self) {
        self.data_cache_mask = u32::MAX;
        self.data_cache_base = 0;
        if self.control_reg.contains(CP15Control::DATA_CACHE) {
            for region in 0..8 {
                let base = self.protection_unit_regions[region].base();
                if self.protection_unit_regions[region].enabled() &&
                    u8::test_bit(self.data_cache_bits, region) &&
                    (base & 0xFF00_0000 == 0x0200_0000)
                {
                    // TODO: temp. disabled data cache due to some bugs.
                    //self.data_cache_base = base;
                    //self.data_cache_mask = u32::MAX - (self.protection_unit_regions[region].size() - 1);
                    //println!("DCACHE: base: {:X} | mask: {:X}", base, self.data_cache_mask);
                    break;
                }
            }
        }

        self.instr_cache_mask = u32::MAX;
        self.instr_cache_base = 0;
        if self.control_reg.contains(CP15Control::I_CACHE) {
            for region in 0..8 {
                let base = self.protection_unit_regions[region].base();
                if self.protection_unit_regions[region].enabled() &&
                    u8::test_bit(self.instr_cache_bits, region) &&
                    (base & 0xFF00_0000 == 0x0200_0000)
                {
                    self.instr_cache_base = base;
                    self.instr_cache_mask = u32::MAX - (self.protection_unit_regions[region].size() - 1);
                    //println!("ICACHE: base: {:X} | mask: {:X}", base, self.instr_cache_mask);
                    break;
                }
            }
        }
    }

    fn prefetch_icache_line(&mut self, addr: u32) -> usize {
        let mut buffer = [0_u32; 8];

        let (data, mut cycles) = self.mem_bus.load_word(MemCycleType::N, addr);
        buffer[0] = data;

        for (buf, offset) in buffer[1..8].iter_mut().zip((4..32).step_by(4)) {
            let (data, load_cycles) = self.mem_bus.load_word(MemCycleType::S, addr + offset);
            *buf = data;
            cycles += load_cycles;
        }

        self.instr_cache.fill_line(addr, &buffer);
        cycles
    }

    fn fill_i_cache_line(&mut self, addr: u32) -> usize {
        let base = addr & 0xFFFF_FFE0;
        let mut buffer = [0_u32; 8];

        let (data, mut cycles) = self.mem_bus.load_word(MemCycleType::N, base);
        buffer[0] = data;

        for (buf, offset) in buffer[1..8].iter_mut().zip((4..32).step_by(4)) {
            let (data, load_cycles) = self.mem_bus.load_word(MemCycleType::S, base + offset);
            *buf = data;
            cycles += load_cycles;
        }

        self.instr_cache.fill_line(base, &buffer);
        cycles
    }

    fn fill_d_cache_line(&mut self, addr: u32) -> usize {
        let base = addr & 0xFFFF_FFE0;

        let mut in_buffer = [0_u32; 8];
        let (data, mut cycles) = self.mem_bus.load_word(MemCycleType::N, base);
        in_buffer[0] = data;

        for (buf, offset) in in_buffer[1..8].iter_mut().zip((4..32).step_by(4)) {
            let (data, load_cycles) = self.mem_bus.load_word(MemCycleType::S, base + offset);
            *buf = data;
            cycles += load_cycles;
        }

        let mut out_buffer = [0_u32; 8];
        if let Some(addr) = self.data_cache.clean_and_fill_line(base, &in_buffer, &mut out_buffer) {
            cycles += self.mem_bus.store_word(MemCycleType::N, addr, out_buffer[0]);
            for (data, offset) in out_buffer[1..8].iter().zip((4..32).step_by(4)) {
                cycles += self.mem_bus.store_word(MemCycleType::S, addr + offset, *data);
            }
        }
        cycles
    }
}

bitflags!{
    #[derive(Default)]
    struct CP15Control: u32 {
        const ITCM_LOAD_MODE    = u32::bit(19);
        const ITCM_ENABLE       = u32::bit(18);
        const DTCM_LOAD_MODE    = u32::bit(17);
        const DTCM_ENABLE       = u32::bit(16);
        const PRE_ARM5_RET      = u32::bit(15);
        const CACHE_REPLACE     = u32::bit(14);
        const HI_VECTORS        = u32::bit(13);
        const I_CACHE           = u32::bit(12);
        const ENDIANNESS        = u32::bit(7);
        const NEW_ABORT         = u32::bit(6);
        const OLD_ADDR_FAULT    = u32::bit(5);
        const EXCEPTION_HANDLE  = u32::bit(4);
        const WRITE_BUFFER      = u32::bit(3);
        const DATA_CACHE        = u32::bit(2);
        const ALIGN_FAULT_CHECK = u32::bit(1);
        const PU_ENABLE         = u32::bit(0);

        const PRESET = u32::bits(3, 6);
    }
}

bitflags!{
    #[derive(Default)]
    struct MemRegion: u32 {
        const BASE_ADDR = u32::bits(12, 31);
        const SIZE      = u32::bits(1, 5);
        const ENABLE    = u32::bit(0);
    }
}

impl MemRegion {
    fn enabled(&self) -> bool {
        self.contains(MemRegion::ENABLE)
    }

    fn size(&self) -> u32 {
        let n = (*self & MemRegion::SIZE).bits() >> 1;
        1 << n
    }

    fn base(&self) -> u32 {
        (*self & MemRegion::BASE_ADDR).bits()
    }
}

impl<R: Renderer> ARM9Mem for DS9InternalMem<R> {
    fn mut_cp15<'a>(&'a mut self) -> &'a mut dyn CoprocV5 {
        self
    }
}

impl<R: Renderer> CoprocV4 for DS9InternalMem<R> {
    /// Transfer from ARM register to Coproc register.
    fn mcr(&mut self, dest_reg: usize, op_reg: usize, data: u32, _op: u32, info: u32) -> usize {
        //println!("write: {:X} => ({}, {}) | info: {}", data, dest_reg, op_reg, info);
        // opcode should always be 0.
        match (dest_reg, op_reg) {
            (0, 0) => {},
            (1, 0) => self.write_control_reg(data),
            (2, 0) => self.write_cache_bits(data, info),
            (3, 0) => {
                self.cache_write_buffer_bits = data as u8;
                // TODO...
                //println!("cache WB: {:X}", data);
            },
            (5, 0) => self.write_access_permission_bits(data, info),
            (6, _) => {
                self.protection_unit_regions[op_reg] = MemRegion::from_bits_truncate(data);
                self.set_cache_masks();
                //println!("region {}: {:X}", op_reg, data);
            },
            (7, _) => {
                let cycles = self.cache_command(op_reg, data, info);
                return cycles;
            },
            (9, 1) => self.write_tcm_settings(data, info),
            (_, _) => panic!("unknown mcr"),
        };

        0
    }

    /// Transfer from Coproc register to ARM register.
    fn mrc(&mut self, src_reg: usize, op_reg: usize, _op: u32, info: u32) -> (u32, usize) {
        // opcode should always be 0.
        let ret = match (src_reg, op_reg) {
            (0, 0) => self.read_id_code(info),
            (1, 0) => self.read_control_reg(),
            (2, 0) => self.read_cache_bits(info),
            (3, 0) => self.cache_write_buffer_bits as u32,
            (5, 0) => self.read_access_permission_bits(info),
            (6, _) => self.protection_unit_regions[op_reg].bits(),
            (9, 1) => self.read_tcm_settings(info),
            // 13 => Process ID (not in NDS)
            // 15 => BIST
            (_, _) => 0,
        };

        (ret, 0)
    }

    /// Transfer from memory to Coproc register.
    fn ldc(&mut self, _transfer_len: bool, _dest_reg: usize, _data: u32) -> usize {0}

    /// Transfer from Coproc register to memory.
    fn stc(&mut self, _transfer_len: bool, _src_reg: usize) -> (u32, usize) {(0, 0)}

    /// Coprocessor data operation.
    fn cdp(&mut self, _op: u32, _reg_cn: usize, _reg_cd: usize, _info: u32, _reg_cm: usize) -> usize {0}
}

impl<R: Renderer> CoprocV5 for DS9InternalMem<R> {
    fn mcr2(&mut self, _dest_reg: usize, _op_reg: usize, _data: u32, _op: u32, _info: u32) -> usize {0}

    fn mrc2(&mut self, _src_reg: usize, _op_reg: usize, _op: u32, _info: u32) -> (u32, usize) {(0,0)}

    fn mcrr(&mut self, _op_reg: usize, _data_lo: u32, _data_hi: u32, _op: u32) -> usize {0}

    fn mrrc(&mut self, _op_reg: usize, _op: u32) -> (u32, u32, usize) {(0,0,0)}
    
    fn ldc2(&mut self, _transfer_len: bool, _dest_reg: usize, _data: u32) -> usize {0}

    fn stc2(&mut self, _transfer_len: bool, _src_reg: usize) -> (u32, usize) {(0,0)}

    fn cdp2(&mut self, _op: u32, _reg_cn: usize, _reg_cd: usize, _info: u32, _reg_cm: usize) -> usize {0}

    fn as_v4<'a>(&'a mut self) -> &'a mut dyn CoprocV4 {
        self
    }
}