mod fifo;
mod adpcm;

use bitflags::bitflags;
use crate::utils::bits::u32;

use fifo::AudioFIFO;
use adpcm::ADPCMGenerator;

bitflags!{
    #[derive(Default)]
    pub struct ChannelControl: u32 {
        const START     = u32::bit(31);
        const FORMAT    = u32::bits(29, 30);
        const REPEAT    = u32::bits(27, 28);
        const WAVE_DUTY = u32::bits(24, 26);
        const PAN       = u32::bits(16, 22);
        const HOLD      = u32::bit(15);
        const VOL_DIV   = u32::bits(8, 9);
        const VOLUME    = u32::bits(0, 6);
    }
}

pub enum ChannelType {
    PCM,
    PSG,
    Noise
}

/// An audio channel.
/// All audio channels can produce PCM in
/// 3 formats.
/// 
/// Some audio channels can produce PSG (square wave),
/// or pseudo-white noise.
pub struct AudioChannel {
    // Registers
    pub control:        ChannelControl,
    pub src_addr:       u32,
    pub timer:          u16,
    pub loop_start_pos: u32,
    pub sound_len:      u32,

    // Internal
    chan_type:      ChannelType,
    dma_mask:       u16,

    timer_counter:      u16,
    current_addr:       u32,
    loop_start_addr:    u32,
    loop_end_addr:      u32,

    // Both of the below counted in nybbles (half-bytes)
    sample_count:   u32,
    sample_len:     u32,

    fifo:           AudioFIFO,
    adpcm_gen:      ADPCMGenerator,

    current_sample: i16,
    current_frame:  (i32, i32),
    left_vol:       i32,
    right_vol:      i32,
}

impl AudioChannel {
    pub fn new(chan_type: ChannelType, dma_mask: u16) -> Self {
        Self {
            control:    ChannelControl::default(),
            src_addr:   0,
            timer:      0,
            loop_start_pos: 0,
            sound_len:  0,

            chan_type:      chan_type,
            dma_mask:       dma_mask,

            timer_counter:      0,
            current_addr:       0,
            loop_start_addr:    0,
            loop_end_addr:      0,

            sample_count:   0,
            sample_len:     0,

            fifo:           AudioFIFO::new(),
            adpcm_gen:      ADPCMGenerator::new(),

            current_sample: 0,
            current_frame:  (0, 0),
            left_vol:       0,
            right_vol:      0,
        }
    }

    pub fn write_control(&mut self, data: u32) {
        let running = self.control.contains(ChannelControl::START);
        self.control = ChannelControl::from_bits_truncate(data);

        if !running && self.control.contains(ChannelControl::START) {
            self.reset();
        }

        let volume = (self.control & ChannelControl::VOLUME).bits() as i32;
        let pan = ((self.control & ChannelControl::PAN).bits() >> 16) as i32;
        self.left_vol = (127 - pan) * volume;
        self.right_vol = pan * volume;
    }

    pub fn write_src_addr(&mut self, data: u32) {
        self.src_addr = data & 0x7FF_FFFF;
        self.current_addr = self.src_addr;
    }

    pub fn write_timer(&mut self, data: u16) {
        self.timer = data;
    }

    pub fn write_loop_start(&mut self, data: u16) {
        self.loop_start_pos = data as u32;
    }

    pub fn write_sound_len(&mut self, data: u32) {
        self.sound_len = data & 0x3F_FFFF;
    }
}

impl AudioChannel {
    /// Clock the internal timer, and possibly advance the sample.
    /// 
    /// Returns the DMA mask bit if it needs more samples (PCM only).
    pub fn clock(&mut self, cycles: usize) -> u16 {
        let (new, overflow) = self.timer_counter.overflowing_add(cycles as u16);
        self.timer_counter = new;

        if overflow {
            let psg_noise = self.control.contains(ChannelControl::FORMAT);
            if !psg_noise && self.fifo.len() == 0 {
                return self.dma_mask;
            }
            
            self.timer_counter = self.timer + new;  // TODO: what if this overflows too?
            self.current_frame = self.generate_sample();
            if !psg_noise && self.fifo.len() <= fifo::RELOAD_SIZE {
                self.dma_mask
            } else {
                0
            }
        } else {
            0
        }
    }

    /// Get the current sample, panned and amplified for each output channel.
    pub fn get_sample(&self) -> Option<(i32, i32)> {
        if self.control.contains(ChannelControl::START) {
            Some(self.current_frame)
        } else {
            None
        }
    }

    /// Get the source addr for a DMA transfer.
    pub fn get_dma_addr(&mut self) -> u32 {
        let addr = self.current_addr;
        self.current_addr += 4;
        if self.current_addr >= self.loop_end_addr {
            self.current_addr = self.loop_start_addr;
        }
        addr
    }

    /// Write a word to the FIFO.
    pub fn write_fifo(&mut self, data: u32) {
        self.fifo.push(data);
    }
}

// Internal
impl AudioChannel {
    /// Reset the current sound.
    fn reset(&mut self) {
        self.timer_counter = self.timer;
        self.current_addr = self.src_addr;
        self.loop_start_addr = self.src_addr + (self.loop_start_pos << 2);
        self.loop_end_addr = self.loop_start_addr + (self.sound_len << 2);
        self.sample_count = 0;
        self.sample_len = (self.loop_start_pos + self.sound_len) << 3;
        self.current_sample = 0;
        self.current_frame = (0, 0);
        self.fifo.clear();
        self.adpcm_gen.reset();

        //println!("reset sound: @{:X} | {} + {} | {:X}", self.src_addr, self.loop_start_pos, self.sound_len, self.control);
    }

    /// Generate a new sample, panned and amplified for each output channel.
    /// 
    /// Returns true if DMA is needed.
    fn generate_sample(&mut self) -> (i32, i32) {
        let sample = match (self.control & ChannelControl::FORMAT).bits() >> 29 {
            0b00 => self.get_pcm8(),
            0b01 => self.get_pcm16(),
            0b10 => self.get_adpcm(),
            0b11 => match self.chan_type {
                ChannelType::PSG => self.get_psg(),
                ChannelType::Noise => self.get_noise(),
                ChannelType::PCM => 0,  // TODO: panic?
            },
            _ => unreachable!()
        };

        let vol_shift = match (self.control & ChannelControl::VOL_DIV).bits() >> 8 {
            0b00 => 0,
            0b01 => 1,
            0b10 => 2,
            0b11 => 4,
            _ => unreachable!()
        };
        let sample = sample >> vol_shift;

        let left = (sample * self.left_vol) >> 14;
        let right = (sample * self.right_vol) >> 14;
        (left, right)
    }

    fn get_pcm8(&mut self) -> i32 {
        self.pcm_step(2);
        (self.fifo.sample_pcm_8() as i32) << 8
    }

    fn get_pcm16(&mut self) -> i32 {
        self.pcm_step(4);
        self.fifo.sample_pcm_16() as i32
    }

    fn get_adpcm(&mut self) -> i32 {
        if self.adpcm_gen.needs_header() {
            let header = self.fifo.get_adpcm_header().expect("trying to generate samples without data");
            let sample = self.adpcm_gen.set_header(header) as i32;
            self.pcm_step(8);
            sample
        } else {
            let compressed_sample = self.fifo.sample_adpcm();
            let sample = self.adpcm_gen.generate_sample(compressed_sample) as i32;
            self.pcm_step(1);
            sample
        }
    }

    fn get_psg(&mut self) -> i32 {
        0   // TODO
    }

    fn get_noise(&mut self) -> i32 {
        0   // TODO
    }

    /// Step PCM
    fn pcm_step(&mut self, nybbles: u32) {
        self.sample_count += nybbles;
        if self.sample_count >= self.sample_len {
            match (self.control & ChannelControl::REPEAT).bits() >> 27 {
                0b00 => self.control.remove(ChannelControl::START), // Manual
                0b01 => {   // Repeat
                    self.adpcm_gen.restore_loop_values();
                    self.sample_count = self.loop_start_pos << 3;
                },
                0b10 => self.control.remove(ChannelControl::START), // TODO: one-shot (+hold)
                0b11 => self.control.remove(ChannelControl::START), // Prohibited
                _ => unreachable!()
            }
        } else if self.sample_count == (self.loop_start_pos << 3) {
            self.adpcm_gen.store_loop_values();
        }
    }
}
