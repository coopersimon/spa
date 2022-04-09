/// Common components.

#[macro_use]
pub mod membusio;
pub mod bios;
pub mod dma;
pub mod wram;
pub mod timers;
pub mod framecomms;
#[cfg(feature = "debug")]
pub mod debug;
