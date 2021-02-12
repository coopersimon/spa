/// Rendering the video.

mod drawing;
mod colour;

use crate::constants::*;

/// Renderer trait. The renderer should implement this.
pub trait Renderer {
    fn new() -> Self;

    /// Render a single line.
    fn render_line(&mut self, mem: &mut super::VideoMemory, line: u16);
    /// Start rendering the frame.
    fn start_frame(&mut self);
    /// Complete rendering the frame.
    fn finish_frame(&mut self);
    /// Copy the frame data into the buffer provided.
    fn get_frame_data(&self, buffer: &mut [u8]);
    /// Get the size of the render target in pixels.
    fn render_size(&self) -> (usize, usize);
}

pub struct ProceduralRenderer {
    renderer:   drawing::SoftwareRenderer,

    target:     Vec<u8>
}

impl Renderer for ProceduralRenderer {
    fn new() -> Self {
        Self {
            renderer:   drawing::SoftwareRenderer::new(),
            target:     vec![0; gba::H_RES * gba::V_RES * 4],
        }
    }

    fn render_line(&mut self, mem: &mut super::VideoMemory, line: u16) {
        self.renderer.setup_caches(mem);
        let start_offset = (line as usize) * (gba::H_RES * 4);
        let end_offset = start_offset + (gba::H_RES * 4);
        self.renderer.draw_line(mem, &mut self.target[start_offset..end_offset], line);
    }

    fn start_frame(&mut self) {
        //println!("Start frame");
    }

    fn finish_frame(&mut self) {
        //println!("Finish frame");
    }

    fn get_frame_data(&self, buffer: &mut [u8]) {
        buffer.copy_from_slice(&self.target);
    }

    fn render_size(&self) -> (usize, usize) {
        (gba::H_RES, gba::V_RES)
    }
}

pub struct DebugTileRenderer {
    renderer:   drawing::SoftwareRenderer,

    target:     Vec<u8>
}

impl Renderer for DebugTileRenderer {
    fn new() -> Self {
        Self {
            renderer:   drawing::SoftwareRenderer::new(),
            target:     vec![0; 256 * 384 * 4],
        }
    }

    fn render_line(&mut self, mem: &mut super::VideoMemory, line: u16) {
        self.renderer.setup_caches(mem);
        if line == 0 {
            self.renderer.draw_8bpp_tiles(mem, &mut self.target);
        }
    }

    fn start_frame(&mut self) {
        //println!("Start frame");
    }

    fn finish_frame(&mut self) {
        //println!("Finish frame");
    }

    fn get_frame_data(&self, buffer: &mut [u8]) {
        buffer.copy_from_slice(&self.target);
    }

    fn render_size(&self) -> (usize, usize) {
        (256, 384)
    }
}
