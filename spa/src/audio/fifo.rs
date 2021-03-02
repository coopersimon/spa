
const FIFO_SIZE: usize = 32;

/// FIFO used for audio. It can fit 32 samples inside.
pub struct FIFO {
    buffer: [i8; FIFO_SIZE],
    len:    usize,
    read:   usize,
    write:  usize,
}

impl FIFO {
    pub fn new() -> Self {
        Self {
            buffer: [0; FIFO_SIZE],
            len:    0,
            read:   0,
            write:  0,
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
    }

    pub fn push(&mut self, data: i8) {
        self.buffer[self.write] = data;
        self.write = (self.write + 1) % FIFO_SIZE;
        if self.len == FIFO_SIZE {
            self.read = self.write;
        } else {
            self.len += 1;
        }
    }

    pub fn pop(&mut self) -> i8 {
        if self.len == 0 {
            0
        } else {
            let ret = self.buffer[self.read];
            self.read = (self.read + 1) % FIFO_SIZE;
            self.len -= 1;
            ret
        }
    }
}

mod fifotest {
    #[test]
    fn test_len() {
        let mut fifo = super::FIFO::new();
        for _ in 0..10 {
            fifo.push(0);
        }
        assert_eq!(fifo.len(), 10);

        for _ in 0..30 {
            fifo.push(0);
        }
        assert_eq!(fifo.len(), 32);
    }
}