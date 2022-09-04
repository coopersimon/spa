
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
    source_rate_recv:   Receiver<f64>,
    target_rate:        f64,
}

impl Resampler {
    pub fn new(sample_recv: Receiver<SamplePacket>, source_rate_recv: Receiver<f64>, source_sample_rate: f64, target_sample_rate: f64) -> Self {
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
        if let Ok(source_sample_rate) = self.source_rate_recv.try_recv() {
            self.converter.set_hz_to_hz(source_sample_rate, self.target_rate);
        }
        while self.converter.is_exhausted() {}
        Some(self.converter.next())
    }
}

// TODO: replace this with an async stream?
struct Source {
    receiver:   Receiver<SamplePacket>,

    current:    SamplePacket,
    n:          usize,
}

impl Source {
    fn new(receiver: Receiver<SamplePacket>) -> Self {
        Source {
            receiver:   receiver,

            current:    Box::new([]),
            n:          0,
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
            self.current = self.receiver.recv().unwrap();
            self.n = 1;
            self.current[0]
        }
    }
}