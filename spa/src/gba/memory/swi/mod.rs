/// Emulated software interrupts.
/// 
/// These can be used in place of a BIOS ROM.

mod test;

//use super::MemoryBus;
use arm::Mem32;
use crate::gba::{
    interrupt::Interrupts
};

/// Emulated software interrupt for GBA.
/// 
/// Implements the BIOS SWI calls, clocks internally.
/// 
/// Input args are regs 0-3. Output args are regs 0, 1, 3.
pub fn emulated_swi(comment: u32, mem: &mut impl Mem32<Addr = u32>, regs: &[u32; 4]) -> [u32; 3] {
    let function = (comment as u8) | ((comment >> 16) as u8);
    match function {
        0x01 => {
            register_ram_reset(mem, regs[0]);
            [regs[0], regs[1], regs[3]]
        },
        0x04 => {
            intr_wait(mem, regs[0], regs[1]);
            [regs[0], regs[1], regs[3]]
        },
        // Maths
        0x06 => {
            divide(regs[0], regs[1])
        },
        0x07 => {
            // TODO: 3 cycles slower
            divide(regs[1], regs[0])
        },   
        0x08 => {
            let res = sqrt(regs[0]);
            [res, 0, 0]
        },
        0x09 => {
            let res = arctan(regs[0]);
            [res, 0, 0]
        },
        /*0x0A => {
            let (cycles, res) = arctan2(regs[0], regs[1]);
            (cycles, [res, 0, 0])
        },*/
        // Memset
        0x0B => {
            cpu_set(mem, regs[0], regs[1], regs[2]);
            [regs[0], regs[1], regs[3]]
        },
        0x0C => {
            cpu_fast_set(mem, regs[0], regs[1], regs[2]);
            [regs[0], regs[1], regs[3]]
        },
        _ => panic!("unsupported SWI 0x{:X}. This ROM requires the BIOS", function),
    }
}

fn register_ram_reset(mem: &mut impl Mem32<Addr = u32>, to_reset: u32) {
    println!("Register RAM reset: {:X}", to_reset);
}

fn intr_wait(mem: &mut impl Mem32<Addr = u32>, check_old_flags: u32, int_flags: u32) {
    /*let interrupts = Interrupts::from_bits_truncate(int_flags as u16);
    if check_old_flags == 0 {
        if self.interrupt_control.interrupt_req.intersects(interrupts) {
            return 0;
        }
    }

    self.interrupt_control.interrupt_master = true;*/
}

/*** MATHS ***/
fn divide(op1: u32, op2: u32) -> [u32; 3] {
    let op1_signed = op1 as i32;
    let op2_signed = op2 as i32;

    let div_res = op1_signed / op2_signed;
    let div_mod = op1_signed % op2_signed;
    let abs_res = div_res.abs();
    [div_res as u32, div_mod as u32, abs_res as u32]
}

fn sqrt(op: u32) -> u32 {
    let sqrt = (op as f64).sqrt();
    sqrt.floor() as u32
}

fn arctan(op: u32) -> u32 {
    // We interpret the input as a 16-bit signed value,
    // With fixed pt (1.1.14)
    let opf = ((op as u16) as i16) as f64;
    // Divide by 0x4000 to normalise to 1.0
    let norm_opf = opf / 16384.0;
    // Convert from radians
    let res = norm_opf.atan() / std::f64::consts::FRAC_PI_2;
    // Convert back to 16-bit fixed point
    ((res * 16384.0).floor() as i32) as u32
}

/*fn arctan2(op1: u32, op2: u32) -> u32 {
    // We interpret the inputs as 16-bit signed values,
    // With fixed pt (1.1.14)
    let x_i = (op1 as u16) as i16;
    let y_i = (op2 as u16) as i16;
    if x_i == 0 {
        if y_i > 0 {
            return 100, 0x4000;
        } else if y_i < 0 {
            return 100, 0x
        }
    }

    // Divide by 0x4000 to normalise to 1.0
    let norm_op1f = op1f / 16384.0;
    let norm_op2f = op2f / 16384.0;



    // Normalise relative to 2 Pi
    let res = norm_op1f.atan2(norm_op2f);
    println!("atan2({}, {}) = {}", norm_op1f, norm_op2f, res);
    let abs_res = if res < 0.0 {
        res + std::f64::consts::PI
    } else {
        res + std::f64::consts::PI
    } / std::f64::consts::TAU;
    println!("norm = {}", abs_res);
    // Convert to 16-bit unsigned.
    (abs_res * 65536.0).floor() as u32
}*/

/*** MEMSET ***/
fn cpu_set(mem: &mut impl Mem32<Addr = u32>, mut src_addr: u32, mut dst_addr: u32, len_mode: u32) {
    use crate::common::bits::u32;
    let mut count = len_mode & 0x1F_FFFF;
    let fixed_src = u32::test_bit(len_mode, 24);
    let use_word = u32::test_bit(len_mode, 26);

    mem.clock(96);

    if use_word {
        if fixed_src {
            let (data, read_cycles) = mem.load_word(arm::MemCycleType::N, src_addr);
            mem.clock(read_cycles);
            while count != 0 {
                let write_cycles = mem.store_word(arm::MemCycleType::N, dst_addr, data);
                dst_addr += 4;
                count -= 1;
                mem.clock(write_cycles + 7);
            }
        } else {
            while count != 0 {
                let (data, read_cycles) = mem.load_word(arm::MemCycleType::N, src_addr);
                let write_cycles = mem.store_word(arm::MemCycleType::N, dst_addr, data);
                src_addr += 4;
                dst_addr += 4;
                count -= 1;
                mem.clock(read_cycles + write_cycles + 9);
            }
        }
    } else {
        mem.clock(2);
        if fixed_src {
            let (data, read_cycles) = mem.load_halfword(arm::MemCycleType::N, src_addr);
            mem.clock(read_cycles);
            while count != 0 {
                let write_cycles = mem.store_halfword(arm::MemCycleType::N, dst_addr, data);
                dst_addr += 2;
                count -= 1;
                mem.clock(write_cycles + 7);
            }
        } else {
            while count != 0 {
                let (data, read_cycles) = mem.load_halfword(arm::MemCycleType::N, src_addr);
                let write_cycles = mem.store_halfword(arm::MemCycleType::N, dst_addr, data);
                src_addr += 2;
                dst_addr += 2;
                count -= 1;
                mem.clock(read_cycles + write_cycles + 9);
            }
        }
    }
}

fn cpu_fast_set(mem: &mut impl Mem32<Addr = u32>, mut src_addr: u32, mut dst_addr: u32, len_mode: u32) {
    use crate::common::bits::u32;
    let mut count = len_mode & 0x1F_FFF8;
    let fixed_src = u32::test_bit(len_mode, 24);

    mem.clock(95);

    if fixed_src {
        let (data, read_cycles) = mem.load_word(arm::MemCycleType::N, src_addr);
        mem.clock(read_cycles + 9);
        while count != 0 {
            // 8 words transferred at a time.
            let mut cycle_type = arm::MemCycleType::N;
            for _ in 0..8 {
                let write_cycles = mem.store_word(cycle_type, dst_addr, data);
                dst_addr += 4;
                count -= 1;
                mem.clock(write_cycles);
                cycle_type = arm::MemCycleType::S;
            }
            mem.clock(6);
        }
    } else {
        while count != 0 {
            // 8 words transferred at a time.
            let mut cycle_type = arm::MemCycleType::N;
            for _ in 0..8 {
                let (data, read_cycles) = mem.load_word(cycle_type, src_addr);
                let write_cycles = mem.store_word(cycle_type, dst_addr, data);
                src_addr += 4;
                dst_addr += 4;
                count -= 1;
                mem.clock(read_cycles + write_cycles + 1);
                cycle_type = arm::MemCycleType::S;
            }
        }
    }
}
