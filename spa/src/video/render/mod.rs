/// Rendering the video.

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