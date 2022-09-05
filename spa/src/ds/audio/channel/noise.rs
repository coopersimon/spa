const LFSR_SEED: u16 = 0x7FFF;

pub struct NoiseGenerator {
    lfsr_counter:  u16,
}

impl NoiseGenerator {
    pub fn new() -> Self {
        Self {
            lfsr_counter: LFSR_SEED,
        }
    }

    pub fn reset(&mut self) {
        self.lfsr_counter = LFSR_SEED;
    }

    pub fn generate_sample(&mut self) -> i16 {
        let low_bit = self.lfsr_counter & 1;
        self.lfsr_counter >>= 1;

        if low_bit == 1 {
            self.lfsr_counter ^= 0x6000;
            super::SAMPLE_MIN
        } else {
            super::SAMPLE_MAX
        }
    }
}