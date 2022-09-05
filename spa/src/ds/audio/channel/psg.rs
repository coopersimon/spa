
pub struct SquareGenerator {
    duty_hi:    u8,
    counter:    u8,
}

impl SquareGenerator {
    pub fn new() -> Self {
        Self {
            duty_hi:    1,
            counter:    0,
        }
    }

    /// Reset the counter. Pass in the wave duty bits from
    /// the sound control register here.
    pub fn reset(&mut self, duty: u8) {
        self.duty_hi = duty + 1;
        self.counter = 0;
    }

    pub fn generate_sample(&mut self) -> i16 {
        let out = if self.counter < self.duty_hi {
            super::SAMPLE_MAX
        } else {
            super::SAMPLE_MIN
        };
        self.counter = (self.counter + 1) & 0x7;
        out
    }
}
