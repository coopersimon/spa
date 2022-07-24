/// Circular buffer that can hold a fixed size of items.
/// 
/// It can then be iterated over.
pub struct CircularBuffer<T: Default> {
    size:   usize,
    start:  usize,
    end:    usize,
    data:   Vec<T>
}

impl<T: Default> CircularBuffer<T> {
    pub fn new() -> Self {
        Self {
            size:   0,
            start:  0,
            end:    1,
            data:   Vec::new()
        }
    }

    /// Resize the buffer and clear all data.
    pub fn resize(&mut self, size: usize) {
        self.size = size;
        self.start = 0;
        self.end = 1;
        if size > self.data.len() {
            self.data.resize_with(size, Default::default)
        }
    }

    /// Push a value onto the end of the buffer.
    /// 
    /// If the buffer is full, it replaces the value at the start.
    pub fn push(&mut self, value: T) {
        self.data[self.end] = value;
        self.end = (self.end + 1) % (self.size + 1);
        if self.end == self.start {
            self.start = (self.start + 1) % self.size;
        }
    }

    /// Get the length of data inside the buffer.
    pub fn len(&self) -> usize {
        (self.size + self.end - self.start) % (self.size + 1)
    }

    /// Test if the buffer is full.
    pub fn is_full(&self) -> bool {
        self.len() == self.size
    }

    /// Get the value at the end of the buffer.
    pub fn end<'a>(&'a self) -> &'a T {
        let index = (self.end - 1) % self.size;
        &self.data[index]
    }

    /// Make an iterator that iterates over the data from start to end.
    pub fn iter<'a>(&'a self) -> CircularBufferIterator<'a, T> {
        CircularBufferIterator {
            index:              self.start,
            circular_buffer:    &self
        }
    }
}

pub struct CircularBufferIterator<'a, T: Default> {
    index:              usize,
    circular_buffer:    &'a CircularBuffer<T>
}

impl<'a, T: Default> Iterator for CircularBufferIterator<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        let len = self.circular_buffer.len();
        let data = &self.circular_buffer.data[self.index % len];
        self.index = (self.index + 1) % len;
        Some(data)
    }
}

impl<'a, T: Default> ExactSizeIterator for CircularBufferIterator<'a, T> {
    fn len(&self) -> usize {
        self.circular_buffer.start + self.circular_buffer.len() - self.index
    }
}
