use std::collections::VecDeque;
use super::GeometryEngineStatus;

enum CommandFifoInterruptCond {
    Never,
    UnderHalf,
    Empty
}

const COMMAND_FIFO_LEN: usize = 256;

pub struct GeomCommandFifo {
    command_fifo:           VecDeque<u32>,
    current_command_args:   usize,

    interrupt_cond:         CommandFifoInterruptCond,
}

impl GeomCommandFifo {
    pub fn new() -> Self {
        Self {
            command_fifo:           VecDeque::with_capacity(256),
            current_command_args:   0,
            interrupt_cond:         CommandFifoInterruptCond::Never,
        }
    }

    /// Push directly to the command buffer.
    pub fn push_command_buffer(&mut self, data: u32) {
        if self.command_fifo.len() == COMMAND_FIFO_LEN {
            panic!("GPU command fifo full");   // TODO: handle
        }
        self.command_fifo.push_back(data);
    }

    /// Push via a memory address.
    pub fn push_command_cpu(&mut self, data: u32, command: u32, num_args: usize) {
        if self.current_command_args > 0 {
            self.push_command_buffer(data);
            self.current_command_args -= 1;
        } else {
            self.push_command_buffer(command);
            if num_args > 0 {
                self.push_command_buffer(data);
                self.current_command_args = num_args - 1;
            }
        }
    }

    /// Pop a value from the command buffer.
    pub fn pop(&mut self) -> Option<u32> {
        self.command_fifo.pop_front()
    }

    pub fn pop_n<'a>(&'a mut self, n: usize) -> impl Iterator<Item = u32> + 'a {
        self.command_fifo.drain(0..n)
    }

    pub fn len(&self) -> u32 {
        self.command_fifo.len() as u32
    }

    pub fn is_empty(&self) -> bool {
        self.command_fifo.is_empty()
    }

    pub fn under_half_full(&self) -> bool {
        const HALF_COMMAND_FIFO_LEN: usize = COMMAND_FIFO_LEN / 2;
        self.command_fifo.len() < HALF_COMMAND_FIFO_LEN
    }
    
    pub fn is_full(&self) -> bool {
        self.command_fifo.len() == COMMAND_FIFO_LEN
    }

    pub fn set_interrupt_cond(&mut self, val: GeometryEngineStatus) {
        self.interrupt_cond = match (val & GeometryEngineStatus::CMD_FIFO_INT).bits() {
            0b01 => CommandFifoInterruptCond::UnderHalf,
            0b10 => CommandFifoInterruptCond::Empty,
            _ => CommandFifoInterruptCond::Never
        };
    }

    pub fn interrupt(&self) -> bool {
        match self.interrupt_cond {
            CommandFifoInterruptCond::Never => false,
            CommandFifoInterruptCond::Empty => self.is_empty(),
            CommandFifoInterruptCond::UnderHalf => self.under_half_full()
        }
    }
}
