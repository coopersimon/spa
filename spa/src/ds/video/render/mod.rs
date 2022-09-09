
use crossbeam_channel::{bounded, Sender};
use parking_lot::Mutex;
use std::sync::Arc;
use crate::common::video::{
    colour::Colour,
    drawing::{
        SoftwareRenderer, RendererMode
    },
    mem::{DispCapSourceB, DispCapMode, DispCapSourceA, VideoRegisters}
};
use super::memory::ARM9VRAM;
use super::{
    memory::{RendererVRAM, GraphicsPowerControl},
    video3d::Software3DRenderer,
    constants::*
};

pub type RenderTarget = Arc<Mutex<Box<[u8]>>>;

/// Renderer trait. The renderer should implement this.
pub trait Renderer {
    fn new(upper: RenderTarget, lower: RenderTarget, vram: RendererVRAM) -> Self;

    /// Render 3D content.
    fn render_3d(&mut self);
    /// Render a single line.
    fn render_line(&mut self, line: u16);
    /// Start rendering the frame.
    fn start_frame(&mut self);
    /// Complete rendering the frame.
    fn finish_frame(&mut self);
    /// Get the size of each render target in pixels.
    fn render_size() -> (usize, usize);
}

enum RenderCommand {
    Normal(u16),
    _3D,
}

pub struct ProceduralRenderer {
    command_tx: Sender<RenderCommand>,
    //reply_rx: Receiver<()>
}

pub struct ProceduralRendererThread {
    engine_a:   SoftwareRenderer,
    engine_b:   SoftwareRenderer,
    engine_3d:  Software3DRenderer,

    upper:      RenderTarget,
    lower:      RenderTarget,
    vram:       RendererVRAM,

    /// Indicates whether this frame should be captured.
    capture:    bool,
    /// Internal line cache. Used for capturing engine A video.
    line_cache: Vec<Colour>,
    /// Metadata used for writing captured video data.
    write_data: Option<CaptureWriteData>,

    /// Line cache for engine A blending and engine B.
    line_cache_b:   Vec<Colour>,
}

struct CaptureWriteData {
    /// Offset in bytes
    offset: u32,
    /// Width of capture in pixels
    size:   u32,
    /// Which VRAM block to capture to.
    /// 0-3 corresponds to A-D.
    block:  u16,
}

impl Renderer for ProceduralRenderer {
    fn new(upper: RenderTarget, lower: RenderTarget, vram: RendererVRAM) -> Self {

        let (command_tx, command_rx) = bounded(1);
        //let (reply_tx, reply_rx) = bounded(1);
        std::thread::spawn(move || {

            let mut data = ProceduralRendererThread {
                engine_a:   SoftwareRenderer::new(RendererMode::NDSA),
                engine_b:   SoftwareRenderer::new(RendererMode::NDSB),
                engine_3d:  Software3DRenderer::new(),

                upper, lower, vram,

                capture:    false,
                line_cache: vec![Colour::black(); H_RES],
                write_data: None,
                line_cache_b: vec![Colour::black(); H_RES],
            };

            //reply_tx.send(()).unwrap();

            while let Ok(cmd) = command_rx.recv() {
                match cmd {
                    RenderCommand::Normal(line) => {
                        if line == 0 {
                            data.start_frame();
                        }
                        data.render_line(line);
                        if line == V_MAX {
                            data.finish_frame();
                        }
                    }
                    RenderCommand::_3D => data.render_3d()
                }
                
                //reply_tx.send(()).unwrap();
            }
        });

        Self { command_tx }
    }

    fn render_3d(&mut self) {
        self.command_tx.send(RenderCommand::_3D).unwrap();
    }

    fn render_line(&mut self, line: u16) {
        //self.reply_rx.recv().unwrap();
        self.command_tx.send(RenderCommand::Normal(line)).unwrap();
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
        {
            let mut engine_a_mem = self.vram.engine_a_mem.lock();
            engine_a_mem.registers.reset_v_count();
            if engine_a_mem.registers.display_capture_enabled() {
                self.capture = true;
            }
        }
        self.vram.engine_b_mem.lock().registers.reset_v_count();
    }

    fn finish_frame(&mut self) {
        if self.capture {
            self.vram.engine_a_mem.lock().registers.clear_display_capture();
            self.capture = false;
        }
    }

    fn render_3d(&mut self) {
        let power_cnt = self.vram.read_power_cnt();

        if power_cnt.contains(GraphicsPowerControl::RENDER_3D) {
            let mut engine_3d_mem = self.vram.engine_3d_vram.lock();
            self.engine_3d.setup_caches(&mut engine_3d_mem);  

            let render_engine = self.vram.render_engine.lock();

            self.engine_3d.draw(&render_engine, &engine_3d_mem, &mut self.engine_a.frame_3d);
        }
    }

    fn render_line(&mut self, line: u16) {
        let power_cnt = self.vram.read_power_cnt();

        if power_cnt.contains(GraphicsPowerControl::ENABLE_A) {
            self.engine_a_line(line as u8, power_cnt.contains(GraphicsPowerControl::DISPLAY_SWAP));
        }

        if power_cnt.contains(GraphicsPowerControl::ENABLE_B) {
            self.engine_b_line(line as u8, power_cnt.contains(GraphicsPowerControl::DISPLAY_SWAP));
        }

        if let Some(write_data) = std::mem::take(&mut self.write_data) {
            let mut lcdc = self.vram.lcdc_vram.lock();
            Self::write_to_vram(&mut lcdc, write_data.block, &self.line_cache[0..(write_data.size as usize)], write_data.offset);
        }
    }

    /// Draw a full line for NDS A engine. Also applies master brightness.
    fn engine_a_line(&mut self, line: u8, display_swap: bool) {
        let start_offset = (line as usize) * (H_RES * 4);
        let end_offset = start_offset + (H_RES * 4);

        let mut screen = if display_swap {
            self.upper.lock()
        } else {
            self.lower.lock()
        };
        let target = &mut screen[start_offset..end_offset];

        let mut engine_a_mem = self.vram.engine_a_mem.lock();
        self.engine_a.setup_caches(&mut engine_a_mem);  
        
        let mut drawn = false;

        if engine_a_mem.registers.in_fblank() {
            for p in target {
                *p = 0;
            }
        } else {
            match engine_a_mem.registers.display_mode() {
                0 => self.engine_a.draw_blank_line(target),
                1 => {
                    self.engine_a.draw(&engine_a_mem, &mut self.line_cache, line);
                    for (colour, out) in self.line_cache.iter().zip(target.chunks_exact_mut(4)) {
                        let colour = engine_a_mem.registers.apply_brightness(*colour);
                        out[0] = colour.r;
                        out[1] = colour.g;
                        out[2] = colour.b;
                    }
                    drawn = true;
                },
                2 => {
                    let lcdc = self.vram.lcdc_vram.lock();
                    let read_offset = (line as u32) * (H_RES as u32) * 2;
                    Self::draw_from_vram(&lcdc, &engine_a_mem.registers, &mut self.line_cache, read_offset);
                    for (colour, out) in self.line_cache.iter().zip(target.chunks_exact_mut(4)) {
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

        if self.capture {
            let write_size = engine_a_mem.registers.vram_capture_write_size();
            if (line as u32) >= write_size.1 {
                // Outside of writing bounds.
                return;
            }

            match engine_a_mem.registers.display_capture_mode() {
                DispCapMode::A(src_a) => match src_a {
                    DispCapSourceA::Engine => {
                        if !drawn {
                            self.engine_a.draw(&engine_a_mem, &mut self.line_cache, line);
                        }
                    },
                    DispCapSourceA::_3D => {
                        let start = (line as usize) * H_RES;
                        let end = start + H_RES;
                        for (a, b) in self.line_cache.iter_mut().zip(&self.engine_a.frame_3d[start..end]) {
                            *a = b.col;
                        }
                    }
                },
                DispCapMode::B(src_b) => match src_b {
                    DispCapSourceB::VRAM => {
                        let lcdc = self.vram.lcdc_vram.lock();
                        let read_offset = engine_a_mem.registers.vram_capture_read_offset() + (line as u32) * (H_RES as u32) * 2;
                        Self::draw_from_vram(&lcdc, &engine_a_mem.registers, &mut self.line_cache, read_offset);
                    },
                    DispCapSourceB::MainRAM => panic!("main mem capture not implemented yet!"),
                },
                DispCapMode::Blend{src_a, src_b, eva, evb} => {
                    match src_a {
                        DispCapSourceA::Engine => {
                            if !drawn {
                                self.engine_a.draw(&engine_a_mem, &mut self.line_cache, line);
                            }
                        },
                        DispCapSourceA::_3D => {
                            let start = (line as usize) * H_RES;
                            let end = start + H_RES;
                            for (a, b) in self.line_cache.iter_mut().zip(&self.engine_a.frame_3d[start..end]) {
                                *a = b.col;
                            }
                        }
                    }
                    match src_b {
                        DispCapSourceB::VRAM => {
                            let lcdc = self.vram.lcdc_vram.lock();
                            let read_offset = engine_a_mem.registers.vram_capture_read_offset() + (line as u32) * (H_RES as u32) * 2;
                            Self::draw_from_vram(&lcdc, &engine_a_mem.registers, &mut self.line_cache_b, read_offset);
                        },
                        DispCapSourceB::MainRAM => panic!("main mem capture not implemented yet!"),
                    }
                    // Blend.
                    for (a, b) in self.line_cache.iter_mut().zip(&self.line_cache_b) {
                        *a = SoftwareRenderer::apply_alpha_blend(eva, evb, *a, *b);
                    }
                }
            }

            self.write_data = Some(CaptureWriteData {
                offset: engine_a_mem.registers.vram_capture_write_offset() + (line as u32) * write_size.0 * 2,
                size:   write_size.0,
                block:  engine_a_mem.registers.write_vram_block()
            });
        }

        engine_a_mem.registers.inc_v_count();
    }

    /// Draw a full line for NDS B engine. Also applies master brightness.
    fn engine_b_line(&mut self, line: u8, display_swap: bool) {
        let start_offset = (line as usize) * (H_RES * 4);
        let end_offset = start_offset + (H_RES * 4);

        let mut screen = if display_swap {
            self.lower.lock()
        } else {
            self.upper.lock()
        };
        let target = &mut screen[start_offset..end_offset];

        let mut engine_b_mem = self.vram.engine_b_mem.lock();
        self.engine_b.setup_caches(&mut engine_b_mem);
        
        if engine_b_mem.registers.in_fblank() {
            self.engine_b.draw_blank_line(target);
        } else {
            match engine_b_mem.registers.display_mode() {
                0 => self.draw_empty_line(target),
                1 => {
                    self.engine_b.draw(&engine_b_mem, &mut self.line_cache_b, line as u8);
                    for (colour, out) in self.line_cache_b.iter().zip(target.chunks_exact_mut(4)) {
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
    /// Read offset is in bytes.
    fn draw_from_vram(mem: &ARM9VRAM, registers: &VideoRegisters, target: &mut [Colour], read_offset: u32) {
        if let Some(vram) = mem.ref_region(registers.read_vram_block()) {
            for (n, out) in target.iter_mut().enumerate() {
                let sub_addr = (n as u32) << 1;
                let addr = (read_offset + sub_addr) & 0x1_FFFF;
                let data = vram.read_halfword(addr);
                *out = Colour::from_555(data);
            }
        }
    }

    /// Write capture to VRAM.
    /// 
    /// Write offset is in bytes.
    fn write_to_vram(mem: &mut ARM9VRAM, write_block: u16, source: &[Colour], write_offset: u32) {
        if let Some(vram) = mem.mut_region(write_block) {
            for (n, colour) in source.iter().enumerate() {
                let sub_addr = (n as u32) << 1;
                let addr = (write_offset + sub_addr) & 0x1_FFFF;
                let raw_colour = colour.to_555() | 0x8000;
                vram.write_halfword(addr, raw_colour);
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
    
    fn render_3d(&mut self) {}

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
