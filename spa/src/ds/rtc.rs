/// Real time clock

use bitflags::bitflags;
use crate::utils::{
    meminterface::MemInterface8,
    bits::u8,
};

#[derive(Clone, Copy, Debug)]
enum RTCState {
    Idle,
    Ready,
    TransferCommand,

    // Commands
    StatusReg1,     // 1 byte
    StatusReg2,     // 1 byte
    DateTime(u8),   // 3/7 bytes
    Int1(u8),       // 1/3 bytes
    Int2(u8),       // 3 bytes
    ClockAdjust,    // 1 byte
    Free,           // 1 byte
}

bitflags! {
    #[derive(Default)]
    pub struct Status1: u8 {
        const POWER_OFF = u8::bit(7);
        const POWER_LO  = u8::bit(6);
        const INT_2     = u8::bit(5);
        const INT_1     = u8::bit(4);

        const HOUR_24   = u8::bit(1);
        const RESET     = u8::bit(0);
    }
}

bitflags! {
    #[derive(Default)]
    pub struct Status2: u8 {
        const TEST          = u8::bit(7);
        const INT_2_ENABLE  = u8::bit(6);
        const INT_1_MODE    = u8::bits(0, 3);
    }
}


pub struct RealTimeClock {
    state:      RTCState,
    transfer:   u8, /// How many bits have been transferred in the current state?
    command:    u8,

    status_1:   u8,
    status_2:   u8,

    year:       u8,
    month:      u8,
    day:        u8,
    weekday:    u8,

    hour:       u8,
    minute:     u8,
    second:     u8,

    alarm1_weekday: u8,
    alarm1_hour:    u8,
    alarm1_minute:  u8,

    alarm2_weekday: u8,
    alarm2_hour:    u8,
    alarm2_minute:  u8,

    clock:      u8,
    free:       u8,
}

impl RealTimeClock {
    pub fn new() -> Self {
        Self {
            state:      RTCState::Idle,
            transfer:   0,
            command:    0,

            status_1:   0,
            status_2:   0,

            year:       0,
            month:      0,
            day:        0,
            weekday:    0,

            hour:       0,
            minute:     0,
            second:     0,

            alarm1_weekday: 0,
            alarm1_hour:    0,
            alarm1_minute:  0,

            alarm2_weekday: 0,
            alarm2_hour:    0,
            alarm2_minute:  0,

            clock:      0,
            free:       0,
        }
    }

    /// Advance RTC and return true if interrupt occurred.
    pub fn clock(&mut self, cycles: usize) -> bool {
        false
    }

    fn process_command(&mut self) {
        //println!("got command: {:X}", self.command);
        use RTCState::*;
        // Should be in format 0110 CCC R : R=Read CCC=Command
        self.state = match (self.command >> 1) & 0b111 {
            0 => StatusReg1,
            1 => StatusReg2,
            2 => DateTime(7),
            3 => DateTime(3),
            4 => if u8::test_bit(self.status_2, 2) {Int1(1)} else {Int1(3)},
            5 => Int2(3),
            6 => ClockAdjust,
            7 => Free,
            _ => unreachable!()
        };
        self.transfer = 8;
        //println!("set command: {:?}", self.state);
    }

    fn read_data(&mut self) -> u8 {
        use RTCState::*;
        match self.state {
            StatusReg1 => self.status_1,
            StatusReg2 => self.status_2,
            DateTime(7) => self.year,
            DateTime(6) => self.month,
            DateTime(5) => self.day,
            DateTime(4) => self.weekday,
            DateTime(3) => self.hour,
            DateTime(2) => self.minute,
            DateTime(1) => self.second,
            Int1(3) => self.alarm1_weekday,
            Int1(2) => self.alarm1_hour,
            Int1(1) => self.alarm1_minute,
            Int2(3) => self.alarm2_weekday,
            Int2(2) => self.alarm2_hour,
            Int2(1) => self.alarm2_minute,
            ClockAdjust => self.clock,
            Free => self.free,
            _ => panic!("reading bit from RTC in unsupported state"),
        }
    }

    /// Write a bit to the register specified by state.
    /// Data is written LSB first.
    fn write_bit(&mut self, state: RTCState, data: u8) {
        let bit = (data & 1) << 7;
        use RTCState::*;
        match state {
            StatusReg1 => self.status_1 = (self.status_1 >> 1) | bit,
            StatusReg2 => self.status_2 = (self.status_2 >> 1) | bit,
            DateTime(7) => self.year = (self.year >> 1) | bit,
            DateTime(6) => self.month = (self.month >> 1) | bit,
            DateTime(5) => self.day = (self.day >> 1) | bit,
            DateTime(4) => self.weekday = (self.weekday >> 1) | bit,
            DateTime(3) => self.hour = (self.hour >> 1) | bit,
            DateTime(2) => self.minute = (self.minute >> 1) | bit,
            DateTime(1) => self.second = (self.second >> 1) | bit,
            Int1(3) => self.alarm1_weekday = (self.alarm1_weekday >> 1) | bit,
            Int1(2) => self.alarm1_hour = (self.alarm1_hour >> 1) | bit,
            Int1(1) => self.alarm1_minute = (self.alarm1_minute >> 1) | bit,
            Int2(3) => self.alarm2_weekday = (self.alarm2_weekday >> 1) | bit,
            Int2(2) => self.alarm2_hour = (self.alarm2_hour >> 1) | bit,
            Int2(1) => self.alarm2_minute = (self.alarm2_minute >> 1) | bit,
            ClockAdjust => self.clock = (self.clock >> 1) | bit,
            Free => self.free = (self.free >> 1) | bit,
            _ => panic!("writing bit to RTC in unsupported state"),
        }
    }

    /// Call when finished reading or writing a parameter byte.
    /// Advances the state.
    fn finish_param(&mut self) {
        use RTCState::*;
        self.state = match self.state {
            DateTime(1) => Ready,
            DateTime(n) => DateTime(n-1),
            Int1(1) => Ready,
            Int1(n) => Int1(n-1),
            Int2(1) => Ready,
            Int2(n) => Int2(n-1),
            _ => Ready,
        };
        self.transfer = 8;
    }
}

impl MemInterface8 for RealTimeClock {
    fn read_byte(&mut self, _addr: u32) -> u8 {
        let data = self.read_data();
        // Extract lowest bit.
        self.transfer -= 1;
        let shift = 7 - self.transfer;
        let bit = (data >> shift) & 1;
        if self.transfer == 0 {
            self.finish_param();
        }
        bit
    }

    fn write_byte(&mut self, _addr: u32, data: u8) {
        use RTCState::*;
        match self.state {
            Idle => if !u8::test_bit(data, 2) && u8::test_bit(data, 1) && u8::test_bit(data, 4) { // CS=Low, SCK=High, WRITE
                //println!("READY");
                self.state = Ready;
            },
            Ready => if u8::test_bit(data, 2) && u8::test_bit(data, 1) && u8::test_bit(data, 4) { // CS=High, SCK=High, WRITE
                //println!("begin transfer");
                self.state = TransferCommand;
                self.transfer = 8;
            } else if !u8::test_bit(data, 2) { // CS=Low
                // Command is finished.
                //println!("Done!");
                self.state = Idle;
            },
            TransferCommand => if !u8::test_bit(data, 1) && u8::test_bit(data, 4) { // SCK=Low, WRITE
                let bit = data & 1;
                self.command = (self.command << 1) | bit; // Shift in data bit. MSB first.
                self.transfer -= 1;
                if self.transfer == 0 {
                    self.process_command();
                }
            },
            state => if !u8::test_bit(data, 1) && u8::test_bit(data, 4) { // SCK=Low, WRITE
                self.write_bit(state, data);
                self.transfer -= 1;
                if self.transfer == 0 {
                    self.finish_param();
                }
            },
        }
    }
}
