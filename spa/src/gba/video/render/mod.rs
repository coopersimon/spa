/// Rendering the video.

mod drawing;
mod colour;

use std::sync::{
    Arc, Mutex
};
use crate::gba::constants::*;

pub type RenderTarget = Arc<Mutex<Box<[u8]>>>;

/// Renderer trait. The renderer should implement this.
pub trait Renderer {
    fn new(target: RenderTarget) -> Self;

    /// Render a single line.
    fn render_line(&mut self, mem: &mut super::VideoMemory, line: u8);
    /// Start rendering the frame.
    fn start_frame(&mut self);
    /// Complete rendering the frame.
    fn finish_frame(&mut self);
    /// Get the size of the render target in pixels.
    fn render_size() -> (usize, usize);
}

pub struct ProceduralRenderer {
    renderer:   drawing::SoftwareRenderer,

    target:     RenderTarget
}

impl Renderer for ProceduralRenderer {
    fn new(target: RenderTarget) -> Self {
        Self {
            renderer:   drawing::SoftwareRenderer::new(),
            target:     target,
        }
    }

    fn render_line(&mut self, mem: &mut super::VideoMemory, line: u8) {
        self.renderer.setup_caches(mem);
        let start_offset = (line as usize) * (H_RES * 4);
        let end_offset = start_offset + (H_RES * 4);
        let mut target = self.target.lock().unwrap();
        self.renderer.draw_line(mem, &mut target[start_offset..end_offset], line);
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

pub struct DebugTileRenderer {
    renderer:   drawing::SoftwareRenderer,

    target:     RenderTarget
}

impl Renderer for DebugTileRenderer {
    fn new(target: RenderTarget) -> Self {
        Self {
            renderer:   drawing::SoftwareRenderer::new(),
            target:     target,
        }
    }

    fn render_line(&mut self, mem: &mut super::VideoMemory, line: u8) {
        self.renderer.setup_caches(mem);
        if line == 0 {
            let mut target = self.target.lock().unwrap();
            self.renderer.draw_8bpp_tiles(mem, &mut target);
        }
    }

    fn start_frame(&mut self) {
        //println!("Start frame");
    }

    fn finish_frame(&mut self) {
        //println!("Finish frame");
    }

    fn render_size() -> (usize, usize) {
        (256, 384)
    }
}
