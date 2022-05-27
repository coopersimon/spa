use crate::utils::bits::u8;
use super::*;

const MAX_LEN: u8 = 64;

pub struct Square2 {
    // Public registers
    pub duty_length_reg:    u8,
    pub vol_envelope_reg:   u8,
    pub freq_lo_reg:        u8,
    pub freq_hi_reg:        u8,

    // Internal registers
    enabled:        bool,
    duty_counter:   DutyCycleCounter,

    volume:         u8,
    volume_counter: Option<u8>,
    volume_modulo:  u8,

    length_counter: u8,
    length_modulo:  u8,

    freq_counter:   usize,
    freq_modulo:    usize,
}

impl Square2 {
    pub fn new() -> Self {
        Self {
            duty_length_reg:    0,
            vol_envelope_reg:   0,
            freq_lo_reg:        0,
            freq_hi_reg:        0,

            enabled:            false,
            duty_counter:       DutyCycleCounter::new(0),

            volume:             0,
            volume_counter:     None,
            volume_modulo:      0,

            length_counter:     0,
            length_modulo:      MAX_LEN,

            freq_counter:       0,
            freq_modulo:        0,
        }
    }

    pub fn set_duty_length_reg(&mut self, val: u8) {
        self.duty_length_reg = val;
    }

    pub fn set_vol_envelope_reg(&mut self, val: u8) {
        self.vol_envelope_reg = val;
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
}

impl GBChannel for Square2 {
    fn sample_clock(&mut self, cycles: usize) {
        self.freq_counter += cycles;
        if self.freq_counter >= self.freq_modulo {
            self.freq_counter -= self.freq_modulo;
            self.duty_counter.step();
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
            match self.duty_counter.read() {
                SquareDuty::Lo => -u4_to_i8(self.volume),
                SquareDuty::Hi => u4_to_i8(self.volume),
            }
        } else {
            0
        }
    }

    fn reset(&mut self) {
        self.duty_length_reg = 0;
        self.vol_envelope_reg = 0;
        self.freq_lo_reg = 0;
        self.freq_hi_reg = 0;

        self.freq_counter = 0;
        self.length_counter = MAX_LEN;
    }
}

impl Square2 {
    fn trigger(&mut self) {
        const LEN_MASK: u8 = u8::bits(0, 5);
        const VOL_MASK: u8 = u8::bits(4, 7);
        const VOL_SWEEP_MASK: u8 = u8::bits(0, 2);

        self.volume = (self.vol_envelope_reg & VOL_MASK) >> 4;
        self.volume_modulo = self.vol_envelope_reg & VOL_SWEEP_MASK;
        self.volume_counter = if self.volume_modulo == 0 {None} else {Some(0)};

        self.freq_counter = 0;
        self.freq_modulo = (2048 - get_freq_modulo(self.freq_hi_reg, self.freq_lo_reg)) * 4;

        self.length_counter = MAX_LEN;
        self.length_modulo = self.duty_length_reg & LEN_MASK;

        self.enabled = true;
    }
}
