/// Serial peripheral interface

mod firmware;

use bitflags::bitflags;
use crate::utils::{
    meminterface::MemInterface16,
    bits::u16,
    bytes
};

use firmware::Firmware;

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

    firmware: Firmware,
}

impl SPI {
    pub fn new(firmware_path: Option<&std::path::Path>) -> Self {
        Self {
            control:    SPIControl::default(),

            firmware:   Firmware::new(firmware_path).unwrap(),
        }
    }
}

impl MemInterface16 for SPI {
    fn read_halfword(&mut self, addr: u32) -> u16 {
        match addr {
            0 => self.control.bits(),
            2 => match (self.control & SPIControl::DEVICE).bits() >> 8 {
                0 => 0, // TODO: power manager
                1 => {
                    let data = self.firmware.read();
                    if !self.control.contains(SPIControl::CHIP_HOLD) {
                        self.firmware.deselect();
                    }
                    bytes::u16::make(0, data)
                },
                2 => 0, // TODO: touchscreen
                3 => 0, // Reserved
                _ => unreachable!()
            },
            _ => unreachable!()
        }
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {
        match addr {
            0 => self.control = SPIControl::from_bits_truncate(data),
            2 => match (self.control & SPIControl::DEVICE).bits() >> 8 {
                0 => {}, // TODO: power manager
                1 => self.firmware.write(bytes::u16::lo(data)),
                2 => {}, // TODO: touchscreen
                3 => {}, // Reserved
                _ => unreachable!()
            },
            _ => unreachable!()
        }
    }
}