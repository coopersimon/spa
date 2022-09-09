/// Debug comms

use super::video::framecomms::debug::DebugFrameReq;
use arm::{
    CPUState,
    Debugger,
    ARMDriver,
    ARMCore,
    Mem32,
    MemCycleType
};
use crossbeam_channel::{Sender, Receiver, bounded, select};

enum Request {
    DoStep,
    GetState,
    GetWord(u32),
    GetHalfword(u32),
    GetByte(u32)
}

enum Response {
    Cycles(usize),
    State(CPUState),
    Word(u32),
    Halfword(u16),
    Byte(u8)
}

/// The interface for the debugger.
/// 
/// Call methods on the main thread.
pub struct DebugInterface<I: Clone> {
    req_send: Sender<Request>,
    res_recv: Receiver<Response>,

    requester: DebugFrameReq<I>,
    default_input: I
}

impl<I: Clone> DebugInterface<I> {
    pub fn new(requester: DebugFrameReq<I>, default_input: I) -> (Self, DebugWrapper) {
        let (req_send, req_recv) = bounded(1);
        let (res_send, res_recv) = bounded(1);
        let wrapper = DebugWrapper {
            req_recv, res_send
        };
        (Self {
            req_send,
            res_recv,
            requester,
            default_input
        }, wrapper)
    }

    pub fn step(&mut self) {
        self.req_send.send(Request::DoStep).unwrap();
        select! {
            recv(self.res_recv) -> msg => match msg.unwrap() {
                Response::Cycles(_) => {},
                _ => panic!("unexpected response")
            },
            recv(self.requester.rx) -> _ => {
                self.requester.tx.send(self.default_input.clone()).expect("couldn't send to cpu thread");

                match self.res_recv.recv().unwrap() {
                    Response::Cycles(_) => {},
                    _ => panic!("unexpected response")
                }
            },
        }
    }

    pub fn get_state(&mut self) -> CPUState {
        self.req_send.send(Request::GetState).unwrap();
        match self.res_recv.recv().unwrap() {
            Response::State(s) => s,
            _ => panic!("unexpected response")
        }
    }

    pub fn get_word(&mut self, addr: u32) -> u32 {
        self.req_send.send(Request::GetWord(addr)).unwrap();
        match self.res_recv.recv().unwrap() {
            Response::Word(d) => d,
            _ => panic!("unexpected response")
        }
    }

    pub fn get_halfword(&mut self, addr: u32) -> u16 {
        self.req_send.send(Request::GetHalfword(addr)).unwrap();
        match self.res_recv.recv().unwrap() {
            Response::Halfword(d) => d,
            _ => panic!("unexpected response")
        }
    }

    pub fn get_byte(&mut self, addr: u32) -> u8 {
        self.req_send.send(Request::GetByte(addr)).unwrap();
        match self.res_recv.recv().unwrap() {
            Response::Byte(d) => d,
            _ => panic!("unexpected response")
        }
    }
}

/// CPU wrapping object for debugging.
pub struct DebugWrapper {
    req_recv: Receiver<Request>,
    res_send: Sender<Response>
}

impl DebugWrapper {
    pub fn run_debug<M: Mem32<Addr = u32>, A: Debugger + ARMDriver + ARMCore<M>>(self, mut cpu: A) {
        use Request::*;
        use Response::*;
        loop {
            match self.req_recv.recv().unwrap() {
                DoStep => {
                    let cycles = cpu.step();
                    self.res_send.send(Cycles(cycles))
                },
                GetState => {
                    self.res_send.send(State(cpu.inspect_state()))
                },
                GetWord(addr) => {
                    let (word, _) = cpu.mut_mem().load_word(MemCycleType::S, addr);
                    self.res_send.send(Word(word))
                },
                GetHalfword(addr) => {
                    let (halfword, _) = cpu.mut_mem().load_halfword(MemCycleType::S, addr);
                    self.res_send.send(Halfword(halfword))
                },
                GetByte(addr) => {
                    let (byte, _) = cpu.mut_mem().load_byte(MemCycleType::S, addr);
                    self.res_send.send(Byte(byte))
                }
            }.unwrap()
        }
    }
}
