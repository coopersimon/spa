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
    utils::bits::u32,
    common::wram::WRAM,
};
use super::memory::DS9MemoryBus;

const DATA_TCM_SIZE: u32 = 16 * 1024;

/// ARM9 on-chip memory.
/// Includes Cache and TCM.
pub struct DS9InternalMem {

    instr_tcm:  WRAM,
    data_tcm:   WRAM,

    // TODO: cache

    mem_bus: DS9MemoryBus,

    control_reg: CP15Control,

    data_cache_bits:            u8,
    instr_cache_bits:           u8,
    cache_write_buffer_bits:    u8,

    data_access_perm_bits:      u16,
    instr_access_perm_bits:     u16,

    protection_unit_regions: [MemRegion; 8],

    data_tcm_region:    MemRegion,
    instr_tcm_region:   MemRegion,

    data_tcm_start: u32,
    data_tcm_end:   u32,
}

impl DS9InternalMem {
    pub fn new(mem_bus: DS9MemoryBus) -> Self {
        Self {
            instr_tcm:  WRAM::new(32 * 1024),
            data_tcm:   WRAM::new(DATA_TCM_SIZE as usize),

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

            data_tcm_start: 0,
            data_tcm_end:   0,
        }
    }
}

impl Mem32 for DS9InternalMem {
    type Addr = u32;

    fn clock(&mut self, cycles: usize) -> Option<ExternalException> {
        self.mem_bus.clock(cycles)
    }

    fn fetch_instr_halfword(&mut self, cycle: MemCycleType, addr: Self::Addr) -> (u16, usize) {
        match addr {
            0..=0x01FF_FFFF => (self.instr_tcm.read_halfword(addr), 1),
            _ => {
                // TODO: try instr cache
                self.mem_bus.fetch_instr_halfword(cycle, addr)
            }
        }
    }

    fn fetch_instr_word(&mut self, cycle: MemCycleType, addr: Self::Addr) -> (u32, usize) {
        match addr {
            0..=0x01FF_FFFF => (self.instr_tcm.read_word(addr), 1),
            _ => {
                // TODO: try instr cache
                self.mem_bus.fetch_instr_word(cycle, addr)
            }
        }
    }

    fn load_byte(&mut self, cycle: MemCycleType, addr: Self::Addr) -> (u8, usize) {
        match addr {
            0..=0x01FF_FFFF => (self.instr_tcm.read_byte(addr), 1),
            _ => if addr >= self.data_tcm_start && addr < self.data_tcm_end {
                (self.data_tcm.read_byte(addr - self.data_tcm_start), 1)
            } else {
                // TODO: try data cache
                self.mem_bus.load_byte(cycle, addr)
            }
        }
    }
    fn store_byte(&mut self, cycle: MemCycleType, addr: Self::Addr, data: u8) -> usize {
        match addr {
            0..=0x01FF_FFFF => {
                self.instr_tcm.write_byte(addr, data);
                1
            },
            _ => if addr >= self.data_tcm_start && addr < self.data_tcm_end {
                self.data_tcm.write_byte(addr - self.data_tcm_start, data);
                1
            } else {
                // TODO: try data cache
                self.mem_bus.store_byte(cycle, addr, data)
            }
        }
    }

    fn load_halfword(&mut self, cycle: MemCycleType, addr: Self::Addr) -> (u16, usize) {
        match addr {
            0..=0x01FF_FFFF => (self.instr_tcm.read_halfword(addr), 1),
            _ => if addr >= self.data_tcm_start && addr < self.data_tcm_end {
                (self.data_tcm.read_halfword(addr - self.data_tcm_start), 1)
            } else {
                // TODO: try data cache
                self.mem_bus.load_halfword(cycle, addr)
            }
        }
    }
    fn store_halfword(&mut self, cycle: MemCycleType, addr: Self::Addr, data: u16) -> usize {
        match addr {
            0..=0x01FF_FFFF => {
                self.instr_tcm.write_halfword(addr, data);
                1
            },
            _ => if addr >= self.data_tcm_start && addr < self.data_tcm_end {
                self.data_tcm.write_halfword(addr - self.data_tcm_start, data);
                1
            } else {
                // TODO: try data cache
                self.mem_bus.store_halfword(cycle, addr, data)
            }
        }
    }

    fn load_word(&mut self, cycle: MemCycleType, addr: Self::Addr) -> (u32, usize) {
        match addr {
            0..=0x01FF_FFFF => (self.instr_tcm.read_word(addr), 1),
            _ => if addr >= self.data_tcm_start && addr < self.data_tcm_end {
                (self.data_tcm.read_word(addr - self.data_tcm_start), 1)
            } else {
                // TODO: try data cache
                self.mem_bus.load_word(cycle, addr)
            }
        }
    }
    fn store_word(&mut self, cycle: MemCycleType, addr: Self::Addr, data: u32) -> usize {
        match addr {
            0..=0x01FF_FFFF => {
                self.instr_tcm.write_word(addr, data);
                1
            },
            _ => if addr >= self.data_tcm_start && addr < self.data_tcm_end {
                self.data_tcm.write_word(addr - self.data_tcm_start, data);
                1
            } else {
                // TODO: try data cache
                self.mem_bus.store_word(cycle, addr, data)
            }
        }
    }
}

// CP15 register functions
impl DS9InternalMem {
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
            self.data_tcm_start = (self.data_tcm_region & MemRegion::BASE_ADDR).bits();
            self.data_tcm_end = self.data_tcm_start + DATA_TCM_SIZE;
        }
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

impl ARM9Mem for DS9InternalMem {
    fn mut_cp15<'a>(&'a mut self) -> &'a mut dyn CoprocV5 {
        self
    }
}

impl CoprocV4 for DS9InternalMem {
    /// Transfer from ARM register to Coproc register.
    fn mcr(&mut self, dest_reg: usize, op_reg: usize, data: u32, _op: u32, info: u32) -> usize {
        // opcode should always be 0.
        match (dest_reg, op_reg) {
            (0, 0) => {},
            (1, 0) => self.write_control_reg(data),
            (2, 0) => self.write_cache_bits(data, info),
            (3, 0) => self.cache_write_buffer_bits = data as u8,
            (5, 0) => self.write_access_permission_bits(data, info),
            (6, _) => self.protection_unit_regions[op_reg] = MemRegion::from_bits_truncate(data),
            // 7 => cache commands
            (9, 1) => self.write_tcm_settings(data, info),
            (_, _) => {},
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

impl CoprocV5 for DS9InternalMem {
    fn mcr2(&mut self, dest_reg: usize, op_reg: usize, data: u32, op: u32, info: u32) -> usize {0}

    fn mrc2(&mut self, src_reg: usize, op_reg: usize, op: u32, info: u32) -> (u32, usize) {(0,0)}

    fn mcrr(&mut self, op_reg: usize, data_lo: u32, data_hi: u32, op: u32) -> usize {0}

    fn mrrc(&mut self, op_reg: usize, op: u32) -> (u32, u32, usize) {(0,0,0)}
    
    fn ldc2(&mut self, transfer_len: bool, dest_reg: usize, data: u32) -> usize {0}

    fn stc2(&mut self, transfer_len: bool, src_reg: usize) -> (u32, usize) {(0,0)}

    fn cdp2(&mut self, op: u32, reg_cn: usize, reg_cd: usize, info: u32, reg_cm: usize) -> usize {0}

    fn as_v4<'a>(&'a mut self) -> &'a mut dyn CoprocV4 {
        self
    }
}