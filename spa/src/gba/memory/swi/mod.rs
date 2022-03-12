/// Emulated software interrupts.
/// 
/// These can be used in place of a BIOS ROM.

mod test;

use arm::{
    Mem32,
    MemCycleType
};
use crate::{
    gba::interrupt::Interrupts,
    common::bytes::{u16, u32},
    common::bits,
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
        // Halt
        0x02 => {
            halt(mem);
            [regs[0], regs[1], regs[3]]
        },
        0x03 => {
            stop(mem);
            [regs[0], regs[1], regs[3]]
        },
        0x04 => {
            intr_wait(mem, regs[0], regs[1]);
            [regs[0], regs[1], regs[3]]
        },
        0x05 => {
            vblank_intr_wait(mem);
            [regs[0], regs[1], regs[3]]
        },
        // Maths
        0x06 => {
            mem.clock(100);
            divide(regs[0], regs[1])
        },
        0x07 => {
            mem.clock(103);
            divide(regs[1], regs[0])
        },   
        0x08 => {
            mem.clock(100);
            let res = sqrt(regs[0]);
            [res, 0, 0]
        },
        0x09 => {
            mem.clock(100);
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
        // Affine set
        0x0E => {
            bg_affine_set(mem, regs[0], regs[1], regs[2]);
            [regs[0], regs[1], regs[3]]
        },
        // Decompression
        0x10 => {
            bit_unpack(mem, regs[0], regs[1], regs[2]);
            [regs[0], regs[1], regs[3]]
        },
        0x11 => {
            lz77_uncomp_byte(mem, regs[0], regs[1]);
            [regs[0], regs[1], regs[3]]
        },
        0x12 => {
            lz77_uncomp_byte(mem, regs[0], regs[1]);
            // TODO: fix bugs here
            //lz77_uncomp_halfword(mem, regs[0], regs[1]);
            [regs[0], regs[1], regs[3]]
        },
        _ => panic!("unsupported SWI 0x{:X}. This ROM requires the BIOS", function),
    }
}

/*** RESET ***/
fn register_ram_reset(mem: &mut impl Mem32<Addr = u32>, to_reset: u32) {
    println!("Register RAM reset: {:X}", to_reset);
}

/*** HALT ***/

fn halt(mem: &mut impl Mem32<Addr = u32>) {
    let cycles = mem.store_byte(MemCycleType::N, 0x0400_0301, 0);
    mem.clock(cycles + 57);
}

fn stop(mem: &mut impl Mem32<Addr = u32>) {
    let cycles = mem.store_byte(MemCycleType::N, 0x0400_0301, 0x80);
    mem.clock(cycles + 57);
}

fn intr_wait(mem: &mut impl Mem32<Addr = u32>, check_old_flags: u32, int_flags: u32) {
    // Set master interrupt flag.
    let cycles = mem.store_halfword(MemCycleType::N, 0x0400_0208, 1);
    mem.clock(cycles);

    let interrupts = Interrupts::from_bits_truncate(int_flags as u16);

    // Clear interrupt mem region. (?)
    //mem.store_halfword(MemCycleType::N, 0x03FF_FFF8, interrupts.bits());
    
    if check_old_flags == 0 {
        // If interrupt is already requested, return immediately.
        let (i_data, _) = mem.load_halfword(MemCycleType::N, 0x0400_0202);
        let old_interrupts = Interrupts::from_bits_truncate(i_data);
        if old_interrupts.intersects(interrupts) {
            return;
        }
    }

    let mut interrupts_set = Interrupts::default();
    while !interrupts_set.intersects(interrupts) {
        // Halt
        let cycles = mem.store_byte(MemCycleType::N, 0x0400_0301, 0);
        mem.clock(cycles);
        // Check interrupts
        let (i_data, _) = mem.load_halfword(MemCycleType::N, 0x0400_0202);
        interrupts_set = Interrupts::from_bits_truncate(i_data);
    }

    // Clear bits that were set in interrupt mem region.
    // TODO: this would work better if this happened after interrupt handler has run
    let (i_data, _) = mem.load_halfword(MemCycleType::N, 0x03FF_FFF8);
    let mut interrupt_mem = Interrupts::from_bits_truncate(i_data);
    interrupt_mem.remove(interrupts_set & interrupts);
    mem.store_halfword(MemCycleType::N, 0x03FF_FFF8, interrupt_mem.bits());
}

fn vblank_intr_wait(mem: &mut impl Mem32<Addr = u32>) {
    intr_wait(mem, 1, 1);
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

/*** AFFINE SET ***/
fn bg_affine_set(mem: &mut impl Mem32<Addr = u32>, src_addr: u32, dst_addr: u32, count: u32) {
    const ANGLE_TRANSFORM: f32 = std::f32::consts::TAU / (0x10000 as f32);
    for i in 0..count {
        let src_addr = src_addr + (i * 20); // Not sure about the offset here
        let dst_addr = dst_addr + (i * 16);

        // Load input data.
        let (bg_center_x, c0) = mem.load_word(MemCycleType::N, src_addr);
        let (bg_center_y, c1) = mem.load_word(MemCycleType::N, src_addr + 4);
        let (scr_center_x, c2) = mem.load_halfword(MemCycleType::N, src_addr + 8);
        let (scr_center_y, c3) = mem.load_halfword(MemCycleType::N, src_addr + 10);
        let (scale_x, c4) = mem.load_halfword(MemCycleType::N, src_addr + 12);
        let (scale_y, c5) = mem.load_halfword(MemCycleType::N, src_addr + 14);
        let (angle, c6) = mem.load_halfword(MemCycleType::N, src_addr + 16);
        mem.clock(c0 + c1 + c2 + c3 + c4 + c5 + c6);
    
        let f_angle = (angle as f32) * ANGLE_TRANSFORM;
        let f_sin_a = f32::sin(f_angle);
        let f_cos_a = f32::cos(f_angle);
        let sin_a = (f_sin_a * 256.0) as i32;
        let cos_a = (f_cos_a * 256.0) as i32;
    
        // Calculate matrix.
        let scale_x = (scale_x as i16) as i32;
        let scale_y = (scale_y as i16) as i32;
        let a = (cos_a * scale_x) >> 8;
        let b = (sin_a * -scale_x) >> 8;
        let c = (sin_a * scale_y) >> 8;
        let d = (cos_a * scale_y) >> 8;
    
        // Calculate X0, Y0
        let screen_x = (scr_center_x as i16) as i32;
        let screen_y = (scr_center_y as i16) as i32;
        let x0 = (bg_center_x as i32) - (a * screen_x + b * screen_y);
        let y0 = (bg_center_y as i32) - (c * screen_x + d * screen_y);

        mem.clock(142);
    
        // Store output data.
        let c0 = mem.store_halfword(MemCycleType::N, dst_addr, a as u16);
        let c1 = mem.store_halfword(MemCycleType::N, dst_addr + 2, b as u16);
        let c2 = mem.store_halfword(MemCycleType::N, dst_addr + 4, c as u16);
        let c3 = mem.store_halfword(MemCycleType::N, dst_addr + 6, d as u16);
        let c4 = mem.store_word(MemCycleType::N, dst_addr + 8, x0 as u32);
        let c5 = mem.store_word(MemCycleType::N, dst_addr + 12, y0 as u32);
        mem.clock(c0 + c1 + c2 + c3 + c4 + c5);
    }
}

/*** DECOMPRESS ***/
fn bit_unpack(mem: &mut impl Mem32<Addr = u32>, src_addr: u32, mut dst_addr: u32, info_ptr: u32) {
    let (info_lo, cycles_lo) = mem.load_word(MemCycleType::N, info_ptr);
    let (info_hi, cycles_hi) = mem.load_word(MemCycleType::S, info_ptr + 4);
    mem.clock(82 + cycles_lo + cycles_hi);

    // Unpack info
    let len = u32::lo(info_lo) as u32;
    let info_mid = u32::hi(info_lo);
    let src_width = u16::lo(info_mid);
    let dst_width = u16::hi(info_mid);
    let data_offset = info_hi & 0x7FFF_FFFF;
    let zero_data = bits::u32::test_bit(info_hi, 31);

    let src_mask = (1 << src_width) - 1;

    // Do unpack
    let mut out = 0_u32;
    let mut out_bit_idx = 0;
    for i in 0..len {
        let (data, cycles) = mem.load_byte(MemCycleType::N, src_addr + i);
        mem.clock(196 + cycles);

        for offset in (0..8).step_by(src_width.into()) {
            let src_data = ((data >> offset) & src_mask) as u32;
            let dst_data = if src_data != 0 || zero_data {
                src_data + data_offset
            } else {
                src_data
            };
            
            out = out | (dst_data << out_bit_idx);
            out_bit_idx += dst_width;
            if out_bit_idx >= 32 {
                let cycles = mem.store_word(MemCycleType::N, dst_addr, out);
                mem.clock(cycles);
                dst_addr += 4;
                out = 0;
                out_bit_idx = 0;
            }
        }
    }

    if out_bit_idx > 0 {
        let cycles = mem.store_word(MemCycleType::N, dst_addr, out);
        mem.clock(cycles);
    }
}

fn lz77_uncomp_byte(mem: &mut impl Mem32<Addr = u32>, mut src_addr: u32, mut dst_addr: u32) {
    let (header, cycles) = mem.load_word(MemCycleType::N, src_addr);
    mem.clock(cycles);
    src_addr += 4;

    let len = header >> 8;
    let end = dst_addr + len;
    'outer: loop {
        // Process block.
        let (flags, cycles) = mem.load_byte(MemCycleType::N, src_addr);
        mem.clock(cycles);
        src_addr += 1;

        for i in (0..8).rev() {
            if bits::u8::test_bit(flags, i) {
                // Compressed
                let (disp_lo, load_cycles_lo) = mem.load_byte(MemCycleType::N, src_addr);
                let (disp_hi, load_cycles_hi) = mem.load_byte(MemCycleType::S, src_addr + 1);
                mem.clock(load_cycles_lo + load_cycles_hi);
                src_addr += 2;

                let displacement = u16::make(disp_lo & 0xF, disp_hi) + 1;
                let mut copy_src_addr = dst_addr - (displacement as u32);
                let copy_len = 3 + ((disp_lo >> 4) & 0xF);

                for _ in 0..copy_len {
                    let (data, load_cycles) = mem.load_byte(MemCycleType::N, copy_src_addr);
                    copy_src_addr += 1;
                    let store_cycles = mem.store_byte(MemCycleType::N, dst_addr, data);
                    dst_addr += 1;
                    mem.clock(load_cycles + store_cycles);
                }
            } else {
                // Raw data
                let (data, load_cycles) = mem.load_byte(MemCycleType::N, src_addr);
                src_addr += 1;
                let store_cycles = mem.store_byte(MemCycleType::N, dst_addr, data);
                dst_addr += 1;
                mem.clock(load_cycles + store_cycles);
            }

            if dst_addr >= end {
                break 'outer;
            }
        }
    }
}

fn lz77_uncomp_halfword(mem: &mut impl Mem32<Addr = u32>, mut src_addr: u32, mut dst_addr: u32) {
    let (header, cycles) = mem.load_word(MemCycleType::N, src_addr);
    mem.clock(cycles);
    src_addr += 4;

    let len = header >> 8;
    let end = dst_addr + len;
    let mut to_write = None;
    'outer: loop {
        // Process block.
        let (flags, cycles) = mem.load_byte(MemCycleType::N, src_addr);
        mem.clock(cycles);
        src_addr += 1;

        for i in (0..8).rev() {
            if bits::u8::test_bit(flags, i) {
                // Compressed
                let (disp_lo, load_cycles_lo) = mem.load_byte(MemCycleType::N, src_addr);
                let (disp_hi, load_cycles_hi) = mem.load_byte(MemCycleType::S, src_addr + 1);
                mem.clock(load_cycles_lo + load_cycles_hi);
                src_addr += 2;

                let displacement = u16::make(disp_lo & 0xF, disp_hi) + 1;
                let mut copy_src_addr = dst_addr - (displacement as u32);
                let copy_len = 3 + ((disp_lo >> 4) & 0xF);

                for _ in 0..copy_len {
                    let (data, load_cycles) = mem.load_byte(MemCycleType::N, copy_src_addr);
                    copy_src_addr += 1;

                    let store_cycles = if let Some(lo_byte) = to_write.take() {
                        let halfword = u16::make(data, lo_byte);
                        let store_cycles = mem.store_halfword(MemCycleType::N, dst_addr, halfword);
                        dst_addr += 2;
                        store_cycles
                    } else {
                        to_write = Some(data);
                        0
                    };

                    mem.clock(load_cycles + store_cycles);
                }
            } else {
                // Raw data
                let (data, load_cycles) = mem.load_byte(MemCycleType::N, src_addr);
                src_addr += 1;

                let store_cycles = if let Some(lo_byte) = to_write.take() {
                    let halfword = u16::make(data, lo_byte);
                    let store_cycles = mem.store_halfword(MemCycleType::N, dst_addr, halfword);
                    dst_addr += 2;
                    store_cycles
                } else {
                    to_write = Some(data);
                    0
                };
                
                mem.clock(load_cycles + store_cycles);
            }

            if dst_addr >= end {
                break 'outer;
            }
        }
    }
}
