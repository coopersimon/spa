/// Real time clock

use bitflags::bitflags;
use chrono::{
    Datelike, Timelike, Local
};
use crate::utils::{
    meminterface::MemInterface8,
    bits::u8, bcd::Bcd8,
};

#[derive(Clone, Copy, PartialEq, Debug)]
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
    write_buf:  u8,
    command:    u8,
    read:       bool,

    status_1:   u8,
    status_2:   u8,

    year:       Bcd8,
    month:      Bcd8,
    day:        Bcd8,
    weekday:    Bcd8,

    hour:       Bcd8,
    minute:     Bcd8,
    second:     Bcd8,

    alarm1_weekday: Bcd8,
    alarm1_hour:    Bcd8,
    alarm1_minute:  Bcd8,

    alarm2_weekday: Bcd8,
    alarm2_hour:    Bcd8,
    alarm2_minute:  Bcd8,

    clock:      u8,
    free:       u8,
}

impl RealTimeClock {
    pub fn new() -> Self {
        let now = Local::now();
        let year = now.year() % 100;
        let weekday = now.weekday().num_days_from_sunday(); // Appears to use Monday = 1, ...
        let rtc = Self {
            state:      RTCState::Idle,
            transfer:   0,
            write_buf:  0,
            command:    0,
            read:       false,

            status_1:   0,
            status_2:   0,

            year:       Bcd8::from_binary(year as u8),
            month:      Bcd8::from_binary(now.month() as u8),
            day:        Bcd8::from_binary(now.day() as u8),
            weekday:    Bcd8::from_binary(weekday as u8),

            hour:       Bcd8::from_binary(now.hour() as u8),
            minute:     Bcd8::from_binary(now.minute() as u8),
            second:     Bcd8::from_binary(now.second() as u8),

            alarm1_weekday: Bcd8::default(),
            alarm1_hour:    Bcd8::default(),
            alarm1_minute:  Bcd8::default(),

            alarm2_weekday: Bcd8::default(),
            alarm2_hour:    Bcd8::default(),
            alarm2_minute:  Bcd8::default(),

            clock:      0,
            free:       0,
        };

        // TODO: Log
        //println!("RTC init: Y {:X} M {:X} D {:X} W {:X}", rtc.year.binary(), rtc.month.binary(), rtc.day.binary(), rtc.weekday.binary());
        //println!("RTC init: h {:X} m {:X} s {:X}", rtc.hour.binary(), rtc.minute.binary(), rtc.second.binary());

        rtc
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
            DateTime(7) => self.year.binary(),
            DateTime(6) => self.month.binary(),
            DateTime(5) => self.day.binary(),
            DateTime(4) => self.weekday.binary(),
            DateTime(3) => self.hour.binary(),
            DateTime(2) => self.minute.binary(),
            DateTime(1) => self.second.binary(),
            Int1(3) => self.alarm1_weekday.binary(),
            Int1(2) => self.alarm1_hour.binary(),
            Int1(1) => self.alarm1_minute.binary(),
            Int2(3) => self.alarm2_weekday.binary(),
            Int2(2) => self.alarm2_hour.binary(),
            Int2(1) => self.alarm2_minute.binary(),
            ClockAdjust => self.clock,
            Free => self.free,
            _ => panic!("reading bit from RTC in unsupported state {:?}", self.state),
        }
    }

    /// Write a bit to the register specified by state.
    /// Data is written LSB first.
    /// 
    /// Data is written into a buffer and needs to be
    /// written back on completion.
    fn write_bit(&mut self, data: u8) {
        let bit = (data & 1) << 7;
        self.write_buf = (self.write_buf >> 1) | bit;
    }

    /// Write back
    fn writeback_buffer(&mut self, state: RTCState) {
        use RTCState::*;
        match state {
            StatusReg1 => self.status_1 = self.write_buf,
            StatusReg2 => self.status_2 = self.write_buf,
            DateTime(7) => self.year = Bcd8::from_bcd(self.write_buf),
            DateTime(6) => self.month = Bcd8::from_bcd(self.write_buf),
            DateTime(5) => self.day = Bcd8::from_bcd(self.write_buf),
            DateTime(4) => self.weekday = Bcd8::from_bcd(self.write_buf),
            DateTime(3) => self.hour = Bcd8::from_bcd(self.write_buf),
            DateTime(2) => self.minute = Bcd8::from_bcd(self.write_buf),
            DateTime(1) => self.second = Bcd8::from_bcd(self.write_buf),
            Int1(3) => self.alarm1_weekday = Bcd8::from_bcd(self.write_buf),
            Int1(2) => self.alarm1_hour = Bcd8::from_bcd(self.write_buf),
            Int1(1) => self.alarm1_minute = Bcd8::from_bcd(self.write_buf),
            Int2(3) => self.alarm2_weekday = Bcd8::from_bcd(self.write_buf),
            Int2(2) => self.alarm2_hour = Bcd8::from_bcd(self.write_buf),
            Int2(1) => self.alarm2_minute = Bcd8::from_bcd(self.write_buf),
            ClockAdjust => self.clock = self.write_buf,
            Free => self.free = self.write_buf,
            _ => panic!("writing bit to RTC in unsupported state"),
        }
        self.write_buf = 0;
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
        let can_read = std::mem::take(&mut self.read);
        if !can_read || self.state == RTCState::Idle {
            return 0;
        }
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
            Idle if !u8::test_bit(data, 2) && u8::test_bit(data, 1) && u8::test_bit(data, 4) => { // CS=Low, SCK=High, WRITE
                //println!("READY");
                self.state = Ready;
            },
            Ready if !u8::test_bit(data, 2) && u8::test_bit(data, 1) && u8::test_bit(data, 4) => { // CS=Low, SCK=High, WRITE
                // Go straight to next command without idle
                //println!("Done + READY");
            },
            Ready if u8::test_bit(data, 2) && u8::test_bit(data, 1) && u8::test_bit(data, 4) => { // CS=High, SCK=High, WRITE
                //println!("begin transfer");
                self.state = TransferCommand;
                self.transfer = 8;
            },
            Ready if !u8::test_bit(data, 2) => { // CS=Low
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
            state if !u8::test_bit(data, 1) && u8::test_bit(data, 4) => { // SCK=Low, WRITE
                self.write_bit(data);
                self.transfer -= 1;
                if self.transfer == 0 {
                    self.writeback_buffer(state);
                    self.finish_param();
                }
            },
            _ if !u8::test_bit(data, 1) && !u8::test_bit(data, 4) => {  // SCK=Low, READ
                self.read = true;
            }
            _ => ()
        }
    }

    fn read_halfword(&mut self, addr: u32) -> u16 {
        self.read_byte(addr) as u16
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {
        self.write_byte(addr, data as u8);
    }
}
