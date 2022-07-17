
use crossbeam_channel::{bounded, Sender, Receiver};
use parking_lot::Mutex;
use std::sync::Arc;
use crate::common::colour::Colour;
use crate::common::drawing::{
    SoftwareRenderer, RendererMode
};
use crate::common::videomem::{DispCapSourceB, DispCapMode, DispCapSourceA, VideoRegisters, VideoMemory};
use super::memory::ARM9VRAM;
use super::{
    memory::{RendererVRAM, EngineAVRAM, EngineBVRAM, GraphicsPowerControl},
    constants::*
};

pub type RenderTarget = Arc<Mutex<Box<[u8]>>>;

/// Renderer trait. The renderer should implement this.
pub trait Renderer {
    fn new(upper: RenderTarget, lower: RenderTarget, vram: RendererVRAM) -> Self;

    /// Render a single line.
    fn render_line(&mut self, line: u16);
    /// Start rendering the frame.
    fn start_frame(&mut self);
    /// Complete rendering the frame.
    fn finish_frame(&mut self);
    /// Get the size of each render target in pixels.
    fn render_size() -> (usize, usize);
}

pub struct ProceduralRenderer {
    command_tx: Sender<u16>,
    reply_rx: Receiver<()>
}

pub struct ProceduralRendererThread {
    engine_a:   SoftwareRenderer,
    engine_b:   SoftwareRenderer,

    upper: RenderTarget,
    lower: RenderTarget,

    vram:   RendererVRAM
}

impl Renderer for ProceduralRenderer {
    fn new(upper: RenderTarget, lower: RenderTarget, vram: RendererVRAM) -> Self {

        let (command_tx, command_rx) = bounded(1);
        let (reply_tx, reply_rx) = bounded(1);
        std::thread::spawn(move || {

            let mut data = ProceduralRendererThread {
                engine_a:   SoftwareRenderer::new(RendererMode::NDSA),
                engine_b:   SoftwareRenderer::new(RendererMode::NDSB),

                upper, lower, vram
            };

            reply_tx.send(()).unwrap();

            while let Ok(line) = command_rx.recv() {
                if line == 0 {
                    data.start_frame();
                }
                data.render_line(line);
                reply_tx.send(()).unwrap();
            }
        });

        Self { command_tx, reply_rx }
    }

    fn render_line(&mut self, line: u16) {
        self.reply_rx.recv().unwrap();
        self.command_tx.send(line).unwrap();
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

impl ProceduralRendererThread {

    fn start_frame(&mut self) {
        self.vram.engine_a_mem.lock().registers.reset_v_count();
        self.vram.engine_b_mem.lock().registers.reset_v_count();
    }

    fn render_line(&mut self, line: u16) {
        let start_offset = (line as usize) * (H_RES * 4);
        let end_offset = start_offset + (H_RES * 4);
        let power_cnt = self.vram.read_power_cnt();

        if power_cnt.contains(GraphicsPowerControl::ENABLE_A) {
            let mut target = if power_cnt.contains(GraphicsPowerControl::DISPLAY_SWAP) {
                self.upper.lock()
            } else {
                self.lower.lock()
            };

            let mut engine_a_mem = self.vram.engine_a_mem.lock();

            self.engine_a.setup_caches(&mut engine_a_mem);    
            self.engine_a_line(&mut engine_a_mem, &mut target[start_offset..end_offset], line as u8);
        }

        if power_cnt.contains(GraphicsPowerControl::ENABLE_B) {
            let mut target = if power_cnt.contains(GraphicsPowerControl::DISPLAY_SWAP) {
                self.lower.lock()
            } else {
                self.upper.lock()
            };

            let mut engine_b_mem = self.vram.engine_b_mem.lock();

            self.engine_b.setup_caches(&mut engine_b_mem);
            self.engine_b_line(&mut engine_b_mem, &mut target[start_offset..end_offset], line as u8);
        }
    }

    /// Draw a full line for NDS A engine. Also applies master brightness
    /// 
    /// Also is responsible for video capture output.
    /// 
    /// TODO: 3D
    fn engine_a_line(&self, engine_a_mem: &mut VideoMemory<EngineAVRAM>, target: &mut [u8], line: u8) {
        
        let mut drawn = false;
        // TODO: avoid alloc every time
        let mut line_cache = vec![Colour::black(); H_RES];

        if engine_a_mem.registers.in_fblank() {
            for p in target {
                *p = 0;
            }
        } else {
            match engine_a_mem.registers.display_mode() {
                0 => self.engine_a.draw_blank_line(target),
                1 => {
                    self.engine_a.draw(&engine_a_mem, &mut line_cache, line);
                    for (colour, out) in line_cache.iter().zip(target.chunks_exact_mut(4)) {
                        let colour = engine_a_mem.registers.apply_brightness(*colour);
                        out[0] = colour.r;
                        out[1] = colour.g;
                        out[2] = colour.b;
                    }
                    drawn = true;
                },
                2 => {
                    let lcdc = self.vram.lcdc_vram.lock();
                    let read_offset = (line as usize) * H_RES;
                    self.draw_from_vram(&lcdc, &engine_a_mem.registers, &mut line_cache, read_offset);
                    for (colour, out) in line_cache.iter().zip(target.chunks_exact_mut(4)) {
                        let colour = engine_a_mem.registers.apply_brightness(*colour);
                        out[0] = colour.r;
                        out[1] = colour.g;
                        out[2] = colour.b;
                    }
                },
                3 => panic!("main mem display not implemented yet!"),
                _ => unreachable!()
            }
        }

        if let Some(disp_cap_mode) = engine_a_mem.registers.display_capture_mode() {
            let write_size = engine_a_mem.registers.vram_capture_write_size();
            if (line as usize) >= write_size.1 {
                // Outside of writing bounds.
                return;
            }

            match disp_cap_mode {
                DispCapMode::A(src_a) => match src_a {
                    DispCapSourceA::Engine => {
                        if !drawn {
                            self.engine_a.draw(&engine_a_mem, &mut line_cache, line);
                        }
                    },
                    DispCapSourceA::_3D => {
                        // TODO
                    }
                },
                DispCapMode::B(src_b) => match src_b {
                    DispCapSourceB::VRAM => {
                        let lcdc = self.vram.lcdc_vram.lock();
                        let read_offset = engine_a_mem.registers.vram_capture_read_offset() + (line as usize) * H_RES;
                        self.draw_from_vram(&lcdc, &engine_a_mem.registers, &mut line_cache, read_offset);
                    },
                    DispCapSourceB::MainRAM => panic!("main mem capture not implemented yet!"),
                },
                DispCapMode::Blend{src_a, src_b, eva, evb} => {
                    let mut line_cache_b = vec![Colour::black(); 256];
                    match src_a {
                        DispCapSourceA::Engine => {
                            if !drawn {
                                self.engine_a.draw(&engine_a_mem, &mut line_cache, line);
                            }
                        },
                        DispCapSourceA::_3D => {
                            // TODO
                        }
                    }
                    match src_b {
                        DispCapSourceB::VRAM => {
                            let lcdc = self.vram.lcdc_vram.lock();
                            let read_offset = engine_a_mem.registers.vram_capture_read_offset() + (line as usize) * H_RES;
                            self.draw_from_vram(&lcdc, &engine_a_mem.registers, &mut line_cache_b, read_offset);
                        },
                        DispCapSourceB::MainRAM => panic!("main mem capture not implemented yet!"),
                    }
                    // Blend.
                    for (a, b) in line_cache.iter_mut().zip(&line_cache_b) {
                        *a = SoftwareRenderer::apply_alpha_blend(eva, evb, *a, *b);
                    }
                }
            }

            // TODO: fix writing here.
            //let mut lcdc = self.vram.lcdc_vram.lock();
            //let write_offset = mem.registers.vram_capture_write_offset() + (line as usize) * write_size.0;
            //self.write_to_vram(&mut lcdc, &mem.registers, &line_cache[0..write_size.0], write_offset);
        }

        engine_a_mem.registers.inc_v_count();
    }

    /// Draw a full line for NDS B engine. Also applies master brightness.
    fn engine_b_line(&self, engine_b_mem: &mut VideoMemory<EngineBVRAM>, target: &mut [u8], line: u8) {
        
        if engine_b_mem.registers.in_fblank() {
            self.engine_b.draw_blank_line(target);
        } else {
            match engine_b_mem.registers.display_mode() {
                0 => self.draw_empty_line(target),
                1 => {
                    let mut line_cache = vec![Colour::black(); 256];
                    self.engine_b.draw(&engine_b_mem, &mut line_cache, line as u8);
                    for (colour, out) in line_cache.iter().zip(target.chunks_exact_mut(4)) {
                        let colour = engine_b_mem.registers.apply_brightness(*colour);
                        out[0] = colour.r;
                        out[1] = colour.g;
                        out[2] = colour.b;
                    }
                },
                _ => unreachable!()
            }
        }

        engine_b_mem.registers.inc_v_count();
    }

    /// For when drawing mode is disabled.
    fn draw_empty_line(&self, target: &mut [u8]) {
        for p in target {
            *p = 0xFF;
        }
    }

    /// Draw bitmap from VRAM.
    /// 
    /// Read offset is in pixels (16-bit chunks)
    fn draw_from_vram(&self, mem: &ARM9VRAM, registers: &VideoRegisters, target: &mut [Colour], read_offset: usize) {
        if let Some(vram) = mem.ref_region(registers.read_vram_block()) {
            let vram_bytes = vram.ref_mem().chunks_exact(2).skip(read_offset);
            for (out, data) in target.iter_mut().zip(vram_bytes) {
                let raw_colour = u16::from_le_bytes(data.try_into().unwrap());
                *out = Colour::from_555(raw_colour);
            }
        }
    }

    /// Write capture to VRAM.
    /// 
    /// Read offset is in pixels (16-bit chunks)
    fn write_to_vram(&self, mem: &mut ARM9VRAM, registers: &VideoRegisters, source: &[Colour], write_offset: usize) {
        if let Some(vram) = mem.mut_region(registers.write_vram_block()) {
            let vram_bytes = vram.mut_mem().chunks_exact_mut(2).skip(write_offset);
            for (colour, data) in source.iter().zip(vram_bytes) {
                let raw_colour = colour.to_555().to_le_bytes();
                data[0] = raw_colour[0];
                data[1] = raw_colour[1] | 0x80; // TODO: alpha channel
            }
        }
    }
}

pub struct DebugTileRenderer {
    engine_a:   SoftwareRenderer,
    engine_b:   SoftwareRenderer,

    upper: RenderTarget,
    lower: RenderTarget,

    vram:   RendererVRAM
}

impl Renderer for DebugTileRenderer {
    fn new(upper: RenderTarget, lower: RenderTarget, vram: RendererVRAM) -> Self {
        Self {
            engine_a:   SoftwareRenderer::new(RendererMode::NDSA),
            engine_b:   SoftwareRenderer::new(RendererMode::NDSB),

            upper, lower, vram
        }
    }

    fn render_line(&mut self, line: u16) {
        if line == 0 {
            {
                let mut engine_a_mem = self.vram.engine_a_mem.lock();
                self.engine_a.setup_caches(&mut engine_a_mem);
                // Choose out.
                let mut target = self.lower.lock();    // TODO: SELECT (POWCNT)
                self.engine_a.draw_4bpp_tiles(&engine_a_mem, &mut target);
            }
            {
                let mut engine_b_mem = self.vram.engine_b_mem.lock();
                self.engine_b.setup_caches(&mut engine_b_mem);
                let mut target = self.upper.lock();    // TODO: SELECT (POWCNT)
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
