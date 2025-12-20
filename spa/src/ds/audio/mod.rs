mod channel;
mod capture;

use bitflags::bitflags;
use crossbeam_channel::Sender;
use dasp::frame::Stereo;

use crate::utils::{
    bits::{u16, u32},
    bytes,
    meminterface::MemInterface32
};
use crate::common::resampler::*;
use channel::*;
use capture::*;

bitflags!{
    #[derive(Default)]
    struct SoundControl: u32 {
        const ENABLE    = u32::bit(15);
        const MIX_CH3   = u32::bit(13);
        const MIX_CH1   = u32::bit(12);
        const RIGHT_OUT = u32::bits(10, 11);
        const LEFT_OUT  = u32::bits(8, 9);
        const VOLUME    = u32::bits(0, 6);
    }
}

const SAMPLE_PACKET_SIZE: usize = 32;
 
const CYCLES_PER_SAMPLE: usize = 512;

/// Cycles per second.
const CLOCK_RATE: usize = 0x1FF61FE;
/// Emulated cycles per second.
const REAL_CLOCK_RATE: usize = 6 * 355 * 263 * 60;

/// Base sample rate for audio.
const BASE_SAMPLE_RATE: f64 = (CLOCK_RATE as f64) / (CYCLES_PER_SAMPLE as f64); // ~32_768;

const REAL_SAMPLE_RATE_RATIO: f64 = (REAL_CLOCK_RATE as f64) / (CLOCK_RATE as f64);
pub const REAL_BASE_SAMPLE_RATE: f64 = BASE_SAMPLE_RATE * REAL_SAMPLE_RATE_RATIO;

pub struct DSAudio {
    control:    SoundControl,
    channels:   [AudioChannel; 16],

    bias:       i32,

    capture:        [AudioCaptureUnit; 2],
    mixer_sample:   (i32, i32),

    // Comms with audio thread
    sample_buffer:      Vec<Stereo<f32>>,
    sample_sender:      Option<Sender<SamplePacket>>,

    cycle_count:        usize,
}

impl DSAudio {
    pub fn new() -> Self {
        use ChannelType::*;
        Self {
            control:    SoundControl::default(),
            channels:   [
                AudioChannel::new(PCM, u16::bit(0)), AudioChannel::new(PCM, u16::bit(1)), AudioChannel::new(PCM, u16::bit(2)), AudioChannel::new(PCM, u16::bit(3)),
                AudioChannel::new(PCM, u16::bit(4)), AudioChannel::new(PCM, u16::bit(5)), AudioChannel::new(PCM, u16::bit(6)), AudioChannel::new(PCM, u16::bit(7)),
                AudioChannel::new(PSG, u16::bit(8)), AudioChannel::new(PSG, u16::bit(9)), AudioChannel::new(PSG, u16::bit(10)), AudioChannel::new(PSG, u16::bit(11)),
                AudioChannel::new(PSG, u16::bit(12)), AudioChannel::new(PSG, u16::bit(13)), AudioChannel::new(Noise, u16::bit(14)), AudioChannel::new(Noise, u16::bit(15))
            ],
            bias:   0x200,

            capture:        [AudioCaptureUnit::new(), AudioCaptureUnit::new()],
            mixer_sample:   (0, 0),

            sample_buffer:  Vec::new(),
            sample_sender:  None,

            cycle_count:    0,
        }
    }

    /// Call to enable audio on the appropriate thread.
    /// 
    /// This should be done before any rendering.
    pub fn enable_audio(&mut self, sample_sender: Sender<SamplePacket>) {
        self.sample_sender = Some(sample_sender);
    }

    /// Advance the channels and generate audio samples.
    /// 
    /// Returns a bit array of the channels that requested DMA,
    /// plus two bools indicating the capture units that requested DMA.
    pub fn clock(&mut self, cycles: usize) -> (u16, bool, bool) {   // TODO: clearer return type
        let mut dma_req = 0;
        let mut capture_0 = false;
        let mut capture_1 = false;

        for (i, channel) in self.channels.iter_mut().enumerate() {
            if channel.clock(cycles) {
                dma_req |= channel.advance_sample();
                capture_0 = capture_0 || (i == 1);
                capture_1 = capture_1 || (i == 3);
            }
        }

        self.cycle_count += cycles;
        if self.cycle_count >= CYCLES_PER_SAMPLE {
            self.cycle_count -= CYCLES_PER_SAMPLE;

            // Generate sample
            let sample = self.generate_sample();
            self.sample_buffer.push(sample);
            
            // Output to audio thread
            if self.sample_buffer.len() > SAMPLE_PACKET_SIZE {
                let sample_packet = self.sample_buffer.drain(..).collect::<SamplePacket>();
                if let Some(s) = &self.sample_sender {
                    let _ = s.send(sample_packet);
                }
            }
        }

        let capture_0_dma = capture_0 && self.capture_0();
        let capture_1_dma = capture_1 && self.capture_1();

        (dma_req, capture_0_dma, capture_1_dma)
    }

    /// Get the address to read from for the DMA transfer for a channel.
    pub fn get_dma_addr(&mut self, chan_idx: usize) -> u32 {
        self.channels[chan_idx].get_dma_addr()
    }

    /// Get the address to write to for the DMA transfer for a capture channel.
    pub fn get_capture_dma_addr(&mut self, chan_idx: usize) -> u32 {
        self.capture[chan_idx].get_dma_addr()
    }

    /// Write to a channel's PCM FIFO.
    pub fn write_fifo(&mut self, chan_idx: usize, data: u32) {
        self.channels[chan_idx].write_fifo(data);
    }

    /// Read from a capture channel's PCM FIFO.
    pub fn read_capture_fifo(&mut self, chan_idx: usize) -> u32 {
        self.capture[chan_idx].read_fifo()
    }
}

impl MemInterface32 for DSAudio {
    fn read_word(&mut self, addr: u32) -> u32 {
        match addr {
            0x0400_0400..=0x0400_04FF => {
                let chan_idx = ((addr >> 4) & 0xF) as usize;
                match addr & 0xF {
                    0x0 => self.channels[chan_idx].control.bits(),
                    0x4 => self.channels[chan_idx].src_addr,
                    0x8 => bytes::u32::make(self.channels[chan_idx].loop_start_pos as u16, self.channels[chan_idx].timer),
                    0xC => self.channels[chan_idx].sound_len,
                    _ => unreachable!()
                }
            },

            0x0400_0500 => self.control.bits(),
            0x0400_0504 => self.bias as u32,

            0x0400_0508 => u32::from_le_bytes([
                self.capture[0].control.bits(),
                self.capture[1].control.bits(),
                0, 0
            ]),
            0x0400_050C => 0,
            0x0400_0510 => self.capture[0].dst_addr,
            0x0400_0514 => self.capture[0].len,
            0x0400_0518 => self.capture[1].dst_addr,
            0x0400_051C => self.capture[1].len,

            _ => panic!("reading from invalid sound addr {:X}", addr),
        }
    }

    fn write_word(&mut self, addr: u32, data: u32) {
        match addr {
            0x0400_0400..=0x0400_04FF => {
                let chan_idx = ((addr >> 4) & 0xF) as usize;
                match addr & 0xF {
                    0x0 => self.channels[chan_idx].write_control(data),
                    0x4 => self.channels[chan_idx].write_src_addr(data),
                    0x8 => {
                        self.channels[chan_idx].write_timer(bytes::u32::lo(data));
                        self.channels[chan_idx].write_loop_start(bytes::u32::hi(data));
                    },
                    0xC => self.channels[chan_idx].write_sound_len(data),
                    _ => unreachable!()
                }
                //println!("AUDIO {}: {:X} => {}", chan_idx, data, addr & 0xF);
            },

            0x0400_0500 => self.control = SoundControl::from_bits_truncate(data),
            0x0400_0504 => self.bias = (data & 0x3FF) as i32,

            0x0400_0508 => {
                let bytes = u32::to_le_bytes(data);
                self.capture[0].write_control(bytes[0]);
                self.capture[1].write_control(bytes[1]);
            },
            0x0400_050C => {},
            0x0400_0510 => self.capture[0].write_dest(data),
            0x0400_0514 => self.capture[0].write_len(data),
            0x0400_0518 => self.capture[1].write_dest(data),
            0x0400_051C => self.capture[1].write_len(data),

            _ => panic!("writing to invalid sound addr {:X}", addr),
        }
    }
}

impl DSAudio {
    fn generate_sample(&mut self) -> Stereo<f32> {
        if !self.control.contains(SoundControl::ENABLE) {
            return [0.0, 0.0];
        }

        let mut mixer_output = (0, 0);
        for (idx, sample) in self.channels.iter()
            .enumerate()
            .filter_map(|(i, c)| c.get_panned_sample().map(|s| (i, s)))
        {
            if idx == 1 {
                if !self.control.contains(SoundControl::MIX_CH1) {
                    mixer_output.0 += sample.0;
                    mixer_output.1 += sample.1;
                }
            } else if idx == 3 {
                if !self.control.contains(SoundControl::MIX_CH3) {
                    mixer_output.0 += sample.0;
                    mixer_output.1 += sample.1;
                }
            } else {
                mixer_output.0 += sample.0;
                mixer_output.1 += sample.1;
            }
        }
        self.mixer_sample = (mixer_output.0 >> 4, mixer_output.1 >> 4);

        let left = match (self.control & SoundControl::LEFT_OUT).bits() >> 8 {
            0b00 => self.mixer_sample.0,
            0b01 => self.channels[1].get_panned_sample().unwrap_or_default().0,
            0b10 => self.channels[3].get_panned_sample().unwrap_or_default().0,
            0b11 => (self.channels[3].get_panned_sample().unwrap_or_default().0 + self.channels[1].get_panned_sample().unwrap_or_default().0) >> 1,
            _ => unreachable!()
        };
        let right = match (self.control & SoundControl::RIGHT_OUT).bits() >> 10 {
            0b00 => self.mixer_sample.1,
            0b01 => self.channels[1].get_panned_sample().unwrap_or_default().1,
            0b10 => self.channels[3].get_panned_sample().unwrap_or_default().1,
            0b11 => (self.channels[3].get_panned_sample().unwrap_or_default().1 + self.channels[1].get_panned_sample().unwrap_or_default().1) >> 1,
            _ => unreachable!()
        };

        let master_left = left * ((self.control & SoundControl::VOLUME).bits() as i32);
        let master_right = right * ((self.control & SoundControl::VOLUME).bits() as i32);

        let adjusted_left = ((master_left) >> 9) + self.bias;
        let adjusted_right = ((master_right) >> 9) + self.bias;

        let clipped_left = std::cmp::max(0, std::cmp::min(0x3FF, adjusted_left));
        let clipped_right = std::cmp::max(0, std::cmp::min(0x3FF, adjusted_right));

        [to_output(clipped_left) * 0.25, to_output(clipped_right) * 0.25]
    }

    fn capture_0(&mut self) -> bool {
        if self.capture[0].control.contains(CaptureControl::START) {
            let sample = if self.capture[0].control.contains(CaptureControl::SOURCE) {
                if self.capture[0].control.contains(CaptureControl::ADD) {
                    self.channels[1].get_sample() + self.channels[0].get_sample()
                } else {
                    self.channels[1].get_sample()
                }
            } else {
                self.mixer_sample.0 as i16
            };

            if self.capture[0].control.contains(CaptureControl::FORMAT) {
                self.capture[0].write_fifo_pcm_8((sample >> 8) as i8)
            } else {
                self.capture[0].write_fifo_pcm_16(sample)
            }
        } else {
            false
        }
    }

    fn capture_1(&mut self) -> bool {
        if self.capture[1].control.contains(CaptureControl::START) {
            let sample = if self.capture[1].control.contains(CaptureControl::SOURCE) {
                if self.capture[1].control.contains(CaptureControl::ADD) {
                    self.channels[3].get_sample() + self.channels[2].get_sample()
                } else {
                    self.channels[3].get_sample()
                }
            } else {
                self.mixer_sample.1 as i16
            };

            if self.capture[1].control.contains(CaptureControl::FORMAT) {
                self.capture[1].write_fifo_pcm_8((sample >> 8) as i8)
            } else {
                self.capture[1].write_fifo_pcm_16(sample)
            }
        } else {
            false
        }
    }
}

#[inline]
fn to_output(sample: i32) -> f32 {
    const VOL_MAX_RECIP: f32 = 1.0 / (0x200 as f32);
    ((sample as f32) * VOL_MAX_RECIP) - 1.0
}
