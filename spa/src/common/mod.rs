/// Common components.

#[macro_use]
pub mod mem;
pub mod peripheral;
pub mod video;
pub mod resampler;

#[cfg(feature = "debug")]
pub mod debug;
