/// Rendering the video.

mod drawing;
mod colour;

use crate::constants::*;

pub type RenderTarget = std::rc::Rc<std::cell::RefCell<[u8]>>;

/// Renderer trait. The renderer should implement this.
pub trait Renderer {
    /// Render a single line.
    fn render_line(&mut self, mem: &mut super::VideoMemory, line: u16);
    /// Start rendering the frame.
    fn start_frame(&mut self);
    /// Complete rendering the frame.
    fn finish_frame(&mut self);
}

pub struct DebugRenderer {}

impl Renderer for DebugRenderer {
    fn render_line(&mut self, _mem: &mut super::VideoMemory, line: u16) {
        println!("Render line {}", line)
    }

    fn start_frame(&mut self) {
        println!("Start frame");
    }

    fn finish_frame(&mut self) {
        println!("Finish frame");
    }
}

pub struct ProceduralRenderer {
    renderer:   drawing::SoftwareRenderer,

    target:     RenderTarget,
}

impl ProceduralRenderer {
    pub fn new(target: RenderTarget) -> Self {
        Self {
            renderer:   drawing::SoftwareRenderer::new(),
            target:     target,
        }
    }
}

impl Renderer for ProceduralRenderer {
    fn render_line(&mut self, mem: &mut super::VideoMemory, line: u16) {
        self.renderer.setup_caches(mem);
        let start_offset = (line as usize) * (gba::H_RES * 4);
        let end_offset = start_offset + (gba::H_RES * 4);
        let mut target = self.target.borrow_mut();
        self.renderer.draw_line(mem, &mut target[start_offset..end_offset], line);
    }

    fn start_frame(&mut self) {
        //println!("Start frame");
    }

    fn finish_frame(&mut self) {
        //println!("Finish frame");
    }
}