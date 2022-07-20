/// Circular buffer that can hold a fixed size of items.
/// 
/// It can then be iterated over.

pub struct CircularBuffer<T> {
    size:   usize,
    start:  usize,
    data:   Vec<T>
}

impl<T> CircularBuffer<T> {
    pub fn new() -> Self {
        Self {
            size:   0,
            start:  0,
            data:   Vec::new()
        }
    }

    /// Resize the buffer and clear all data.
    pub fn resize(&mut self, size: usize) {
        self.size = size;
        self.start = 0;
        self.data.clear();
    } 

    /// Push a value onto the end of the buffer.
    /// 
    /// If the buffer is full, it replaces the value at the start.
    pub fn push(&mut self, value: T) {
        if self.len() >= self.size {
            self.data[self.start] = value;
            self.start += 1;
        } else {
            self.data.push(value);
        }
    }

    /// Get the length of data inside the buffer.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Test if the buffer is full.
    pub fn is_full(&self) -> bool {
        self.data.len() == self.size
    }

    /// Get the value at the end of the buffer.
    pub fn end<'a>(&'a self) -> &'a T {
        let index = self.start - 1 % self.data.len();
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

pub struct CircularBufferIterator<'a, T> {
    index:              usize,
    circular_buffer:    &'a CircularBuffer<T>
}

impl<'a, T> Iterator for CircularBufferIterator<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        let len = self.circular_buffer.len();
        let data = &self.circular_buffer.data[self.index % len];
        self.index = (self.index + 1) % len;
        Some(data)
    }
}

impl<'a, T> ExactSizeIterator for CircularBufferIterator<'a, T> {
    fn len(&self) -> usize {
        self.circular_buffer.start + self.circular_buffer.len() - self.index
    }
}
