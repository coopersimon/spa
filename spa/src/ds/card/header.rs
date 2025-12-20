
/// The header of the DS card.
/// 
/// For internal usage.
pub struct CardHeader(Vec<u8>);

impl CardHeader {
    pub fn new(data: Vec<u8>) -> Self {
        Self(data)
    }

    pub fn get_u32(&self, at: usize) -> u32 {
        u32::from_le_bytes(self.0[at..(at + 4)].try_into().unwrap())
    }

    pub fn as_slice<'a>(&'a self) -> &'a [u8] {
        &self.0
    }

    /// Header-defined game name.
    pub fn game_name(&self) -> String {
        String::from_utf8(self.0[0..0xC].to_vec()).unwrap()
    }

    /// Offset of icon/title segment.
    pub fn icon_title_offset(&self) -> u32 {
        self.get_u32(0x68)
    }

    /// Where to load the initial ARM9 code from (ROM addr).
    pub fn arm9_rom_offset(&self) -> u32 {
        self.get_u32(0x20)
    }

    /// Where to start executing the ARM9.
    pub fn arm9_entry_addr(&self) -> u32 {
        self.get_u32(0x24)
    }

    /// Where to load the initial ARM9 code to (RAM/bus addr).
    pub fn arm9_ram_addr(&self) -> u32 {
        self.get_u32(0x28)
    }

    /// Number of bytes to load from card to RAM.
    pub fn arm9_size(&self) -> u32 {
        self.get_u32(0x2C)
    }

    /// Where to load the initial ARM7 code from (ROM addr).
    pub fn arm7_rom_offset(&self) -> u32 {
        self.get_u32(0x30)
    }

    /// Where to start executing the ARM7.
    pub fn arm7_entry_addr(&self) -> u32 {
        self.get_u32(0x34)
    }

    /// Where to load the initial ARM7 code to (RAM/bus addr).
    pub fn arm7_ram_addr(&self) -> u32 {
        self.get_u32(0x38)
    }

    /// Number of bytes to load from card to RAM.
    pub fn arm7_size(&self) -> u32 {
        self.get_u32(0x3C)
    }

    /// Setting for card read.
    pub fn rom_ctrl(&self) -> u32 {
        self.get_u32(0x60)
    }
}