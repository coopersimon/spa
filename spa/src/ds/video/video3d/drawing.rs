use super::{
    render::RenderingEngine,
    types::*, geometry::N
};
use fixed::{types::I23F9, traits::ToFixed};
use crate::{
    ds::video::{memory::Engine3DVRAM, video3d::types::Vertex},
    common::colour::*,
    utils::bits::u16, utils::bytes
};

#[derive(Copy, Clone, Default)]
struct Attributes {
    opaque_id:  u8,
    trans_id:   u8,
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
                fog:        render_engine.fog_enabled,
            };
            self.attr_buffer.fill(clear_attrs);
            self.depth_buffer.fill(render_engine.clear_depth);
            target.fill(ColourAlpha { col: render_engine.clear_colour, alpha: render_engine.clear_alpha });
        }
    }
    
    fn draw_opaque_polygons(&mut self, render_engine: &RenderingEngine, vram: &Engine3DVRAM, target: &mut [ColourAlpha], line: u8) {
        let y = N::from_num(line);

        for p in render_engine.polygon_ram.opaque_polygons.iter()
            //.skip_while(|el| el.y_max < line || el.y_min > line)
            //.take_while(|el| el.y_max >= line && el.y_min <= line)
        {
            if p.y_max < y || p.y_min > y {
                continue;
            }
            let polygon = &render_engine.polygon_ram.polygons[p.polygon_index];
            let mode = polygon.attrs.mode();
            
            let b = Self::find_intersect_points(render_engine, polygon, y);
            if b.is_none() {
                // TODO: why _can_ this return none?
                continue;
            }
            let [vtx_a, vtx_b] = b.unwrap();

            let x_diff = N::ONE.checked_div(vtx_b.screen_p.x - vtx_a.screen_p.x).unwrap_or(N::ZERO);

            // TODO: wireframe
            for x_idx in vtx_a.screen_p.x.to_num::<i16>()..=vtx_b.screen_p.x.to_num::<i16>() {
                let x = N::from_num(x_idx);
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

                // Interpolate vertex colour
                let vtx_colour = ColourAlpha {
                    col: Self::interpolate_vertex_colour(vtx_a.colour, vtx_b.colour, factor_a, factor_b),
                    alpha: 0x1F
                };

                let tex_coords = Self::interpolate_tex_coords(vtx_a.tex_coords, vtx_b.tex_coords, factor_a, factor_b);
                let tex_colour = Self::lookup_tex_colour(tex_coords, polygon.tex, polygon.palette, vram);

                let frag_colour = if let Some(tex_colour) = tex_colour {
                    Self::blend_frag_tex_colour(render_engine, mode, vtx_colour, tex_colour)
                } else {
                    Self::blend_fragment_colour(render_engine, mode, vtx_colour)
                };
                if frag_colour.alpha > 0 {
                    target[x_idx as usize] = frag_colour;
                    self.depth_buffer[x_idx as usize] = depth;
                    self.attr_buffer[x_idx as usize].opaque_id = polygon.attrs.id();
                    self.attr_buffer[x_idx as usize].fog = polygon.attrs.contains(PolygonAttrs::FOG_BLEND_ENABLE);
                }
            }
        }
    }
    
    fn draw_trans_polygons(&mut self, render_engine: &RenderingEngine, vram: &Engine3DVRAM, target: &mut [ColourAlpha], line: u8) {
        let y = N::from_num(line);

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
        let mode = polygon.attrs.mode();
        
        let [vtx_a, vtx_b] = Self::find_intersect_points(render_engine, polygon, y).unwrap();

        let x_diff = N::ONE.checked_div(vtx_b.screen_p.x - vtx_a.screen_p.x).unwrap_or(N::ZERO);

        for x_idx in vtx_a.screen_p.x.to_num::<i16>()..=vtx_b.screen_p.x.to_num::<i16>() {
            let id = polygon.attrs.id();
            // TODO: only extract for shadow polygons?
            let stencil_mask = std::mem::replace(&mut self.stencil_buffer[x_idx as usize], false);
            if self.attr_buffer[x_idx as usize].trans_id == id && polygon.attrs.alpha() != 0x1F {
                continue;
            }

            let x = N::from_num(x_idx);
            let factor_b = (x - vtx_a.screen_p.x) * x_diff;
            let factor_a = N::ONE - factor_b;

            let depth = Self::interpolate_depth(vtx_a.depth, vtx_b.depth, factor_a, factor_b);

            // Evaluate depth
            if self.depth_buffer[x_idx as usize] <= depth { // TODO: remove fractional part?
                if id == 0 && mode == PolygonMode::Shadow {
                    // Shadow polygon mask
                    self.stencil_buffer[x_idx as usize] = true;
                    continue;
                }
                // Point is behind buffer value.
                if !polygon.attrs.contains(PolygonAttrs::RENDER_EQ_DEPTH) || self.depth_buffer[x_idx as usize] < depth {
                    continue;
                }
            }

            if mode == PolygonMode::Shadow &&
                (!stencil_mask || self.attr_buffer[x_idx as usize].opaque_id == id) {
                // We only want to draw the shadow if it passes depth,
                // is masked, and doesn't match the IDs
                continue;
            }

            // Interpolate vertex colour
            let vtx_colour = ColourAlpha {
                col: Self::interpolate_vertex_colour(vtx_a.colour, vtx_b.colour, factor_a, factor_b),
                alpha: polygon.attrs.alpha()
            };
            
            let tex_coords = Self::interpolate_tex_coords(vtx_a.tex_coords, vtx_b.tex_coords, factor_a, factor_b);
            let tex_colour = Self::lookup_tex_colour(tex_coords, polygon.tex, polygon.palette, vram);

            let frag_colour = if let Some(tex_colour) = tex_colour {
                Self::blend_frag_tex_colour(render_engine, mode, vtx_colour, tex_colour)
            } else {
                Self::blend_fragment_colour(render_engine, mode, vtx_colour)
            };
            if render_engine.control.contains(Display3DControl::ALPHA_TEST_ENABLE) && frag_colour.alpha < render_engine.alpha_test {
                continue;
            } else if frag_colour.alpha == 0 {
                continue;
            } else if frag_colour.alpha != 0x1F && self.attr_buffer[x_idx as usize].trans_id == id {
                continue;
            }

            // We are sure that we want to render this fragment.
            if render_engine.control.contains(Display3DControl::BLENDING_ENABLE) {
                target[x_idx as usize] = Self::blend_buffer_colour(
                    frag_colour, target[x_idx as usize],
                    mode == PolygonMode::Shadow
                );
            } else {
                target[x_idx as usize] = frag_colour;
            }
            self.depth_buffer[x_idx as usize] = depth;
            self.attr_buffer[x_idx as usize].trans_id = id;
            self.attr_buffer[x_idx as usize].fog = self.attr_buffer[x_idx as usize].fog && polygon.attrs.contains(PolygonAttrs::FOG_BLEND_ENABLE);
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
    fn interpolate_tex_coords(tex_coords_a: TexCoords, tex_coords_b: TexCoords, factor_a: N, factor_b: N) -> TexCoords {
        let s = (tex_coords_a.s.to_fixed::<N>() * factor_a) + (tex_coords_b.s.to_fixed::<N>() * factor_b);
        let t = (tex_coords_a.t.to_fixed::<N>() * factor_a) + (tex_coords_b.t.to_fixed::<N>() * factor_b);
        TexCoords { s: s.to_fixed(), t: t.to_fixed() }
    }

    /// Lookup texture colour.
    fn lookup_tex_colour(tex_coords: TexCoords, tex_attrs: TextureAttrs, palette: u16, vram: &Engine3DVRAM) -> Option<ColourAlpha> {
        match tex_attrs.format() {
            1 => Some(Self::lookup_a3i5_tex(tex_coords, tex_attrs, palette, vram)),
            2 => Some(Self::lookup_2bpp_tex(tex_coords, tex_attrs, palette, vram)),
            3 => Some(Self::lookup_4bpp_tex(tex_coords, tex_attrs, palette, vram)),
            4 => Some(Self::lookup_8bpp_tex(tex_coords, tex_attrs, palette, vram)),
            5 => Some(Self::lookup_4x4_tex(tex_coords, tex_attrs, palette, vram)),
            6 => Some(Self::lookup_a5i3_tex(tex_coords, tex_attrs, palette, vram)),
            7 => Some(Self::lookup_dir_tex(tex_coords, tex_attrs, vram)),
            _ => None,
        }
    }

    /// Extract texture coordinates.
    fn get_tex_coords(tex_coords: TexCoords, tex_attrs: TextureAttrs) -> (u32, u32) {
        let width = tex_attrs.width();
        let height = tex_attrs.height();
        let base_tex_s = tex_coords.s.to_num::<i32>();
        let base_tex_t = tex_coords.t.to_num::<i32>();
        let unsigned_tex_s = base_tex_s as u32;
        let unsigned_tex_t = base_tex_t as u32;

        let tex_s = if tex_attrs.contains(TextureAttrs::REPEAT_S) {
            let mask = width - 1;
            if tex_attrs.contains(TextureAttrs::FLIP_S) {
                if (unsigned_tex_s & width) == 0 {  // Don't flip
                    unsigned_tex_s & mask
                } else {    // flip
                    let s = unsigned_tex_s & mask;
                    mask - s
                }
            } else {
                unsigned_tex_s & mask
            }
        } else {
            if base_tex_s >= (width as i32) {
                width - 1
            } else if base_tex_s < 0 {
                0
            } else {
                unsigned_tex_s
            }
        };
        
        let tex_t = if tex_attrs.contains(TextureAttrs::REPEAT_T) {
            let mask = height - 1;
            if tex_attrs.contains(TextureAttrs::FLIP_T) {
                if (unsigned_tex_t & height) == 0 {  // Don't flip
                    unsigned_tex_t & mask
                } else {    // flip
                    let t = unsigned_tex_t & mask;
                    mask - t
                }
            } else {
                unsigned_tex_t & mask
            }
        } else {
            if base_tex_t >= (height as i32) {
                height - 1
            } else if base_tex_t < 0 {
                0
            } else {
                unsigned_tex_t
            }
        };

        (tex_s, tex_t)
    }

    /// Lookup 2bpp texel colour.
    fn lookup_2bpp_tex(tex_coords: TexCoords, tex_attrs: TextureAttrs, palette: u16, vram: &Engine3DVRAM) -> ColourAlpha {
        let (s, t) = Self::get_tex_coords(tex_coords, tex_attrs);
        let width = tex_attrs.width();
        let pos = (width * t) + s;
        let offset = pos / 4;
        let shift = (pos % 4) << 1;
        let addr = tex_attrs.addr();

        let data = (vram.get_tex_byte(addr + offset) >> shift) & 0b11;

        if (data == 0) && tex_attrs.contains(TextureAttrs::TRANSPARENT_0) {
            ColourAlpha::transparent()
        } else {
            let palette_offset = (data as u32) << 1;
            let palette_addr = (palette as u32) << 3;
            let colour = vram.get_palette_halfword(palette_addr + palette_offset);
            ColourAlpha {col: Colour::from_555(colour), alpha: 0x1F}
        }
    }
    
    /// Lookup 4bpp texel colour.
    fn lookup_4bpp_tex(tex_coords: TexCoords, tex_attrs: TextureAttrs, palette: u16, vram: &Engine3DVRAM) -> ColourAlpha {
        let (s, t) = Self::get_tex_coords(tex_coords, tex_attrs);
        let width = tex_attrs.width();
        let pos = (width * t) + s;
        let offset = pos / 2;
        let shift = (pos % 2) << 2;
        let addr = tex_attrs.addr();

        let data = (vram.get_tex_byte(addr + offset) >> shift) & 0xF;

        if (data == 0) && tex_attrs.contains(TextureAttrs::TRANSPARENT_0) {
            ColourAlpha::transparent()
        } else {
            let palette_offset = (data as u32) << 1;
            let palette_addr = (palette as u32) << 4;
            let colour = vram.get_palette_halfword(palette_addr + palette_offset);
            ColourAlpha {col: Colour::from_555(colour), alpha: 0x1F}
        }
    }
    
    /// Lookup 8bpp texel colour.
    fn lookup_8bpp_tex(tex_coords: TexCoords, tex_attrs: TextureAttrs, palette: u16, vram: &Engine3DVRAM) -> ColourAlpha {
        let (s, t) = Self::get_tex_coords(tex_coords, tex_attrs);
        let width = tex_attrs.width();
        let pos = (width * t) + s;
        let offset = pos;
        let addr = tex_attrs.addr();

        let data = vram.get_tex_byte(addr + offset);

        if (data == 0) && tex_attrs.contains(TextureAttrs::TRANSPARENT_0) {
            ColourAlpha::transparent()
        } else {
            let palette_offset = (data as u32) << 1;
            let palette_addr = (palette as u32) << 4;
            let colour = vram.get_palette_halfword(palette_addr + palette_offset);
            ColourAlpha {col: Colour::from_555(colour), alpha: 0x1F}
        }
    }
    
    /// Lookup direct texel colour.
    fn lookup_dir_tex(tex_coords: TexCoords, tex_attrs: TextureAttrs, vram: &Engine3DVRAM) -> ColourAlpha {
        let (s, t) = Self::get_tex_coords(tex_coords, tex_attrs);
        let width = tex_attrs.width();
        let pos = (width * t) + s;
        let offset = pos * 2;
        let addr = tex_attrs.addr();

        let data = vram.get_tex_halfword(addr + offset);
        ColourAlpha {col: Colour::from_555(data), alpha: ((data >> 15) as u8) * 0x1F}
    }

    fn lookup_a3i5_tex(tex_coords: TexCoords, tex_attrs: TextureAttrs, palette: u16, vram: &Engine3DVRAM) -> ColourAlpha {
        let (s, t) = Self::get_tex_coords(tex_coords, tex_attrs);
        let width = tex_attrs.width();
        let pos = (width * t) + s;
        let offset = pos;
        let addr = tex_attrs.addr();

        let data = vram.get_tex_byte(addr + offset);
        let palette_data = data & 0x1F;
        let alpha_data = (data & 0xE0) >> 5;

        let palette_offset = (palette_data as u32) << 1;
        let palette_addr = (palette as u32) << 4;
        let colour = vram.get_palette_halfword(palette_addr + palette_offset);
        let alpha = (alpha_data << 2) | (alpha_data >> 1);
        ColourAlpha {col: Colour::from_555(colour), alpha}
    }
    
    fn lookup_a5i3_tex(tex_coords: TexCoords, tex_attrs: TextureAttrs, palette: u16, vram: &Engine3DVRAM) -> ColourAlpha {
        let (s, t) = Self::get_tex_coords(tex_coords, tex_attrs);
        let width = tex_attrs.width();
        let pos = (width * t) + s;
        let offset = pos;
        let addr = tex_attrs.addr();

        let data = vram.get_tex_byte(addr + offset);
        let palette_data = data & 0x7;
        let alpha_data = (data & 0xF8) >> 3;

        let palette_offset = (palette_data as u32) << 1;
        let palette_addr = (palette as u32) << 4;
        let colour = vram.get_palette_halfword(palette_addr + palette_offset);
        ColourAlpha {col: Colour::from_555(colour), alpha: alpha_data}
    }
    
    fn lookup_4x4_tex(tex_coords: TexCoords, tex_attrs: TextureAttrs, palette: u16, vram: &Engine3DVRAM) -> ColourAlpha {
        let (s, t) = Self::get_tex_coords(tex_coords, tex_attrs);
        let block_s = s / 4;
        let block_t = t / 4;
        let block_width = tex_attrs.width();
        let block_addr = tex_attrs.addr() + (block_t * block_width) + (block_s * 4);
        
        let sub_block_s = s % 4;
        let sub_block_t = t % 4;

        let shift = sub_block_s << 1;
        let block_data = (vram.get_tex_byte(block_addr + sub_block_t) >> shift) & 0b11;

        let block_palette_upper = (block_addr >> 2) & 0x1_0000;
        let block_palette_addr = 0x2_0000 + (block_palette_upper | (block_addr / 2));
        let block_palette_data = vram.get_tex_halfword(block_palette_addr);

        let base_palette_addr = (palette as u32) << 4;
        let block_palette_offset = ((block_palette_data & 0x3FFF) as u32) << 2;
        let palette_addr = base_palette_addr + block_palette_offset;
        
        let transparent_3 = !u16::test_bit(block_palette_data, 15);
        if u16::test_bit(block_palette_data, 14) {
            // Interpolation mode.
            match block_data {
                0 => {
                    let colour = vram.get_palette_halfword(palette_addr);
                    ColourAlpha {col: Colour::from_555(colour), alpha: 0x1F}
                },
                1 => {
                    let colour = vram.get_palette_halfword(palette_addr + 2);
                    ColourAlpha {col: Colour::from_555(colour), alpha: 0x1F}
                },
                2 if transparent_3 => {
                    let col_0 = vram.get_palette_halfword(palette_addr);
                    let col_1 = vram.get_palette_halfword(palette_addr + 2);
                    let r = (col_0 & 0x1F) + (col_1 & 0x1F);
                    let g = ((col_0 >> 5) & 0x1F) + ((col_1 >> 5) & 0x1F);
                    let b = ((col_0 >> 10) & 0x1F) + ((col_1 >> 10) & 0x1F);
                    ColourAlpha {
                        col: Colour {
                            r: ((r << 2) | (r >> 4)) as u8,
                            g: ((g << 2) | (g >> 4)) as u8,
                            b: ((b << 2) | (b >> 4)) as u8,
                        }, alpha: 0x1F
                    }
                },
                2 => {
                    let col_0 = vram.get_palette_halfword(palette_addr);
                    let col_1 = vram.get_palette_halfword(palette_addr + 2);
                    let r = (col_0 & 0x1F) * 5 + (col_1 & 0x1F) * 3;
                    let g = ((col_0 >> 5) & 0x1F) * 5 + ((col_1 >> 5) & 0x1F) * 3;
                    let b = ((col_0 >> 10) & 0x1F) * 5 + ((col_1 >> 10) & 0x1F) * 3;
                    ColourAlpha {
                        col: Colour {
                            r: r as u8,
                            g: g as u8,
                            b: b as u8,
                        }, alpha: 0x1F
                    }
                },
                3 if transparent_3 => ColourAlpha::transparent(),
                3 => {
                    let col_0 = vram.get_palette_halfword(palette_addr);
                    let col_1 = vram.get_palette_halfword(palette_addr + 2);
                    let r = (col_0 & 0x1F) * 3 + (col_1 & 0x1F) * 5;
                    let g = ((col_0 >> 5) & 0x1F) * 3 + ((col_1 >> 5) & 0x1F) * 5;
                    let b = ((col_0 >> 10) & 0x1F) * 3 + ((col_1 >> 10) & 0x1F) * 5;
                    ColourAlpha {
                        col: Colour {
                            r: r as u8,
                            g: g as u8,
                            b: b as u8,
                        }, alpha: 0x1F
                    }
                },
                _ => unreachable!()
            }
        } else {
            if block_data == 3 && transparent_3 {
                ColourAlpha::transparent()
            } else {
                let palette_offset = (block_data as u32) << 1;
                let colour = vram.get_palette_halfword(palette_addr + palette_offset);
                ColourAlpha {col: Colour::from_555(colour), alpha: 0x1F}
            }
        }
    }

    /// Use the polygon's specified blending mode to blend the fragment's colour.
    /// 
    /// Use the vertex colour and texture colour as sources.
    fn blend_fragment_colour(render_engine: &RenderingEngine, mode: PolygonMode, vtx_colour: ColourAlpha) -> ColourAlpha {
        match mode {
            PolygonMode::Modulation | PolygonMode::Decal => vtx_colour,
            PolygonMode::ToonHighlight => {
                let index = (vtx_colour.col.r >> 3) as usize;
                let table_colour = render_engine.toon_table[index];
                if render_engine.control.contains(Display3DControl::HIGHLIGHT_SHADING) {
                    let r = vtx_colour.col.r.saturating_add(table_colour.r);
                    let g = vtx_colour.col.g.saturating_add(table_colour.g);
                    let b = vtx_colour.col.b.saturating_add(table_colour.b);
                    ColourAlpha {
                        col: Colour { r, g, b },
                        alpha: vtx_colour.alpha
                    }
                } else {
                    ColourAlpha {
                        col: table_colour,
                        alpha: vtx_colour.alpha
                    }
                }
            },
            PolygonMode::Shadow => vtx_colour,
        }
    }

    /// Use the polygon's specified blending mode to blend the fragment's colour.
    /// 
    /// Use the vertex colour and texture colour as sources.
    fn blend_frag_tex_colour(render_engine: &RenderingEngine, mode: PolygonMode, vtx_colour: ColourAlpha, tex_colour: ColourAlpha) -> ColourAlpha {
        match mode {
            PolygonMode::Modulation => {
                let r = ((vtx_colour.col.r as u16) + 1) * ((tex_colour.col.r as u16) + 1) - 1;
                let g = ((vtx_colour.col.g as u16) + 1) * ((tex_colour.col.g as u16) + 1) - 1;
                let b = ((vtx_colour.col.b as u16) + 1) * ((tex_colour.col.b as u16) + 1) - 1;
                let a = (vtx_colour.alpha as u16) * (tex_colour.alpha as u16);

                ColourAlpha {
                    col: Colour {
                        r: bytes::u16::hi(r),
                        g: bytes::u16::hi(g),
                        b: bytes::u16::hi(b)
                    },
                    alpha: (a >> 5) as u8
                }
            },
            PolygonMode::Decal => {
                if tex_colour.alpha == 0 {
                    vtx_colour
                } else if tex_colour.alpha == 0x1F {
                    tex_colour
                } else {
                    let tex_alpha = tex_colour.alpha as u16;
                    let vtx_alpha = 0x1F - tex_alpha;
                    let r = ((vtx_colour.col.r as u16) * vtx_alpha) + ((tex_colour.col.r as u16) * tex_alpha);
                    let g = ((vtx_colour.col.g as u16) * vtx_alpha) + ((tex_colour.col.g as u16) * tex_alpha);
                    let b = ((vtx_colour.col.b as u16) * vtx_alpha) + ((tex_colour.col.b as u16) * tex_alpha);
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
            PolygonMode::ToonHighlight => {
                let index = (vtx_colour.col.r >> 3) as usize;
                let table_colour = render_engine.toon_table[index];
                if render_engine.control.contains(Display3DControl::HIGHLIGHT_SHADING) {
                    let frag_r = ((vtx_colour.col.r as u16) + 1) * ((tex_colour.col.r as u16) + 1) - 1;
                    let frag_g = ((vtx_colour.col.g as u16) + 1) * ((tex_colour.col.g as u16) + 1) - 1;
                    let frag_b = ((vtx_colour.col.b as u16) + 1) * ((tex_colour.col.b as u16) + 1) - 1;

                    let r = bytes::u16::hi(frag_r).saturating_add(table_colour.r);
                    let g = bytes::u16::hi(frag_g).saturating_add(table_colour.g);
                    let b = bytes::u16::hi(frag_b).saturating_add(table_colour.b);
                    let a = (vtx_colour.alpha as u16) * (tex_colour.alpha as u16);
                    ColourAlpha {
                        col: Colour { r, g, b },
                        alpha: (a >> 5) as u8
                    }
                } else {
                    let r = ((table_colour.r as u16) + 1) * ((tex_colour.col.r as u16) + 1) - 1;
                    let g = ((table_colour.g as u16) + 1) * ((tex_colour.col.g as u16) + 1) - 1;
                    let b = ((table_colour.b as u16) + 1) * ((tex_colour.col.b as u16) + 1) - 1;
                    let a = (vtx_colour.alpha as u16) * (tex_colour.alpha as u16);
                    ColourAlpha {
                        col: Colour {
                            r: bytes::u16::hi(r),
                            g: bytes::u16::hi(g),
                            b: bytes::u16::hi(b)
                        },
                        alpha: (a >> 5) as u8
                    }
                }
            },
            PolygonMode::Shadow => vtx_colour  // TODO: blend with tex??,
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
            let frag_alpha = (frag_colour.alpha + 1) as u32;
            let buffer_alpha = (31 - frag_colour.alpha) as u32;
            let r = ((frag_colour.col.r as u32) * frag_alpha) + ((buffer_colour.col.r as u32) * buffer_alpha);
            let g = ((frag_colour.col.g as u32) * frag_alpha) + ((buffer_colour.col.g as u32) * buffer_alpha);
            let b = ((frag_colour.col.b as u32) * frag_alpha) + ((buffer_colour.col.b as u32) * buffer_alpha);
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
