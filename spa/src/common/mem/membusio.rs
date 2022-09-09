/// IO on the bus.
/// There are a ton of devices that sit on IO so use this macro to construct the functions.
macro_rules! MemoryBusIO {
    {$(($start_addr:expr, $end_addr:expr, $device:ident)),*} => {
        fn io_read_byte(&mut self, addr: u32) -> u8 {
            match addr {
                $($start_addr..=$end_addr => self.$device.read_byte(addr),)*
                _ => 0//panic!("trying to load from unmapped io address ${:08X}", addr),
            }
        }
        fn io_write_byte(&mut self, addr: u32, data: u8) {
            match addr {
                $($start_addr..=$end_addr => self.$device.write_byte(addr, data),)*
                _ => {}//panic!("trying to write to unmapped io address ${:08X}", addr),
            }
        }

        fn io_read_halfword(&mut self, addr: u32) -> u16 {
            match addr {
                $($start_addr..=$end_addr => self.$device.read_halfword(addr),)*
                _ => 0//panic!("trying to load from unmapped io address ${:08X}", addr),
            }
        }
        fn io_write_halfword(&mut self, addr: u32, data: u16) {
            match addr {
                $($start_addr..=$end_addr => self.$device.write_halfword(addr, data),)*
                _ => {}//panic!("trying to write to unmapped io address ${:08X}", addr),
            }
        }

        fn io_read_word(&mut self, addr: u32) -> u32 {
            match addr {
                $($start_addr..=$end_addr => self.$device.read_word(addr),)*
                _ => 0//panic!("trying to load from unmapped io address ${:08X}", addr),
            }
        }
        fn io_write_word(&mut self, addr: u32, data: u32) {
            match addr {
                $($start_addr..=$end_addr => self.$device.write_word(addr, data),)*
                _ => {}//panic!("trying to write to unmapped io address ${:08X}", addr),
            }
        }
    };
}
