// Audio channels.
pub mod square1;
pub mod square2;
pub mod wave;
pub mod noise;

use crate::utils::{
    bits::u8,
    bytes::u16
};

pub trait GBChannel {
    /// Clock the channel and recalculate the output if necessary.
    /// 
    /// Call at 2^14 Hz.
    fn sample_clock(&mut self, cycles: usize);

    /// Call at 256Hz, to decrement the length counter.
    fn length_clock(&mut self);

    /// Call at 64Hz, for volume envelope.
    fn envelope_clock(&mut self);

    /// Get the current output sample.
    fn get_sample(&self) -> i8;

    /// Reset all internal timers and buffers.
    fn reset(&mut self);
}

#[derive(Clone, Copy)]
enum SquareDuty {
    Lo,
    Hi
}
const DUTY_0: [SquareDuty; 8] = [SquareDuty::Lo, SquareDuty::Lo, SquareDuty::Lo, SquareDuty::Lo, SquareDuty::Lo, SquareDuty::Lo, SquareDuty::Lo, SquareDuty::Hi];
const DUTY_1: [SquareDuty; 8] = [SquareDuty::Hi, SquareDuty::Lo, SquareDuty::Lo, SquareDuty::Lo, SquareDuty::Lo, SquareDuty::Lo, SquareDuty::Lo, SquareDuty::Hi];
const DUTY_2: [SquareDuty; 8] = [SquareDuty::Hi, SquareDuty::Lo, SquareDuty::Lo, SquareDuty::Lo, SquareDuty::Lo, SquareDuty::Hi, SquareDuty::Hi, SquareDuty::Hi];
const DUTY_3: [SquareDuty; 8] = [SquareDuty::Lo, SquareDuty::Hi, SquareDuty::Hi, SquareDuty::Hi, SquareDuty::Hi, SquareDuty::Hi, SquareDuty::Hi, SquareDuty::Lo];

struct DutyCycleCounter {
    pattern:    &'static [SquareDuty; 8],
    index:      usize
}

impl DutyCycleCounter {
    fn new(duty: u8) -> Self {
        Self {
            pattern: match duty & 0x3 {
                0 => &DUTY_0,
                1 => &DUTY_1,
                2 => &DUTY_2,
                3 => &DUTY_3,
                _ => unreachable!()
            },
            index: 0
        }
    }

    fn step(&mut self) {
        self.index = (self.index + 1) % 8;
    }

    fn read(&self) -> SquareDuty {
        self.pattern[self.index]
    }
}

const MAX_VOL: u8 = 15;
const MIN_VOL: u8 = 0;

fn get_freq_modulo(hi_reg: u8, lo_reg: u8) -> usize {
    const HI_FREQ_MASK: u8 = u8::bits(0, 2);
    let hi = hi_reg & HI_FREQ_MASK;
    u16::make(hi, lo_reg) as usize
}

// Convert from 4-bit unsigned samples to 7-bit unsigned.
#[inline]
const fn u4_to_i8(amplitude: u8) -> i8 {
    ((amplitude << 3) | (amplitude >> 1)) as i8
}

// Convert from 4-bit signed samples to 8-bit signed.
#[inline]
const fn i4_to_i8(amplitude: u8) -> i8 {
    let signed = (amplitude as i8) - 8;
    let hi = signed << 4;
    let lo = (signed & 7) << 1;
    hi | lo
}