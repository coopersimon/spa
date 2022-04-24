
use std::sync::{
    Arc, Mutex
};
use super::{
    memory::DSVideoMemory,
    constants::*
};

pub type RenderTarget = Arc<Mutex<Box<[u8]>>>;

/// Renderer trait. The renderer should implement this.
pub trait Renderer {
    fn new(upper: RenderTarget, lower: RenderTarget) -> Self;

    /// Render a single line.
    fn render_line(&mut self, mem: &mut DSVideoMemory, line: u16);
    /// Start rendering the frame.
    fn start_frame(&mut self);
    /// Complete rendering the frame.
    fn finish_frame(&mut self);
    /// Get the size of each render target in pixels.
    fn render_size() -> (usize, usize);
}

pub struct ProceduralRenderer {
    //renderer:   SoftwareRenderer,

    upper: RenderTarget,
    lower: RenderTarget
}

impl Renderer for ProceduralRenderer {
    fn new(upper: RenderTarget, lower: RenderTarget) -> Self {
        Self {
            //renderer:   SoftwareRenderer::new(H_RES),
            upper, lower
        }
    }

    fn render_line(&mut self, mem: &mut DSVideoMemory, line: u16) {
        //self.renderer.setup_caches(mem);
        //let start_offset = (line as usize) * (H_RES * 4);
        //let end_offset = start_offset + (H_RES * 4);
        //let mut target = self.target.lock().unwrap();
        //self.renderer.draw_line(mem, &mut target[start_offset..end_offset], line);
    }

    fn start_frame(&mut self) {
        //println!("Start frame");
    }

    fn finish_frame(&mut self) {
        //println!("Finish frame");
    }

    fn render_size() -> (usize, usize) {
        (H_RES, V_RES)
    }
}
