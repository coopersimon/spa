
use std::sync::{
    Arc, Mutex
};
use crate::common::{
    videomem::VideoRegisters,
    drawing::{SoftwareRenderer, RendererMode}
};
use super::{
    memory::{DSVideoMemory, GraphicsPowerControl, VRAMRegion, ARM9VRAM},
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

        if mem.power_cnt.contains(GraphicsPowerControl::ENABLE_A) {
            let mut engine_a_mem = mem.engine_a_mem.lock().unwrap();
            
            let mut target = if mem.power_cnt.contains(GraphicsPowerControl::DISPLAY_SWAP) {
                self.upper.lock().unwrap()
            } else {
                self.lower.lock().unwrap()
            };

            match engine_a_mem.registers.display_mode() {
                0 => self.draw_blank_line(&mut target[start_offset..end_offset]),
                1 => {
                    self.engine_a.setup_caches(&mut engine_a_mem);
                    self.engine_a.draw_line(&engine_a_mem, &mut target[start_offset..end_offset], line as u8)
                },
                2 => self.draw_vram(&mem.vram, &engine_a_mem.registers, &mut target[start_offset..end_offset], line as u32),
                //3 => self.draw_blank_line(&mut target[start_offset..end_offset]),// TODO
                _ => unreachable!()
            }

            // TODO: capture
        }

        if mem.power_cnt.contains(GraphicsPowerControl::ENABLE_B) {
            let mut engine_b_mem = mem.engine_b_mem.lock().unwrap();

            let mut target = if mem.power_cnt.contains(GraphicsPowerControl::DISPLAY_SWAP) {
                self.lower.lock().unwrap()
            } else {
                self.upper.lock().unwrap()
            };

            match engine_b_mem.registers.display_mode() {
                0 => self.draw_blank_line(&mut target[start_offset..end_offset]),
                1 => {
                    self.engine_b.setup_caches(&mut engine_b_mem);
                    self.engine_b.draw_line(&engine_b_mem, &mut target[start_offset..end_offset], line as u8)
                },
                _ => unreachable!()
            }
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

impl ProceduralRenderer {
    /// For when drawing mode is disabled.
    fn draw_blank_line(&self, target: &mut [u8]) {
        for p in target {
            *p = 0xFF;
        }
    }

    // TODO: move to drawing module?
    /// Draw bitmap from VRAM.
    pub fn draw_vram(&self, mem: &ARM9VRAM, registers: &VideoRegisters, target: &mut [u8], line: u32) {
        let read_offset = line * (H_RES as u32) * 2;
        let region = match registers.vram_block() {
            0 => VRAMRegion::A,
            1 => VRAMRegion::B,
            2 => VRAMRegion::C,
            3 => VRAMRegion::D,
            _ => unreachable!(),
        };
        // TODO: what to do if this fails?
        if let Some(vram) = mem.ref_block(region) {
            for x in 0..H_RES {
                let dest = x * 4;
                let data = vram.read_halfword(read_offset + (x as u32) * 2);
                let colour = crate::common::drawing::colour::Colour::from_555(data);
                target[dest] = colour.r;
                target[dest + 1] = colour.g;
                target[dest + 2] = colour.b;
            }
        }
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
                let mut engine_a_mem = mem.engine_a_mem.lock().unwrap();
                self.engine_a.setup_caches(&mut engine_a_mem);
                // Choose out.
                let mut target = self.lower.lock().unwrap();    // TODO: SELECT (POWCNT)
                self.engine_a.draw_4bpp_tiles(&engine_a_mem, &mut target);
            }
            {
                let mut engine_b_mem = mem.engine_b_mem.lock().unwrap();
                self.engine_b.setup_caches(&mut engine_b_mem);
                let mut target = self.upper.lock().unwrap();    // TODO: SELECT (POWCNT)
                self.engine_b.draw_4bpp_tiles(&engine_b_mem, &mut target);
            }
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
