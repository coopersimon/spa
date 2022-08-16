
/// 8-bit binary coded decimal number.
/// Range from 00-99
#[derive(Clone, Copy, Default)]
pub struct Bcd8(u8);

impl Bcd8 {
    /// Make BCD from a binary value.
    /// Binary value must be in the range 0-99 (decimal).
    pub fn from_binary(value: u8) -> Self {
        if value > 99 {
            // TODO: probably shouldn't panic.
            panic!("value is too large: {} ({:X})", value, value)
        } else {
            let tens = value / 10;
            let units = value % 10;
            Self((tens * 0x10) + units)
        }
    }

    /// Make BCD from a binary value already in BCD format.
    /// BCD value must have each nybble be in the range 0-9.
    pub fn from_bcd(bcd_value: u8) -> Self {
        if bcd_value > 0x99 {
            panic!("too large...");
        }
        Self(bcd_value)
    }

    /// Get the BCD value as a u8.
    /// Each nybble is in the range 0-9.
    pub fn binary(&self) -> u8 {
        self.0
    }
}

mod tests {
    #[test]
    fn test_bcd() {
        use super::Bcd8;

        let vals = [
            (30_u8, 0x30_u8),
            (77_u8, 0x77_u8),
        ];

        for (decimal, bcd_hex) in &vals {
            let bcd = Bcd8::from_binary(*decimal);
            let bcd_bits = bcd.binary();
            assert_eq!(bcd_bits, *bcd_hex);
        }
    }
}