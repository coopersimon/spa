
const FIFO_SIZE: usize = 8;
pub const RELOAD_SIZE: usize = FIFO_SIZE / 2;

pub struct AudioFIFO {
    buffer: [u32; FIFO_SIZE],
    len:    usize,
    read:   usize,
    write:  usize,

    nybble: usize,
}

impl AudioFIFO {
    pub fn new() -> Self {
        Self {
            buffer: [0; FIFO_SIZE],
            len:    0,
            read:   0,
            write:  0,

            nybble: 0,
        }
    }

    /// Current length of the FIFO.
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn clear(&mut self) {
        for i in 0..FIFO_SIZE {
            self.buffer[i] = 0;
        }
        self.len = 0;
        self.read = 0;
        self.write = 0;
        self.nybble = 0;
    }

    pub fn push(&mut self, data: u32) {
        self.buffer[self.write] = data;
        self.write = (self.write + 1) % FIFO_SIZE;
        if self.len == FIFO_SIZE {
            self.read = self.write;
        } else {
            self.len += 1;
        }
    }

    fn pop(&mut self) {
        if self.len > 0 {
            self.read = (self.read + 1) % FIFO_SIZE;
            self.len -= 1;
        }
    }

    /// Read the oldest value in the FIFO as PCM 8,
    /// and advance the FIFO.
    pub fn sample_pcm_8(&mut self) -> i8 {
        if self.len == 0 {
            0
        } else {
            let shift = self.nybble * 4;
            let sample = (self.buffer[self.read] >> shift) & 0xFF;
            self.nybble += 2;
            if self.nybble == 8 {
                self.nybble = 0;
                self.pop();
            }
            sample as u8 as i8
        }
    }

    /// Read the oldest value in the FIFO as PCM 16,
    /// and advance the FIFO.
    pub fn sample_pcm_16(&mut self) -> i16 {
        if self.len == 0 {
            0
        } else {
            let shift = self.nybble * 4;
            let sample = (self.buffer[self.read] >> shift) & 0xFFFF;
            self.nybble += 4;
            if self.nybble == 8 {
                self.nybble = 0;
                self.pop();
            }
            sample as u16 as i16
        }
    }

    /// Read the header of an ADPCM sample,
    /// and advance the FIFO.
    /// 
    /// Returns None if there is no data.
    pub fn get_adpcm_header(&mut self) -> Option<u32> {
        if self.len == 0 {
            None
        } else {
            let header = self.buffer[self.read];
            self.pop();
            Some(header)
        }
    }

    /// Read the oldest value in the FIFO as ADPCM,
    /// and advance the FIFO.
    pub fn sample_adpcm(&mut self) -> u8 {
        if self.len == 0 {
            0
        } else {
            let shift = self.nybble * 4;
            let sample = (self.buffer[self.read] >> shift) & 0xF;
            self.nybble += 1;
            if self.nybble == 8 {
                self.nybble = 0;
                self.pop();
            }
            sample as u8
        }
    }
}
