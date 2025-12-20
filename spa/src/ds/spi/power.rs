use bitflags::bitflags;
use crate::utils::bits::u8;

bitflags! {
    #[derive(Default)]
    pub struct PowerControl: u8 {
        const DS_SYS_POWER      = u8::bit(6);
        const POWER_LED_ENABLE  = u8::bit(5);
        const POWER_LED_BLINK   = u8::bit(4);
        const UPPER_BACKLIGHT   = u8::bit(3);
        const LOW_BACKLIGHT     = u8::bit(2);
        const SOUND_AMP_MUTE    = u8::bit(1);
        const SOUND_AMP_ENABLE  = u8::bit(0);
    }
}

enum State {
    Idle,
    /// Read from a register
    Read(u8),
    /// Write to a register
    Write(u8)
}

pub struct PowerManager {
    state:      State,
    read_buffer: u8,

    control:        PowerControl,
    mic_amp_enable: bool,
    mic_amp_gain:   u8,
}

impl PowerManager {
    pub fn new() -> Self {
        Self {
            state:          State::Idle,
            read_buffer:    0,
            control:        PowerControl::SOUND_AMP_ENABLE,
            mic_amp_enable: false,
            mic_amp_gain:   0,
        }
    }

    pub fn deselect(&mut self) {
        self.state = State::Idle;
    }

    pub fn read(&mut self) -> u8 {
        self.read_buffer
    }

    pub fn write(&mut self, data: u8) {
        match self.state {
            State::Idle => {
                self.state = if u8::test_bit(data, 7) {
                    State::Read(data & 0x3)
                } else {
                    State::Write(data & 0x3)
                };
            },
            State::Read(n) => {
                // Strobe.
                self.read_buffer = match n {
                    0 => self.control.bits(),
                    1 => 0, // Battery status
                    2 => if self.mic_amp_enable {1} else {0},
                    3 => self.mic_amp_gain,
                    _ => 0
                };
            },
            State::Write(n) => match n {
                0 => self.control = PowerControl::from_bits_truncate(data),
                2 => self.mic_amp_enable = u8::test_bit(data, 0),
                3 => self.mic_amp_gain = data & 0x3,
                _ => {}
            }
        }
    }
}
