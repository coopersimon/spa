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
        x = key_buf[0x112 + ((z_idx >> 16) & 0xFF)] + x;
        x = key_buf[0x212 + ((z_idx >> 8) & 0xFF)] ^ x;
        x = key_buf[0x312 + (z_idx & 0xFF)] + x;
        x = y ^ x;
        y = z;
    }
    x = x ^ key_buf[1];
    y = y ^ key_buf[0];
    u64::make(y, x)
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
