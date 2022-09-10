use crate::utils::bits::u8;

const ADPCM_TABLE: [i16; 89] = [
    0x0007, 0x0008, 0x0009, 0x000A, 0x000B, 0x000C, 0x000D, 0x000E, 0x0010, 0x0011, 0x0013, 0x0015,
    0x0017, 0x0019, 0x001C, 0x001F, 0x0022, 0x0025, 0x0029, 0x002D, 0x0032, 0x0037, 0x003C, 0x0042,
    0x0049, 0x0050, 0x0058, 0x0061, 0x006B, 0x0076, 0x0082, 0x008F, 0x009D, 0x00AD, 0x00BE, 0x00D1,
    0x00E6, 0x00FD, 0x0117, 0x0133, 0x0151, 0x0173, 0x0198, 0x01C1, 0x01EE, 0x0220, 0x0256, 0x0292,
    0x02D4, 0x031C, 0x036C, 0x03C3, 0x0424, 0x048E, 0x0502, 0x0583, 0x0610, 0x06AB, 0x0756, 0x0812,
    0x08E0, 0x09C3, 0x0ABD, 0x0BD0, 0x0CFF, 0x0E4C, 0x0FBA, 0x114C, 0x1307, 0x14EE, 0x1706, 0x1954,
    0x1BDC, 0x1EA5, 0x21B6, 0x2515, 0x28CA, 0x2CDF, 0x315B, 0x364B, 0x3BB9, 0x41B2, 0x4844, 0x4F7E,
    0x5771, 0x602F, 0x69CE, 0x7462, 0x7FFF
];

const INDEX_TABLE: [isize; 8] = [-1, -1, -1, -1, 2, 4, 6, 8];

pub struct ADPCMGenerator {
    initialised:    bool,
    current_value:  i16,
    current_index:  usize,

    loop_value:     i16,
    loop_index:     usize,
}

impl ADPCMGenerator {
    pub fn new() -> Self {
        Self {
            initialised:    false,
            current_value:  0,
            current_index:  0,
            loop_value:     0,
            loop_index:     0,
        }
    }

    pub fn reset(&mut self) {
        self.initialised = false;
        self.current_value = 0;
        self.current_index = 0;
        self.loop_value = 0;
        self.loop_index = 0;
    }

    pub fn needs_header(&self) -> bool {
        !self.initialised
    }

    pub fn set_header(&mut self, header: u32) -> i16 {
        self.initialised = true;
        self.current_value = (header & 0xFFFF) as u16 as i16;
        let index = (header >> 16) & 0x7F;
        self.current_index = std::cmp::min(index as usize, 88);
        self.current_value
    }

    pub fn generate_sample(&mut self, data: u8) -> i16 {
        let table_value = ADPCM_TABLE[self.current_index];
        let shifted_data = ((data & 0x7) * 2) + 1;
        let diff = (shifted_data as i32).saturating_mul(table_value as i32) >> 4;
        if u8::test_bit(data, 3) {
            self.current_value = self.current_value.saturating_sub(diff as i16);
        } else {
            self.current_value = self.current_value.saturating_add(diff as i16);
        }

        let index_table_val = INDEX_TABLE[(data & 0x7) as usize];
        let new_index = (self.current_index as isize) + index_table_val;
        self.current_index = std::cmp::min(88, std::cmp::max(0, new_index)) as usize;
        
        self.current_value
    }

    /// To be called when passing loop init value.
    pub fn store_loop_values(&mut self) {
        self.loop_value = self.current_value;
        self.loop_index = self.current_index;
    }

    /// To be called when looping.
    pub fn restore_loop_values(&mut self) {
        self.current_value = self.loop_value;
        self.current_index = self.loop_index;
    }
}