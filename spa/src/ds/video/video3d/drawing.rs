use super::{
    render::RenderingEngine,
    types::{Display3DControl, PolygonAttrs, Coords, Polygon}, geometry::N
};
use fixed::{types::I12F4, types::I23F9, traits::ToFixed};
use crate::{
    ds::video::{memory::Engine3DVRAM, video3d::types::Vertex},
    common::colour::*,
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

    pub fn draw(&mut self, render_engine: &RenderingEngine, vram: &Engine3DVRAM, target: &mut [ColourAlpha], line: u8) {
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
    fn clear_buffers(&mut self, render_engine: &RenderingEngine, vram: &Engine3DVRAM, target: &mut [ColourAlpha], line: u8) {
        self.stencil_buffer.fill(false);

        if render_engine.control.contains(Display3DControl::CLEAR_IMAGE) {
            let clear_colour_image = vram.tex_2.as_ref().expect("using clear colour image without mapped vram");
            let clear_depth_image = vram.tex_3.as_ref().expect("using clear depth image without mapped vram");

            let image_y = line.wrapping_add(render_engine.clear_image_y);
            let image_y_addr = (image_y as u32) * 256 * 2;

            for x in 0..=255_u8 {
                let image_x = x.wrapping_add(render_engine.clear_image_x);
                let addr = image_y_addr + (image_x as u32) * 2;
                let colour = clear_colour_image.read_halfword(addr);
                let depth = clear_depth_image.read_halfword(addr);
                let clear_attrs = Attributes {
                    opaque_id:  render_engine.clear_poly_id,
                    trans_id:   render_engine.clear_poly_id,
                    alpha:      if u16::test_bit(colour, 15) {0x1F} else {0},
                    fog:        u16::test_bit(depth, 15),
                };

                self.attr_buffer[x as usize] = clear_attrs;
                self.depth_buffer[x as usize] = I23F9::from_bits(((depth & 0x7FFF) as i32) << 9);   // TODO: frac part.
                target[x as usize].col = Colour::from_555(colour);
                target[x as usize].alpha = if u16::test_bit(colour, 15) {0x1F} else {0};
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
            target.fill(ColourAlpha { col: render_engine.clear_colour, alpha: render_engine.clear_alpha });
        }
    }
    
    fn draw_opaque_polygons(&mut self, render_engine: &RenderingEngine, vram: &Engine3DVRAM, target: &mut [ColourAlpha], line: u8) {
        let y = N::from_num(line) + N::from_num(0.5_f32);

        for p in render_engine.polygon_ram.opaque_polygons.iter()
            //.skip_while(|el| el.y_max < line || el.y_min > line)
            //.take_while(|el| el.y_max >= line && el.y_min <= line)
        {
            if p.y_max < y || p.y_min > y {
                continue;
            }
            let polygon = &render_engine.polygon_ram.polygons[p.polygon_index];
            
            let [vtx_a, vtx_b] = Self::find_intersect_points(render_engine, polygon, y).unwrap();

            let x_diff = N::ONE.checked_div(vtx_b.screen_p.x - vtx_a.screen_p.x).unwrap_or(N::ZERO);

            for x_idx in vtx_a.screen_p.x.to_num::<i16>()..=vtx_b.screen_p.x.to_num::<i16>() {
                let x = N::from_num(x_idx) + N::from_num(0.5_f32);
                let factor_b = (x - vtx_a.screen_p.x) * x_diff;
                let factor_a = N::ONE - factor_b;

                let depth = (vtx_a.depth * factor_a.to_fixed::<I23F9>()) + (vtx_b.depth * factor_b.to_fixed::<I23F9>());

                // Evaluate depth
                if self.depth_buffer[x_idx as usize] <= depth { // TODO: remove fractional part?
                    // Point is behind buffer value.
                    if !polygon.attrs.contains(PolygonAttrs::RENDER_EQ_DEPTH) || self.depth_buffer[x_idx as usize] < depth {
                        continue;
                    }
                }

                // We are sure that we want to render this fragment.
                self.depth_buffer[x_idx as usize] = depth;
                self.attr_buffer[x_idx as usize].opaque_id = polygon.attrs.id();
                self.attr_buffer[x_idx as usize].fog = polygon.attrs.contains(PolygonAttrs::FOG_BLEND_ENABLE);

                // Interpolate vertex colour
                let r = (N::from_num(vtx_a.colour.r) * factor_a) + (N::from_num(vtx_b.colour.r) * factor_b);
                let g = (N::from_num(vtx_a.colour.g) * factor_a) + (N::from_num(vtx_b.colour.g) * factor_b);
                let b = (N::from_num(vtx_a.colour.b) * factor_a) + (N::from_num(vtx_b.colour.b) * factor_b);
                let frag_colour = Colour {
                    r: r.to_num::<u8>(),
                    g: g.to_num::<u8>(),
                    b: b.to_num::<u8>()
                };
                
                let tex_format = polygon.tex.format();
                let tex_colour = if tex_format == 0 {
                    // No texture.
                    ColourAlpha {
                        col: Colour { r: 0xFF, g: 0xFF, b: 0xFF },
                        alpha: 0x1F
                    }
                } else {
                    let tex_s = (vtx_a.tex_coords.x * factor_a) + (vtx_b.tex_coords.x * factor_b);
                    let tex_t = (vtx_a.tex_coords.y * factor_a) + (vtx_b.tex_coords.y * factor_b);
                    //let tex_coords = a.tex_coords;
                    // TODO: Lookup tex colour + alpha
                    let tex_colour = Colour { r: 0xFF, g: 0xFF, b: 0xFF };
                    let tex_alpha = 0x1F;
                    ColourAlpha {
                        col: tex_colour,
                        alpha: tex_alpha
                    }
                };

                target[x_idx as usize] = Self::blend_fragment_colour(render_engine, polygon, frag_colour, tex_colour)
            }
        }
    }

    /// Find the first two points where this polygon intersects the render line.
    /// 
    /// Returns the two points with interpolated attributes, in order of x position.
    fn find_intersect_points(render_engine: &RenderingEngine, polygon: &Polygon, y: N) -> Option<[Vertex; 2]> {
        let n_vertices = polygon.vertex_indices.len();

        // Find start and end points.
        let mut lines = [None, None];
        for i in 0..n_vertices {
            // Find where render line intersects polygon lines.
            let v_index_a = polygon.vertex_indices[i];
            let v_index_b = polygon.vertex_indices[(i + 1) % n_vertices];

            let vtx_a = &render_engine.polygon_ram.vertices[v_index_a];
            let vtx_b = &render_engine.polygon_ram.vertices[v_index_b];

            if (y > vtx_a.screen_p.y && y > vtx_b.screen_p.y) || (y < vtx_a.screen_p.y && y < vtx_b.screen_p.y) {
                // This line does not intersect the render line.
                continue;
            }

            // Weight of point a (normalised between 0-1)
            let factor_a = (y - vtx_b.screen_p.y).checked_div(vtx_a.screen_p.y - vtx_b.screen_p.y).unwrap_or(N::ONE);   // TODO: one dot polygon?
            // X coordinate where the render line intersects the polygon line.
            let intersect_x = factor_a * (vtx_a.screen_p.x - vtx_b.screen_p.x) + vtx_b.screen_p.x;
            let factor_b = N::ONE - factor_a;

            // Interpolate attributes
            let depth = (vtx_a.depth * factor_a.to_fixed::<I23F9>()) + (vtx_b.depth * factor_b.to_fixed::<I23F9>());
            let r = (N::from_num(vtx_a.colour.r) * factor_a) + (N::from_num(vtx_b.colour.r) * factor_b);
            let g = (N::from_num(vtx_a.colour.g) * factor_a) + (N::from_num(vtx_b.colour.g) * factor_b);
            let b = (N::from_num(vtx_a.colour.b) * factor_a) + (N::from_num(vtx_b.colour.b) * factor_b);
            let tex_s = (vtx_a.tex_coords.x * factor_a) + (vtx_b.tex_coords.x * factor_b);
            let tex_t = (vtx_a.tex_coords.y * factor_a) + (vtx_b.tex_coords.y * factor_b);

            let vertex = Vertex {
                screen_p:   Coords { x: intersect_x, y: y },
                depth:      depth,
                colour:     Colour { r: r.to_num::<u8>(), g: g.to_num::<u8>(), b: b.to_num::<u8>() },
                tex_coords: Coords { x: tex_s, y: tex_t }
            };

            if lines[0].is_none() {
                // First line.
                lines[0] = Some(vertex);
            } else if lines[1].is_none() {
                // Second line - we are done.
                lines[1] = Some(vertex);
                break;
            }
        }

        if let [Some(vtx_a), Some(vtx_b)] = lines {
            if vtx_a.screen_p.x < vtx_b.screen_p.x {
                Some([vtx_a, vtx_b])
            } else {
                Some([vtx_b, vtx_a])
            }
        } else {
            None
        }
    }

    fn blend_fragment_colour(render_engine: &RenderingEngine, polygon: &Polygon, frag_colour: Colour, tex_colour: ColourAlpha) -> ColourAlpha {
        match polygon.attrs.mode() {
            FRAG_MODE_MODULATION => {
                let r = ((frag_colour.r as u16) + 1) * ((tex_colour.col.r as u16) + 1) - 1;
                let g = ((frag_colour.g as u16) + 1) * ((tex_colour.col.g as u16) + 1) - 1;
                let b = ((frag_colour.b as u16) + 1) * ((tex_colour.col.b as u16) + 1) - 1;
                let a = (tex_colour.alpha as u16) * 0x1F;

                ColourAlpha {
                    col: Colour {
                        r: bytes::u16::hi(r),
                        g: bytes::u16::hi(g),
                        b: bytes::u16::hi(b)
                    },
                    alpha: (a >> 5) as u8
                }
            },

            FRAG_MODE_DECAL => {
                if tex_colour.alpha == 0 {
                    ColourAlpha {
                        col: frag_colour,
                        alpha: 0x1F
                    }
                } else if tex_colour.alpha == 0x1F {
                    tex_colour
                } else {
                    let tex_alpha = tex_colour.alpha as u16;
                    let frag_alpha = 0x1F - tex_alpha;
                    let r = ((frag_colour.r as u16) * frag_alpha).saturating_add((tex_colour.col.r as u16) * tex_alpha);
                    let g = ((frag_colour.g as u16) * frag_alpha).saturating_add((tex_colour.col.g as u16) * tex_alpha);
                    let b = ((frag_colour.b as u16) * frag_alpha).saturating_add((tex_colour.col.b as u16) * tex_alpha);
                    ColourAlpha {
                        col: Colour {
                            r: (r >> 5) as u8,
                            g: (g >> 5) as u8,
                            b: (b >> 5) as u8
                        },
                        alpha: 0x1F
                    }
                }
            },

            FRAG_MODE_TOON_HIGHLIGHT => {
                let index = (frag_colour.r >> 3) as usize;
                let table_colour = render_engine.toon_table[index];
                if render_engine.control.contains(Display3DControl::HIGHLIGHT_SHADING) {
                    // TODO
                    ColourAlpha {
                        col: frag_colour,
                        alpha: 0x1F
                    }
                } else {
                    // TODO
                    ColourAlpha {
                        col: frag_colour,
                        alpha: 0x1F
                    }
                }
            },

            FRAG_MODE_SHADOW => panic!("cannot use shadow mode on opaque polygons"), // invalid for opaque polygons
            _ => unreachable!()
        }
    }
}
