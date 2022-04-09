
use crossbeam_channel::{Sender, Receiver, bounded};
use std::{
    sync::Arc,
    sync::Mutex,
};
use crate::FrameBuffer;

pub fn new_frame_comms<I>(frame_size: usize) -> (FrameSender<I>, FrameRequester<I>) {
    let buffer = vec![0; frame_size];
    let frame_buffer = Arc::new(Mutex::new(buffer.into_boxed_slice()));
    let (sync_tx, sync_rx) = bounded(1);
    let (data_tx, data_rx) = bounded(1);
    (
        FrameSender{frame_buffer: frame_buffer.clone(), tx: data_tx, rx: sync_rx},
        FrameRequester{frame_buffer: frame_buffer, tx: sync_tx, rx: data_rx}
    )
}

#[cfg(feature = "debug")]
pub mod debug {
    use super::*;
    pub fn new_debug_frame_comms<I>(frame_size: usize) -> (FrameSender<I>, DebugFrameReq<I>) {
        let buffer = vec![0; frame_size];
        let frame_buffer = Arc::new(Mutex::new(buffer.into_boxed_slice()));
        let (sync_tx, sync_rx) = bounded(1);
        let (data_tx, data_rx) = bounded(1);
        (
            FrameSender{frame_buffer: frame_buffer, tx: data_tx, rx: sync_rx},
            DebugFrameReq{tx: sync_tx, rx: data_rx}
        )
    }
    
    pub struct DebugFrameReq<I> {
        tx: Sender<I>,
        rx: Receiver<()>
    }
    
    impl<I> DebugFrameReq<I> {
        /// Check if CPU thread has told us processing for the frame is complete.
        /// 
        /// Then force it to continue if so.
        pub fn check_and_continue(&mut self, input: I) {
            // Wait for CPU thread to let us know its processing is complete.
            match self.rx.try_recv() {
                // Let CPU thread know processing can continue.
                Ok(_) => self.tx.send(input).expect("couldn't send to cpu thread"),
                Err(_) => {},
            }
        }
    }    
}

pub struct FrameRequester<I> {
    frame_buffer:   Arc<Mutex<FrameBuffer>>,

    tx: Sender<I>,
    rx: Receiver<()>
}

impl<I> FrameRequester<I> {
    /// Indicate to the CPU thread that it is ready for a new frame.
    /// 
    /// Extracts the next frame, and sends user input since last frame.
    pub fn get_frame(&mut self, buffer: &mut [u8], input: I) {
        // Wait for CPU thread to let us know its processing is complete.
        self.rx.recv().expect("couldn't get from cpu thread");
        // Copy frame into buffer.
        let frame = self.frame_buffer.lock().unwrap();
        buffer.copy_from_slice(&(*frame));
        // Let CPU thread know processing can continue.
        self.tx.send(input).expect("couldn't send to cpu thread");
    }
}

pub struct FrameSender<I> {
    frame_buffer:   Arc<Mutex<FrameBuffer>>,
    
    tx: Sender<()>,
    rx: Receiver<I>
}

impl<I> FrameSender<I> {

    /// Clone the frame buffer Arc.
    pub fn get_frame_buffer(&self) -> Arc<Mutex<FrameBuffer>> {
        self.frame_buffer.clone()
    }

    /// Indicate to the main thread that it has completed processing for the frame.
    /// 
    /// Then block until the main thread indicates that processing for the next frame can begin.
    /// 
    /// Returns any input changed since last time.
    pub fn sync_frame(&mut self) -> I {
        self.tx.send(()).expect("couldn't send to main thread");
        self.rx.recv().expect("couldn't get from main thread")
    }
}
