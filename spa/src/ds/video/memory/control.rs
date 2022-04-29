
use bitflags::bitflags;
use crate::utils::bits::u8;
use super::VRAMRegion;

bitflags!{
    #[derive(Default)]
    pub struct VRAMControl: u8 {
        const ENABLE    = u8::bit(7);
        const OFFSET    = u8::bits(3, 4);
        const MST       = u8::bits(0, 2);
    }
}

#[derive(PartialEq, Clone, Copy)]
pub enum Slot {
    LCDC(VRAMRegion),
    ARM7(ARM7),
    EngineA(EngineA),
    EngineB(EngineB),
    Texture(Texture)
}

#[derive(PartialEq, Clone, Copy)]
pub enum ARM7 {
    Lo,
    Hi
}

#[derive(PartialEq, Clone, Copy)]
pub enum EngineA {
    Bg0,
    Bg01,
    Bg02,
    Bg03,
    Bg1,
    Bg2,
    Bg3,

    Obj0,
    Obj01,
    Obj02,
    Obj03,
    Obj1,

    BgExtPalette0,
    BgExtPalette2,

    ObjExtPalette,
}

#[derive(PartialEq, Clone, Copy)]
pub enum EngineB {
    Bg0,
    Bg01,

    Obj,

    BgExtPalette,
    ObjExtPalette,
}

#[derive(PartialEq, Clone, Copy)]
pub enum Texture {
    Tex0,
    Tex1,
    Tex2,
    Tex3,

    Palette0,
    Palette1,
    Palette4,
    Palette5,
}

impl VRAMControl {
    /// Get the slot that this region should be mapped to.
    /// 
    /// Region 0-8 corresponds to region A-I.
    pub fn get_slot(self, region: VRAMRegion) -> Slot {
        use VRAMRegion::*;
        match region {
            A | B   => self.slot_ab(region),
            C       => self.slot_c(),
            D       => self.slot_d(),
            E       => self.slot_e(),
            F | G   => self.slot_fg(region),
            H       => self.slot_h(),
            I       => self.slot_i(),
        }
    }

    pub fn slot_ab(self, lcdc: VRAMRegion) -> Slot {
        match (self & VRAMControl::MST).bits() {
            0b01 => Slot::EngineA(match (self & VRAMControl::OFFSET).bits() {
                0b00 => EngineA::Bg0,
                0b01 => EngineA::Bg1,
                0b10 => EngineA::Bg2,
                _    => EngineA::Bg3,
            }),
            0b10 => Slot::EngineA(match (self & VRAMControl::OFFSET).bits() {
                0 => EngineA::Obj0,
                _ => EngineA::Obj1
            }),
            0b11 => Slot::Texture(match (self & VRAMControl::OFFSET).bits() {
                0b00 => Texture::Tex0,
                0b01 => Texture::Tex1,
                0b10 => Texture::Tex2,
                _    => Texture::Tex3,
            }),
            _ => Slot::LCDC(lcdc),
        }
    }

    pub fn slot_c(self) -> Slot {
        match (self & VRAMControl::MST).bits() {
            0b001 => Slot::EngineA(match (self & VRAMControl::OFFSET).bits() {
                0b00 => EngineA::Bg0,
                0b01 => EngineA::Bg1,
                0b10 => EngineA::Bg2,
                _    => EngineA::Bg3,
            }),
            0b010 => Slot::ARM7(match (self & VRAMControl::OFFSET).bits() {
                0 => ARM7::Lo,
                _ => ARM7::Hi
            }),
            0b011 => Slot::Texture(match (self & VRAMControl::OFFSET).bits() {
                0b00 => Texture::Tex0,
                0b01 => Texture::Tex1,
                0b10 => Texture::Tex2,
                _    => Texture::Tex3,
            }),
            0b100 => Slot::EngineB(EngineB::Bg0),
            _ => Slot::LCDC(VRAMRegion::C),
        }
    }

    pub fn slot_d(self) -> Slot {
        match (self & VRAMControl::MST).bits() {
            0b001 => Slot::EngineA(match (self & VRAMControl::OFFSET).bits() {
                0b00 => EngineA::Bg0,
                0b01 => EngineA::Bg1,
                0b10 => EngineA::Bg2,
                _    => EngineA::Bg3,
            }),
            0b010 => Slot::ARM7(match (self & VRAMControl::OFFSET).bits() {
                0 => ARM7::Lo,
                _ => ARM7::Hi
            }),
            0b011 => Slot::Texture(match (self & VRAMControl::OFFSET).bits() {
                0b00 => Texture::Tex0,
                0b01 => Texture::Tex1,
                0b10 => Texture::Tex2,
                _    => Texture::Tex3,
            }),
            0b100 => Slot::EngineB(EngineB::Obj),
            _ => Slot::LCDC(VRAMRegion::D),
        }
    }

    pub fn slot_e(self) -> Slot {
        match (self & VRAMControl::MST).bits() {
            0b001 => Slot::EngineA(EngineA::Bg0),
            0b010 => Slot::EngineA(EngineA::Obj0),
            0b011 => Slot::Texture(Texture::Palette0),
            0b100 => Slot::EngineA(EngineA::BgExtPalette0),
            _ => Slot::LCDC(VRAMRegion::E),
        }
    }

    pub fn slot_fg(self, lcdc: VRAMRegion) -> Slot {
        match (self & VRAMControl::MST).bits() {
            0b001 => Slot::EngineA(match (self & VRAMControl::OFFSET).bits() {
                0b00 => EngineA::Bg0,
                0b01 => EngineA::Bg01,
                0b10 => EngineA::Bg02,
                _    => EngineA::Bg03,
            }),
            0b010 => Slot::EngineA(match (self & VRAMControl::OFFSET).bits() {
                0b00 => EngineA::Obj0,
                0b01 => EngineA::Obj01,
                0b10 => EngineA::Obj02,
                _    => EngineA::Obj03,
            }),
            0b011 => Slot::Texture(match (self & VRAMControl::OFFSET).bits() {
                0b00 => Texture::Palette0,
                0b01 => Texture::Palette1,
                0b10 => Texture::Palette4,
                _    => Texture::Palette5,
            }),
            0b100 => Slot::EngineA(match (self & VRAMControl::OFFSET).bits() {
                0 => EngineA::BgExtPalette0,
                _ => EngineA::BgExtPalette2,
            }),
            0b101 => Slot::EngineA(EngineA::ObjExtPalette),
            _ => Slot::LCDC(lcdc),
        }
    }

    pub fn slot_h(self) -> Slot {
        match (self & VRAMControl::MST).bits() {
            0b01 => Slot::EngineB(EngineB::Bg0),
            0b10 => Slot::EngineB(EngineB::BgExtPalette),
            _ => Slot::LCDC(VRAMRegion::H),
        }
    }

    pub fn slot_i(self) -> Slot {
        match (self & VRAMControl::MST).bits() {
            0b01 => Slot::EngineB(EngineB::Bg01),
            0b10 => Slot::EngineB(EngineB::Obj),
            0b11 => Slot::EngineB(EngineB::ObjExtPalette),
            _ => Slot::LCDC(VRAMRegion::I),
        }
    }
}