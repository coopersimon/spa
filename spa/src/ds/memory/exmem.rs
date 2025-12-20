// External memory control

use bitflags::bitflags;
use crate::utils::{
    bits::u16,
    meminterface::MemInterface16
};
use std::sync::{
    Arc,
    atomic::{AtomicU16, Ordering}
};

bitflags! {
    #[derive(Default)]
    pub struct GBAAccess: u16 {
        const PHI_PIN_OUT   = u16::bits(5, 6);
        const WAIT_S        = u16::bit(4);
        const WAIT_N        = u16::bits(2, 3);
        const SRAM_WAIT     = u16::bits(0, 1);
    }
}

bitflags! {
    #[derive(Default)]
    pub struct AccessRights: u16 {
        const MAIN_RAM_PRIO = u16::bit(15);
        const MAIN_MEM_SYNC = u16::bit(14);
        const NDS_SET       = u16::bit(13);
        const NDS_CARD      = u16::bit(11);
        const GBA_CART      = u16::bit(7);
    }
}



/// Used in ARM9.
pub struct ExMemControl {
    gba_access:     GBAAccess,
    access_rights:  Arc<AtomicU16>,
}

impl ExMemControl {
    pub fn new() -> (Self, ExMemStatus) {
        let access_rights = Arc::new(AtomicU16::new(0));
        (Self {
            gba_access:     GBAAccess::default(),
            access_rights:  access_rights.clone()
        }, ExMemStatus {
            gba_access:     GBAAccess::default(),
            access_rights:  access_rights
        })
    }

    pub fn has_gba_access(&self) -> bool {
        let access = AccessRights::from_bits_truncate(self.access_rights.load(Ordering::Acquire));
        !access.contains(AccessRights::GBA_CART)
    }

    pub fn has_nds_access(&self) -> bool {
        let access = AccessRights::from_bits_truncate(self.access_rights.load(Ordering::Acquire));
        !access.contains(AccessRights::NDS_CARD)
    }
}

impl MemInterface16 for ExMemControl {
    fn read_halfword(&mut self, addr: u32) -> u16 {
        match addr {
            0x0400_0204 => self.gba_access.bits() | self.access_rights.load(Ordering::Acquire) | AccessRights::NDS_SET.bits(),
            _ => 0
        }
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {
        match addr {
            0x0400_0204 => {
                self.gba_access = GBAAccess::from_bits_truncate(data);
                let access_rights = AccessRights::from_bits_truncate(data);
                self.access_rights.store(access_rights.bits(), Ordering::Release);
            }
            _ => {}
        }
    }
}

/// Used in ARM7.
pub struct ExMemStatus {
    gba_access:     GBAAccess,
    access_rights:  Arc<AtomicU16>,
}

impl ExMemStatus {
    pub fn has_gba_access(&self) -> bool {
        let access = AccessRights::from_bits_truncate(self.access_rights.load(Ordering::Acquire));
        access.contains(AccessRights::GBA_CART)
    }

    pub fn has_nds_access(&self) -> bool {
        let access = AccessRights::from_bits_truncate(self.access_rights.load(Ordering::Acquire));
        access.contains(AccessRights::NDS_CARD)
    }
}

impl MemInterface16 for ExMemStatus {
    fn read_halfword(&mut self, addr: u32) -> u16 {
        match addr {
            0x0400_0204 => self.gba_access.bits() | self.access_rights.load(Ordering::Acquire),
            _ => 0
        }
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {
        match addr {
            0x0400_0204 => self.gba_access = GBAAccess::from_bits_truncate(data),
            _ => {}
        }
    }
}
