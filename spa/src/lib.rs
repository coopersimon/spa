#[macro_use]
mod common;
mod utils;

pub mod gba;
pub mod ds;

pub type FrameBuffer = Box<[u8]>;
#[cfg(feature = "debug")]
pub use common::debug::DebugInterface;