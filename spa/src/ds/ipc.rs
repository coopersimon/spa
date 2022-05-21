/// Inter-proess (CPU) communication

use bitflags::bitflags;
use crossbeam_channel::{Sender, Receiver, TrySendError, TryRecvError, bounded};

use std::sync::{
    atomic::{AtomicBool, AtomicU8, Ordering},
    Arc
};

use crate::utils::{
    bits::u32,
    meminterface::MemInterface32
};
use super::interrupt::Interrupts;

bitflags! {
    #[derive(Default)]
    pub struct IPCFifoControl: u32 {
        const ENABLE_FIFO       = u32::bit(15);
        const ERROR             = u32::bit(14);

        const RECV_FIFO_IRQ     = u32::bit(10);
        const RECV_FIFO_FULL    = u32::bit(9);
        const RECV_FIFO_EMPTY   = u32::bit(8);

        const SEND_FIFO_FLUSH   = u32::bit(3);
        const SEND_FIFO_IRQ     = u32::bit(2);
        const SEND_FIFO_FULL    = u32::bit(1);
        const SEND_FIFO_EMPTY   = u32::bit(0);

        const WRITE_BITS        = u32::bit(2) | u32::bit(10) | u32::bit(15);
    }
}

pub struct IPC {
    send:       Sender<u32>,
    recv:       Receiver<u32>,
    last_word:  u32,    // Last word received.

    // 4 bits of data.
    atomic_write:   Arc<AtomicU8>,
    atomic_read:    Arc<AtomicU8>,

    ipc_fifo_control:   IPCFifoControl,

    // To ensure interrupts are edge-triggered.
    was_send_empty: bool,
    was_recv_empty: bool,

    irq_enable:     bool,
    irq_req_in:     Arc<AtomicBool>,
    irq_req_out:    Arc<AtomicBool>,

    name: String
}

impl IPC {
    pub fn new() -> (IPC, IPC) {
        let (send_a, recv_a) = bounded(16);
        let (send_b, recv_b) = bounded(16);
        let atomic_a = Arc::new(AtomicU8::new(0));
        let atomic_b = Arc::new(AtomicU8::new(0));
        let irq_req_a = Arc::new(AtomicBool::new(false));
        let irq_req_b = Arc::new(AtomicBool::new(false));

        (Self{
            send:               send_a,
            recv:               recv_b,
            last_word:          0,
            atomic_write:       atomic_a.clone(),
            atomic_read:        atomic_b.clone(),
            ipc_fifo_control:   IPCFifoControl::default(),
            was_send_empty:     true,
            was_recv_empty:     true,
            irq_enable:         false,
            irq_req_in:         irq_req_a.clone(),
            irq_req_out:        irq_req_b.clone(),
            name:               "ARM9".to_string(),
        }, Self{
            send:               send_b,
            recv:               recv_a,
            last_word:          0,
            atomic_write:       atomic_b,
            atomic_read:        atomic_a,
            ipc_fifo_control:   IPCFifoControl::default(),
            was_send_empty:     true,
            was_recv_empty:     true,
            irq_enable:         false,
            irq_req_in:         irq_req_b,
            irq_req_out:        irq_req_a,
            name:               "ARM7".to_string(),
        })
    }

    pub fn get_interrupts(&mut self) -> Interrupts {
        let mut interrupts = Interrupts::default();
        if self.ipc_fifo_control.contains(IPCFifoControl::SEND_FIFO_IRQ) {
            let is_send_empty = self.send.is_empty();
            if is_send_empty && !self.was_send_empty {
                interrupts.insert(Interrupts::IPC_SEND_EMPTY);
            }
            self.was_send_empty = is_send_empty;
        }
        if self.ipc_fifo_control.contains(IPCFifoControl::RECV_FIFO_IRQ) {
            let is_recv_empty = self.recv.is_empty();
            if !is_recv_empty && self.was_recv_empty {
                interrupts.insert(Interrupts::IPC_RECV_NEMPTY);
            }
            self.was_recv_empty = is_recv_empty;
        }
        if self.irq_enable && self.irq_req_in.swap(false, Ordering::AcqRel) {
            interrupts.insert(Interrupts::IPC_SYNC);
        }
        interrupts
    }
}

impl IPC {
    fn read_sync_reg(&self) -> u32 {
        let mut out = self.atomic_read.load(Ordering::Acquire) as u32;
        out |= (self.atomic_write.load(Ordering::Acquire) as u32) << 8;
        if self.irq_enable {
            out |= u32::bit(14);
        }
        out
    }

    fn write_sync_reg(&mut self, data: u32) {
        let to_write = ((data >> 8) & 0xF) as u8;
        self.atomic_write.store(to_write, Ordering::Release);
        if u32::test_bit(data, 13) {
            self.irq_req_out.store(true, Ordering::Release);
        }
        self.irq_enable = u32::test_bit(data, 14);
    }

    fn read_control_reg(&self) -> u32 {
        let mut data = self.ipc_fifo_control;
        data.set(IPCFifoControl::SEND_FIFO_EMPTY, self.send.is_empty());
        data.set(IPCFifoControl::SEND_FIFO_FULL, self.send.is_full());
        data.set(IPCFifoControl::RECV_FIFO_EMPTY, self.recv.is_empty());
        data.set(IPCFifoControl::RECV_FIFO_FULL, self.recv.is_full());
        data.bits()
    }
    
    fn write_control_reg(&mut self, data: u32) {
        let control_data = IPCFifoControl::from_bits_truncate(data);
        if control_data.contains(IPCFifoControl::ERROR) {
            self.ipc_fifo_control.remove(IPCFifoControl::ERROR);
        }
        if control_data.contains(IPCFifoControl::SEND_FIFO_FLUSH) {
            // TODO: clear
        }
        self.ipc_fifo_control = (control_data & IPCFifoControl::WRITE_BITS) | (self.ipc_fifo_control & IPCFifoControl::ERROR);
    }

    fn fifo_read(&mut self) -> u32 {
        if self.ipc_fifo_control.contains(IPCFifoControl::ENABLE_FIFO) {
            match self.recv.try_recv() {
                Ok(word) => self.last_word = word,
                Err(TryRecvError::Empty) => self.ipc_fifo_control.insert(IPCFifoControl::ERROR),
                err => panic!("{:?}", err),
            }
        }
        //println!("READ {:X} from fifo (from {})", self.last_word, self.name);
        self.last_word
    }

    fn fifo_write(&mut self, data: u32) {
        //println!("WRITE {:X} to fifo (from {})", data, self.name);
        if self.ipc_fifo_control.contains(IPCFifoControl::ENABLE_FIFO) {
            match self.send.try_send(data) {
                Ok(()) => {},
                Err(TrySendError::Full(_)) => self.ipc_fifo_control.insert(IPCFifoControl::ERROR),
                err => panic!("{:?}", err),
            }
        }
    }
}

impl MemInterface32 for IPC {
    fn read_word(&mut self, addr: u32) -> u32 {
        match addr {
            0x0400_0180 => self.read_sync_reg(),
            0x0400_0184 => self.read_control_reg(),
            0x0410_0000 => self.fifo_read(),
            _ => 0,
        }
    }

    fn write_word(&mut self, addr: u32, data: u32) {
        match addr {
            0x0400_0180 => self.write_sync_reg(data),
            0x0400_0184 => self.write_control_reg(data),
            0x0400_0188 => self.fifo_write(data),
            _ => {},
        }
    }
}
