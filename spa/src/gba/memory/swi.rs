/// Emulated software interrupts.
/// 
/// These can be used in place of a BIOS ROM.

use super::MemoryBus;
use crate::gba::{
    video::Renderer,
    interrupt::Interrupts
};

pub fn emulated_swi<R: Renderer>(comment: u32, mem: &mut MemoryBus<R>, r0: u32, r1: u32, r2: u32, r3: u32) -> (usize, u32, u32, u32) {
    let function = (comment as u8) | ((comment >> 16) as u8);
    match function {
        0x01 => (mem.register_ram_reset(r0), r0, r1, r3),
        0x04 => (mem.intr_wait(r0, r1), r0, r1, r3),
        0x06 => mem.divide(r0, r1),
        0x08 => {
            let (cycles, r0) = mem.sqrt(r0);
            (cycles, r0, r1, r3)
        },
        0x0B => (mem.cpu_set(r0, r1, r2), r0, r1, r3),
        0x0C => (mem.cpu_fast_set(r0, r1, r2), r0, r1, r3),
        _ => panic!("unsupported SWI 0x{:X}. This ROM requires the BIOS", function),
    }
}

impl<R: Renderer> MemoryBus<R> {
    fn register_ram_reset(&mut self, to_reset: u32) -> usize {
        println!("Register RAM reset: {:X}", to_reset);
        0
    }

    fn intr_wait(&mut self, check_old_flags: u32, int_flags: u32) -> usize {
        /*let interrupts = Interrupts::from_bits_truncate(int_flags as u16);
        if check_old_flags == 0 {
            if self.interrupt_control.interrupt_req.intersects(interrupts) {
                return 0;
            }
        }

        self.interrupt_control.interrupt_master = true;*/
        0

    }
    
    fn cpu_set(&mut self, mut src_addr: u32, mut dst_addr: u32, len_mode: u32) -> usize {
        use arm::Mem32;
        use crate::common::bits::u32;
        let mut count = len_mode & 0x1F_FFFF;
        let fixed_src = u32::test_bit(len_mode, 24);
        let use_word = u32::test_bit(len_mode, 26);

        let mut total_cycles = 0;   // TODO: base?

        if use_word {
            if fixed_src {
                let (data, read_cycles) = self.load_word(arm::MemCycleType::N, src_addr);
                total_cycles += read_cycles;
                while count != 0 {
                    let write_cycles = self.store_word(arm::MemCycleType::N, dst_addr, data);
                    dst_addr += 4;
                    count -= 1;
                    total_cycles += write_cycles;
                }
            } else {
                while count != 0 {
                    let (data, read_cycles) = self.load_word(arm::MemCycleType::N, src_addr);
                    let write_cycles = self.store_word(arm::MemCycleType::N, dst_addr, data);
                    src_addr += 4;
                    dst_addr += 4;
                    count -= 1;
                    total_cycles += read_cycles + write_cycles;
                }
            }
        } else {
            if fixed_src {
                let (data, read_cycles) = self.load_halfword(arm::MemCycleType::N, src_addr);
                total_cycles += read_cycles;
                while count != 0 {
                    let write_cycles = self.store_halfword(arm::MemCycleType::N, dst_addr, data);
                    dst_addr += 2;
                    count -= 1;
                    total_cycles += write_cycles;
                }
            } else {
                while count != 0 {
                    let (data, read_cycles) = self.load_halfword(arm::MemCycleType::N, src_addr);
                    let write_cycles = self.store_halfword(arm::MemCycleType::N, dst_addr, data);
                    src_addr += 2;
                    dst_addr += 2;
                    count -= 1;
                    total_cycles += read_cycles + write_cycles;
                }
            }
        }

        total_cycles
    }

    fn cpu_fast_set(&mut self, mut src_addr: u32, mut dst_addr: u32, len_mode: u32) -> usize {
        use arm::Mem32;
        use crate::common::bits::u32;
        let mut count = len_mode & 0x1F_FFF8;
        let fixed_src = u32::test_bit(len_mode, 24);

        let mut total_cycles = 0;   // TODO: base?

        if fixed_src {
            let (data, read_cycles) = self.load_word(arm::MemCycleType::N, src_addr);
            total_cycles += read_cycles;
            while count != 0 {
                // 8 words transferred at a time.
                let mut cycle_type = arm::MemCycleType::N;
                for _ in 0..8 {
                    let write_cycles = self.store_word(cycle_type, dst_addr, data);
                    dst_addr += 4;
                    count -= 1;
                    total_cycles += write_cycles;
                    cycle_type = arm::MemCycleType::S;
                }
            }
        } else {
            while count != 0 {
                // 8 words transferred at a time.
                let mut cycle_type = arm::MemCycleType::N;
                for _ in 0..8 {
                    let (data, read_cycles) = self.load_word(cycle_type, src_addr);
                    let write_cycles = self.store_word(cycle_type, dst_addr, data);
                    src_addr += 4;
                    dst_addr += 4;
                    count -= 1;
                    total_cycles += read_cycles + write_cycles;
                    cycle_type = arm::MemCycleType::S;
                }
            }
        }

        total_cycles
    }

    fn divide(&mut self, op1: u32, op2: u32) -> (usize, u32, u32, u32) {
        println!("Divide {:X} / {:X}", op1, op2);
        let op1_signed = op1 as i32;
        let op2_signed = op2 as i32;

        let div_res = op1_signed / op2_signed;
        let div_mod = op1_signed % op2_signed;
        let abs_res = div_res.abs();
        let cycles = 10;    // TODO
        (cycles, div_res as u32, div_mod as u32, abs_res as u32)
    }

    fn sqrt(&mut self, op: u32) -> (usize, u32) {
        println!("Sqrt {:X}", op);
        let sqrt = (op as f64).sqrt();
        let cycles = 10;    // TODO
        (cycles, sqrt.floor() as u32)
    }
}
