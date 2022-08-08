use super::{
    render::RenderingEngine,
    types::{Display3DControl, PolygonAttrs, Coords, Polygon, PolygonOrder}, geometry::N
};
use fixed::{types::I12F4, types::I23F9, traits::ToFixed};
use crate::{
    ds::video::{memory::Engine3DVRAM, video3d::types::Vertex, render},
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

        self.draw_trans_polygons(render_engine, vram, target, line);

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

            // TODO: wireframe
            for x_idx in vtx_a.screen_p.x.to_num::<i16>()..=vtx_b.screen_p.x.to_num::<i16>() {
                // TODO: lookup transparent texel and blend as trans pixel

                let x = N::from_num(x_idx) + N::from_num(0.5_f32);
                let factor_b = (x - vtx_a.screen_p.x) * x_diff;
                let factor_a = N::ONE - factor_b;

                let depth = Self::interpolate_depth(vtx_a.depth, vtx_b.depth, factor_a, factor_b);

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
                let vtx_colour = ColourAlpha {
                    col: Self::interpolate_vertex_colour(vtx_a.colour, vtx_b.colour, factor_a, factor_b),
                    alpha: 0x1F
                };

                let tex_format = polygon.tex.format();
                let tex_colour = if tex_format == 0 {
                    // No texture.
                    ColourAlpha {
                        col: Colour { r: 0xFF, g: 0xFF, b: 0xFF },
                        alpha: 0x1F
                    }
                } else {
                    let tex_coords = Self::interpolate_tex_coords(vtx_a.tex_coords, vtx_b.tex_coords, factor_a, factor_b);
                    // TODO: Lookup tex colour + alpha
                    let tex_colour = Colour { r: 0xFF, g: 0xFF, b: 0xFF };
                    let tex_alpha = 0x1F;
                    ColourAlpha {
                        col: tex_colour,
                        alpha: tex_alpha
                    }
                };

                target[x_idx as usize] = Self::blend_fragment_colour(render_engine, polygon, vtx_colour, tex_colour);
            }
        }
    }
    
    fn draw_trans_polygons(&mut self, render_engine: &RenderingEngine, vram: &Engine3DVRAM, target: &mut [ColourAlpha], line: u8) {
        let y = N::from_num(line) + N::from_num(0.5_f32);

        if render_engine.polygon_ram.use_manual_mode {
            render_engine.polygon_ram.trans_polygon_manual.iter()
                .for_each(|p| self.draw_trans_polygon(render_engine, vram, target, y, p))
        } else {
            render_engine.polygon_ram.trans_polygon_auto.iter()
                .for_each(|p| self.draw_trans_polygon(render_engine, vram, target, y, p))
            //.skip_while(|el| el.y_max < line || el.y_min > line)
            //.take_while(|el| el.y_max >= line && el.y_min <= line)
        }
    }

    fn draw_trans_polygon(&mut self, render_engine: &RenderingEngine, vram: &Engine3DVRAM, target: &mut [ColourAlpha], y: N, p: &PolygonOrder) {
        if p.y_max < y || p.y_min > y {
            return;
        }
        let polygon = &render_engine.polygon_ram.polygons[p.polygon_index];
        
        let [vtx_a, vtx_b] = Self::find_intersect_points(render_engine, polygon, y).unwrap();

        let x_diff = N::ONE.checked_div(vtx_b.screen_p.x - vtx_a.screen_p.x).unwrap_or(N::ZERO);

        for x_idx in vtx_a.screen_p.x.to_num::<i16>()..=vtx_b.screen_p.x.to_num::<i16>() {
            let id = polygon.attrs.id();
            // TODO: only extract for shadow polygons?
            let stencil_mask = std::mem::replace(&mut self.stencil_buffer[x_idx as usize], false);
            if self.attr_buffer[x_idx as usize].trans_id == id {
                continue;
            }

            let x = N::from_num(x_idx) + N::from_num(0.5_f32);
            let factor_b = (x - vtx_a.screen_p.x) * x_diff;
            let factor_a = N::ONE - factor_b;

            let depth = Self::interpolate_depth(vtx_a.depth, vtx_b.depth, factor_a, factor_b);

            // Evaluate depth
            if self.depth_buffer[x_idx as usize] <= depth { // TODO: remove fractional part?
                if id == 0 && polygon.attrs.mode() == FRAG_MODE_SHADOW {    // TODO: & draw back surface?
                    // Shadow polygon mask
                    self.stencil_buffer[x_idx as usize] = true;
                    continue;
                }
                // Point is behind buffer value.
                if !polygon.attrs.contains(PolygonAttrs::RENDER_EQ_DEPTH) || self.depth_buffer[x_idx as usize] < depth {
                    continue;
                }
            }

            if polygon.attrs.mode() == FRAG_MODE_SHADOW &&
                (!stencil_mask || self.attr_buffer[x_idx as usize].opaque_id == id) {
                // We only want to draw the shadow if it passes depth,
                // is masked, and doesn't match the IDs
                continue;
            }

            // We are sure that we want to render this fragment.
            self.depth_buffer[x_idx as usize] = depth;
            self.attr_buffer[x_idx as usize].trans_id = id;
            self.attr_buffer[x_idx as usize].fog = self.attr_buffer[x_idx as usize].fog && polygon.attrs.contains(PolygonAttrs::FOG_BLEND_ENABLE);

            // Interpolate vertex colour
            let vtx_colour = ColourAlpha {
                col: Self::interpolate_vertex_colour(vtx_a.colour, vtx_b.colour, factor_a, factor_b),
                alpha: polygon.attrs.alpha()
            };
            
            let tex_format = polygon.tex.format();
            let tex_colour = if tex_format == 0 {
                // No texture.
                ColourAlpha {
                    col: Colour { r: 0xFF, g: 0xFF, b: 0xFF },
                    alpha: 0x1F
                }
            } else {
                let tex_coords = Self::interpolate_tex_coords(vtx_a.tex_coords, vtx_b.tex_coords, factor_a, factor_b);
                // TODO: Lookup tex colour + alpha
                let tex_colour = Colour { r: 0xFF, g: 0xFF, b: 0xFF };
                let tex_alpha = 0x1F;
                ColourAlpha {
                    col: tex_colour,
                    alpha: tex_alpha
                }
            };

            let frag_colour = Self::blend_fragment_colour(render_engine, polygon, vtx_colour, tex_colour);
            if render_engine.control.contains(Display3DControl::ALPHA_TEST_ENABLE) {
                if frag_colour.alpha < render_engine.alpha_test {
                    return;
                }
            }

            if render_engine.control.contains(Display3DControl::BLENDING_ENABLE) {
                target[x_idx as usize] = Self::blend_buffer_colour(
                    frag_colour, target[x_idx as usize],
                    polygon.attrs.mode() == FRAG_MODE_SHADOW
                );
            } else if frag_colour.alpha > 0 {
                target[x_idx as usize] = frag_colour;
            }
        }
    }
}

// Static helpers.
impl Software3DRenderer {
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
            let depth = Self::interpolate_depth(vtx_a.depth, vtx_b.depth, factor_a, factor_b);
            let frag_colour = Self::interpolate_vertex_colour(vtx_a.colour, vtx_b.colour, factor_a, factor_b);
            let tex_coords = Self::interpolate_tex_coords(vtx_a.tex_coords, vtx_b.tex_coords, factor_a, factor_b);

            let vertex = Vertex {
                screen_p:   Coords { x: intersect_x, y: y },
                depth:      depth,
                colour:     frag_colour,
                tex_coords: tex_coords
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

    #[inline]
    fn interpolate_depth(depth_a: I23F9, depth_b: I23F9, factor_a: N, factor_b: N) -> I23F9 {
        (depth_a * factor_a.to_fixed::<I23F9>()) + (depth_b * factor_b.to_fixed::<I23F9>())
    }

    #[inline]
    fn interpolate_vertex_colour(colour_a: Colour, colour_b: Colour, factor_a: N, factor_b: N) -> Colour {
        let r = (N::from_num(colour_a.r) * factor_a) + (N::from_num(colour_b.r) * factor_b);
        let g = (N::from_num(colour_a.g) * factor_a) + (N::from_num(colour_b.g) * factor_b);
        let b = (N::from_num(colour_a.b) * factor_a) + (N::from_num(colour_b.b) * factor_b);
        Colour {
            r: r.to_num::<u8>(),
            g: g.to_num::<u8>(),
            b: b.to_num::<u8>()
        }
    }
    
    #[inline]
    fn interpolate_tex_coords(tex_coords_a: Coords, tex_coords_b: Coords, factor_a: N, factor_b: N) -> Coords {
        let tex_s = (tex_coords_a.x * factor_a) + (tex_coords_b.x * factor_b);
        let tex_t = (tex_coords_a.y * factor_a) + (tex_coords_b.y * factor_b);
        Coords { x: tex_s, y: tex_t }
    }

    /// Use the polygon's specified blending mode to blend the fragment's colour.
    fn blend_fragment_colour(render_engine: &RenderingEngine, polygon: &Polygon, vtx_colour: ColourAlpha, tex_colour: ColourAlpha) -> ColourAlpha {
        match polygon.attrs.mode() {
            FRAG_MODE_MODULATION => {
                let r = ((vtx_colour.col.r as u16) + 1) * ((tex_colour.col.r as u16) + 1) - 1;
                let g = ((vtx_colour.col.g as u16) + 1) * ((tex_colour.col.g as u16) + 1) - 1;
                let b = ((vtx_colour.col.b as u16) + 1) * ((tex_colour.col.b as u16) + 1) - 1;
                let a = (tex_colour.alpha as u16) * (vtx_colour.alpha as u16);

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
                    vtx_colour
                } else if tex_colour.alpha == 0x1F {
                    tex_colour
                } else {
                    let tex_alpha = tex_colour.alpha as u16;
                    let vtx_alpha = 0x1F - tex_alpha;
                    let r = ((vtx_colour.col.r as u16) * vtx_alpha).saturating_add((tex_colour.col.r as u16) * tex_alpha);
                    let g = ((vtx_colour.col.g as u16) * vtx_alpha).saturating_add((tex_colour.col.g as u16) * tex_alpha);
                    let b = ((vtx_colour.col.b as u16) * vtx_alpha).saturating_add((tex_colour.col.b as u16) * tex_alpha);
                    ColourAlpha {
                        col: Colour {
                            r: (r >> 5) as u8,
                            g: (g >> 5) as u8,
                            b: (b >> 5) as u8
                        },
                        alpha: vtx_colour.alpha
                    }
                }
            },

            FRAG_MODE_TOON_HIGHLIGHT => {
                let index = (vtx_colour.col.r >> 3) as usize;
                let table_colour = render_engine.toon_table[index];
                if render_engine.control.contains(Display3DControl::HIGHLIGHT_SHADING) {
                    // TODO
                    vtx_colour
                } else {
                    // TODO
                    vtx_colour
                }
            },

            FRAG_MODE_SHADOW => {
                vtx_colour  // TODO: blend with tex??
            },

            _ => unreachable!()
        }
    }

    fn blend_buffer_colour(frag_colour: ColourAlpha, buffer_colour: ColourAlpha, shadow_mode: bool) -> ColourAlpha {
        if frag_colour.alpha == 0 {
            buffer_colour
        } else if frag_colour.alpha == 0x1F || buffer_colour.alpha == 0 {
            ColourAlpha {
                col: frag_colour.col,
                alpha: if shadow_mode {buffer_colour.alpha} else {frag_colour.alpha}
            }
        } else {
            let frag_alpha = (frag_colour.alpha + 1) as u16;
            let buffer_alpha = (31 - frag_colour.alpha) as u16;
            let r = ((frag_colour.col.r as u16) * frag_alpha) * ((buffer_colour.col.r as u16) * buffer_alpha);
            let g = ((frag_colour.col.g as u16) * frag_alpha) * ((buffer_colour.col.g as u16) * buffer_alpha);
            let b = ((frag_colour.col.b as u16) * frag_alpha) * ((buffer_colour.col.b as u16) * buffer_alpha);
            let a = if shadow_mode {
                buffer_colour.alpha
            } else {
                std::cmp::max(frag_colour.alpha, buffer_colour.alpha)
            };
    
            ColourAlpha {
                col: Colour {
                    r: (r >> 5) as u8,
                    g: (g >> 5) as u8,
                    b: (b >> 5) as u8
                },
                alpha: a
            }
        }
    }
}
