/// Serial peripheral interface

mod power;
mod firmware;
mod touchscreen;

use bitflags::bitflags;
use crate::utils::{
    meminterface::MemInterface16,
    bits::u16,
    bytes
};

use power::PowerManager;
use firmware::Firmware;
use touchscreen::Touchscreen;

bitflags!{
    #[derive(Default)]
    pub struct SPIControl: u16 {
        const ENABLE        = u16::bit(15);
        const INT_REQ       = u16::bit(14);
        const CHIP_HOLD     = u16::bit(11);
        const TRANSFER_SIZE = u16::bit(10);
        const DEVICE        = u16::bits(8, 9);
        const BUSY          = u16::bit(7);
        const BAUDRATE      = u16::bits(0, 1);
    }
}

pub struct SPI {
    control: SPIControl,

    power_man:      PowerManager,
    firmware:       Firmware,
    touchscreen:    Touchscreen,
}

impl SPI {
    pub fn new(firmware_path: Option<&std::path::Path>) -> Self {
        Self {
            control:    SPIControl::default(),

            power_man:      PowerManager::new(),
            firmware:       Firmware::new(firmware_path).unwrap(),
            touchscreen:    Touchscreen::new(),
        }
    }
}

impl MemInterface16 for SPI {
    fn read_halfword(&mut self, addr: u32) -> u16 {
        match addr {
            0 => self.control.bits(),
            2 => match (self.control & SPIControl::DEVICE).bits() >> 8 {
                0 => {
                    let data = self.power_man.read();
                    if !self.control.contains(SPIControl::CHIP_HOLD) {
                        self.power_man.deselect();
                    }
                    bytes::u16::make(0, data)
                },
                1 => {
                    let data = self.firmware.read();
                    if !self.control.contains(SPIControl::CHIP_HOLD) {
                        self.firmware.deselect();
                    }
                    bytes::u16::make(0, data)
                },
                2 => {
                    let data = self.touchscreen.read();
                    if !self.control.contains(SPIControl::CHIP_HOLD) {
                        self.touchscreen.deselect();
                    }
                    bytes::u16::make(0, data)
                },
                3 => 0, // Reserved
                x => unreachable!(),
            },
            _ => unreachable!()
        }
    }

    fn read_word(&mut self, addr: u32) -> u32 {
        match addr {
            0 => self.control.bits() as u32,
            _ => unreachable!()
        }
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {
        match addr {
            0 => self.control = SPIControl::from_bits_truncate(data),
            2 => match (self.control & SPIControl::DEVICE).bits() >> 8 {
                0 => self.power_man.write(bytes::u16::lo(data)),
                1 => self.firmware.write(bytes::u16::lo(data)),
                2 => self.touchscreen.write(bytes::u16::lo(data)),
                3 => {}, // Reserved
                x => unreachable!(),
            },
            _ => unreachable!()
        }
    }
}