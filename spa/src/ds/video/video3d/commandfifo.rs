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
    status_bits:            GeometryEngineStatus,
}

impl GeomCommandFifo {
    pub fn new() -> Self {
        Self {
            command_fifo:           VecDeque::with_capacity(COMMAND_FIFO_LEN),
            current_command_args:   0,
            interrupt_cond:         CommandFifoInterruptCond::Never,
            status_bits:            GeometryEngineStatus::empty(),
        }
    }

    /// Push directly to the command buffer.
    pub fn push_command_buffer(&mut self, data: u32) {
        if self.command_fifo.len() == COMMAND_FIFO_LEN {
            panic!("GPU command fifo full");   // TODO: handle (ignore incoming data? / freeze)
        }
        //println!("GX PUSH: {:X}", data);
        self.command_fifo.push_back(data);
    }

    /// Push via a memory address.
    pub fn push_command_cpu(&mut self, data: u32, command: u32, num_args: usize) {
        //println!("GX CMD: {:X} ({:X})", command, data);
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

    pub fn pop_n<'a>(&'a mut self, n: usize) -> Option<impl Iterator<Item = u32> + 'a> {
        if self.command_fifo.len() >= n {
            Some(self.command_fifo.drain(0..n))
        } else {
            None
        }
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
        self.status_bits = val & GeometryEngineStatus::CMD_FIFO_INT;
        self.interrupt_cond = match self.status_bits.bits() >> 30 {
            0b01 => CommandFifoInterruptCond::UnderHalf,
            0b10 => CommandFifoInterruptCond::Empty,
            _ => CommandFifoInterruptCond::Never
        };
    }

    pub fn get_interrupt_cond(&self) -> GeometryEngineStatus {
        self.status_bits
    }

    pub fn interrupt(&self) -> bool {
        match self.interrupt_cond {
            CommandFifoInterruptCond::Never => false,
            CommandFifoInterruptCond::Empty => self.is_empty(),
            CommandFifoInterruptCond::UnderHalf => self.under_half_full()
        }
    }
}
