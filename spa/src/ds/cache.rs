
use bitflags::bitflags;
use crate::{
    utils::bits::u32, common::mem::ram::RAM
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

    pub fn invalidate_set_line(&mut self, set: u32, index: u32) {
        self.sets[index as usize].lines[set as usize].invalidate();
    }

    pub fn clean_line<'a>(&'a mut self, addr: u32, buffer: &mut [u32]) -> bool {
        let index = ((addr & self.index_mask) >> 5) as usize;
        let set = &mut self.sets[index];
        let tag = addr & self.tag_mask;
        for line in &mut set.lines {
            if line.tag == tag {
                return line.ref_dirty_data(buffer).is_some();
            }
        }
        false
    }

    pub fn clean_set_line<'a>(&'a mut self, set: u32, index: u32, buffer: &mut [u32]) -> Option<u32> {
        self.sets[index as usize].lines[set as usize]
            .ref_dirty_data(buffer)
    }

    pub fn clean_and_invalidate_line<'a>(&'a mut self, addr: u32, buffer: &mut [u32]) -> bool {
        let index = ((addr & self.index_mask) >> 5) as usize;
        let set = &mut self.sets[index];
        let tag = addr & self.tag_mask;
        for line in &mut set.lines {
            if line.tag == tag {
                return line.ref_dirty_data_and_invalidate(buffer).is_some();
            }
        }
        false
    }

    pub fn clean_and_invalidate_set_line<'a>(&'a mut self, set: u32, index: u32, buffer: &mut [u32]) -> Option<u32> {
        self.sets[index as usize].lines[set as usize]
            .ref_dirty_data_and_invalidate(buffer)
    }

    /// Fill a line with new data.
    pub fn fill_line(&mut self, addr: u32, data: &[u32]) {
        let index = ((addr & self.index_mask) >> 5) as usize;
        let tag = addr & self.tag_mask;
        self.sets[index].fill_line(tag, data);
    }

    /// Fill a line with new data, returning the old data and addr if it needs to be flushed.
    pub fn clean_and_fill_line(&mut self, addr: u32, data_in: &[u32], data_out: &mut [u32]) -> Option<u32> {
        let index = (addr & self.index_mask) >> 5;
        let tag = addr & self.tag_mask;
        self.sets[index as usize].clean_and_fill_line(tag, data_in, data_out).map(|tag| tag | (index << 5))
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
                return Some(line.data.read_byte(addr & LINE_MASK));
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
                line.data.write_byte(addr & LINE_MASK, data);
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
                let data = line.data.read_halfword(addr & LINE_MASK);
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
                line.data.write_halfword(addr & LINE_MASK, data);
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
                let data = line.data.read_word(addr & LINE_MASK);
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
                line.data.write_word(addr & LINE_MASK, data);
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
            lines:      [CacheLine::new(), CacheLine::new(), CacheLine::new(), CacheLine::new()],
            replace:    0,
        }
    }

    /// Set all lines to invalid.
    fn invalidate_all(&mut self) {
        for line in &mut self.lines {
            line.invalidate();
        }
    }

    /// Fill a line with new data.
    fn fill_line(&mut self, tag: u32, data: &[u32]) {
        self.lines[self.replace].fill(tag, data);
        self.replace = (self.replace + 1) & 3;
    }

    /// Fill a line with new data, and fill the data_out buffer with the old data if dirty.
    /// 
    /// Returns the tag of the old data if it was dirty.
    fn clean_and_fill_line(&mut self, tag: u32, data_in: &[u32], data_out: &mut [u32]) -> Option<u32> {
        let dirty_tag = self.lines[self.replace].ref_dirty_data(data_out);
        self.fill_line(tag, data_in);
        dirty_tag
    }
}

const LINE_SIZE: usize = 32;
const LINE_MASK: u32 = 0x1F;
/// An address of all 1s is not cacheable.
/// This saves us an extra valid bit check.
const INVALID: u32 = u32::MAX;

struct CacheLine {
    data:   RAM,
    tag:    u32,
    dirty:  bool,
}

impl CacheLine {
    fn new() -> Self {
        Self {
            data:   RAM::new(LINE_SIZE),
            tag:    INVALID,
            dirty:  false,
        }
    }

    fn ref_dirty_data<'a>(&'a mut self, data_out: &mut [u32]) -> Option<u32> {
        if self.dirty {
            self.dirty = false;
            for (i, out) in data_out.iter_mut().enumerate() {
                *out = self.data.read_word((i * 4) as u32);
            }
            Some(self.tag)
        } else {
            None
        }
    }

    fn ref_dirty_data_and_invalidate<'a>(&'a mut self, data_out: &mut [u32]) -> Option<u32> {
        let tag = std::mem::replace(&mut self.tag, INVALID);
        if self.dirty {
            self.dirty = false;
            for (i, out) in data_out.iter_mut().enumerate() {
                *out = self.data.read_word((i * 4) as u32);
            }
            Some(tag)
        } else {
            None
        }
    }

    /// Mark existing data as invalid, without flushing.
    #[inline]
    fn invalidate(&mut self) {
        self.tag = INVALID;
        self.dirty = false;
    }

    /// Replace existing data with new data.
    /// Make sure to flush existing dirty data first.
    fn fill(&mut self, tag: u32, data: &[u32]) {
        self.dirty = false;
        self.tag = tag;
        for (i, data_word) in data.iter().enumerate() {
            self.data.write_word((i * 4) as u32, *data_word);
        }
    }
}
