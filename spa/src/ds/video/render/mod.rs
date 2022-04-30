
use std::sync::{
    Arc, Mutex
};
use crate::common::drawing::{SoftwareRenderer, RendererMode};
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
    engine_a:   SoftwareRenderer,
    engine_b:   SoftwareRenderer,

    upper: RenderTarget,
    lower: RenderTarget,

    engine_a_output: Vec<u8>
}

impl Renderer for ProceduralRenderer {
    fn new(upper: RenderTarget, lower: RenderTarget) -> Self {
        Self {
            engine_a:   SoftwareRenderer::new(RendererMode::NDSA),
            engine_b:   SoftwareRenderer::new(RendererMode::NDSB),

            upper, lower,

            engine_a_output: vec![0; V_RES * H_RES * 4]
        }
    }

    fn render_line(&mut self, mem: &mut DSVideoMemory, line: u16) {
        let start_offset = (line as usize) * (H_RES * 4);
        let end_offset = start_offset + (H_RES * 4);
        {
            let mut engine_a_mem = mem.engine_a_mem.lock().unwrap();
            self.engine_a.setup_caches(&mut engine_a_mem);
            // Choose out.
            self.engine_a.draw_line(&engine_a_mem, &mut self.engine_a_output[start_offset..end_offset], line as u8);
            // TODO: composite engine A
        }
        {
            let mut engine_b_mem = mem.engine_b_mem.lock().unwrap();
            self.engine_b.setup_caches(&mut engine_b_mem);
            let mut target = self.upper.lock().unwrap();    // TODO: SELECT (POWCNT)
            self.engine_b.draw_line(&engine_b_mem, &mut target[start_offset..end_offset], line as u8);
        }
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
