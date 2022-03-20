use crate::utils::bits::u8;
use super::*;

const MAX_LEN: u8 = 64;

pub struct Noise {
    // Public registers
    pub length_reg:         u8,
    pub vol_envelope_reg:   u8,
    pub poly_counter_reg:   u8,
    pub trigger_reg:        u8,

    // Internal registers
    enabled:        bool,
    lfsr_counter:   u16,

    volume:         u8,
    volume_counter: Option<u8>,
    volume_modulo:  u8,

    length_counter: u8,
    length_modulo:  u8,

    freq_counter:   usize,
    freq_modulo:    usize,
}

impl Noise {
    pub fn new() -> Self {
        Self {
            length_reg:         0,
            vol_envelope_reg:   0,
            poly_counter_reg:   0,
            trigger_reg:        0,

            enabled:            false,
            lfsr_counter:       0xFFFF,

            volume:             0,
            volume_counter:     None,
            volume_modulo:      0,

            length_counter:     0,
            length_modulo:      MAX_LEN,

            freq_counter:       0,
            freq_modulo:        0,
        }
    }

    pub fn set_length_reg(&mut self, val: u8) {
        self.length_reg = val;
    }

    pub fn set_vol_envelope_reg(&mut self, val: u8) {
        self.vol_envelope_reg = val;
    }

    pub fn set_poly_counter_reg(&mut self, val: u8) {
        self.poly_counter_reg = val;
    }

    pub fn set_trigger_reg(&mut self, val: u8) {
        self.trigger_reg = val;
        // And trigger event...
        if u8::test_bit(val, 7) {
            self.trigger();
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl GBChannel for Noise {
    fn sample_clock(&mut self, cycles: usize) {
        self.freq_counter += cycles;
        if self.freq_counter >= self.freq_modulo {
            self.freq_counter -= self.freq_modulo;
            self.lfsr_step();
        }
    }

    fn length_clock(&mut self) {
        if self.enabled && u8::test_bit(self.trigger_reg, 6) {
            self.length_counter -= 1;
            if self.length_counter == self.length_modulo {
                self.enabled = false;
            }
        }
    }

    fn envelope_clock(&mut self) {
        if let Some(counter) = self.volume_counter {
            let new_count = counter + 1;
            self.volume_counter = if new_count >= self.volume_modulo {
                match u8::test_bit(self.vol_envelope_reg, 3) {
                    false if self.volume > MIN_VOL => {
                        self.volume -= 1;
                        Some(0)
                    },
                    true if self.volume < MAX_VOL => {
                        self.volume += 1;
                        Some(0)
                    },
                    _ => None
                }
            } else {
                Some(new_count)
            };
        }
    }

    fn get_sample(&self) -> i8 {
        if self.enabled {
            if (self.lfsr_counter & 1) == 1 {
                -u4_to_i8(self.volume)
            } else {
                u4_to_i8(self.volume)
            }
        } else {
            0
        }
    }

    fn reset(&mut self) {
        self.length_reg = 0;
        self.vol_envelope_reg = 0;
        self.poly_counter_reg = 0;
        self.trigger_reg = 0;

        self.freq_counter = 0;
        self.length_counter = MAX_LEN;
    }
}

impl Noise {
    fn trigger(&mut self) {
        const LEN_MASK: u8 = u8::bits(0, 5);
        const VOL_MASK: u8 = u8::bits(4, 7);
        const VOL_SWEEP_MASK: u8 = u8::bits(0, 2);
        const FREQ_SHIFT_MASK: u8 = u8::bits(4, 7);
        const FREQ_DIVISOR_MASK: u8 = u8::bits(0, 2);

        self.volume = (self.vol_envelope_reg & VOL_MASK) >> 4;
        self.volume_modulo = self.vol_envelope_reg & VOL_SWEEP_MASK;
        self.volume_counter = if self.volume_modulo == 0 {None} else {Some(0)};

        let freq_modulo_shift = (self.poly_counter_reg & FREQ_SHIFT_MASK) >> 4;
        self.freq_modulo = match self.poly_counter_reg & FREQ_DIVISOR_MASK {
            0 => 8,
            x => (x as usize) * 16,
        } << freq_modulo_shift;
        self.freq_counter = 0;

        self.length_counter = MAX_LEN;
        self.length_modulo = self.length_reg & LEN_MASK;

        self.lfsr_counter = 0xFFFF;

        self.enabled = true;
    }

    fn lfsr_step(&mut self) {
        const LFSR_MASK: u16 = 0x3FFF;
        const LFSR_7BIT_MASK: u16 = 0xFFBF;

        let low_bit = self.lfsr_counter & 1;
        self.lfsr_counter >>= 1;
        let xor_bit = (self.lfsr_counter & 1) ^ low_bit;

        self.lfsr_counter = (self.lfsr_counter & LFSR_MASK) | (xor_bit << 14);
        if u8::test_bit(self.poly_counter_reg, 3) {
            self.lfsr_counter = (self.lfsr_counter & LFSR_7BIT_MASK) | (xor_bit << 6);
        }
    }
}
