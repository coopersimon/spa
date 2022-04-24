/// Common components.

#[macro_use]
pub mod membusio;
pub mod bios;

pub mod dma;
pub mod wram;

pub mod timers;
pub mod joypad;

pub mod framecomms;
pub mod videomem;
pub mod drawing;

#[cfg(feature = "debug")]
pub mod debug;
