use crate::common::bits::u8;
use super::*;

const MAX_LEN: u16 = 256;
const WAVE_PATTERN_BANK_SIZE: usize = 16;

enum ShiftAmount {
    Mute,
    Full,
    Half,
    Quarter
}

pub struct Wave {
    // Public registers
    pub playback_reg:   u8,
    pub length_reg:     u8,
    pub vol_reg:        u8,
    pub freq_lo_reg:    u8,
    pub freq_hi_reg:    u8,

    // Sample table
    wave_pattern:   [u8; 32],

    // Internal registers
    enabled:        bool,
    pattern_index:  usize,

    shift_amount:   ShiftAmount,

    length_counter: u16,
    length_modulo:  u16,

    freq_counter:   usize,
    freq_modulo:    usize,

}

impl Wave {
    pub fn new() -> Self {
        Self {
            playback_reg:   0,
            length_reg:     0,
            vol_reg:        0,
            freq_lo_reg:    0,
            freq_hi_reg:    0,

            wave_pattern:   [0; 32],

            enabled:            false,
            pattern_index:      0,

            shift_amount:       ShiftAmount::Mute,

            length_counter:     0,
            length_modulo:      MAX_LEN,

            freq_counter:       0,
            freq_modulo:        0,
        }
    }

    pub fn set_playback_reg(&mut self, val: u8) {
        self.playback_reg = val;
    }

    pub fn set_length_reg(&mut self, val: u8) {
        self.length_reg = val;
    }

    pub fn set_vol_reg(&mut self, val: u8) {
        self.vol_reg = val;
    }

    pub fn set_freq_lo_reg(&mut self, val: u8) {
        self.freq_lo_reg = val;
    }

    pub fn set_freq_hi_reg(&mut self, val: u8) {
        self.freq_hi_reg = val;
        // And trigger event...
        if u8::test_bit(val, 7) {
            self.trigger();
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn write_wave(&mut self, addr: u32, val: u8) {
        if u8::test_bit(self.playback_reg, 6) {
            self.wave_pattern[(addr as usize) + WAVE_PATTERN_BANK_SIZE] = val;
        } else {
            self.wave_pattern[addr as usize] = val;
        }
    }

    pub fn read_wave(&self, addr: u32) -> u8 {
        if u8::test_bit(self.playback_reg, 6) {
            self.wave_pattern[(addr as usize) + WAVE_PATTERN_BANK_SIZE]
        } else {
            self.wave_pattern[addr as usize]
        }
    }
}

impl GBChannel for Wave {
    fn sample_clock(&mut self, cycles: usize) {
        self.freq_counter += cycles;
        if self.freq_counter >= self.freq_modulo {
            self.freq_counter -= self.freq_modulo;
            let pattern_offset = if u8::test_bit(self.playback_reg, 5) {
                32
            } else {
                0
            };
            self.pattern_index = pattern_offset + (self.pattern_index + 1) % 32;
        }
    }

    fn length_clock(&mut self) {
        if self.enabled && u8::test_bit(self.freq_hi_reg, 6) {
            self.length_counter -= 1;
            if self.length_counter == self.length_modulo {
                self.enabled = false;
            }
        }
    }

    fn envelope_clock(&mut self) {
    }

    fn get_sample(&self) -> i8 {
        if self.enabled {
            self.read_wave_pattern()
        } else {
            0
        }
    }

    fn reset(&mut self) {
        self.pattern_index = 0;
        self.freq_lo_reg = 0;
        self.freq_hi_reg = 0;

        self.freq_counter = 0;
        self.length_counter = MAX_LEN;

        self.enabled = false;
    }
}

impl Wave {
    fn trigger(&mut self) {
        const SHIFT_MASK: u8 = u8::bits(5, 6);

        self.shift_amount = match (self.vol_reg & SHIFT_MASK) >> 5 {
            0 => ShiftAmount::Mute,
            1 => ShiftAmount::Full,
            2 => ShiftAmount::Half,
            3 => ShiftAmount::Quarter,
            _ => unreachable!()
        };

        self.freq_counter = 0;
        self.freq_modulo = (2048 - get_freq_modulo(self.freq_hi_reg, self.freq_lo_reg)) * 2;

        self.length_counter = MAX_LEN;
        self.length_modulo = self.length_reg as u16;

        self.enabled = true;
    }

    fn read_wave_pattern(&self) -> i8 {
        let u8_index = self.pattern_index / 2;
        let shift = 4 * ((self.pattern_index + 1) % 2);
        let raw_sample = (self.wave_pattern[u8_index] >> shift) & 0xF;

        match self.shift_amount {
            ShiftAmount::Mute => 0,
            ShiftAmount::Full => i4_to_i8(raw_sample),
            ShiftAmount::Half => i4_to_i8(raw_sample) >> 1,
            ShiftAmount::Quarter => i4_to_i8(raw_sample) >> 2,
        }
    }
}
