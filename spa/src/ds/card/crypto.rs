/// Blowfish encryption for NDS card.

use crate::utils::bytes::u64;

/// Decrypt key-1 encoded block.
pub fn key_1_decrypt(block: u64, key_buf: &[u32]) -> u64 {
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

/*fn key_1_inner(key_buf: &[u32], i: usize, x: u32, y: u32) -> (u32, u32) {
    let z = key_buf[i] ^ x;
    let z_idx = z as usize;
    let mut x = key_buf[0x12 + ((z_idx >> 24) & 0xFF)];
    x = key_buf[0x112 + ((z_idx >> 16) & 0xFF)].wrapping_add(x);
    x = key_buf[0x212 + ((z_idx >> 8) & 0xFF)] ^ x;
    x = key_buf[0x312 + (z_idx & 0xFF)].wrapping_add(x);
    (y ^ x, z)
}*/

/// Encrypt block with key-1.
fn key_1_encrypt(block: u64, key_buf: &[u32]) -> u64 {
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

/// Apply key code to key-1 buffer.
/// Creates a unique code for each game.
fn key_1_apply(key_code: &mut [u32; 3], key_buf: &[u32]) -> Vec<u32> {
    // Encrypt input id code.
    let key_code_hi = key_1_encrypt(u64::make(key_code[2], key_code[1]), key_buf);
    let key_code_lo = key_1_encrypt(u64::make(u64::lo(key_code_hi), key_code[0]), key_buf);
    key_code[2] = u64::hi(key_code_hi);
    key_code[1] = u64::hi(key_code_lo);
    key_code[0] = u64::lo(key_code_lo);
    // Initial encoding of buffer start.
    let mut key_out = vec![0; key_buf.len()];
    key_out.clone_from_slice(key_buf);
    for i in 0..0x12 {
        let key_code_i = key_code[i & 1].swap_bytes();
        key_out[i] = key_out[i] ^ key_code_i;
    }
    // Waterfall encryption of remainder of buffer.
    let mut scratch = 0;
    for i in 0..0x209 {
        scratch = key_1_encrypt(scratch, &key_out);
        key_out[(i*2)] = u64::hi(scratch);
        key_out[(i*2)+1] = u64::lo(scratch);
    }
    key_out
}

/// Encrypt KEY 1 to level "2", in order to decode incoming commands.
pub fn key_1_init(id_code: u32, key_buf: &[u32]) -> Vec<u32> {
    let mut key_code = [id_code, id_code.wrapping_div(2), id_code.wrapping_mul(2)];
    let level_1_key = key_1_apply(&mut key_code, key_buf);
    key_1_apply(&mut key_code, &level_1_key)
}

/// Encrypt or decrypt a byte of data with key-2.
///
/// As long as the keys (x, y) are the same,
/// the data in and out will be inverted.
pub fn key_2_encrypt(data: u8, x: u64, y: u64) -> (u8, u64, u64) {
    let x_out_lo = ((x >> 5) ^ (x >> 17) ^ (x >> 18) ^ (x >> 31)) & 0xFF;
    let x_out = ((x << 8) | x_out_lo) & 0x7F_FFFF_FFFF;
    let y_out_lo = ((y >> 5) ^ (y >> 23) ^ (y >> 18) ^ (y >> 31)) & 0xFF;
    let y_out = ((y << 8) | y_out_lo) & 0x7F_FFFF_FFFF;
    let data_out = data ^ (x_out_lo as u8) ^ (y_out_lo as u8);
    (data_out, x_out, y_out)
}
