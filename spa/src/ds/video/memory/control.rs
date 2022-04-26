
use bitflags::bitflags;
use crate::utils::bits::u8;

bitflags!{
    #[derive(Default)]
    pub struct VRAMControl: u8 {
        const ENABLE    = u8::bit(7);
        const OFFSET    = u8::bits(3, 4);
        const MST       = u8::bits(0, 2);
    }
}

pub enum Slot {
    LCDC(LCDC),
    ARM7(ARM7),
    EngineA(EngineA),
    EngineB(EngineB),
    Texture(Texture)
}

pub enum LCDC {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I
}

pub enum ARM7 {
    LO,
    HI
}

pub enum EngineA {
    BG_0,
    BG_01,
    BG_02,
    BG_03,
    BG_1,
    BG_2,
    BG_3,

    OBJ_0,
    OBJ_01,
    OBJ_02,
    OBJ_03,
    OBJ_1,

    BG_EXT_PALETTE_0,
    BG_EXT_PALETTE_2,

    OBJ_EXT_PALETTE,
}

pub enum EngineB {
    BG_0,
    BG_01,

    OBJ,

    BG_EXT_PALETTE,
    OBJ_EXT_PALETTE,
}

pub enum Texture {
    TEX_0,
    TEX_1,
    TEX_2,
    TEX_3,

    PALETTE_0,
    PALETTE_1,
    PALETTE_4,
    PALETTE_5,
}

impl VRAMControl {
    pub fn slot_ab(self) -> Slot {
        match (self & VRAMControl::MST).bits() {
            0b01 => Slot::EngineA(match (self & VRAMControl::OFFSET).bits() {
                0b00 => EngineA::BG_0,
                0b01 => EngineA::BG_1,
                0b10 => EngineA::BG_2,
                _    => EngineA::BG_3,
            }),
            0b10 => Slot::EngineA(match (self & VRAMControl::OFFSET).bits() {
                0 => EngineA::OBJ_0,
                _ => EngineA::OBJ_1
            }),
            0b11 => Slot::Texture(match (self & VRAMControl::OFFSET).bits() {
                0b00 => Texture::TEX_0,
                0b01 => Texture::TEX_1,
                0b10 => Texture::TEX_2,
                _    => Texture::TEX_3,
            }),
            _ => Slot::LCDC(LCDC::A),
        }
    }

    pub fn slot_cd(self) -> Slot {
        match (self & VRAMControl::MST).bits() {
            0b001 => Slot::EngineA(match (self & VRAMControl::OFFSET).bits() {
                0b00 => EngineA::BG_0,
                0b01 => EngineA::BG_1,
                0b10 => EngineA::BG_2,
                _    => EngineA::BG_3,
            }),
            0b010 => Slot::ARM7(match (self & VRAMControl::OFFSET).bits() {
                0 => ARM7::LO,
                _ => ARM7::HI
            }),
            0b011 => Slot::Texture(match (self & VRAMControl::OFFSET).bits() {
                0b00 => Texture::TEX_0,
                0b01 => Texture::TEX_1,
                0b10 => Texture::TEX_2,
                _    => Texture::TEX_3,
            }),
            0b100 => Slot::EngineB(EngineB::BG_0),  // TODO: D=OBJ
            _ => Slot::LCDC(LCDC::C),
        }
    }

    pub fn slot_e(self) -> Slot {
        match (self & VRAMControl::MST).bits() {
            0b001 => Slot::EngineA(EngineA::BG_0),
            0b010 => Slot::EngineA(EngineA::OBJ_0),
            0b011 => Slot::Texture(Texture::PALETTE_0),
            0b100 => Slot::EngineA(EngineA::BG_EXT_PALETTE_0),
            _ => Slot::LCDC(LCDC::E),
        }
    }

    pub fn slot_fg(self) -> Slot {
        match (self & VRAMControl::MST).bits() {
            0b001 => Slot::EngineA(match (self & VRAMControl::OFFSET).bits() {
                0b00 => EngineA::BG_0,
                0b01 => EngineA::BG_01,
                0b10 => EngineA::BG_02,
                _    => EngineA::BG_03,
            }),
            0b010 => Slot::EngineA(match (self & VRAMControl::OFFSET).bits() {
                0b00 => EngineA::OBJ_0,
                0b01 => EngineA::OBJ_01,
                0b10 => EngineA::OBJ_02,
                _    => EngineA::OBJ_03,
            }),
            0b011 => Slot::Texture(match (self & VRAMControl::OFFSET).bits() {
                0b00 => Texture::PALETTE_0,
                0b01 => Texture::PALETTE_1,
                0b10 => Texture::PALETTE_4,
                _    => Texture::PALETTE_5,
            }),
            0b100 => Slot::EngineA(match (self & VRAMControl::OFFSET).bits() {
                0 => EngineA::BG_EXT_PALETTE_0,
                _ => EngineA::BG_EXT_PALETTE_2,
            }),
            0b101 => Slot::EngineA(EngineA::OBJ_EXT_PALETTE),
            _ => Slot::LCDC(LCDC::F),
        }
    }

    pub fn slot_h(self) -> Slot {
        match (self & VRAMControl::MST).bits() {
            0b01 => Slot::EngineB(EngineB::BG_0),
            0b10 => Slot::EngineB(EngineB::BG_EXT_PALETTE),
            _ => Slot::LCDC(LCDC::H),
        }
    }

    pub fn slot_i(self) -> Slot {
        match (self & VRAMControl::MST).bits() {
            0b01 => Slot::EngineB(EngineB::BG_01),
            0b10 => Slot::EngineB(EngineB::OBJ),
            0b11 => Slot::EngineB(EngineB::OBJ_EXT_PALETTE),
            _ => Slot::LCDC(LCDC::I),
        }
    }
}