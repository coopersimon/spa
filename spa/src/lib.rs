#[macro_use]
mod common;
mod utils;

pub mod gba;
pub mod ds;

pub type FrameBuffer = Box<[u8]>;
