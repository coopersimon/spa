pub mod key1;
mod u64;

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


#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
