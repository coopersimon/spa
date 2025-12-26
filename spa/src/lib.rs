#[macro_use]
mod common;
mod utils;

pub mod gba;
pub mod ds;

use crate::common::resampler::Resampler;

pub enum Button {
    A,
    B,
    X,
    Y,
    Start,
    Select,
    Left,
    Right,
    Up,
    Down,
    L,
    R
}

#[derive(Clone, Copy)]
pub struct Coords<T> {
    pub x: T,
    pub y: T
}

/// Represents a GBA or NDS.
/// 
/// The public interface.
pub trait Device {
    /// Drives the emulator and returns a pair of frames.
    /// 
    /// This should be called at 60fps.
    /// The frames are in the format R8G8B8A8.
    /// 
    /// The lower frame will contain no data, if this device is a GBA.
    fn frame(&mut self, upper_frame: &mut [u8], lower_frame: &mut [u8]);

    /// Returns the render size of each screen.
    /// 
    /// GBA only has one screen, so the second screen will be (0, 0)
    fn render_size(&self) -> [Coords<usize>; 2];

    fn set_button(&mut self, button: Button, pressed: bool);

    /// Call with Some((x, y)) when the touchscreen is pressed.
    /// Coordinates should be between 0.0 and 1.0.
    /// 
    /// Call with None when the touchscreen is released.
    /// 
    /// Has no effect on GBA.
    fn touchscreen_pressed(&mut self, coords: Option<Coords<f64>>);

    /// Call this at the start to enable audio.
    /// It creates a AudioHandler that can be sent to the audio thread.
    fn enable_audio(&mut self, sample_rate: f64) -> Option<AudioHandler>;

    fn trigger_debug(&mut self) {}
}

/// Created by a Device.
pub struct AudioHandler {
    resampler:    Resampler,
}

impl AudioHandler {
    /// Fill the provided buffer with samples.
    /// The format is PCM interleaved stereo.
    pub fn get_audio_packet(&mut self, buffer: &mut [f32]) {
        for (o_frame, i_frame) in buffer.chunks_exact_mut(2).zip(&mut self.resampler) {
            o_frame.copy_from_slice(&i_frame);
        }
    }
}

pub type FrameBuffer = Box<[u8]>;
#[cfg(feature = "debug")]
pub use common::debug::DebugInterface;