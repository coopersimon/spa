/// Audio for GBA

mod fifo;
mod gb;

use bitflags::bitflags;
use crossbeam_channel::Sender;
use dasp::frame::Stereo;

use crate::utils::{
    meminterface::MemInterface8,
    bits::u8,
    bytes::u16
};
use crate::common::resampler::*;
use gb::*;

bitflags! {
    #[derive(Default)]
    struct ChannelEnables: u8 {
        const LEFT_4    = u8::bit(7);
        const LEFT_3    = u8::bit(6);
        const LEFT_2    = u8::bit(5);
        const LEFT_1    = u8::bit(4);
        const RIGHT_4   = u8::bit(3);
        const RIGHT_3   = u8::bit(2);
        const RIGHT_2   = u8::bit(1);
        const RIGHT_1   = u8::bit(0);
    }
}

bitflags! {
    #[derive(Default)]
    struct FifoMixing: u8 {
        const B_RESET_FIFO      = u8::bit(7);
        const B_TIMER_SELECT    = u8::bit(6);
        const B_ENABLE_LEFT     = u8::bit(5);
        const B_ENABLE_RIGHT    = u8::bit(4);
        const A_RESET_FIFO      = u8::bit(3);
        const A_TIMER_SELECT    = u8::bit(2);
        const A_ENABLE_LEFT     = u8::bit(1);
        const A_ENABLE_RIGHT    = u8::bit(0);
    }
}

bitflags! {
    #[derive(Default)]
    struct MasterVolume: u8 {
        const SOUND_B_VOL       = u8::bit(3);
        const SOUND_A_VOL       = u8::bit(2);
        const GB_VOLUME         = u8::bits(0, 1);
    }
}

bitflags! {
    #[derive(Default)]
    struct PowerControl: u8 {
        const POWER     = u8::bit(7);
        const PLAYING_4 = u8::bit(3);
        const PLAYING_3 = u8::bit(2);
        const PLAYING_2 = u8::bit(1);
        const PLAYING_1 = u8::bit(0);
    }
}

const SAMPLE_PACKET_SIZE: usize = 64;
// TODO: move these to consts?

/// Base sample rate for audio.
const BASE_SAMPLE_RATE: usize = 32_768;
/// Cycles per second.
const CLOCK_RATE: usize = 16_777_216;
/// Emulated cycles per second.
const REAL_CLOCK_RATE: f64 = 16_853_760.0;

const REAL_SAMPLE_RATE_RATIO: f64 = REAL_CLOCK_RATE / (CLOCK_RATE as f64);
pub const REAL_BASE_SAMPLE_RATE: f64 = (BASE_SAMPLE_RATE as f64) * REAL_SAMPLE_RATE_RATIO;

pub struct GBAAudio {
    // GB channels
    square_1:   square1::Square1,
    square_2:   square2::Square2,
    wave:       wave::Wave,
    noise:      noise::Noise,

    // Control registers
    gb_vol:         u8,
    gb_enable:      ChannelEnables,
    master_vol:     MasterVolume,
    fifo_mixing:    FifoMixing,
    sound_on:       bool,
    soundbias:      u16,

    // Fifo
    fifo_a:         fifo::FIFO,
    fifo_b:         fifo::FIFO,

    // Comms with audio thread
    sample_buffer:      Vec<Stereo<f32>>,
    sample_sender:      Option<Sender<SamplePacket>>,
    rate_sender:        Option<Sender<f64>>,

    sample_rate:        usize,
    cycles_per_sample:  usize,
    cycle_count:        usize,

    gb_cycle_count:     usize,
    frame_count:        usize,
    frame_cycle_count:  usize,
}

impl GBAAudio {
    pub fn new() -> Self {
        Self {
            square_1:   square1::Square1::new(),
            square_2:   square2::Square2::new(),
            wave:       wave::Wave::new(),
            noise:      noise::Noise::new(),

            gb_vol:         0,
            gb_enable:      ChannelEnables::default(),
            master_vol:     MasterVolume::default(),
            fifo_mixing:    FifoMixing::default(),
            sound_on:       false,
            soundbias:      0x200,

            fifo_a:         fifo::FIFO::new(),
            fifo_b:         fifo::FIFO::new(),

            sample_buffer:      Vec::new(),
            sample_sender:      None,
            rate_sender:        None,

            sample_rate:        BASE_SAMPLE_RATE,
            cycles_per_sample:  CLOCK_RATE / BASE_SAMPLE_RATE,
            cycle_count:        0,

            gb_cycle_count:     0,
            frame_count:        0,
            frame_cycle_count:  0,
        }
    }

    /// Call to enable audio on the appropriate thread.
    /// 
    /// This should be done before any rendering.
    pub fn enable_audio(&mut self, sample_sender: Sender<SamplePacket>, rate_sender: Sender<f64>) {
        self.sample_sender = Some(sample_sender);
        self.rate_sender = Some(rate_sender);
    }

    pub fn clock(&mut self, cycles: usize) {
        // Modify channels
        self.gb_cycle_count += cycles;
        while self.gb_cycle_count >= 4 {
            self.gb_cycle_count -= 4;
            self.clock_channels();
        }
        
        self.cycle_count += cycles;
        if self.cycle_count >= self.cycles_per_sample {
            self.cycle_count -= self.cycles_per_sample;

            // Generate sample
            let sample = self.generate_sample();
            self.sample_buffer.push(sample);
            
            // Output to audio thread
            if self.sample_buffer.len() > SAMPLE_PACKET_SIZE {
                let sample_packet = self.sample_buffer.drain(..).collect::<SamplePacket>();
                if let Some(s) = &self.sample_sender {
                    s.send(sample_packet).expect("Error sending!");
                }
            }
        }
    }

    /// Called when timer 0 overflows.
    pub fn timer_0_tick(&mut self) {
        if self.sound_on {
            if !self.fifo_mixing.contains(FifoMixing::A_TIMER_SELECT) {
                self.tick_fifo_a();
            }
            if !self.fifo_mixing.contains(FifoMixing::B_TIMER_SELECT) {
                self.tick_fifo_b();
            }
        }
    }

    /// Called when timer 1 overflows.
    pub fn timer_1_tick(&mut self) {
        if self.sound_on {
            if self.fifo_mixing.contains(FifoMixing::A_TIMER_SELECT) {
                self.tick_fifo_a();
            }
            if self.fifo_mixing.contains(FifoMixing::B_TIMER_SELECT) {
                self.tick_fifo_b();
            }
        }
    }

    /// Returns true if fifo A is empty and needs more samples via DMA 1
    pub fn dma_1(&mut self) -> bool {
        self.fifo_a.len() < 16
    }

    /// Returns true if fifo B is empty and needs more samples via DMA 2
    pub fn dma_2(&mut self) -> bool {
        self.fifo_b.len() < 16
    }
}

impl MemInterface8 for GBAAudio {
    fn read_byte(&mut self, addr: u32) -> u8 {
        match addr {

            0x0400_0060 => self.square_1.sweep_reg,
            0x0400_0062 => self.square_1.duty_length_reg,
            0x0400_0063 => self.square_1.vol_envelope_reg,
            0x0400_0064 => self.square_1.freq_lo_reg,
            0x0400_0065 => self.square_1.freq_hi_reg,

            0x0400_0068 => self.square_2.duty_length_reg,
            0x0400_0069 => self.square_2.vol_envelope_reg,
            0x0400_006C => self.square_2.freq_lo_reg,
            0x0400_006D => self.square_2.freq_hi_reg,

            0x0400_0070 => self.wave.playback_reg,
            0x0400_0072 => self.wave.length_reg,
            0x0400_0073 => self.wave.vol_reg,
            0x0400_0074 => self.wave.freq_lo_reg,
            0x0400_0075 => self.wave.freq_hi_reg,

            0x0400_0078 => self.noise.length_reg,
            0x0400_0079 => self.noise.vol_envelope_reg,
            0x0400_007C => self.noise.poly_counter_reg,
            0x0400_007D => self.noise.trigger_reg,

            0x0400_0080 => self.gb_vol,
            0x0400_0081 => self.gb_enable.bits(),
            0x0400_0082 => self.master_vol.bits(),
            0x0400_0083 => self.fifo_mixing.bits(),
            0x0400_0084 => {
                let mut sound_on = PowerControl::default();
                sound_on.set(PowerControl::PLAYING_1, self.square_1.is_enabled());
                sound_on.set(PowerControl::PLAYING_2, self.square_2.is_enabled());
                sound_on.set(PowerControl::PLAYING_3, self.wave.is_enabled());
                sound_on.set(PowerControl::PLAYING_4, self.noise.is_enabled());
                sound_on.bits()
            },
            0x0400_0088 => u16::lo(self.soundbias),
            0x0400_0089 => u16::hi(self.soundbias),

            0x0400_0090..=0x0400_009F => self.wave.read_wave(addr - 0x0400_0090),

            _ => 0
        }
    }

    fn write_byte(&mut self, addr: u32, data: u8) {
        match addr {

            0x0400_0060 => self.square_1.set_sweep_reg(data),
            0x0400_0062 => self.square_1.set_duty_length_reg(data),
            0x0400_0063 => self.square_1.set_vol_envelope_reg(data),
            0x0400_0064 => self.square_1.set_freq_lo_reg(data),
            0x0400_0065 => self.square_1.set_freq_hi_reg(data),

            0x0400_0068 => self.square_2.set_duty_length_reg(data),
            0x0400_0069 => self.square_2.set_vol_envelope_reg(data),
            0x0400_006C => self.square_2.set_freq_lo_reg(data),
            0x0400_006D => self.square_2.set_freq_hi_reg(data),

            0x0400_0070 => self.wave.set_playback_reg(data),
            0x0400_0072 => self.wave.set_length_reg(data),
            0x0400_0073 => self.wave.set_vol_reg(data),
            0x0400_0074 => self.wave.set_freq_lo_reg(data),
            0x0400_0075 => self.wave.set_freq_hi_reg(data),

            0x0400_0078 => self.noise.set_length_reg(data),
            0x0400_0079 => self.noise.set_vol_envelope_reg(data),
            0x0400_007C => self.noise.set_poly_counter_reg(data),
            0x0400_007D => self.noise.set_trigger_reg(data),

            0x0400_0080 => self.gb_vol = data,
            0x0400_0081 => self.gb_enable = ChannelEnables::from_bits_truncate(data),
            0x0400_0082 => self.master_vol = MasterVolume::from_bits_truncate(data),
            0x0400_0083 => {
                self.fifo_mixing = FifoMixing::from_bits_truncate(data);
                if self.fifo_mixing.contains(FifoMixing::A_RESET_FIFO) {
                    self.fifo_a.clear();
                }
                if self.fifo_mixing.contains(FifoMixing::B_RESET_FIFO) {
                    self.fifo_b.clear();
                }
            },
            0x0400_0084 => {
                let sound_on = PowerControl::from_bits_truncate(data);
                self.sound_on = sound_on.contains(PowerControl::POWER);
                if !self.sound_on {
                    self.reset();
                }
            },
            0x0400_0088 => self.soundbias = u16::set_lo(self.soundbias, data),
            0x0400_0089 => {
                let new_bias = u16::set_hi(self.soundbias, data);
                self.set_sound_bias(new_bias);
            },

            0x0400_0090..=0x0400_009F => self.wave.write_wave(addr - 0x0400_0090, data),

            0x0400_00A0..=0x0400_00A3 => self.write_fifo_a(data),
            0x0400_00A4..=0x0400_00A7 => self.write_fifo_b(data),

            _ => {}
        }
    }

    /*fn write_word(&mut self, addr: u32, data: u32) {
        match addr {

            0x00 => {
                let bytes = data.to_le_bytes();
                self.square_1.set_sweep_reg(bytes[0]);
            }
            0x00 => 
            0x02 => self.square_1.set_duty_length_reg(data),
            0x03 => self.square_1.set_vol_envelope_reg(data),
            0x04 => self.square_1.set_freq_lo_reg(data),
            0x05 => self.square_1.set_freq_hi_reg(data),

            0x08 => self.square_2.set_duty_length_reg(data),
            0x09 => self.square_2.set_vol_envelope_reg(data),
            0x0C => self.square_2.set_freq_lo_reg(data),
            0x0D => self.square_2.set_freq_hi_reg(data),

            0x10 => self.wave.set_playback_reg(data),
            0x12 => self.wave.set_length_reg(data),
            0x13 => self.wave.set_vol_reg(data),
            0x14 => self.wave.set_freq_lo_reg(data),
            0x15 => self.wave.set_freq_hi_reg(data),

            0x18 => self.noise.set_length_reg(data),
            0x19 => self.noise.set_vol_envelope_reg(data),
            0x1C => self.noise.set_poly_counter_reg(data),
            0x1D => self.noise.set_trigger_reg(data),

            0x20 => self.gb_vol = data,
            0x21 => self.gb_enable = ChannelEnables::from_bits_truncate(data),
            0x22 => self.master_vol = MasterVolume::from_bits_truncate(data),
            0x23 => {
                self.fifo_mixing = FifoMixing::from_bits_truncate(data);
                if self.fifo_mixing.contains(FifoMixing::A_RESET_FIFO) {
                    for d in &mut self.fifo_a {
                        *d = 0;
                    }
                    self.buffer_a = 0;
                    self.write_index_a = 0;
                    self.read_index_a = 0;
                }
                if self.fifo_mixing.contains(FifoMixing::B_RESET_FIFO) {
                    for d in &mut self.fifo_b {
                        *d = 0;
                    }
                    self.buffer_b = 0;
                    self.write_index_b = 0;
                    self.read_index_b = 0;
                }
            },
            0x24 => {
                let sound_on = PowerControl::from_bits_truncate(data);
                self.sound_on = sound_on.contains(PowerControl::POWER);
                if !self.sound_on {
                    self.reset();
                }
            },
            0x28 => self.soundbias = u16::set_lo(self.soundbias, data),
            0x29 => {
                let new_bias = u16::set_hi(self.soundbias, data);
                self.set_sound_bias(new_bias);
            },

            0x30..=0x3F => self.wave.write_wave(addr - 0x30, data),

            0x40..=0x43 => self.write_fifo_a(data),
            0x44..=0x47 => self.write_fifo_b(data),

            _ => {}
        }
    }*/
}

impl GBAAudio {
    fn generate_sample(&mut self) -> Stereo<f32> {
        if self.sound_on {
            let bias = (self.soundbias & 0x3FE) as i16;
            let (gb_left, gb_right) = self.mix_gb_samples();

            let (fifo_left, fifo_right) = self.mix_fifo_samples();

            let left = clamp(gb_left + fifo_left + bias, 0, 0x3FF);
            let right = clamp(gb_right + fifo_right + bias, 0, 0x3FF);

            [to_output(left), to_output(right)]
        } else {
            [0.0, 0.0]
        }
    }

    fn mix_gb_samples(&mut self) -> (i16, i16) {
        let square_1 = self.square_1.get_sample() as i16;
        let square_2 = self.square_2.get_sample() as i16;
        let wave = self.wave.get_sample() as i16;
        let noise = self.noise.get_sample() as i16;

        let left_1 = if self.gb_enable.contains(ChannelEnables::LEFT_1) {square_1} else {0};
        let left_2 = if self.gb_enable.contains(ChannelEnables::LEFT_2) {square_2} else {0};
        let left_3 = if self.gb_enable.contains(ChannelEnables::LEFT_3) {wave} else {0};
        let left_4 = if self.gb_enable.contains(ChannelEnables::LEFT_4) {noise} else {0};

        let right_1 = if self.gb_enable.contains(ChannelEnables::RIGHT_1) {square_1} else {0};
        let right_2 = if self.gb_enable.contains(ChannelEnables::RIGHT_2) {square_2} else {0};
        let right_3 = if self.gb_enable.contains(ChannelEnables::RIGHT_3) {wave} else {0};
        let right_4 = if self.gb_enable.contains(ChannelEnables::RIGHT_4) {noise} else {0};

        let gb_vol_left = ((self.gb_vol >> 4) & 0x7) as i16;
        let gb_vol_right = (self.gb_vol & 0x7) as i16;
        let gb_left = left_1 + left_2 + left_3 + left_4;
        let gb_right = right_1 + right_2 + right_3 + right_4;

        let gb_left_mixed = (gb_vol_left * gb_left) / 7;
        let gb_right_mixed = (gb_vol_right * gb_right) / 7;
        match (self.master_vol & MasterVolume::GB_VOLUME).bits() {
            0b00 => (gb_left_mixed >> 2, gb_right_mixed >> 2),
            0b01 => (gb_left_mixed >> 1, gb_right_mixed >> 1),
            0b10 => (gb_left_mixed, gb_right_mixed),
            _ => unreachable!()
        }
    }

    fn mix_fifo_samples(&mut self) -> (i16, i16) {
        let fifo_a = if self.master_vol.contains(MasterVolume::SOUND_A_VOL) {
            (self.fifo_a.sample() as i16) << 2
        } else {
            (self.fifo_a.sample() as i16) << 1
        };

        let fifo_b = if self.master_vol.contains(MasterVolume::SOUND_B_VOL) {
            (self.fifo_b.sample() as i16) << 2
        } else {
            (self.fifo_b.sample() as i16) << 1
        };

        let (mut left, mut right) = (0, 0);
        if self.fifo_mixing.contains(FifoMixing::A_ENABLE_LEFT) {
            left += fifo_a;
        }
        if self.fifo_mixing.contains(FifoMixing::B_ENABLE_LEFT) {
            left += fifo_b;
        }
        if self.fifo_mixing.contains(FifoMixing::A_ENABLE_RIGHT) {
            right += fifo_a;
        }
        if self.fifo_mixing.contains(FifoMixing::B_ENABLE_RIGHT) {
            right += fifo_b;
        }

        (left, right)
    }

    fn reset(&mut self) {
        self.square_1.reset();
        self.square_2.reset();
        self.wave.reset();
        self.noise.reset();

        self.gb_vol = 0;
        self.gb_enable = ChannelEnables::default();
    }

    fn set_sound_bias(&mut self, new_val: u16) {
        self.soundbias = new_val;
        let new_sample_rate = match (new_val >> 14) & 0b11 {
            0b00 => BASE_SAMPLE_RATE,
            0b01 => BASE_SAMPLE_RATE * 2,
            0b10 => BASE_SAMPLE_RATE * 4,
            0b11 => BASE_SAMPLE_RATE * 8,
            _ => unreachable!()
        };
        if new_sample_rate != self.sample_rate {
            self.sample_rate = new_sample_rate;
            self.cycles_per_sample = CLOCK_RATE / self.sample_rate;
            if let Some(sender) = &self.rate_sender {
                let real_sample_rate = REAL_SAMPLE_RATE_RATIO * (new_sample_rate as f64);
                sender.send(real_sample_rate).unwrap();
            }
        }
    }

    fn write_fifo_a(&mut self, data: u8) {
        self.fifo_a.push(data as i8);
    }
    fn write_fifo_b(&mut self, data: u8) {
        self.fifo_b.push(data as i8);
    }

    /// Advance FIFO A.
    fn tick_fifo_a(&mut self) {
        self.fifo_a.pop();
    }

    /// Advance FIFO B.
    fn tick_fifo_b(&mut self) {
        self.fifo_b.pop();
    }

    /// Call every 4 GBA clocks.
    fn clock_channels(&mut self) {
        const FRAME_MODULO: usize = 8192; // Clock rate / 8192 = 512
        // Advance samples
        self.square_1.sample_clock(1);
        self.square_2.sample_clock(1);
        self.wave.sample_clock(1);
        self.noise.sample_clock(1);

        self.frame_cycle_count += 1;
        // Clock length and sweeping at 512Hz
        if self.frame_cycle_count >= FRAME_MODULO {
            self.frame_cycle_count -= FRAME_MODULO;

            // Clock length at 256Hz
            if self.frame_count % 2 == 0 {
                self.square_1.length_clock();
                self.square_2.length_clock();
                self.wave.length_clock();
                self.noise.length_clock();
            }

            // Clock envelope sweep at 64Hz
            if self.frame_count == 7 {
                self.square_1.envelope_clock();
                self.square_2.envelope_clock();
                self.noise.envelope_clock();
            }
            
            // Clock frequency sweep at 128Hz
            if self.frame_count % 4 == 2 {
                self.square_1.sweep_clock();
            }

            self.frame_count = (self.frame_count + 1) % 8;
        }
    }
}

#[inline]
const fn clamp(n: i16, low: i16, high: i16) -> i16 {
    if n < low {
        low
    } else if n > high {
        high
    } else {
        n
    }
}

#[inline]
fn to_output(sample: i16) -> f32 {
    let shifted = (sample >> 1) as f32;
    (shifted / 256.0) - 1.0
}
