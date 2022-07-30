use super::{
    render::RenderingEngine,
    types::{Display3DControl, PolygonAttrs, Coords}
};
use fixed::{types::I12F4, types::I23F9, traits::ToFixed};
use crate::{
    ds::video::memory::Engine3DVRAM,
    common::colour::Colour,
    utils::bits::u16, utils::bytes
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
    depth_buffer:   Vec<I23F9>,
}

impl Software3DRenderer {
    pub fn new() -> Self {
        Self {
            stencil_buffer: vec![false; 256],
            attr_buffer:    vec![Default::default(); 256],
            depth_buffer:   vec![I23F9::ZERO; 256],
        }
    }

    pub fn draw(&mut self, render_engine: &RenderingEngine, vram: &Engine3DVRAM, target: &mut [Colour], line: u8) {
        self.clear_buffers(render_engine, vram, target, line);

        self.draw_opaque_polygons(render_engine, vram, target, line);

        // TODO: Draw translucent polygons (sorted or manual)

        if render_engine.control.contains(Display3DControl::EDGE_MARKING) {
            // TODO: Edge marking in buffer
        }

        if render_engine.control.contains(Display3DControl::FOG_ENABLE) {
            // TODO: Fog in buffer
        }

        // Anti-aliasing (after 2d-blend?)

    }
}

const FRAG_MODE_MODULATION: u8 = 0b00;
const FRAG_MODE_DECAL: u8 = 0b01;
const FRAG_MODE_TOON_HIGHLIGHT: u8 = 0b10;
const FRAG_MODE_SHADOW: u8 = 0b11;

impl Software3DRenderer {
    /// Fill the drawing buffers with clear values or clear image.
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
                self.depth_buffer[x as usize] = I23F9::from_bits(((depth & 0x7FFF) as i32) << 9);   // TODO: frac part.
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

    fn draw_opaque_polygons(&mut self, render_engine: &RenderingEngine, vram: &Engine3DVRAM, target: &mut [Colour], line: u8) {
        let y = I12F4::from_num(line) + I12F4::from_bits(0b1000);

        for p in render_engine.polygon_ram.opaque_polygons.iter()
            .skip_while(|el| el.y_max < line || el.y_min > line)
            .take_while(|el| el.y_max >= line && el.y_min <= line)
        {
            let polygon = &render_engine.polygon_ram.polygons[p.polygon_index];
            let x_min = polygon.x_min.to_num::<u8>();
            let x_max = polygon.x_max.to_num::<u8>();   // TODO: +1 ?
            let vertices = polygon.vertex_indices.iter()
                .map(|i| &render_engine.polygon_ram.vertices[*i])
                .collect::<Vec<_>>();

            let area = edge_function(vertices[0].screen_p, vertices[1].screen_p, vertices[2].screen_p);
            let area_recip = I23F9::ONE / area.to_fixed::<I23F9>();

            'polygon_test: for x_idx in x_min..=x_max {
                // Test against the centre of the pixel.
                let x = I12F4::from_num(x_idx) + I12F4::from_bits(0b1000);
                let p = Coords {x, y};
                let mut interpolation_factors = Vec::new(); // TODO: not vec
                // Check if point is inside polygon.
                for i in 0..polygon.vertex_indices.len() {
                    // Find determinant of line vector and vector from point to pixel.
                    let j = (i + 1) % polygon.vertex_indices.len();
                    let v_i = vertices[i];
                    let v_j = vertices[j];
                    let factor = edge_function(v_i.screen_p, v_j.screen_p, p);
                    if factor < I12F4::ZERO {
                        // Point is outside this polygon.
                        continue 'polygon_test;
                    }
                    let normalised_factor = factor.to_fixed::<I23F9>() * area_recip;
                    interpolation_factors.push(normalised_factor);
                }
                
                let depth = interpolation_factors[0] * vertices[2].depth +
                    interpolation_factors[1] * vertices[0].depth +
                    interpolation_factors[2] * vertices[1].depth;

                if self.depth_buffer[x_idx as usize] <= depth { // TODO: remove fractional part?
                    // Point is behind buffer value.
                    if polygon.attrs.contains(PolygonAttrs::RENDER_EQ_DEPTH) && self.depth_buffer[x_idx as usize] < depth {
                        continue;
                    }
                }

                self.depth_buffer[x_idx as usize] = depth;

                self.attr_buffer[x_idx as usize].opaque_id = polygon.attrs.id();
                self.attr_buffer[x_idx as usize].fog = polygon.attrs.contains(PolygonAttrs::FOG_BLEND_ENABLE);

                let frag_colour = interpolate_colour(&interpolation_factors, &vertices.iter().map(|v| v.colour).collect::<Vec<_>>());
                // TODO: Interpolate tex coords
                // TODO: Lookup tex colour
                let tex_colour = Colour::black();
                // TODO: Lookup tex alpha
                let tex_alpha = 0x1F_u16;

                // Blend fragment colour.
                match polygon.attrs.mode() {
                    FRAG_MODE_MODULATION => {
                        let r = ((frag_colour.r as u16) + 1) * ((tex_colour.r as u16) + 1) - 1;
                        let g = ((frag_colour.g as u16) + 1) * ((tex_colour.g as u16) + 1) - 1;
                        let b = ((frag_colour.b as u16) + 1) * ((tex_colour.b as u16) + 1) - 1;
                        target[x_idx as usize] = Colour {
                            r: bytes::u16::hi(r),
                            g: bytes::u16::hi(g),
                            b: bytes::u16::hi(b)
                        };

                        let alpha = tex_alpha * 0x1F;
                        self.attr_buffer[x_idx as usize].alpha = (alpha >> 5) as u8;
                    },
                    FRAG_MODE_DECAL => {
                        if tex_alpha == 0 {
                            target[x_idx as usize] = frag_colour;
                        } else if tex_alpha == 0x1F {
                            target[x_idx as usize] = tex_colour;
                        } else {
                            let frag_alpha = 0x1F - tex_alpha;
                            let r = ((frag_colour.r as u16) * frag_alpha) * ((tex_colour.r as u16) * tex_alpha);
                            let g = ((frag_colour.g as u16) * frag_alpha) * ((tex_colour.g as u16) * tex_alpha);
                            let b = ((frag_colour.b as u16) * frag_alpha) * ((tex_colour.b as u16) * tex_alpha);
                            target[x_idx as usize] = Colour {
                                r: bytes::u16::hi(r),
                                g: bytes::u16::hi(g),
                                b: bytes::u16::hi(b)
                            };
                        }
                        self.attr_buffer[x_idx as usize].alpha = 0x1F;
                    },
                    FRAG_MODE_TOON_HIGHLIGHT => {
                        let index = (frag_colour.r >> 3) as usize;
                        let table_colour = render_engine.toon_table[index];
                        if render_engine.control.contains(Display3DControl::HIGHLIGHT_SHADING) {
                            // TODO
                        } else {
                            // TODO
                        }
                    },
                    FRAG_MODE_SHADOW => (), // invalid for opaque polygons
                    _ => unreachable!()
                }
            }
        }
    }
}

/// Find the determinant of the matrix defined by the columns:
/// (v1 - v0), (v2 - v0)
/// 
/// This is equal to 2* the area of the triangle defined by these vertices,
/// and the weighted normal to the plane defined by these vertices.
#[inline]
fn edge_function(v0: Coords, v1: Coords, v2: Coords) -> I12F4 {
    let a = (v2.x - v0.x) * (v1.y - v0.y);
    let b = (v2.y - v0.y) * (v1.x - v0.x);
    a - b
}

fn interpolate_colour(factors: &[I23F9], colours: &[Colour]) -> Colour {
    let r = factors[0] * colours[2].r.to_fixed::<I23F9>()
        + factors[1] * colours[0].r.to_fixed::<I23F9>()
        + factors[2] * colours[1].r.to_fixed::<I23F9>();
    
    let g = factors[0] * colours[2].g.to_fixed::<I23F9>()
        + factors[1] * colours[0].g.to_fixed::<I23F9>()
        + factors[2] * colours[1].g.to_fixed::<I23F9>();
    
    let b = factors[0] * colours[2].b.to_fixed::<I23F9>()
        + factors[1] * colours[0].b.to_fixed::<I23F9>()
        + factors[2] * colours[1].b.to_fixed::<I23F9>();
    
    Colour {
        r: r.to_num::<u8>(),
        g: g.to_num::<u8>(),
        b: b.to_num::<u8>()
    }
}
