/// Rendering the video.

use parking_lot::Mutex;
use std::sync::Arc;
use super::memory::VRAMRenderRef;
use super::constants::*;
use crate::common::video::{
    colour::Colour,
    drawing::{
        SoftwareRenderer, RendererMode
    },
    mem::VideoMemory
};

pub type RenderTarget = Arc<Mutex<Box<[u8]>>>;

/// Renderer trait. The renderer should implement this.
pub trait Renderer {
    fn new(target: RenderTarget) -> Self;

    /// Render a single line.
    fn render_line(&mut self, mem: &mut VideoMemory<VRAMRenderRef>, line: u8);
    /// Start rendering the frame.
    fn start_frame(&mut self, mem: &mut VideoMemory<VRAMRenderRef>);
    /// Complete rendering the frame.
    fn finish_frame(&mut self);
    /// Get the size of the render target in pixels.
    fn render_size() -> (usize, usize);
}

pub struct ProceduralRenderer {
    renderer:   SoftwareRenderer,

    target:     RenderTarget
}

impl Renderer for ProceduralRenderer {
    fn new(target: RenderTarget) -> Self {
        Self {
            renderer:   SoftwareRenderer::new(RendererMode::GBA),
            target:     target,
        }
    }

    fn render_line(&mut self, mem: &mut VideoMemory<VRAMRenderRef>, line: u8) {
        self.renderer.setup_caches(mem);
        let start_offset = (line as usize) * (H_RES * 4);
        let end_offset = start_offset + (H_RES * 4);
        let mut target = self.target.lock();

        {
            let target = &mut target[start_offset..end_offset];
            if mem.registers.in_fblank() {
                self.renderer.draw_blank_line(target);
            } else {
                // TODO: don't re-alloc every time
                let mut line_cache = vec![Colour::black(); 240];
                self.renderer.draw(mem, &mut line_cache, line);
                for (colour, out) in line_cache.iter().zip(target.chunks_exact_mut(4)) {
                    out[0] = colour.r;
                    out[1] = colour.g;
                    out[2] = colour.b;
                }
            }
        }

        mem.registers.inc_v_count();
    }

    fn start_frame(&mut self, mem: &mut VideoMemory<VRAMRenderRef>) {
        mem.registers.reset_v_count();
    }

    fn finish_frame(&mut self) {
        //println!("Finish frame");
    }

    fn render_size() -> (usize, usize) {
        (H_RES, V_RES)
    }
}

pub struct DebugTileRenderer {
    renderer:   SoftwareRenderer,

    target:     RenderTarget
}

impl Renderer for DebugTileRenderer {
    fn new(target: RenderTarget) -> Self {
        Self {
            renderer:   SoftwareRenderer::new(RendererMode::GBA),
            target:     target,
        }
    }

    fn render_line(&mut self, mem: &mut VideoMemory<VRAMRenderRef>, line: u8) {
        self.renderer.setup_caches(mem);
        if line == 0 {
            let mut target = self.target.lock();
            self.renderer.draw_8bpp_tiles(mem, &mut target);
        }
    }

    fn start_frame(&mut self, _mem: &mut VideoMemory<VRAMRenderRef>) {
        //println!("Start frame");
    }

    fn finish_frame(&mut self) {
        //println!("Finish frame");
    }

    fn render_size() -> (usize, usize) {
        (256, 384)
    }
}
