
use crossbeam_channel::{Sender, Receiver, bounded};
use std::{
    sync::Arc,
    sync::Mutex,
};
use crate::FrameBuffer;
use crate::joypad::Buttons;

pub fn new_frame_comms(frame_size: usize) -> (FrameSender, FrameRequester) {
    let buffer = vec![0; frame_size];
    let frame_buffer = Arc::new(Mutex::new(buffer.into_boxed_slice()));
    let (sync_tx, sync_rx) = bounded(1);
    let (data_tx, data_rx) = bounded(1);
    (
        FrameSender{frame_buffer: frame_buffer.clone(), tx: data_tx, rx: sync_rx},
        FrameRequester{frame_buffer: frame_buffer, tx: sync_tx, rx: data_rx}
    )
}

pub struct FrameRequester {
    frame_buffer:   Arc<Mutex<FrameBuffer>>,

    tx: Sender<Buttons>,
    rx: Receiver<()>
}

impl FrameRequester {
    /// Indicate to the CPU thread that it is ready for a new frame.
    /// 
    /// Extracts the next frame, and sends buttons pressed since last time.
    pub fn get_frame(&mut self, buffer: &mut [u8], buttons: Buttons) {
        // Wait for CPU thread to let us know its processing is complete.
        self.rx.recv().expect("couldn't get from cpu thread");
        // Copy frame into buffer.
        let frame = self.frame_buffer.lock().unwrap();
        buffer.copy_from_slice(&(*frame));
        // Let CPU thread know processing can continue.
        self.tx.send(buttons).expect("couldn't send to cpu thread");
    }
}

pub struct FrameSender {
    frame_buffer:   Arc<Mutex<FrameBuffer>>,
    
    tx: Sender<()>,
    rx: Receiver<Buttons>
}

impl FrameSender {

    /// Clone the frame buffer Arc.
    pub fn get_frame_buffer(&self) -> Arc<Mutex<FrameBuffer>> {
        self.frame_buffer.clone()
    }

    /// Indicate to the main thread that it has completed processing for the frame.
    /// 
    /// Then block until the main thread indicates that processing for the next frame can begin.
    /// 
    /// Returns any buttons pressed since last time.
    pub fn sync_frame(&mut self) -> Buttons {
        self.tx.send(()).expect("couldn't send to main thread");
        self.rx.recv().expect("couldn't get from main thread")
    }
}
