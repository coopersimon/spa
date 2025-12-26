use crate::u64;

/// Decrypt key-1 encoded block.
pub fn decrypt(block: u64, key_buf: &[u32]) -> u64 {
    const OFFSETS: [usize; 16] = [0x11, 0x10, 0xF, 0xE, 0xD, 0xC, 0xB, 0xA, 0x9, 0x8, 0x7, 0x6, 0x5, 0x4, 0x3, 0x2];
    let mut y = u64::lo(block);
    let mut x = u64::hi(block);
    for i in OFFSETS {
        let z = key_buf[i] ^ x;
        let z_idx = z as usize;
        x = key_buf[0x12 + ((z_idx >> 24) & 0xFF)];
        x = key_buf[0x112 + ((z_idx >> 16) & 0xFF)].wrapping_add(x);
        x = key_buf[0x212 + ((z_idx >> 8) & 0xFF)] ^ x;
        x = key_buf[0x312 + (z_idx & 0xFF)].wrapping_add(x);
        x = y ^ x;
        y = z;
    }
    x = x ^ key_buf[1];
    y = y ^ key_buf[0];
    u64::make(y, x)
}

/// Encrypt block with key-1.
pub fn encrypt(block: u64, key_buf: &[u32]) -> u64 {
    let mut y = u64::lo(block);
    let mut x = u64::hi(block);
    for i in 0..0x10 {
        let z = key_buf[i] ^ x;
        let z_idx = z as usize;
        x = key_buf[0x12 + ((z_idx >> 24) & 0xFF)];
        x = key_buf[0x112 + ((z_idx >> 16) & 0xFF)].wrapping_add(x);
        x = key_buf[0x212 + ((z_idx >> 8) & 0xFF)] ^ x;
        x = key_buf[0x312 + (z_idx & 0xFF)].wrapping_add(x);
        x = y ^ x;
        y = z;
    }
    x = x ^ key_buf[0x10];
    y = y ^ key_buf[0x11];
    u64::make(y, x)
}

/*fn key_1_inner(key_buf: &[u32], i: usize, x: u32, y: u32) -> (u32, u32) {
    let z = key_buf[i] ^ x;
    let z_idx = z as usize;
    let mut x = key_buf[0x12 + ((z_idx >> 24) & 0xFF)];
    x = key_buf[0x112 + ((z_idx >> 16) & 0xFF)].wrapping_add(x);
    x = key_buf[0x212 + ((z_idx >> 8) & 0xFF)] ^ x;
    x = key_buf[0x312 + (z_idx & 0xFF)].wrapping_add(x);
    (y ^ x, z)
}*/

/// Apply key code to key-1 buffer.
/// Creates a unique code for each game.
/// 
/// Modifies key code, so that the process can be repeated if necessary.
/// 
/// Modulo should be between 1 and 3 (inclusive).
pub fn apply(key_code: &mut [u32; 3], key_buf: &[u32], modulo: usize) -> Vec<u32> {
    // Encrypt input id code.
    let key_code_hi = encrypt(u64::make(key_code[2], key_code[1]), key_buf);
    let key_code_lo = encrypt(u64::make(u64::lo(key_code_hi), key_code[0]), key_buf);
    key_code[2] = u64::hi(key_code_hi);
    key_code[1] = u64::hi(key_code_lo);
    key_code[0] = u64::lo(key_code_lo);
    // Initial encoding of buffer start.
    let mut key_out = vec![0; key_buf.len()];
    key_out.clone_from_slice(key_buf);
    for i in 0..0x12 {
        let key_code_i = key_code[i % modulo].swap_bytes();
        key_out[i] = key_out[i] ^ key_code_i;
    }
    // Waterfall encryption of remainder of buffer.
    let mut scratch = 0;
    for i in 0..0x209 {
        scratch = encrypt(scratch, &key_out);
        key_out[i*2] = u64::hi(scratch);
        key_out[i*2+1] = u64::lo(scratch);
    }
    key_out
}

pub fn init(id_code: u32, key_buf: &[u32], modulo: usize, level: usize) -> Vec<u32> {
    let mut key_code = [id_code, id_code.wrapping_div(2), id_code.wrapping_mul(2)];
    let level_1_key = apply(&mut key_code, key_buf, modulo);
    if level == 1 {
        level_1_key
    } else if level == 2 {
        apply(&mut key_code, &level_1_key, modulo)
    } else {
        let level_2_key = apply(&mut key_code, &level_1_key, modulo);
        key_code[1] = key_code[1].wrapping_mul(2);
        key_code[2] = key_code[2].wrapping_div(2);
        apply(&mut key_code, &level_2_key, modulo)
    }
}
