use super::{render::RenderingEngine, types::Display3DControl};
use crate::{
    ds::video::memory::Engine3DVRAM,
    common::colour::Colour, utils::bits::u16
};

#[derive(Copy, Clone, Default)]
struct Attributes {
    opaque_id:  u8,
    trans_id:   u8,
    alpha:      u8,
    fog:        bool,
}

/// Render NDS 3D graphics.
pub struct Software3DRenderer {
    stencil_buffer: Vec<bool>,
    attr_buffer:    Vec<Attributes>,
    depth_buffer:   Vec<u32>,
}

impl Software3DRenderer {
    pub fn new() -> Self {
        Self {
            stencil_buffer: vec![false; 256],
            attr_buffer:    vec![Default::default(); 256],
            depth_buffer:   vec![0; 256],
        }
    }

    pub fn draw(&mut self, render_engine: &RenderingEngine, vram: &Engine3DVRAM, target: &mut [Colour], line: u8) {
        self.clear_buffers(render_engine, vram, target, line);

        // Draw opaque polygons (sorted)
        for p in render_engine.polygon_ram.opaque_polygons.iter()
            .skip_while(|el| el.y_max < line || el.y_min > line)
            .take_while(|el| el.y_max >= line && el.y_min <= line)
        {
            let polygon = &render_engine.polygon_ram.polygons[p.polygon_index];
            // TODO: from x_min to x_max
            for x in 0..=255_u8 {
                // Check if inside
                // Interpolate depth, test depth
                // Stencil ??
                // Find fragment colour, tex colour, blend
                // Alpha blend with buffer colour
            }
        }

        // Edge marking in buffer
        // Fog in buffer

        // Anti-aliasing (after 2d-blend?)

    }
}

impl Software3DRenderer {
    fn clear_buffers(&mut self, render_engine: &RenderingEngine, vram: &Engine3DVRAM, target: &mut [Colour], line: u8) {
        self.stencil_buffer.fill(false);

        if render_engine.control.contains(Display3DControl::CLEAR_IMAGE) {
            let clear_colour_image = vram.tex_2.as_ref().expect("using clear colour image without mapped vram");
            let clear_depth_image = vram.tex_3.as_ref().expect("using clear depth image without mapped vram");

            let image_y = line.wrapping_add(render_engine.clear_image_y);
            let image_y_addr = (image_y as u32) * 256 * 2;
            let mut clear_attrs = Attributes {
                opaque_id:  render_engine.clear_poly_id,
                trans_id:   render_engine.clear_poly_id,
                alpha:      render_engine.clear_alpha,
                fog:        render_engine.fog_enabled,
            };

            for x in 0..=255_u8 {
                let image_x = x.wrapping_add(render_engine.clear_image_x);
                let addr = image_y_addr + (image_x as u32) * 2;
                let colour = clear_colour_image.read_halfword(addr);
                let depth = clear_depth_image.read_halfword(addr);
                clear_attrs.alpha = if u16::test_bit(colour, 15) {0x1F} else {0};
                clear_attrs.fog = u16::test_bit(depth, 15);

                self.attr_buffer[x as usize] = clear_attrs;
                self.depth_buffer[x as usize] = ((depth & 0x7FFF) as u32) << 9;
                target[x as usize] = Colour::from_555(colour);
            }
        } else {
            let clear_attrs = Attributes {
                opaque_id:  render_engine.clear_poly_id,
                trans_id:   render_engine.clear_poly_id,
                alpha:      render_engine.clear_alpha,
                fog:        render_engine.fog_enabled,
            };
            self.attr_buffer.fill(clear_attrs);
            self.depth_buffer.fill(render_engine.clear_depth);
            target.fill(render_engine.clear_colour);
        }
    }
}