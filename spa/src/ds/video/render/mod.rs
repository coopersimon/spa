
use std::sync::{
    Arc, Mutex
};
use crate::common::videomem::VideoMemory;

pub type RenderTarget = Arc<Mutex<Box<[u8]>>>;

/// Renderer trait. The renderer should implement this.
pub trait Renderer {
    fn new(target: RenderTarget) -> Self;

    /// Render a single line.
    fn render_line(&mut self, mem: &mut VideoMemory, line: u16);
    /// Start rendering the frame.
    fn start_frame(&mut self);
    /// Complete rendering the frame.
    fn finish_frame(&mut self);
    /// Get the size of the render target in pixels.
    fn render_size() -> (usize, usize);
}
