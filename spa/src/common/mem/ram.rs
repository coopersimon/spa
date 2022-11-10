/// RAM
#[cfg(not(feature = "fast"))]
use std::convert::TryInto;

/// Generic, general purpose RAM. Used for work RAM, video RAM, ROM backing, and more.
/// 
/// Can read and write quantities of 8, 16, and 32 bits.
/// 
/// Note that 16 and 32-bit accesses MUST be aligned, or the program will behave in an undefined manner.
pub struct RAM(Vec<u8>);

impl From<Vec<u8>> for RAM {
    fn from(buffer: Vec<u8>) -> Self {
        Self(buffer)
    }
}

impl RAM {
    pub fn new(size: usize) -> Self {
        Self(vec![0; size])
    }

    /// Get the size of the memory in bytes.
    pub fn len(&self) -> u32 {
        self.0.len() as u32
    }
    /// Get the mask of the address range.
    /// 
    /// Only works if the length is a power of two.
    pub fn mask(&self) -> u32 {
        self.len() - 1
    }

    pub fn ref_mem<'a>(&'a self) -> &'a [u8] {
        &self.0
    }

    #[inline]
    pub fn read_byte(&self, addr: u32) -> u8 {
        self.0[addr as usize]
    }
    #[inline]
    pub fn write_byte(&mut self, addr: u32, data: u8) {
        self.0[addr as usize] = data;
    }

    #[inline]
    pub fn read_halfword(&self, addr: u32) -> u16 {
        if cfg!(feature = "fast") {
            unsafe {
                let buffer_ptr = self.0.as_ptr();
                let src = buffer_ptr.offset(addr as isize);
                *(src.cast())
            }
        } else {
            let start = addr as usize;
            let end = start + 2;
            let data = (self.0[start..end]).try_into().unwrap();
            u16::from_le_bytes(data)
        }
    }
    #[inline]
    pub fn write_halfword(&mut self, addr: u32, data: u16) {
        if cfg!(feature = "fast") {
            unsafe {
                let buffer_ptr = self.0.as_mut_ptr();
                let dest = buffer_ptr.offset(addr as isize);
                *(dest.cast()) = data;
            }
        } else {
            let start = addr as usize;
            let end = start + 2;
            for (dest, byte) in self.0[start..end].iter_mut().zip(&data.to_le_bytes()) {
                *dest = *byte;
            }
        }
    }

    #[inline]
    pub fn read_word(&self, addr: u32) -> u32 {
        if cfg!(feature = "fast") {
            unsafe {
                let buffer_ptr = self.0.as_ptr();
                let src = buffer_ptr.offset(addr as isize);
                *(src.cast())
            }
        } else {
            let start = addr as usize;
            let end = start + 4;
            let data = (self.0[start..end]).try_into().unwrap();
            u32::from_le_bytes(data)
        }
    }
    #[inline]
    pub fn write_word(&mut self, addr: u32, data: u32) {
        if cfg!(feature = "fast") {
            unsafe {
                let buffer_ptr = self.0.as_mut_ptr();
                let dest = buffer_ptr.offset(addr as isize);
                *(dest.cast()) = data;
            }
        } else {
            let start = addr as usize;
            let end = start + 4;
            for (dest, byte) in self.0[start..end].iter_mut().zip(&data.to_le_bytes()) {
                *dest = *byte;
            }
        }
    }
}