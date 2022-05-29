
/// 8-bit binary coded decimal number.
/// Range from 00-99
#[derive(Clone, Copy, Default)]
pub struct Bcd8(u8);

impl TryFrom<u8> for Bcd8 {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if value > 0x99 {
            Err("value is too large!")
        } else if value & 0xF > 0x9 {
            // Subtract 0xA, add 0x10
            let bcd_value = value + 0x6;
            if bcd_value > 0x99 {
                Err("value is too large!")
            } else {
                Ok(Bcd8(bcd_value))
            }
        } else {
            Ok(Bcd8(value))
        }
    }
}

impl Into<u8> for Bcd8 {
    fn into(self) -> u8 {
        self.0
    }
}
