use bitflags::bitflags;
use crate::utils::bits::u8;

bitflags! {
    #[derive(Default)]
    pub struct TSCControl: u8 {
        const START         = u8::bit(7);
        const CHANNEL_SEL   = u8::bits(4, 6);
        const CONV_MODE     = u8::bit(3);
        const REF_SELECT    = u8::bit(2);
        const POWER_MODE    = u8::bits(0, 1);
    }
}

enum Channel {
    Idle,
    Temp0,
    TouchscreenY,
    Battery,
    TouchscreenZ1,
    TouchscreenZ2,
    TouchscreenX,
    AUX,
    Temp1
}

const X_RELEASED: u16 = 0x000;
const Y_RELEASED: u16 = 0xFFF;

pub struct Touchscreen {
    control:    TSCControl,
    channel:    Channel,
    can_read:   bool,
    // All values are 12-bit.
    // This requires 2 byte-sized reads for each value.
    read_lo:    bool,

    x:      u16,
    y:      u16,
    aux:    u16,
}

impl Touchscreen {
    pub fn new() -> Self {
        Self {
            control:    TSCControl::default(),
            channel:    Channel::Temp0,
            can_read:   false,
            read_lo:    true,

            x:          X_RELEASED,
            y:          Y_RELEASED,
            aux:        0,
        }
    }

    /// Write values from touchscreen input.
    /// X and Y should be 0.0 - 1.0
    pub fn write_tsc_values(&mut self, coords: Option<(f64, f64)>) {
        //const X_DIFF: f64 = (0xED0 - 0x100) as f64;
        //const Y_DIFF: f64 = (0xF20 - 0x0B0) as f64;
        if let Some((x, y)) = coords {
            //self.x = ((x * X_DIFF) as u16) + 0x100;
            //self.y = ((y * Y_DIFF) as u16) + 0x0B0;
            self.x = (x * (0xFF0 as f64)) as u16;
            self.y = (y * (0xBF0 as f64)) as u16;
        } else {
            self.x = X_RELEASED;
            self.y = Y_RELEASED;
        }
    }

    /// Write values from microphone input.
    pub fn write_aux_value(&mut self, aux: u16) {
        self.aux = aux;
    }

    pub fn deselect(&mut self) {
        self.channel = Channel::Idle;
        self.can_read = false;
    }

    pub fn read(&mut self) -> u8 {
        use Channel::*;
        if self.can_read {
            match self.channel {
                Idle => 0,
                Temp0 => 0,
                TouchscreenY => self.read_12bit(self.y),
                Battery => 0,
                TouchscreenZ1 => 0,
                TouchscreenZ2 => 0,
                TouchscreenX => self.read_12bit(self.x),
                AUX => self.read_12bit(self.aux),
                Temp1 => 0
            }
        } else {
            0
        }
    }

    pub fn write(&mut self, data: u8) {
        let control = TSCControl::from_bits_truncate(data);
        if control.contains(TSCControl::START) {
            use Channel::*;
            self.control = control;
            self.channel = match (control & TSCControl::CHANNEL_SEL).bits() >> 4 {
                0 => Temp0,
                1 => TouchscreenY,
                2 => Battery,
                3 => TouchscreenZ1,
                4 => TouchscreenZ2,
                5 => TouchscreenX,
                6 => AUX,
                7 => Temp1,
                _ => unreachable!()
            };
            self.read_lo = true;
        }
        self.can_read = true;
    }
}

impl Touchscreen {
    // Read the upper or lower section of a 12-bit value.
    fn read_12bit(&mut self, value: u16) -> u8 {
        if self.read_lo {
            self.read_lo = false;
            (value << 3) as u8
        } else {
            (value >> 5) as u8
        }
    }
}