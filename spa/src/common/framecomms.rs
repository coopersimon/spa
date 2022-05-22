
use crossbeam_channel::{Sender, Receiver, bounded};
use parking_lot::Mutex;
use std::{
    sync::Arc
};
use crate::FrameBuffer;

/// Make a link between the realtime world and the emulator.
/// 
/// The main thread (real-time) requests frames and provides inputs since the last frame.
/// 
/// I is the input type.
/// 
/// 1 frame is required for GBA, 2 for NDS.
pub fn new_frame_comms<I>(frame_size: usize, frame_count: usize) -> (FrameSender<I>, FrameRequester<I>) {
    let frame_buffers = (0..frame_count)
        .map(|_| vec![0; frame_size])
        .map(|buffer| Arc::new(Mutex::new(buffer.into_boxed_slice())))
        .collect::<Vec<_>>();
    let (sync_tx, sync_rx) = bounded(1);
    let (data_tx, data_rx) = bounded(1);
    (
        FrameSender{frame_buffers: frame_buffers.clone(), tx: data_tx, rx: sync_rx},
        FrameRequester{frame_buffers: frame_buffers, tx: sync_tx, rx: data_rx}
    )
}

#[cfg(feature = "debug")]
pub mod debug {
    use super::*;
    pub fn new_debug_frame_comms<I>(frame_size: usize, frame_count: usize) -> (FrameSender<I>, DebugFrameReq<I>) {
        let frame_buffers = (0..frame_count)
            .map(|_| vec![0; frame_size])
            .map(|buffer| Arc::new(Mutex::new(buffer.into_boxed_slice())))
            .collect::<Vec<_>>();
        let (sync_tx, sync_rx) = bounded(1);
        let (data_tx, data_rx) = bounded(1);
        (
            FrameSender{frame_buffers: frame_buffers, tx: data_tx, rx: sync_rx},
            DebugFrameReq{tx: sync_tx, rx: data_rx}
        )
    }
    
    pub struct DebugFrameReq<I> {
        pub tx: Sender<I>,
        pub rx: Receiver<()>
    }
}

pub struct FrameRequester<I> {
    frame_buffers:   Vec<Arc<Mutex<FrameBuffer>>>,

    tx: Sender<I>,
    rx: Receiver<()>
}

impl<I> FrameRequester<I> {
    /// Indicate to the CPU thread that it is ready for a new frame set.
    /// 
    /// Extracts the next frame set, and sends user input since last frame.
    pub fn get_frame(&mut self, buffers: &mut [&mut [u8]], input: I) {
        // Wait for CPU thread to let us know its processing is complete.
        self.rx.recv().expect("couldn't get from cpu thread");
        // Copy frame into buffer.
        for (frame_buffer, out_buffer) in self.frame_buffers.iter().zip(buffers) {
            let frame = frame_buffer.lock();
            out_buffer.copy_from_slice(&(*frame));
        }
        // Let CPU thread know processing can continue.
        self.tx.send(input).expect("couldn't send to cpu thread");
    }
}

pub struct FrameSender<I> {
    frame_buffers:   Vec<Arc<Mutex<FrameBuffer>>>,
    
    tx: Sender<()>,
    rx: Receiver<I>
}

impl<I> FrameSender<I> {

    /// Clone a frame buffer Arc.
    pub fn get_frame_buffer(&self, idx: usize) -> Arc<Mutex<FrameBuffer>> {
        self.frame_buffers[idx].clone()
    }

    /// Indicate to the main thread that it has completed processing for the frame set.
    /// 
    /// Then block until the main thread indicates that processing for the next frame set can begin.
    /// 
    /// Returns any input changed since last time.
    pub fn sync_frame(&mut self) -> I {
        self.tx.send(()).expect("couldn't send to main thread");
        self.rx.recv().expect("couldn't get from main thread")
    }
}
