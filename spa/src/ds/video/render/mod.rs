
use parking_lot::Mutex;
use std::sync::Arc;
use crate::common::drawing::{SoftwareRenderer, RendererMode};
use super::{
    memory::{DSVideoMemory, GraphicsPowerControl},
    constants::*
};

pub type RenderTarget = Arc<Mutex<Box<[u8]>>>;

/// Renderer trait. The renderer should implement this.
pub trait Renderer {
    fn new(upper: RenderTarget, lower: RenderTarget) -> Self;

    /// Render a single line.
    fn render_line(&mut self, mem: &mut DSVideoMemory, line: u16);
    /// Start rendering the frame.
    fn start_frame(&mut self, mem: &mut DSVideoMemory);
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
}

impl Renderer for ProceduralRenderer {
    fn new(upper: RenderTarget, lower: RenderTarget) -> Self {
        Self {
            engine_a:   SoftwareRenderer::new(RendererMode::NDSA),
            engine_b:   SoftwareRenderer::new(RendererMode::NDSB),

            upper, lower,
        }
    }

    fn render_line(&mut self, mem: &mut DSVideoMemory, line: u16) {
        let start_offset = (line as usize) * (H_RES * 4);
        let end_offset = start_offset + (H_RES * 4);

        if mem.power_cnt.contains(GraphicsPowerControl::ENABLE_A) {
            let mut engine_a_mem = mem.engine_a_mem.lock();
            
            let mut target = if mem.power_cnt.contains(GraphicsPowerControl::DISPLAY_SWAP) {
                self.upper.lock()
            } else {
                self.lower.lock()
            };

            self.engine_a.setup_caches(&mut engine_a_mem);
            self.engine_a.draw_line_nds_a(&engine_a_mem, &mut mem.vram, &mut target[start_offset..end_offset], line as u8);
            engine_a_mem.registers.inc_v_count();
        }

        if mem.power_cnt.contains(GraphicsPowerControl::ENABLE_B) {
            let mut engine_b_mem = mem.engine_b_mem.lock();

            let mut target = if mem.power_cnt.contains(GraphicsPowerControl::DISPLAY_SWAP) {
                self.lower.lock()
            } else {
                self.upper.lock()
            };

            self.engine_b.setup_caches(&mut engine_b_mem);
            self.engine_b.draw_line_nds_b(&engine_b_mem, &mut target[start_offset..end_offset], line as u8);
            engine_b_mem.registers.inc_v_count();
        }
    }

    fn start_frame(&mut self, mem: &mut DSVideoMemory) {
        mem.engine_a_mem.lock().registers.reset_v_count();
        mem.engine_b_mem.lock().registers.reset_v_count();
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
    engine_a:   SoftwareRenderer,
    engine_b:   SoftwareRenderer,

    upper: RenderTarget,
    lower: RenderTarget,
}

impl Renderer for DebugTileRenderer {
    fn new(upper: RenderTarget, lower: RenderTarget) -> Self {
        Self {
            engine_a:   SoftwareRenderer::new(RendererMode::NDSA),
            engine_b:   SoftwareRenderer::new(RendererMode::NDSB),

            upper, lower
        }
    }

    fn render_line(&mut self, mem: &mut DSVideoMemory, line: u16) {
        if line == 0 {
            {
                let mut engine_a_mem = mem.engine_a_mem.lock();
                self.engine_a.setup_caches(&mut engine_a_mem);
                // Choose out.
                let mut target = self.lower.lock();    // TODO: SELECT (POWCNT)
                self.engine_a.draw_4bpp_tiles(&engine_a_mem, &mut target);
            }
            {
                let mut engine_b_mem = mem.engine_b_mem.lock();
                self.engine_b.setup_caches(&mut engine_b_mem);
                let mut target = self.upper.lock();    // TODO: SELECT (POWCNT)
                self.engine_b.draw_4bpp_tiles(&engine_b_mem, &mut target);
            }
        }
    }

    fn start_frame(&mut self, _mem: &mut DSVideoMemory) {
        //println!("Start frame");
    }

    fn finish_frame(&mut self) {
        //println!("Finish frame");
    }

    fn render_size() -> (usize, usize) {
        (256, 384)
    }
}
