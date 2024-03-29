use arm::{
    Mem32, MemCycleType, ARM7TDMI, ARMDriver, ARMCore
};

use std::path::{PathBuf, Path};

use crate::utils::{
    bytes::u16,
    meminterface::MemInterface8
};
use crate::common::mem::bios::BIOS;
use super::emulated_swi;

const TEST_RAM_SIZE: u32 = 32 * 1024;

pub struct TestMem {
    ram: Vec<u8>,
    bios: BIOS,

    halted: bool,
    bios_i_flags: u16,  // 0x0300_7FF8
    real_i_flags: u16,  // 0x0400_0202

    cycle_count: usize,
}

impl TestMem {
    pub fn new(bios_path: Option<&Path>) -> Box<Self> {
        let bios = if let Some(path) = bios_path {
            BIOS::new_from_file(path).unwrap()
        } else {
            super::super::construct_bios()
        };
        Box::new(Self {
            ram: vec![0; TEST_RAM_SIZE as usize],
            bios: bios,

            halted: false,
            bios_i_flags: 3,    // VBLANK | HBLANK
            real_i_flags: 0,

            cycle_count: 0,
        })
    }
}

impl MemInterface8 for TestMem {
    fn read_byte(&mut self, addr: u32) -> u8 {
        if addr == 0x03FF_FFF8 {
            u16::lo(self.bios_i_flags)
        } else if addr == 0x03FF_FFF9 {
            u16::hi(self.bios_i_flags)
        } else if addr == 0x03FF_FFFC {
            0x08
        } else if addr == 0x03FF_FFFD {
            0x00
        } else if addr == 0x03FF_FFFE {
            0x00
        } else if addr == 0x03FF_FFFF {
            0x08
        } else if addr == 0x0400_0202 {
            u16::lo(self.real_i_flags)
        } else if addr == 0x0400_0203 {
            u16::hi(self.real_i_flags)
        } else if addr >= TEST_RAM_SIZE {
            self.ram[(addr % TEST_RAM_SIZE) as usize]
        } else {
            self.bios.read_byte(addr)
        }
    }
    fn write_byte(&mut self, addr: u32, data: u8) {
        if addr == 0x0400_0301 {
            // halt addr
            self.halted = true;
        } else if addr == 0x03FF_FFF8 {
            self.bios_i_flags = u16::set_lo(self.bios_i_flags, data);
        } else if addr == 0x03FF_FFF9 {
            self.bios_i_flags = u16::set_hi(self.bios_i_flags, data);
        } else if addr == 0x0400_0202 {
            self.real_i_flags = u16::set_lo(self.real_i_flags, data);
        } else if addr == 0x0400_0203 {
            self.real_i_flags = u16::set_hi(self.real_i_flags, data);
        } else {
            self.ram[(addr % TEST_RAM_SIZE) as usize] = data;
        }
    }
}

impl Mem32 for TestMem {
    type Addr = u32;

    fn load_byte(&mut self, _cycle: MemCycleType, addr: Self::Addr) -> (u8, usize) {
        (self.read_byte(addr), 1)
    }
    fn store_byte(&mut self, _cycle: MemCycleType, addr: Self::Addr, data: u8) -> usize {
        self.write_byte(addr, data);
        1
    }

    fn load_halfword(&mut self, _cycle: MemCycleType, addr: Self::Addr) -> (u16, usize) {
        (self.read_halfword(addr), 1)
    }
    fn store_halfword(&mut self, _cycle: MemCycleType, addr: Self::Addr, data: u16) -> usize {
        self.write_halfword(addr, data);
        1
    }

    fn load_word(&mut self, _cycle: MemCycleType, addr: Self::Addr) -> (u32, usize) {
        (self.read_word(addr), 1)
    }
    fn store_word(&mut self, _cycle: MemCycleType, addr: Self::Addr, data: u32) -> usize {
        self.write_word(addr, data);
        1
    }

    fn clock(&mut self, cycles: usize) -> Option<arm::ExternalException> {
        self.cycle_count += cycles;
        if self.halted {
            // set vblank + hblank
            self.real_i_flags = 3;
            self.halted = false;
        }
        //if self.real_i_flags != 0 {
        //    Some(arm::ExternalException::IRQ)
        //} else {
            None
        //}
    }
}

fn run_real_bios(regs: &[u32; 4], swi_call: u8) -> (usize, [u32; 3]) {
    // SETUP
    let mem = TestMem::new(Some(&PathBuf::from("../spa-bin/gba_bios.bin")));
    let mut cpu = ARM7TDMI::new(mem).build();
    cpu.do_branch(0x0800_0000);
    cpu.write_cpsr(arm::CPSR::SVC);
    cpu.write_reg(13, 0x0300_7FE0);
    cpu.write_cpsr(arm::CPSR::IRQ);
    cpu.write_reg(13, 0x0300_7FA0);
    cpu.write_cpsr(arm::CPSR::SYS);
    cpu.write_reg(13, 0x0300_7F00);
    cpu.write_cpsr(arm::CPSR::USR);

    // SWI
    let swi_instr = 0xEF00_0000 | ((swi_call as u32) << 16);
    cpu.mut_mem().store_word(MemCycleType::N, 0x0800_0000, swi_instr);
    for (reg, val) in regs.iter().enumerate() {
        cpu.write_reg(reg, *val);
    }
    // TRIGGER
    cpu.step();
    cpu.step();
    cpu.step();

    let mut cycle_count = 0;
    while cpu.read_reg(15) != 0x0800_0004 {
        //let pc = cpu.read_reg(15);
        //if cpu.read_cpsr().contains(arm::CPSR::T) {
        //    println!("pc: {:X} ({})", pc, arm::armv4::decode_thumb(cpu.mut_mem().load_halfword(MemCycleType::S, pc).0));
        //} else {
        //    println!("pc: {:X} ({})", pc, arm::armv4::decode_arm(cpu.mut_mem().load_word(MemCycleType::S, pc).0));
        //}
        cycle_count += cpu.step();
    }

    (cycle_count, [cpu.read_reg(0), cpu.read_reg(1), cpu.read_reg(3)])
}

// Write mem_set memory to `mem_write_addr`
// Returns the same size block of memory from `mem_out_addr`
fn run_real_bios_mem(regs: &[u32; 4], mem_write_addr: u32, mem_set: &[u8], mem_out_addr: u32, swi_call: u8) -> (usize, Vec<u8>) {
    // SETUP
    let mut mem = TestMem::new(Some(&PathBuf::from("../spa-bin/gba_bios.bin")));
    for (i, data) in mem_set.iter().enumerate() {
        mem.write_byte(mem_write_addr + (i as u32), *data);
    }
    let mut cpu = ARM7TDMI::new(mem).build();
    cpu.do_branch(0x0800_0000);
    cpu.write_cpsr(arm::CPSR::SVC);
    cpu.write_reg(13, 0x0300_7FE0);
    cpu.write_cpsr(arm::CPSR::IRQ);
    cpu.write_reg(13, 0x0300_7FA0);
    cpu.write_cpsr(arm::CPSR::SYS);
    cpu.write_reg(13, 0x0300_7F00);
    cpu.write_cpsr(arm::CPSR::USR);

    // SWI
    let swi_instr = 0xEF00_0000 | ((swi_call as u32) << 16);
    cpu.mut_mem().store_word(MemCycleType::N, 0x0800_0000, swi_instr);
    cpu.mut_mem().store_word(MemCycleType::N, 0x0800_0008, 0xE1A0_F00E);    // Interrupt handler: MOV R15, R14
    for (reg, val) in regs.iter().enumerate() {
        cpu.write_reg(reg, *val);
    }
    // TRIGGER
    cpu.step();
    cpu.step();
    cpu.step();

    let mut cycle_count = 0;
    while cpu.read_reg(15) != 0x0800_0004 {
        //let pc = cpu.read_reg(15);
        //if cpu.read_cpsr().contains(arm::CPSR::T) {
        //    println!("pc: {:X} ({})", pc, arm::armv4::decode_thumb(cpu.mut_mem().load_halfword(MemCycleType::S, pc).0));
        //} else {
        //    println!("pc: {:X} ({})", pc, arm::armv4::decode_arm(cpu.mut_mem().load_word(MemCycleType::S, pc).0));
        //}
        cycle_count += cpu.step();
    }

    let out_len = mem_set.len();
    let mem_ref = cpu.mut_mem();
    let out = (mem_out_addr..(mem_out_addr + (out_len as u32)))
        .map(|i| mem_ref.read_byte(i))
        .collect::<Vec<_>>();
    (cycle_count, out)
}

fn compare(regs: &[u32; 4], swi_call: u8, test_regs: usize) -> (bool, bool) {
    let (real_cycles, real_reg_outs) = run_real_bios(regs, swi_call);
    println!("Real: {:?} => {:?} | cycles: {}", regs, real_reg_outs, real_cycles);

    let mut mem = TestMem::new(None);
    let emu_reg_outs = emulated_swi(swi_call as u32, mem.as_mut(), regs);
    let emu_cycles = mem.cycle_count;
    println!("Emu : {:?} => {:?} | cycles: {}", regs, emu_reg_outs, emu_cycles);
    
    let compare_reg_outs = real_reg_outs.iter()
        .zip(&emu_reg_outs)
        .take(test_regs)
        .fold(true, |acc, (a, b)| acc && (a == b));

    (compare_reg_outs, real_cycles == emu_cycles)
}

fn compare_mem(regs: &[u32; 4], mem_write_addr: u32, mem_set: &[u8], mem_out_addr: u32, swi_call: u8) -> (bool, bool) {
    let (real_cycles, real_mem_out) = run_real_bios_mem(regs, mem_write_addr, mem_set, mem_out_addr, swi_call);
    println!("Real: {:?} => {:?} | cycles: {}", regs, real_mem_out, real_cycles);

    let mut mem = TestMem::new(None);
    for (i, data) in mem_set.iter().enumerate() {
        mem.write_byte(mem_write_addr + (i as u32), *data);
    }
    let _ = emulated_swi(swi_call as u32, mem.as_mut(), regs);
    let emu_cycles = mem.cycle_count;
    
    let out_len = mem_set.len();
    let emu_mem_out = (0..out_len)
        .map(|i| mem.read_byte(mem_out_addr + (i as u32)))
        .collect::<Vec<_>>();

    println!("Emu : {:?} => {:?} | cycles: {}", regs, emu_mem_out, emu_cycles);

    let compare_mem = real_mem_out.iter()
        .zip(&emu_mem_out)
        .fold(true, |acc, (a, b)| acc && (a == b));

    (compare_mem, real_cycles == emu_cycles)
}

#[test]
fn test_halt() {
    let data = vec![
        [0, 0, 0, 0]
    ];

    for regs in data.iter() {
        let (reg_outs, cycles) = compare(regs, 0x02, 0);
        assert_eq!(reg_outs, true);
    }
}

#[test]
fn test_stop() {
    let data = vec![
        [0, 0, 0, 0]
    ];

    for regs in data.iter() {
        let (reg_outs, cycles) = compare(regs, 0x03, 0);
        assert_eq!(reg_outs, true);
    }
}

#[test]
fn test_intrwait() {
    let data = vec![
        //[0, 1, 0, 0],
        [1, 1, 0, 0]    // effectively vblank_intrwait
    ];

    for regs in data.iter() {
        let (mem_out, cycles) = compare_mem(regs, 0, &vec![0, 0], 0x03FF_FFF8, 0x04);
        assert_eq!(mem_out, true);
    }
}

#[test]
fn test_vblank_wait() {
    let data = vec![
        [0, 0, 0, 0]
    ];

    for regs in data.iter() {
        let (mem_out, cycles) = compare_mem(regs, 0, &vec![0, 0], 0x03FF_FFF8, 0x05);
        assert_eq!(mem_out, true);
    }
}

#[test]
fn test_div() {
    /*let (real_cycles, r) = run_real_bios(&[0x50, 0x10, 0, 0], 0x06);
    println!("DIV | {}, {} => {}, {}, {} | cycles: {}", 0x50, 0x10, r[0], r[1], r[2], real_cycles);
    let mut mem = TestMem::new(None);
    let (emu_cycles, [r0, r1, r3]) = emulated_swi(0x06, mem.as_mut(), &[0x50, 0x10, 0, 0]);
    println!("DIV | {}, {} => {}, {}, {} | cycles: {}", 0x50, 0x10, r0, r1, r3, emu_cycles);*/

    let data = vec![
        [0x50, 0x10, 0, 0]
    ];

    for regs in data.iter() {
        let (reg_outs, cycles) = compare(regs, 0x06, 3);
        assert_eq!(reg_outs, true);
    }
}

#[test]
fn test_sqrt() {
    let data = vec![
        [4, 0, 0, 0],
        [2000000, 0, 0, 0]
    ];

    for regs in data.iter() {
        let (reg_outs, cycles) = compare(regs, 0x08, 1);
        assert_eq!(reg_outs, true);
    }
}

#[test]
fn test_arctan() {
    let data = vec![
        [0x4000, 0, 0, 0],  // 1.0
        [0x1234, 0, 0, 0],
        [0xC000, 0, 0, 0],  // -1.0
        [0x8CEF, 0, 0, 0],
    ];

    for regs in data.iter() {
        let (reg_outs, cycles) = compare(regs, 0x09, 1);
        assert_eq!(reg_outs, true);
    }
}

#[test]
fn test_arctan2() {
    let data = vec![
        [0, 0, 0, 0],  // 0, 0
        [0, 0xC000, 0, 0],  // 0, -1.0
        [0, 0x4000, 0, 0],  // 0, -1.0
        [0x4000, 0, 0, 0],  // 1.0, 0.0
        [0xC000, 0, 0, 0],  // 1.0, 0.0
        [0x4000, 0x4000, 0, 0],  // 1.0, 1.0
        [0x4000, 0xC000, 0, 0],  // 1.0, -1.0
        [0xC000, 0xC000, 0, 0],  // -1.0, -1.0
    ];

    for regs in data.iter() {
        let (reg_outs, cycles) = compare(regs, 0x0A, 1);
        assert_eq!(reg_outs, true);
    }
}

#[test]
fn test_cpuset_word_copy() {
    let data = vec![
        [0x0300_0100, 0x0300_0200, 0x0400_0020, 0],
        [0x0300_0100, 0x0300_0200, 0x0400_0040, 0],
        [0x0300_0100, 0x0300_0200, 0x0400_0080, 0],
    ];
    
    let mem = (0..0x100).map(|i| i as u8).collect::<Vec<_>>();

    for regs in data.iter() {
        let (mem_out, cycles) = compare_mem(regs, 0x0300_0100, &mem, 0x0300_0200, 0x0B);
        assert_eq!(mem_out, true);
        assert_eq!(cycles, true);
    }
}

#[test]
fn test_cpuset_word_set() {
    let data = vec![
        [0x0300_0100, 0x0300_0200, 0x0500_0020, 0],
        [0x0300_0100, 0x0300_0200, 0x0500_0040, 0],
    ];
    
    let mem = (1..0x101).map(|i| i as u8).collect::<Vec<_>>();

    for regs in data.iter() {
        let (mem_out, cycles) = compare_mem(regs, 0x0300_0100, &mem, 0x0300_0200, 0x0B);
        assert_eq!(mem_out, true);
        assert_eq!(cycles, true);
    }
}

#[test]
fn test_cpuset_halfword_copy() {
    let data = vec![
        [0x0300_0100, 0x0300_0200, 0x0000_0010, 0],
        [0x0300_0100, 0x0300_0200, 0x0000_0020, 0],
        [0x0300_0100, 0x0300_0200, 0x0000_0040, 0],
    ];
    
    let mem = (0..0x100).map(|i| i as u8).collect::<Vec<_>>();

    for regs in data.iter() {
        let (mem_out, cycles) = compare_mem(regs, 0x0300_0100, &mem, 0x0300_0200, 0x0B);
        assert_eq!(mem_out, true);
        assert_eq!(cycles, true);
    }
}

#[test]
fn test_cpuset_halfword_set() {
    let data = vec![
        [0x0300_0100, 0x0300_0200, 0x0100_0010, 0],
        [0x0300_0100, 0x0300_0200, 0x0100_0020, 0],
        [0x0300_0100, 0x0300_0200, 0x0100_0040, 0],
    ];
    
    let mem = (1..0x101).map(|i| i as u8).collect::<Vec<_>>();

    for regs in data.iter() {
        let (mem_out, cycles) = compare_mem(regs, 0x0300_0100, &mem, 0x0300_0200, 0x0B);
        assert_eq!(mem_out, true);
        assert_eq!(cycles, true);
    }
}

#[test]
fn test_cpufastset_copy() {
    let data = vec![
        [0x0300_0100, 0x0300_0200, 0x0000_0020, 0],
        [0x0300_0100, 0x0300_0200, 0x0000_0040, 0],
        [0x0300_0100, 0x0300_0200, 0x0000_0080, 0],
    ];
    
    let mem = (0..0x100).map(|i| i as u8).collect::<Vec<_>>();

    for regs in data.iter() {
        let (mem_out, cycles) = compare_mem(regs, 0x0300_0100, &mem, 0x0300_0200, 0x0C);
        assert_eq!(mem_out, true);
        assert_eq!(cycles, true);
    }
}

#[test]
fn test_cpufastset_set() {
    let data = vec![
        [0x0300_0100, 0x0300_0200, 0x0100_0020, 0],
        [0x0300_0100, 0x0300_0200, 0x0100_0040, 0],
        [0x0300_0100, 0x0300_0200, 0x0100_0080, 0],
    ];
    
    let mem = (1..0x101).map(|i| i as u8).collect::<Vec<_>>();

    for regs in data.iter() {
        let (mem_out, cycles) = compare_mem(regs, 0x0300_0100, &mem, 0x0300_0200, 0x0C);
        assert_eq!(mem_out, true);
        assert_eq!(cycles, true);
    }
}

#[test]
fn test_bit_unpack() {
    let data = vec![
        [0x0300_0100, 0x0300_0200, 0x0300_00F8, 0],
    ];
    
    let mut mem = vec![0x10, 0x00, 0x1, 0x4, 0, 0, 0, 0];
    mem.extend((0..0x10).map(|i| i as u8));

    for regs in data.iter() {
        let (mem_out, _cycles) = compare_mem(regs, 0x0300_00F8, &mem, 0x0300_0200, 0x10);
        assert_eq!(mem_out, true);
        //assert_eq!(cycles, true);
    }
}

#[test]
fn test_lz77_byte() {
    let data = vec![
        [0x0300_0100, 0x0300_0200, 0x0300_00F8, 0],
    ];
    
    let mut mem = vec![0x10, 0x00, 0x1, 0x4, 0, 0, 0, 0];
    mem.extend((0..0x10).map(|i| i as u8));

    for regs in data.iter() {
        let (mem_out, _cycles) = compare_mem(regs, 0x0300_00F8, &mem, 0x0300_0200, 0x10);
        assert_eq!(mem_out, true);
        //assert_eq!(cycles, true);
    }
}

#[test]
fn test_bg_affine_set() {
    let regs = [0x0300_0100, 0x0300_0120, 1, 0];
    
    let mut data = vec![
        vec![
            0, 0, 0, 0, // BG X0
            0, 0, 0, 0, // BG Y0
            0, 0,       // Scr X0
            0, 0,       // Scr Y0
            0, 2,       // Scale X
            0, 2,       // Scale Y
            0, 0,       // Angle
        ],
        vec![
            0, 0, 0, 0, // BG X0
            0, 0, 0, 0, // BG Y0
            120, 0,       // Scr X0
            80, 0,       // Scr Y0
            0, 1,       // Scale X
            0, 1,       // Scale Y
            0, 0,       // Angle
        ],
        vec![
            0, 0, 0, 0, // BG X0
            0, 0, 0, 0, // BG Y0
            120, 0,       // Scr X0
            80, 0,       // Scr Y0
            0, 1,       // Scale X
            0, 1,       // Scale Y
            0, 45,       // Angle
        ],
        vec![
            0, 100, 0, 0, // BG X0
            0, 36, 0, 0, // BG Y0
            120, 0,       // Scr X0
            80, 0,       // Scr Y0
            128, 1,       // Scale X
            64, 1,       // Scale Y
            0, 126,       // Angle
        ],
    ];

    for mem in data.iter_mut() {
        mem.extend((0..0x20).map(|i| i as u8));
        let (mem_out, _cycles) = compare_mem(&regs, 0x0300_0100, &mem, 0x0300_0100, 0x0E);
        assert_eq!(mem_out, true);
        //assert_eq!(cycles, true);
    }
}

#[test]
fn test_obj_affine_set() {
    let regs = [0x0300_0100, 0x0300_0110, 1, 2];
    
    let mut data = vec![
        vec![
            0, 2,       // Scale X
            0, 2,       // Scale Y
            0, 0,       // Angle
        ],
        vec![
            128, 1,       // Scale X
            32, 1,       // Scale Y
            0, 233,       // Angle
        ],
    ];

    for mem in data.iter_mut() {
        mem.extend((0..0x20).map(|i| i as u8));
        let (mem_out, _cycles) = compare_mem(&regs, 0x0300_0100, &mem, 0x0300_0100, 0x0F);
        assert_eq!(mem_out, true);
        //assert_eq!(cycles, true);
    }
}
