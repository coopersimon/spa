
use bitflags::bitflags;
use crate::{
    utils::bits::u32
};

bitflags!{
    #[derive(Default)]
    pub struct SetLine: u32 {
        const SET       = u32::bits(30, 31);
        const I_INDEX   = u32::bits(5, 11);
        const D_INDEX   = u32::bits(5, 10);
    }
}

impl SetLine {
    pub fn set_idx(&self) -> u32 {
        (*self & SetLine::SET).bits() >> 30
    }
    pub fn instr_index(&self) -> u32 {
        (*self & SetLine::I_INDEX).bits() >> 5
    }
    pub fn data_index(&self) -> u32 {
        (*self & SetLine::D_INDEX).bits() >> 5
    }

    /// Get the actual address offset of the line.
    pub fn data_offset(&self) -> u32 {
        (*self & SetLine::D_INDEX).bits()
    }
}

/// ARM9 on-chip cache.
/// Includes data and instr.
pub struct Cache {
    sets:   Vec<CacheSet>,
    index_mask:     u32,
    tag_mask:       u32,
}

impl Cache {
    pub fn new(num_lines: u32) -> Self {
        let mut sets = Vec::new();
        for _ in 0..num_lines {
            sets.push(CacheSet::new());
        }
        let index_mask_base = (num_lines << 5) - 1;
        Self {
            sets,
            index_mask: index_mask_base,
            tag_mask:   u32::MAX - index_mask_base
        }
    }

    pub fn invalidate_all(&mut self) {
        for set in &mut self.sets {
            set.invalidate_all();
        }
    }

    pub fn invalidate_line(&mut self, addr: u32) {
        let index = ((addr & self.index_mask) >> 5) as usize;
        let set = &mut self.sets[index];
        let tag = addr & self.tag_mask;
        for line in &mut set.lines {
            if line.tag == tag {
                line.invalidate();
            }
        }
    }

    pub fn clean_line<'a>(&'a mut self, addr: u32) -> Option<&'a [u8]> {
        let index = ((addr & self.index_mask) >> 5) as usize;
        let set = &mut self.sets[index];
        let tag = addr & self.tag_mask;
        for line in &mut set.lines {
            if line.tag == tag {
                return line.ref_dirty_data().map(|(_, d)| d);
            }
        }
        None
    }

    pub fn clean_set_line<'a>(&'a mut self, set: u32, index: u32) -> Option<(u32, &'a [u8])> {
        self.sets[index as usize].lines[set as usize]
            .ref_dirty_data()
    }

    pub fn clean_and_invalidate_line<'a>(&'a mut self, addr: u32) -> Option<&'a [u8]> {
        let index = ((addr & self.index_mask) >> 5) as usize;
        let set = &mut self.sets[index];
        let tag = addr & self.tag_mask;
        for line in &mut set.lines {
            if line.tag == tag {
                return line.ref_dirty_data_and_invalidate().map(|(_, d)| d);
            }
        }
        None
    }

    pub fn clean_and_invalidate_set_line<'a>(&'a mut self, set: u32, index: u32) -> Option<(u32, &'a [u8])> {
        self.sets[index as usize].lines[set as usize]
            .ref_dirty_data_and_invalidate()
    }

    pub fn fill_line(&mut self, addr: u32, data: &[u8]) {
        let index = ((addr & self.index_mask) >> 5) as usize;
        let tag = addr & self.tag_mask;
        self.sets[index].fill_line(tag, data);
    }

    /// Fill a line with new data, returning the old data and addr if it needs to be flushed.
    pub fn clean_and_fill_line(&mut self, addr: u32, data_in: &[u8], data_out: &mut [u8]) -> Option<u32> {
        let index = (addr & self.index_mask) >> 5;
        let tag = addr & self.tag_mask;
        self.sets[index as usize].clean_and_fill_line(tag, data_in, data_out).map(|addr| addr | (index << 5))
    }
}

// Data methods.
impl Cache {
    pub fn read_byte(&self, addr: u32) -> Option<u8> {
        let index = ((addr & self.index_mask) >> 5) as usize;
        let set = &self.sets[index];
        let tag = addr & self.tag_mask;
        for line in &set.lines {
            if line.tag == tag {
                return Some(line.data[(addr & LINE_MASK) as usize]);
            }
        }
        None
    }

    pub fn write_byte(&mut self, addr: u32, data: u8) -> bool {
        let index = ((addr & self.index_mask) >> 5) as usize;
        let set = &mut self.sets[index];
        let tag = addr & self.tag_mask;
        for line in &mut set.lines {
            if line.tag == tag {
                line.data[(addr & LINE_MASK) as usize] = data;
                line.dirty = true;
                return true;
            }
        }
        false
    }

    pub fn read_halfword(&self, addr: u32) -> Option<u16> {
        let index = ((addr & self.index_mask) >> 5) as usize;
        let set = &self.sets[index];
        let tag = addr & self.tag_mask;
        for line in &set.lines {
            if line.tag == tag {
                let line_addr = (addr & LINE_MASK) as usize;
                let data = u16::from_le_bytes([
                    line.data[line_addr],
                    line.data[line_addr+1]
                ]);
                return Some(data);
            }
        }
        None
    }

    pub fn write_halfword(&mut self, addr: u32, data: u16) -> bool {
        let index = ((addr & self.index_mask) >> 5) as usize;
        let set = &mut self.sets[index];
        let tag = addr & self.tag_mask;
        for line in &mut set.lines {
            if line.tag == tag {
                let line_addr = (addr & LINE_MASK) as usize;
                let bytes = data.to_le_bytes();
                line.data[line_addr] = bytes[0];
                line.data[line_addr+1] = bytes[1];
                line.dirty = true;
                return true;
            }
        }
        false
    }

    pub fn read_word(&self, addr: u32) -> Option<u32> {
        let index = ((addr & self.index_mask) >> 5) as usize;
        let set = &self.sets[index];
        let tag = addr & self.tag_mask;
        for line in &set.lines {
            if line.tag == tag {
                let line_addr = (addr & LINE_MASK) as usize;
                let data = u32::from_le_bytes([
                    line.data[line_addr],
                    line.data[line_addr+1],
                    line.data[line_addr+2],
                    line.data[line_addr+3]
                ]);
                return Some(data);
            }
        }
        None
    }

    pub fn write_word(&mut self, addr: u32, data: u32) -> bool {
        let index = ((addr & self.index_mask) >> 5) as usize;
        let set = &mut self.sets[index];
        let tag = addr & self.tag_mask;
        for line in &mut set.lines {
            if line.tag == tag {
                let line_addr = (addr & LINE_MASK) as usize;
                let bytes = data.to_le_bytes();
                line.data[line_addr] = bytes[0];
                line.data[line_addr+1] = bytes[1];
                line.data[line_addr+2] = bytes[2];
                line.data[line_addr+3] = bytes[3];
                line.dirty = true;
                return true;
            }
        }
        false
    }
}

struct CacheSet {
    lines:      [CacheLine; 4],
    replace:    usize,
}

impl CacheSet {
    fn new() -> Self {
        Self {
            lines:      [CacheLine::new(); 4],
            replace:    0,
        }
    }

    fn invalidate_all(&mut self) {
        for line in &mut self.lines {
            line.invalidate();
        }
    }

    fn invalidate_line(&mut self, set: u32) {
        self.lines[set as usize].invalidate();
    }

    fn fill_line(&mut self, tag: u32, data: &[u8]) {
        self.lines[self.replace].fill(tag, data);
        self.replace = (self.replace + 1) & 3;
    }

    fn clean_and_fill_line(&mut self, tag: u32, data_in: &[u8], data_out: &mut [u8]) -> Option<u32> {
        let mut dirty_addr = None;
        if self.lines[self.replace].dirty {
            data_out.clone_from_slice(&self.lines[self.replace].data);
            dirty_addr = Some(self.lines[self.replace].tag);
        }
        self.fill_line(tag, data_in);
        dirty_addr
    }
}

const LINE_SIZE: usize = 32;
const LINE_MASK: u32 = 0x1F;
/// An address of all 1s is not cacheable.
/// This saves us an extra valid bit check.
const INVALID: u32 = u32::MAX;

#[derive(Clone, Copy)]
struct CacheLine {
    data:   [u8; LINE_SIZE],
    tag:    u32,
    dirty:  bool,
}

impl CacheLine {
    fn new() -> Self {
        Self {
            data:   [0; LINE_SIZE],
            tag:    INVALID,
            dirty:  false,
        }
    }

    fn ref_dirty_data<'a>(&'a mut self) -> Option<(u32, &'a [u8])> {
        if self.dirty {
            self.dirty = false;
            Some((self.tag, &self.data))
        } else {
            None
        }
    }

    fn ref_dirty_data_and_invalidate<'a>(&'a mut self) -> Option<(u32, &'a [u8])> {
        let tag = std::mem::replace(&mut self.tag, INVALID);
        if self.dirty {
            self.dirty = false;
            Some((tag, &self.data))
        } else {
            None
        }
    }

    #[inline]
    fn invalidate(&mut self) {
        self.tag = INVALID;
        self.dirty = false;
    }

    /// Replace existing data with new data.
    /// Make sure to flush existing dirty data first.
    fn fill(&mut self, tag: u32, data: &[u8]) {
        self.dirty = false;
        self.tag = tag;
        self.data.clone_from_slice(data);
    }
}
