use bitflags::bitflags;
use crate::utils::{
    bits::u8,
    bytes::u16
};

bitflags!{
    #[derive(Default)]
    pub struct CaptureControl: u8 {
        const START     = u8::bit(7);
        const FORMAT    = u8::bit(3);
        const ONE_SHOT  = u8::bit(2);
        const SOURCE    = u8::bit(1);
        const ADD       = u8::bit(0);
    }
}

pub struct AudioCaptureUnit {
    pub control:    CaptureControl,
    pub dst_addr:   u32,
    pub len:        u32,

    fifo:           CaptureFIFO,
    current_addr:   u32,
    count:          u32,
    end:            u32,
}

impl AudioCaptureUnit {
    pub fn new() -> Self {
        Self {
            control:    CaptureControl::default(),
            dst_addr:   0,
            len:        0,

            fifo:           CaptureFIFO::new(),
            current_addr:   0,
            count:          0,
            end:            0,
        }
    }

    pub fn write_control(&mut self, data: u8) {
        self.control = CaptureControl::from_bits_truncate(data);
        if self.control.contains(CaptureControl::START) {
            self.fifo.clear();
            self.reset();
        }
    }

    pub fn write_dest(&mut self, data: u32) {
        self.dst_addr = data & 0x7FF_FFFC;
    }

    pub fn write_len(&mut self, data: u32) {
        if data == 0 {
            self.len = 1;
        } else {
            self.len = data & 0xFFFF;
        }
    }

    /// Write a PCM 8 sample to the FIFO.
    /// Returns true if a transfer is needed.
    pub fn write_fifo_pcm_8(&mut self, data: i8) -> bool {
        self.fifo.push(data as u8);
        self.count += 1;
        if self.count >= self.end {
            self.stop();
            true
        } else {
            self.fifo.len() > DMA_SIZE
        }
    }

    /// Write a PCM 16 sample to the FIFO.
    /// Returns true if a transfer is needed.
    pub fn write_fifo_pcm_16(&mut self, data: i16) -> bool {
        self.fifo.push(u16::lo(data as u16));
        self.fifo.push(u16::hi(data as u16));
        self.count += 2;
        if self.count >= self.end {
            self.stop();
            true
        } else {
            self.fifo.len() > DMA_SIZE
        }
    }

    /// Get the destination addr for a DMA transfer.
    pub fn get_dma_addr(&mut self) -> u32 {
        let addr = self.current_addr;
        self.current_addr += 4;
        addr
    }

    /// Read a word from the FIFO for DMA.
    pub fn read_fifo(&mut self) -> u32 {
        u32::from_le_bytes([
            self.fifo.pop(),
            self.fifo.pop(),
            self.fifo.pop(),
            self.fifo.pop()
        ])
    }

    /// Loop or stop the capture.
    fn stop(&mut self) {
        if self.control.contains(CaptureControl::ONE_SHOT) {
            self.control.remove(CaptureControl::START);
        } else {
            self.reset();
        }
    }

    fn reset(&mut self) {
        self.current_addr = self.dst_addr;
        self.count = 0;
        self.end = self.len << 2;
    }
}

const FIFO_SIZE: usize = 32;
const DMA_SIZE: usize = FIFO_SIZE / 2;

struct CaptureFIFO {
    buffer: [u8; FIFO_SIZE],
    len:    usize,
    read:   usize,
    write:  usize,
}

impl CaptureFIFO {
    fn new() -> Self {
        Self {
            buffer: [0; FIFO_SIZE],
            len:    0,
            read:   0,
            write:  0,
        }
    }

    /// Current length of the FIFO.
    fn len(&self) -> usize {
        self.len
    }

    fn clear(&mut self) {
        for i in 0..FIFO_SIZE {
            self.buffer[i] = 0;
        }
        self.len = 0;
        self.read = 0;
        self.write = 0;
    }

    fn push(&mut self, data: u8) {
        self.buffer[self.write] = data;
        self.write = (self.write + 1) % FIFO_SIZE;
        if self.len == FIFO_SIZE {
            self.read = self.write;
        } else {
            self.len += 1;
        }
    }

    fn pop(&mut self) -> u8 {
        if self.len > 0 {
            let data = self.buffer[self.read];
            self.read = (self.read + 1) % FIFO_SIZE;
            self.len -= 1;
            data
        } else {
            0
        }
    }
}