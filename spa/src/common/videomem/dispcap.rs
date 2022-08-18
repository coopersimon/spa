// Display capture types (for NDS)

use bitflags::bitflags;
use crate::utils::bits::u16;

bitflags! {
    #[derive(Default)]
    pub struct DisplayCaptureLo: u16 {
        const EVB   = u16::bits(8, 12);
        const EVA   = u16::bits(0, 4);
    }
}

impl DisplayCaptureLo {
    fn eva(self) -> u16 {
        (self & DisplayCaptureLo::EVA).bits()
    }
    fn evb(self) -> u16 {
        (self & DisplayCaptureLo::EVB).bits() >> 8
    }
}

bitflags! {
    #[derive(Default)]
    pub struct DisplayCaptureHi: u16 {
        const ENABLE        = u16::bit(15);
        const MODE          = u16::bits(13, 14);
        const READ_OFFSET   = u16::bits(10, 11);
        const SRC_B         = u16::bit(9);
        const SRC_A         = u16::bit(8);
        const WRITE_SIZE    = u16::bits(4, 5);
        const WRITE_OFFSET  = u16::bits(2, 3);
        const VRAM_DEST     = u16::bits(0, 1);
    }
}

impl DisplayCaptureHi {
    pub fn mode(self, disp_capture_lo: DisplayCaptureLo) -> DispCapMode {
        match (self & DisplayCaptureHi::MODE).bits() >> 13 {
            0b00 => DispCapMode::A(self.source_a()),
            0b01 => DispCapMode::B(self.source_b()),
            _ =>    DispCapMode::Blend {
                src_a: self.source_a(),
                src_b: self.source_b(),
                eva: disp_capture_lo.eva(),
                evb: disp_capture_lo.evb()
            }
        }
    }
    fn source_a(&self) -> DispCapSourceA {
        if self.contains(DisplayCaptureHi::SRC_A) {
            DispCapSourceA::_3D
        } else {
            DispCapSourceA::Engine
        }
    }
    fn source_b(&self) -> DispCapSourceB {
        if self.contains(DisplayCaptureHi::SRC_B) {
            DispCapSourceB::MainRAM
        } else {
            DispCapSourceB::VRAM
        }
    }
}

pub enum DispCapSourceA {
    Engine,
    _3D
}

pub enum DispCapSourceB {
    VRAM,
    MainRAM
}

pub enum DispCapMode {
    A(DispCapSourceA),
    B(DispCapSourceB),
    Blend{
        src_a: DispCapSourceA,
        src_b: DispCapSourceB,
        eva: u16,
        evb: u16
    }
}
