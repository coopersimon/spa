use bitflags::bitflags;
use crate::utils::bits::u16;
use crate::utils::bytes::{self, u32, u64};
use crate::utils::meminterface::MemInterface16;
use crate::common::mem::ram::RAM;

bitflags! {
    #[derive(Default)]
    pub struct WifiIRQFlags: u16 {
        const PRE_BEACON_TIMESLOT       = u16::bit(15);
        const BEACON_TIMESLOT           = u16::bit(14);
        const POST_BEACON_TIMESLOT      = u16::bit(13);
        const MULTIPLAY_CMD_DONE        = u16::bit(12);
        const RF_WAKEUP                 = u16::bit(11);
        const RXBUF_COUNT_EXPIRED       = u16::bit(9);
        const TXBUF_COUNT_EXPIRED       = u16::bit(8);
        const TX_START                  = u16::bit(7);
        const RX_START                  = u16::bit(6);
        const TX_ERROR_HALF_OVERFLOW    = u16::bit(5);
        const RX_EVENT_HALF_OVERFLOW    = u16::bit(4);
        const TX_ERROR_INC              = u16::bit(3);
        const RX_EVENT_INC              = u16::bit(2);
        const TX_COMPLETE               = u16::bit(1);
        const RX_COMPLETE               = u16::bit(0);
    }
}

struct BasebandChip {
    regs: [u8; 256]
}

impl BasebandChip {
    fn new() -> Self {
        Self {
            regs: [0; 256]
        }
    }

    fn read_reg(&self, index: u8) -> u8 {
        match index {
            0x00 => 0x6D, // Chip ID
            0x01..=0x0C |
            0x13..=0x15 |
            0x1B..=0x26 |
            0x28..=0x4C |
            0x4E..=0x5C |
            0x62..=0x63 |
            0x65 |
            0x67..=0x68 => self.regs[index as usize],
            0x5D => 0x1,
            0x64 => 0xFF, // or 0x3F?
            _ => 0,
        }
    }

    fn write_reg(&mut self, index: u8, data: u8) {
        match index {
            0x01..=0x0C |
            0x13..=0x15 |
            0x1B..=0x26 |
            0x28..=0x4C |
            0x4E..=0x5C |
            0x62..=0x63 |
            0x65 |
            0x67..=0x68 => self.regs[index as usize] = data,
            _ => {}
        }
    }
}

struct RFChip {
    regs: [u32; 32]
}

impl RFChip {
    fn new() -> Self {
        Self {
            regs: [0; 32]
        }
    }

    fn read_reg(&self, index: u8) -> u32 {
        self.regs[(index & 0x1F) as usize]
    }

    fn write_reg(&mut self, index: u8, data: u32) {
        self.regs[(index & 0x1F) as usize] = data;
    }
}

pub struct Wifi {
    id: u16,
    tx_master_enable: bool,
    wep_mode: u16,

    interrupt_req: WifiIRQFlags,
    interrupt_enable: WifiIRQFlags,
    interrupt_latch: WifiIRQFlags,

    mac_addr: [u16; 3],
    bssid: [u16; 3],
    aid_low: u16,
    aid_full: u16,
    wep_enable: bool,

    power_us: u16,
    power_tx: u16,
    power_state: u16,
    force_power_state: u16,
    unknown_power: u16,

    rx_control: u16, // TODO: flags?
    rx_filter: u16,
    rx_filter_2: u16,
    rx_fifo_start_addr: u16,
    rx_fifo_end_addr: u16,
    rx_fifo_write_cursor: u16,
    rx_fifo_write_latch: u16,
    rx_fifo_read_addr: u16,
    rx_fifo_read_cursor: u16,
    rx_gap_addr: u16,
    rx_gap_offset: u16,
    rx_buf_count: u16,
    rx_len_crop: u16,

    rx_stats_inc_flags: u16,
    rx_stats_inc_irq: u16,
    rx_stats_half_overflow_flags: u16,
    rx_stats_half_overflow_irq: u16,
    rx_stats: [u8; 16],
    rx_ok_count: u8,
    rx_err_count: u8,

    multiplay_rx_err_count: [u8; 16],

    tx_stat_control: u16,
    tx_req_flags: u16,
    tx_busy: u16,
    tx_stat: u16,
    tx_header_control: u16,
    tx_seq_number: u16,

    tx_fifo_write_addr: u16,
    tx_gap_addr: u16,
    tx_gap_offset: u16,
    tx_beacon: u16,
    tx_cmd: u16,
    tx_loc_1: u16,
    tx_loc_2: u16,
    tx_loc_3: u16,
    tx_tim_loc: u16,
    tx_buf_count: u16,

    tx_retry_limit: u16,
    tx_err_count: u16,

    rf_pins: u16,
    rf_status: u16,
    rxtx_addr: u16,

    counter_control: u16,
    counter: u64,
    counter_compare_control: u16,
    counter_compare: u64,
    beacon_counter: u16,
    post_beacon_counter: u16,
    beacon_interval: u16,
    pre_beacon_time: u16,
    listen_counter: u16,
    listen_interval: u16,
    content_free: u16,

    cmd_count_enable: bool,
    cmd_count: u16,
    cmd_total_time: u16,
    cmd_reply_time: u16,
    tx_buf_reply_1: u16,
    tx_buf_reply_2: u16,

    misc_config: [u16; 19],
    rx_mac_addr: [u16; 3],

    baseband_write: u16,
    baseband_read: u16,
    baseband_serial_busy: bool,
    baseband_mode: u16,
    baseband_power: u16,
    baseband_chip: BasebandChip,

    rf_data_1: u16,
    rf_data_2: u16,
    rf_serial_busy: bool,
    rf_serial_control: u16,
    rf_chip: RFChip,

    preamble_control: u16,

    random_gen: u16,
    random_latch: u16,

    ram: RAM
}

impl Wifi {
    pub fn new() -> Self {
        Self {
            id: 0x1440, // DS
            tx_master_enable: false,
            wep_mode: 0,

            interrupt_req: WifiIRQFlags::from_bits_truncate(0),
            interrupt_enable: WifiIRQFlags::from_bits_truncate(0),
            interrupt_latch: WifiIRQFlags::from_bits_truncate(0),

            mac_addr: [0; 3],
            bssid: [0; 3],
            aid_low: 0,
            aid_full: 0,
            wep_enable: true,

            power_us: 0,
            power_tx: 0,
            power_state: 0,
            force_power_state: 0,
            unknown_power: 0,

            rx_control: 0,
            rx_filter: 0,
            rx_filter_2: 0,
            rx_fifo_start_addr: 0,
            rx_fifo_end_addr: 0,
            rx_fifo_write_cursor: 0,
            rx_fifo_write_latch: 0,
            rx_fifo_read_addr: 0,
            rx_fifo_read_cursor: 0,
            rx_gap_addr: 0,
            rx_gap_offset: 0,
            rx_buf_count: 0,
            rx_len_crop: 0,

            rx_stats_inc_flags: 0,
            rx_stats_inc_irq: 0,
            rx_stats_half_overflow_flags: 0,
            rx_stats_half_overflow_irq: 0,
            rx_stats: [0; 16],
            rx_ok_count: 0,
            rx_err_count: 0,

            multiplay_rx_err_count: [0; 16],

            tx_stat_control: 0,
            tx_req_flags: 0,
            tx_busy: 0,
            tx_stat: 0,
            tx_header_control: 0,
            tx_seq_number: 0,

            tx_fifo_write_addr: 0,
            tx_gap_addr: 0,
            tx_gap_offset: 0,
            tx_beacon: 0,
            tx_cmd: 0,
            tx_loc_1: 0,
            tx_loc_2: 0,
            tx_loc_3: 0,
            tx_tim_loc: 0,
            tx_buf_count: 0,

            tx_retry_limit: 0,
            tx_err_count: 0,

            rf_pins: 0,
            rf_status: 0,
            rxtx_addr: 0,

            counter_control: 0,
            counter: 0,
            counter_compare_control: 0,
            counter_compare: 0,
            beacon_counter: 0,
            post_beacon_counter: 0,
            beacon_interval: 0,
            pre_beacon_time: 0,
            listen_counter: 0,
            listen_interval: 0,
            content_free: 0,

            cmd_count_enable: false,
            cmd_count: 0,
            cmd_total_time: 0,
            cmd_reply_time: 0,
            tx_buf_reply_1: 0,
            tx_buf_reply_2: 0,

            misc_config: [0; 19],
            rx_mac_addr: [0; 3],

            baseband_write: 0,
            baseband_read: 0,
            baseband_serial_busy: false,
            baseband_mode: 0,
            baseband_power: 0,
            baseband_chip: BasebandChip::new(),

            rf_data_1: 0,
            rf_data_2: 0,
            rf_serial_busy: false,
            rf_serial_control: 0,
            rf_chip: RFChip::new(),

            preamble_control: 0,

            random_gen: 0x07FF, // ? start value
            random_latch: 0x07FF, // ? start value

            ram: RAM::new(0x2000)
        }
    }

    pub fn fast_boot(&mut self) {
        self.tx_retry_limit = 0x0707;
        self.power_us = 1;
        self.power_tx = 3;
        self.power_state = 0x200;
        self.rx_fifo_start_addr = 0x4000;
        self.rx_fifo_end_addr = 0x4800;
        self.beacon_interval = 0x64;
        self.tx_req_flags = 0x50;
        self.preamble_control = 1;
        self.rx_filter = 0x401;
        self.rx_filter_2 = 0x8;
        self.cmd_count_enable = true;
        self.counter_compare = 0xFFFF_FFFF_FFFF_FC00;
        self.post_beacon_counter = 0xFFFF;
        self.baseband_mode = 0x100;
        self.baseband_power = 0x800D;
        self.rf_serial_control = 0x18;
        self.rf_pins = 0x4;
        self.rf_status = 0x9;
        self.ram.write_halfword(0x1F70, 0xFFFF);
        self.ram.write_halfword(0x1F72, 0xFFFF);
        self.ram.write_halfword(0x1F76, 0xFFFF);
        self.ram.write_halfword(0x1F7E, 0xFFFF);
        self.misc_config[15] = 0x3F03;
        self.misc_config[18] = 0xFFFF;
    }

    pub fn clock(&mut self, _cycles: usize) -> bool {
        //for _ in 0..cycles {
            let random_rotate = (self.random_gen << 1) | (self.random_gen >> 10);
            self.random_gen = ((self.random_gen & 1) ^ random_rotate) & 0x7FF;
        //}

        self.interrupt_req.insert(self.interrupt_latch);
        let trigger_irq = self.interrupt_latch.intersects(self.interrupt_enable);
        self.interrupt_latch = WifiIRQFlags::empty();
        trigger_irq
    }
}

impl MemInterface16 for Wifi {
    fn read_halfword(&mut self, addr: u32) -> u16 {
        let data = match addr {
            0x0480_4000..=0x0480_5FFF => {
                self.ram.read_halfword(addr - 0x0480_4000)
            },
            
            0x0480_8000 => self.id,
            0x0480_8004 => if self.tx_master_enable {1} else {0},
            0x0480_8006 => self.wep_mode,
            0x0480_8008 => self.tx_stat_control,
            0x0480_8010 => self.interrupt_req.bits(),
            0x0480_8012 => self.interrupt_enable.bits(),

            0x0480_8018 => self.mac_addr[0],
            0x0480_801A => self.mac_addr[1],
            0x0480_801C => self.mac_addr[2],
            0x0480_8020 => self.bssid[0],
            0x0480_8022 => self.bssid[1],
            0x0480_8024 => self.bssid[2],

            0x0480_8028 => self.aid_low,
            0x0480_802A => self.aid_full,
            0x0480_802C => self.tx_retry_limit,
            0x0480_8030 => self.rx_control,
            0x0480_8032 => if self.wep_enable {u16::bit(15)} else {0},

            0x0480_8036 => self.power_us,
            0x0480_8038 => self.power_tx,
            0x0480_803C => self.power_state,
            0x0480_8040 => self.force_power_state,
            0x0480_8044 => self.read_random(),
            0x0480_8048 => self.unknown_power,

            0x0480_8050 => self.rx_fifo_start_addr,
            0x0480_8052 => self.rx_fifo_end_addr,
            0x0480_8054 => self.rx_fifo_write_cursor,
            0x0480_8056 => self.rx_fifo_write_latch,
            0x0480_8058 => self.rx_fifo_read_addr,
            0x0480_805A => self.rx_fifo_read_cursor,
            0x0480_805C => self.rx_buf_count,
            0x0480_8060 => self.read_rx_fifo(),
            0x0480_8062 => self.rx_gap_addr,
            0x0480_8064 => self.rx_gap_offset,

            0x0480_8068 => self.tx_fifo_write_addr,
            0x0480_806C => self.tx_buf_count,
            0x0480_8074 => self.tx_gap_addr,
            0x0480_8076 => self.tx_gap_offset,
            0x0480_8080 => self.tx_beacon,
            0x0480_8084 => self.tx_tim_loc,
            0x0480_8088 => self.listen_counter,
            0x0480_808C => self.beacon_interval,
            0x0480_808E => self.listen_interval,
            0x0480_8090 => self.tx_cmd,
            0x0480_8094 => self.tx_buf_reply_1,
            0x0480_8098 => self.tx_buf_reply_2,
            0x0480_80A0 => self.tx_loc_1,
            0x0480_80A4 => self.tx_loc_2,
            0x0480_80A8 => self.tx_loc_3,

            0x0480_80B0 => self.tx_req_flags,
            0x0480_80B6 => self.tx_busy,
            0x0480_80B8 => self.tx_stat,

            0x0480_80BC => self.preamble_control,
            0x0480_80C0 => self.cmd_total_time,
            0x0480_80C4 => self.cmd_reply_time,

            0x0480_80D0 => self.rx_filter,
            0x0480_80DA => self.rx_len_crop,
            0x0480_80E0 => self.rx_filter_2,

            0x0480_80E8 => self.counter_control,
            0x0480_80EA => self.counter_compare_control,
            0x0480_80EC => self.misc_config[15],
            0x0480_80EE => if self.cmd_count_enable {1} else {0},
            0x0480_80F0 => (self.counter_compare & 0xFFFF) as u16,
            0x0480_80F2 => ((self.counter_compare >> 16) & 0xFFFF) as u16,
            0x0480_80F4 => ((self.counter_compare >> 32) & 0xFFFF) as u16,
            0x0480_80F6 => ((self.counter_compare >> 48) & 0xFFFF) as u16,
            0x0480_80F8 => (self.counter & 0xFFFF) as u16,
            0x0480_80FA => ((self.counter >> 16) & 0xFFFF) as u16,
            0x0480_80FC => ((self.counter >> 32) & 0xFFFF) as u16,
            0x0480_80FE => ((self.counter >> 48) & 0xFFFF) as u16,
            0x0480_810C => self.content_free,
            0x0480_8110 => self.pre_beacon_time,
            0x0480_8118 => self.cmd_count,
            0x0480_811C => self.beacon_counter,
            0x0480_8120 => self.misc_config[0],
            0x0480_8122 => self.misc_config[1],
            0x0480_8124 => self.misc_config[2],
            0x0480_8128 => self.misc_config[3],
            0x0480_8130 => self.misc_config[4],
            0x0480_8132 => self.misc_config[5],
            0x0480_8134 => self.post_beacon_counter,
            0x0480_8140 => self.misc_config[6],
            0x0480_8142 => self.misc_config[7],
            0x0480_8144 => self.misc_config[8],
            0x0480_8146 => self.misc_config[9],
            0x0480_8148 => self.misc_config[10],
            0x0480_814A => self.misc_config[11],
            0x0480_814C => self.misc_config[12],
            0x0480_8150 => self.misc_config[13],
            0x0480_8154 => self.misc_config[14],

            0x0480_815C => self.baseband_read,
            0x0480_815E => if self.baseband_serial_busy {1} else {0},
            0x0480_8160 => self.baseband_mode,
            0x0480_8168 => self.baseband_power,

            0x0480_817C => self.rf_data_2,
            0x0480_817E => self.rf_data_1,
            0x0480_8180 => if self.rf_serial_busy {1} else {0},
            0x0480_8184 => self.rf_serial_control,

            0x0480_8194 => self.tx_header_control,
            0x0480_819C => self.rf_pins,
            0x0480_81A8 => self.rx_stats_inc_flags,
            0x0480_81AA => self.rx_stats_inc_irq,
            0x0480_81AC => self.rx_stats_half_overflow_flags,
            0x0480_81AE => self.rx_stats_half_overflow_irq,
            0x0480_81B0..=0x0480_81BF => {
                let stat_offset = (addr - 0x0480_81B0) as usize;
                bytes::u16::make(self.rx_stats[stat_offset + 1], self.rx_stats[stat_offset])
            },
            0x0480_81C0 => self.tx_err_count,
            0x0480_81C4 => bytes::u16::make(self.rx_err_count, self.rx_ok_count),
            0x0480_81D0..=0x0480_81DF => {
                let offset = (addr - 0x0480_81D0) as usize;
                bytes::u16::make(self.multiplay_rx_err_count[offset + 1], self.multiplay_rx_err_count[offset])
            },
            0x0480_8210 => self.tx_seq_number,
            0x0480_8214 => self.rf_status,
            //0x0480_8254 => self.misc_config[18],
            0x0480_824C => self.rx_mac_addr[0],
            0x0480_824E => self.rx_mac_addr[1],
            0x0480_8250 => self.rx_mac_addr[2],
            0x0480_8268 => self.rxtx_addr,


            /*_ => {
                0
            }*/
            _ => panic!("reading from unknown wifi addr {:X}", addr),
        };
        //println!("wifi read {:X} from {:X}", data, addr);
        data
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {
        //println!("wifi write {:X} => {:X}", data, addr);
        match addr {
            0x0480_4000..=0x0480_5FFF => {
                self.ram.write_halfword(addr - 0x0480_4000, data);
            },
            0x0480_8004 => self.write_reset(data),
            0x0480_8006 => self.wep_mode = data & 0x7F,
            0x0480_8008 => self.tx_stat_control = data,
            0x0480_800A => {}, // Unknown
            0x0480_8010 => self.clear_interrupt_flags(data),
            0x0480_8012 => self.interrupt_enable = WifiIRQFlags::from_bits_truncate(data),

            0x0480_8018 => self.mac_addr[0] = data,
            0x0480_801A => self.mac_addr[1] = data,
            0x0480_801C => self.mac_addr[2] = data,
            0x0480_8020 => self.bssid[0] = data,
            0x0480_8022 => self.bssid[1] = data,
            0x0480_8024 => self.bssid[2] = data,

            0x0480_8028 => self.aid_low = data,
            0x0480_802A => self.aid_full = data,
            0x0480_802C => self.tx_retry_limit = data & 0xFF,
            0x0480_8030 => self.set_rx_control(data),
            0x0480_8032 => self.wep_enable = u16::test_bit(data, 15),

            0x0480_8036 => self.power_us = data,
            0x0480_8038 => self.power_tx = data,
            0x0480_803C => self.power_state = data & 0xFF,
            0x0480_8040 => self.write_force_power_state(data),
            0x0480_8048 => self.unknown_power = data & 0x3,

            0x0480_8050 => self.rx_fifo_start_addr = data,
            0x0480_8052 => self.rx_fifo_end_addr = data,
            0x0480_8054 => {},
            0x0480_8056 => self.rx_fifo_write_latch = data,
            0x0480_8058 => self.rx_fifo_read_addr = data,
            0x0480_805A => self.rx_fifo_read_cursor = data,
            0x0480_805C => self.rx_buf_count = data,
            0x0480_8060 => {},
            0x0480_8062 => self.rx_gap_addr = data,
            0x0480_8064 => self.rx_gap_offset = data,

            0x0480_8068 => self.tx_fifo_write_addr = data,
            0x0480_806C => self.tx_buf_count = data & 0xFFF,
            0x0480_8070 => self.write_tx_fifo(data),
            0x0480_8074 => self.tx_gap_addr = data,
            0x0480_8076 => self.tx_gap_offset = data,
            0x0480_8080 => self.tx_beacon = data,
            0x0480_8084 => self.tx_tim_loc = data,
            0x0480_8088 => self.listen_counter = data,
            0x0480_808C => self.beacon_interval = data,
            0x0480_808E => self.listen_interval = data,
            0x0480_8090 => self.tx_cmd = data,
            0x0480_8094 => self.tx_buf_reply_1 = data,
            0x0480_80A0 => self.tx_loc_1 = data,
            0x0480_80A4 => self.tx_loc_2 = data,
            0x0480_80A8 => self.tx_loc_3 = data,

            0x0480_80AC => self.tx_req_flags &= !(data & 0xF),
            0x0480_80AE => self.tx_req_flags |= data & 0xF,

            0x0480_80B4 => self.tx_reset(data),
            0x0480_80BC => self.preamble_control = data,
            0x0480_80C0 => self.cmd_total_time = data,
            0x0480_80C4 => self.cmd_reply_time = data,

            0x0480_80D0 => self.rx_filter = data,
            0x0480_80D4 => self.misc_config[16] = data,
            0x0480_80D8 => self.misc_config[17] = data,
            0x0480_80DA => self.rx_len_crop = data,
            0x0480_80E0 => self.rx_filter_2 = data,

            0x0480_80E8 => self.counter_control = data,
            0x0480_80EA => self.counter_compare_control = data,
            0x0480_80EC => self.misc_config[15] = data,
            0x0480_80EE => self.cmd_count_enable = u16::test_bit(data, 0),
            0x0480_80F0 => self.counter_compare = u64::set_halfword(self.counter_compare, data, 0),
            0x0480_80F2 => self.counter_compare = u64::set_halfword(self.counter_compare, data, 1),
            0x0480_80F4 => self.counter_compare = u64::set_halfword(self.counter_compare, data, 2),
            0x0480_80F6 => self.counter_compare = u64::set_halfword(self.counter_compare, data, 3),
            0x0480_80F8 => self.counter = u64::set_halfword(self.counter, data, 0),
            0x0480_80FA => self.counter = u64::set_halfword(self.counter, data, 1),
            0x0480_80FC => self.counter = u64::set_halfword(self.counter, data, 2),
            0x0480_80FE => self.counter = u64::set_halfword(self.counter, data, 3),
            0x0480_810C => self.content_free = data,
            0x0480_8110 => self.pre_beacon_time = data,
            0x0480_8118 => self.cmd_count = data,
            0x0480_811C => self.beacon_counter = data,
            0x0480_8120 => self.misc_config[0] = data,
            0x0480_8122 => self.misc_config[1] = data,
            0x0480_8124 => self.misc_config[2] = data,
            0x0480_8128 => self.misc_config[3] = data,
            0x0480_8130 => self.misc_config[4] = data,
            0x0480_8132 => self.misc_config[5] = data,
            0x0480_8134 => self.post_beacon_counter = data,
            0x0480_8140 => self.misc_config[6] = data,
            0x0480_8142 => self.misc_config[7] = data,
            0x0480_8144 => self.misc_config[8] = data,
            0x0480_8146 => self.misc_config[9] = data,
            0x0480_8148 => self.misc_config[10] = data,
            0x0480_814A => self.misc_config[11] = data,
            0x0480_814C => self.misc_config[12] = data,
            0x0480_8150 => self.misc_config[13] = data,
            0x0480_8154 => self.misc_config[14] = data,

            0x0480_8158 => self.baseband_control(data),
            0x0480_815A => self.baseband_write = data,
            0x0480_815E => self.baseband_serial_busy = u16::test_bit(data, 0),
            0x0480_8160 => self.baseband_mode = data & 0x4080,
            0x0480_8168 => self.baseband_power = data & 0x800F,

            0x0480_817C => self.rf_data_command(data),
            0x0480_817E => self.rf_data_1 = data,
            0x0480_8184 => self.rf_serial_control = data & 0x413F,

            0x0480_8194 => self.tx_header_control = data,
            0x0480_81A0 => {}, // unknown
            0x0480_81A2 => {}, // unknown
            0x0480_81AA => self.rx_stats_inc_irq = data,
            0x0480_81AE => self.rx_stats_half_overflow_irq = data,

            0x0480_821C => self.interrupt_latch = WifiIRQFlags::from_bits_truncate(data),
            0x0480_824C => self.rx_mac_addr[0] = data,
            0x0480_824E => self.rx_mac_addr[1] = data,
            0x0480_8250 => self.rx_mac_addr[2] = data,
            0x0480_8254 => self.misc_config[18] = data,
            0x0480_8290 => {}, // wired/wireless switch

            _ => panic!("writing to unknown wifi addr {:X}", addr),
        }
    }
}

impl Wifi {
    fn write_reset(&mut self, data: u16) {
        self.tx_master_enable = u16::test_bit(data, 0);
        // TODO...
    }

    fn clear_interrupt_flags(&mut self, data: u16) {
        let flags_to_clear = WifiIRQFlags::from_bits_truncate(data);
        self.interrupt_req.remove(flags_to_clear);
        // TODO: half overflow flags
    }

    fn read_random(&mut self) -> u16 {
        let data = self.random_latch;
        self.random_latch = self.random_gen;
        data
    }

    fn write_force_power_state(&mut self, data: u16) {
        self.force_power_state = data;
        if u16::test_bit(data, 15) {
            self.power_state = (data & 0x1) << 9;
        }
    }

    fn set_rx_control(&mut self, data: u16) {
        if u16::test_bit(data, 0) {
            self.rx_fifo_write_cursor = self.rx_fifo_write_latch;
        }
        self.rx_control = data;
    }

    fn read_rx_fifo(&mut self) -> u16 {
        let data = self.ram.read_halfword((self.rx_fifo_read_addr & 0x1FFE) as u32);
        self.rx_fifo_read_addr = self.rx_fifo_read_addr + 2;
        if self.rx_fifo_read_addr == self.rx_fifo_end_addr {
            self.rx_fifo_read_addr = self.rx_fifo_start_addr;
        }
        if self.rx_fifo_read_addr == self.rx_gap_addr {
            let gap_size = self.rx_gap_offset * 2;
            self.rx_fifo_read_addr = self.rx_fifo_read_addr + gap_size;
        }
        if self.rx_buf_count == 1 {
            self.rx_buf_count = 0;
            self.interrupt_latch.insert(WifiIRQFlags::RXBUF_COUNT_EXPIRED);
        }
        else if self.rx_buf_count > 0 {
            self.rx_buf_count -= 1;
        }
        data
    }

    fn write_tx_fifo(&mut self, data: u16) {
        self.ram.write_halfword((self.tx_fifo_write_addr & 0x1FFE) as u32, data);
        self.tx_fifo_write_addr = self.tx_fifo_write_addr + 2;
        if self.tx_fifo_write_addr == self.tx_gap_addr {
            let gap_size = self.tx_gap_offset * 2;
            self.tx_fifo_write_addr = self.tx_fifo_write_addr + gap_size;
        }
        if self.tx_buf_count == 1 {
            self.tx_buf_count = 0;
            self.interrupt_latch.insert(WifiIRQFlags::TXBUF_COUNT_EXPIRED);
        }
        else if self.tx_buf_count > 0 {
            self.tx_buf_count -= 1;
        }
    }

    fn tx_reset(&mut self, _data: u16) {
        // TODO...
    }

    fn baseband_control(&mut self, data: u16) {
        let index = (data & 0xFF) as u8;
        let direction = data >> 12;
        if direction == 0x5 {
            self.baseband_chip.write_reg(index, self.baseband_write as u8);
        } else if direction == 0x6 {
            self.baseband_read = self.baseband_chip.read_reg(index) as u16;
        }
    }

    fn rf_data_command(&mut self, data: u16) {
        self.rf_data_2 = data;
        if u16::test_bit(self.rf_serial_control, 8) {
            panic!("RF type 3 unsupported");
        } else {
            let index = ((self.rf_data_2 >> 2) & 0x1F) as u8;
            if u16::test_bit(self.rf_data_2, 7) {
                // read
                let rf_data = self.rf_chip.read_reg(index);
                self.rf_data_2 = (self.rf_data_2 & 0xFFFC) | (u32::hi(rf_data) & 0x3);
                self.rf_data_1 = u32::lo(rf_data);
            } else {
                // write
                let rf_data = u32::make(self.rf_data_2 & 0x3, self.rf_data_1);
                self.rf_chip.write_reg(index, rf_data);
            }
        }
    }
}