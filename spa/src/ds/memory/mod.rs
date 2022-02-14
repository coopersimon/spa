
use crate::common::{
    wram::WRAM
};


/// Memory bus for DS ARM9 processor.
pub struct DS9MemoryBus {

    itcm:       WRAM,
    dtcm:       WRAM,

    wram:       WRAM,
    fast_wram:  WRAM,
}

/// Memory bus for DS ARM7 processor.
pub struct DS7MemoryBus {

}