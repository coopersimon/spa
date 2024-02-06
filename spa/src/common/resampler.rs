
use crossbeam_channel::Receiver;
use dasp::{
    frame::{Frame, Stereo},
    interpolate::sinc::Sinc,
    ring_buffer::Fixed,
    signal::{
        interpolate::Converter,
        Signal,
    }
};

pub type SamplePacket = Box<[Stereo<f32>]>;

/// Resample from the GBA/NDS rate to the output sample rate.
pub struct Resampler {
    converter:          Converter<Source, Sinc<[Stereo<f32>; 2]>>,
    source_rate_recv:   Option<Receiver<f64>>,
    target_rate:        f64,
}

impl Resampler {
    pub fn new(sample_recv: Receiver<SamplePacket>, source_rate_recv: Option<Receiver<f64>>, source_sample_rate: f64, target_sample_rate: f64) -> Self {
        let sinc = Sinc::new(Fixed::from([Stereo::EQUILIBRIUM; 2]));
        Resampler {
            converter:          Source::new(sample_recv).from_hz_to_hz(sinc, source_sample_rate, target_sample_rate),
            source_rate_recv:   source_rate_recv,
            target_rate:        target_sample_rate,
        }
    }
}

impl Iterator for Resampler {
    type Item = Stereo<f32>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(source_sample_rate) = self.source_rate_recv.as_ref().and_then(|r| r.try_recv().ok()) {
            self.converter.set_hz_to_hz(source_sample_rate, self.target_rate);
        }
        while self.converter.is_exhausted() {
            println!("dropping audio.");
        }
        Some(self.converter.next())
    }
}

// TODO: replace this with an async stream?
struct Source {
    receiver:    Receiver<SamplePacket>,

    current:     SamplePacket,
    n:           usize,
    damp_factor: f32,
}

impl Source {
    fn new(receiver: Receiver<SamplePacket>) -> Self {
        Source {
            receiver:    receiver,

            current:     Box::new([]),
            n:           0,
            damp_factor: 1.0,
        }
    }
}

impl Signal for Source {
    type Frame = Stereo<f32>;

    fn next(&mut self) -> Self::Frame {
        if self.n < self.current.len() {
            let out = self.current[self.n];
            self.n += 1;
            out
        } else {
            if let Ok(result) = self.receiver.try_recv() {
                self.damp_factor = 1.0;
                self.current = result;
                self.n = 1;
                self.current[0]
            } else {
                let out = self.current[self.n - 1];
                self.damp_factor = self.damp_factor - 0.001;
                if self.damp_factor < 0.0 {
                    self.damp_factor = 0.0;
                }
                [out[0] * self.damp_factor, out[1] * self.damp_factor]
            }
        }
    }
}