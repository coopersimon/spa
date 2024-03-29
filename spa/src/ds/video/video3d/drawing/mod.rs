mod palette;

use super::{
    render::RenderingEngine,
    types::*, geometry::N, interpolate::*
};
use fixed::types::I40F24;
use fixed::traits::ToFixed;
use crate::{
    ds::video::{memory::Engine3DVRAM, video3d::types::Vertex},
    common::video::colour::*,
    utils::bits::u16, utils::bytes
};

use palette::TexPaletteCache;

#[derive(Copy, Clone, Default)]
struct Attributes {
    opaque_id:  u8,
    trans_id:   u8,
    fog:        bool,
    edge:       bool,
}

/// Render NDS 3D graphics.
pub struct Software3DRenderer {
    palette_cache:  TexPaletteCache,
    stencil_buffer: Vec<bool>,
    attr_buffer:    Vec<Attributes>,
    depth_buffer:   Vec<Depth>,
}

impl Software3DRenderer {
    pub fn new() -> Self {
        Self {
            palette_cache:  TexPaletteCache::new(),
            stencil_buffer: vec![false; 256 * 192],
            attr_buffer:    vec![Default::default(); 256 * 192],
            depth_buffer:   vec![Depth::ZERO; 256 * 192],
        }
    }

    pub fn setup_caches(&mut self, vram: &mut Engine3DVRAM) {
        self.palette_cache.update_tex(&vram.ref_tex_palette());
    }

    pub fn draw(&mut self, render_engine: &RenderingEngine, vram: &Engine3DVRAM, target: &mut [ColourAlpha]) {
        self.clear_buffers(render_engine, vram, target);

        self.draw_opaque_polygons(render_engine, vram, target);

        self.draw_trans_polygons(render_engine, vram, target);

        if render_engine.control.contains(Display3DControl::EDGE_MARKING) {
            self.mark_edges(render_engine, target);
        }

        if render_engine.control.contains(Display3DControl::FOG_ENABLE) {
            self.draw_fog(render_engine, target);
        }

        // Anti-aliasing (after 2d-blend?)

    }
}

impl Software3DRenderer {
    /// Fill the drawing buffers with clear values or clear image.
    fn clear_buffers(&mut self, render_engine: &RenderingEngine, vram: &Engine3DVRAM, target: &mut [ColourAlpha]) {
        self.stencil_buffer.fill(false);

        if render_engine.control.contains(Display3DControl::CLEAR_IMAGE) {
            let clear_colour_image = vram.tex_2.as_ref().expect("using clear colour image without mapped vram");
            let clear_depth_image = vram.tex_3.as_ref().expect("using clear depth image without mapped vram");

            for y in 0..192_u8 {
                let y_idx_base = (y as usize) * 256;

                let image_y = y.wrapping_add(render_engine.clear_image_y);
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
                        edge:       false,
                    };
    
                    let idx = y_idx_base + (x as usize);
                    self.attr_buffer[idx] = clear_attrs;
                    self.depth_buffer[idx] = Depth::from_bits(((depth & 0x7FFF) as i32) << 9);   // TODO: frac part.
                    target[idx].col = Colour::from_555(colour);
                    target[idx].alpha = if u16::test_bit(colour, 15) {0x1F} else {0};
                }
            }
        } else {
            let clear_attrs = Attributes {
                opaque_id:  render_engine.clear_poly_id,
                trans_id:   render_engine.clear_poly_id,
                fog:        render_engine.fog_enabled,
                edge:       false,
            };
            self.attr_buffer.fill(clear_attrs);
            self.depth_buffer.fill(render_engine.clear_depth);
            target.fill(ColourAlpha { col: render_engine.clear_colour, alpha: render_engine.clear_alpha });
        }
    }
    
    fn draw_opaque_polygons(&mut self, render_engine: &RenderingEngine, vram: &Engine3DVRAM, target: &mut [ColourAlpha]) {
        for p in render_engine.polygon_ram.opaque_polygons.iter() {

            let polygon = &render_engine.polygon_ram.polygons[p.polygon_index];
            let mode = polygon.attrs.mode();
            
            let (mut x_min_prev, mut x_max_prev) = (256, 0);
            let (y_min, y_max) = (p.y_min.to_num::<u8>(), std::cmp::min(p.y_max.to_num::<u8>(), 191));
            for y_idx in y_min..=y_max {

                let y = N::from_num(y_idx);
                let y_idx_base = (y_idx as usize) * 256;

                let Some([vtx_a, vtx_b]) = Self::find_intersect_points(render_engine, polygon, y.clamp(p.y_min, p.y_max)) else {
                    continue;
                };

                let (min, max) = (vtx_a.screen_p.x.to_num::<i16>(), vtx_b.screen_p.x.to_num::<i16>());
                let x_diff = (vtx_b.screen_p.x - vtx_a.screen_p.x).to_fixed::<I40F24>().checked_recip().unwrap_or(I40F24::ZERO);

                let x_max = std::cmp::min(max, 255);
                for x_idx in min..=x_max {
                    let x = N::from_num(x_idx);
                    let factor_b = ((x - vtx_a.screen_p.x).to_fixed::<I40F24>() * x_diff).to_fixed::<N>().clamp(N::ZERO, N::ONE);
                    let factor_a = N::ONE - factor_b;

                    let depth = interpolate_depth(vtx_a.depth, vtx_b.depth, factor_a, factor_b);

                    let idx = y_idx_base + (x_idx as usize);
                    if !Self::test_depth(polygon.render_eq_depth(), self.depth_buffer[idx], depth) {
                        continue;
                    }

                    let top_edge = x_idx < x_min_prev || x_idx > x_max_prev;
                    let bottom_edge = false; // TODO: compare with next line.
                    let edge = top_edge || bottom_edge || (x_idx == min) || (x_idx == x_max);
                    if !edge && polygon.is_wireframe() {
                        continue;
                    }

                    // Interpolate vertex colour
                    let vtx_colour = ColourAlpha {
                        col: interpolate_vertex_colour(vtx_a.colour, vtx_b.colour, factor_a, factor_b),
                        alpha: 0x1F
                    };

                    let tex_coords = interpolate_tex_coords_p(vtx_a.tex_coords, vtx_b.tex_coords, factor_a, factor_b, vtx_a.depth, vtx_b.depth);
                    let tex_colour = self.lookup_tex_colour(tex_coords, polygon.tex, polygon.palette, vram);

                    let frag_colour = if let Some(tex_colour) = tex_colour {
                        Self::blend_frag_tex_colour(render_engine, mode, vtx_colour, tex_colour)
                    } else {
                        Self::blend_fragment_colour(render_engine, mode, vtx_colour)
                    };
                    if frag_colour.alpha > 0 {
                        target[idx] = frag_colour;
                        self.depth_buffer[idx] = depth;
                        self.attr_buffer[idx].opaque_id = polygon.attrs.id();
                        self.attr_buffer[idx].fog = polygon.attrs.contains(PolygonAttrs::FOG_BLEND_ENABLE);
                        self.attr_buffer[idx].edge = edge;
                    }
                }

                x_min_prev = min;
                x_max_prev = max;
            }
            
        }
    }
    
    fn draw_trans_polygons(&mut self, render_engine: &RenderingEngine, vram: &Engine3DVRAM, target: &mut [ColourAlpha]) {
        if render_engine.polygon_ram.use_manual_mode {
            render_engine.polygon_ram.trans_polygon_manual.iter()
                .for_each(|p| self.draw_trans_polygon(render_engine, vram, target, p));
        } else {
            render_engine.polygon_ram.trans_polygon_auto.iter()
                .for_each(|p| self.draw_trans_polygon(render_engine, vram, target, p));
        }
    }

    fn draw_trans_polygon(&mut self, render_engine: &RenderingEngine, vram: &Engine3DVRAM, target: &mut [ColourAlpha], p: &PolygonOrder) {
        let polygon = &render_engine.polygon_ram.polygons[p.polygon_index];
        let mode = polygon.attrs.mode();
        
        let (y_min, y_max) = (p.y_min.to_num::<u8>(), std::cmp::min(p.y_max.to_num::<u8>(), 191));
        for y_idx in y_min..=y_max {

            let y = N::from_num(y_idx);
            let y_idx_base = (y_idx as usize) * 256;

            let Some([vtx_a, vtx_b]) = Self::find_intersect_points(render_engine, polygon, y.clamp(p.y_min, p.y_max)) else {
                continue;
            };

            let (min, max) = (vtx_a.screen_p.x.to_num::<i16>(), vtx_b.screen_p.x.to_num::<i16>());
            let x_diff = (vtx_b.screen_p.x - vtx_a.screen_p.x).to_fixed::<I40F24>().checked_recip().unwrap_or(I40F24::ZERO);

            for x_idx in min..=std::cmp::min(max, 255) {
                let id = polygon.attrs.id();
                let idx = y_idx_base + (x_idx as usize);
                // TODO: only extract for shadow polygons?
                let stencil_mask = std::mem::replace(&mut self.stencil_buffer[idx], false);
                if self.attr_buffer[idx].trans_id == id && polygon.attrs.alpha() != 0x1F {
                    continue;
                }

                let x = N::from_num(x_idx);
                let factor_b = ((x - vtx_a.screen_p.x).to_fixed::<I40F24>() * x_diff).to_fixed::<N>().clamp(N::ZERO, N::ONE);
                let factor_a = N::ONE - factor_b;

                let depth = interpolate_depth(vtx_a.depth, vtx_b.depth, factor_a, factor_b);

                if !Self::test_depth(polygon.render_eq_depth(), self.depth_buffer[idx], depth) {
                    if id == 0 && mode == PolygonMode::Shadow {
                        // Shadow polygon mask
                        self.stencil_buffer[idx] = true;
                    }
                    continue;
                }

                if mode == PolygonMode::Shadow &&
                    (!stencil_mask || self.attr_buffer[idx].opaque_id == id) {
                    // We only want to draw the shadow if it passes depth,
                    // is masked, and doesn't match the IDs
                    continue;
                }

                // Interpolate vertex colour
                let vtx_colour = ColourAlpha {
                    col: interpolate_vertex_colour(vtx_a.colour, vtx_b.colour, factor_a, factor_b),
                    alpha: polygon.attrs.alpha()
                };
                
                let tex_coords = interpolate_tex_coords_p(vtx_a.tex_coords, vtx_b.tex_coords, factor_a, factor_b, vtx_a.depth, vtx_b.depth);
                let tex_colour = self.lookup_tex_colour(tex_coords, polygon.tex, polygon.palette, vram);

                let frag_colour = if let Some(tex_colour) = tex_colour {
                    Self::blend_frag_tex_colour(render_engine, mode, vtx_colour, tex_colour)
                } else {
                    Self::blend_fragment_colour(render_engine, mode, vtx_colour)
                };
                if frag_colour.alpha == 0 {
                    continue;
                } else if render_engine.control.contains(Display3DControl::ALPHA_TEST_ENABLE) && frag_colour.alpha < render_engine.alpha_test {
                    continue;
                } else if frag_colour.alpha != 0x1F && self.attr_buffer[idx].trans_id == id {
                    // TODO: fix the logic here.
                    //continue;
                }

                // We are sure that we want to render this fragment.
                if render_engine.control.contains(Display3DControl::BLENDING_ENABLE) {
                    target[idx] = Self::blend_buffer_colour(
                        frag_colour, target[idx],
                        mode == PolygonMode::Shadow
                    );
                } else {
                    target[idx] = frag_colour;
                }

                if frag_colour.alpha != 0x1F {
                    if polygon.attrs.contains(PolygonAttrs::ALPHA_DEPTH) {
                        self.depth_buffer[idx] = depth;
                    }
                    self.attr_buffer[idx].trans_id = id;
                    self.attr_buffer[idx].fog = self.attr_buffer[idx].fog && polygon.attrs.contains(PolygonAttrs::FOG_BLEND_ENABLE);
                } else {
                    self.depth_buffer[idx] = depth;
                    self.attr_buffer[idx].opaque_id = id;
                    self.attr_buffer[idx].fog = polygon.attrs.contains(PolygonAttrs::FOG_BLEND_ENABLE);
                }
            }
        }
        
    }

    fn draw_fog(&mut self, render_engine: &RenderingEngine, target: &mut [ColourAlpha]) {
        let fog_shift = (render_engine.control & Display3DControl::FOG_SHIFT).bits() >> 8;
        let fog_interval = 0x400 >> fog_shift;
        let fog_min = render_engine.fog_offset + fog_interval;
        let fog_max = render_engine.fog_offset + (fog_interval << 5);
        let fog_diff = Depth::from_num(fog_interval);
        for idx in 0..(256 * 192) {
            if !self.attr_buffer[idx].fog {
                continue;
            }

            // Upper 15 bits of depth.
            let depth = (self.depth_buffer[idx].to_num::<i32>() & 0x7FFF) as u16;
            let fog_density = if depth < fog_min {
                render_engine.fog_table[0]
            } else if depth >= fog_max  {
                render_engine.fog_table[31]
            } else {
                // Interpolate fog.
                // TODO: what precision?
                let fog_index = (Depth::from_num(depth) - Depth::from_num(fog_min)) / fog_diff;
                let a = render_engine.fog_table[fog_index.to_num::<usize>()];
                let b = render_engine.fog_table[fog_index.ceil().to_num::<usize>()];
                let frac = fog_index.frac();
                let density = Depth::from_num(a) * (Depth::ONE - frac) + Depth::from_num(b) * frac;
                density.to_num::<u8>()
            } as u16;

            if render_engine.control.contains(Display3DControl::FOG_MODE) {
                if fog_density == 127 {
                    target[idx].alpha = render_engine.fog_alpha;
                } else {
                    let buffer_density = 128 - fog_density;
                    let alpha = (target[idx].alpha as u16) * buffer_density + (render_engine.fog_alpha as u16) * fog_density;
                    target[idx].alpha = (alpha >> 7) as u8;
                }
            } else {
                if fog_density == 127 {
                    target[idx].col = render_engine.fog_colour;
                    target[idx].alpha = render_engine.fog_alpha;
                } else {
                    let buffer_density = 128 - fog_density;
                    let r = (target[idx].col.r as u16) * buffer_density + (render_engine.fog_colour.r as u16) * fog_density;
                    let g = (target[idx].col.g as u16) * buffer_density + (render_engine.fog_colour.g as u16) * fog_density;
                    let b = (target[idx].col.b as u16) * buffer_density + (render_engine.fog_colour.b as u16) * fog_density;
                    let alpha = (target[idx].alpha as u16) * buffer_density + (render_engine.fog_alpha as u16) * fog_density;
                    target[idx] = ColourAlpha {
                        col: Colour {
                            r: (r >> 7) as u8,
                            g: (g >> 7) as u8,
                            b: (b >> 7) as u8
                        },
                        alpha: (alpha >> 7) as u8
                    }
                }
            }
        }
    }

    fn mark_edges(&mut self, render_engine: &RenderingEngine, target: &mut [ColourAlpha]) {
        for y in 0..=191 {
            let offset = y * 256;
            for x in 0..=255 {
                let index = offset + x;
                
                if !self.attr_buffer[index].edge {
                    continue;
                }
                let this = (self.attr_buffer[index].opaque_id, self.depth_buffer[index]);
                let left = if x == 0 {
                    (render_engine.clear_poly_id, render_engine.clear_depth)
                } else {(self.attr_buffer[index - 1].opaque_id, self.depth_buffer[index - 1])};
                let right = if x == 255 {
                    (render_engine.clear_poly_id, render_engine.clear_depth)
                } else {(self.attr_buffer[index + 1].opaque_id, self.depth_buffer[index + 1])};
                let top = if y == 0 {
                    (render_engine.clear_poly_id, render_engine.clear_depth)
                } else {(self.attr_buffer[index - 256].opaque_id, self.depth_buffer[index - 256])};
                let bottom = if y == 191 {
                    (render_engine.clear_poly_id, render_engine.clear_depth)
                } else {(self.attr_buffer[index + 256].opaque_id, self.depth_buffer[index + 256])};

                if Self::check_edge(this, left, right, top, bottom) {
                    let edge_index = (this.0 >> 3) as usize;
                    target[index].col = render_engine.edge_colour[edge_index];
                }
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
        // Find start and end points.
        let mut lines = [None, None];
        for i in 0..polygon.num_vertices {
            // Find where render line intersects polygon lines.
            let v_index_a = polygon.vertex_indices[i as usize];
            let v_index_b = polygon.vertex_indices[((i + 1) % polygon.num_vertices) as usize];

            let vtx_a = &render_engine.polygon_ram.vertices[v_index_a as usize];
            let vtx_b = &render_engine.polygon_ram.vertices[v_index_b as usize];

            if (y > vtx_a.screen_p.y && y > vtx_b.screen_p.y) || (y < vtx_a.screen_p.y && y < vtx_b.screen_p.y) || (vtx_a.screen_p.y == vtx_b.screen_p.y) {
                // This line does not intersect the render line.
                continue;
            }

            // Weight of point a (normalised between 0-1)
            let factor_a = ((y - vtx_b.screen_p.y).to_fixed::<I40F24>() / (vtx_a.screen_p.y - vtx_b.screen_p.y).to_fixed::<I40F24>())
                .to_fixed::<N>().clamp(N::ZERO, N::ONE);   // TODO: one dot polygon?
            // X coordinate where the render line intersects the polygon line.
            let intersect_x = factor_a.to_fixed::<I40F24>() * (vtx_a.screen_p.x - vtx_b.screen_p.x).to_fixed::<I40F24>() + vtx_b.screen_p.x.to_fixed::<I40F24>();
            let factor_b = N::ONE - factor_a;

            // Interpolate attributes
            let depth = interpolate_depth(vtx_a.depth, vtx_b.depth, factor_a, factor_b);
            let frag_colour = interpolate_vertex_colour(vtx_a.colour, vtx_b.colour, factor_a, factor_b);
            let tex_coords = interpolate_tex_coords_p(vtx_a.tex_coords, vtx_b.tex_coords, factor_a, factor_b, vtx_a.depth, vtx_b.depth);

            let vertex = Vertex {
                screen_p:   Coords { x: intersect_x.to_fixed::<N>(), y: y },
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

    /// Returns true if the fragment passes the depth test.
    fn test_depth(render_eq: bool, buffer_depth: Depth, frag_depth: Depth) -> bool {
        if render_eq {
            (buffer_depth - Depth::ONE) >= frag_depth || (buffer_depth + Depth::ONE) >= frag_depth
        } else {
            buffer_depth > frag_depth
        }
    }

    /// Lookup texture colour.
    fn lookup_tex_colour(&self, tex_coords: TexCoords, tex_attrs: TextureAttrs, palette: u16, vram: &Engine3DVRAM) -> Option<ColourAlpha> {
        match tex_attrs.format() {
            1 => Some(self.lookup_a3i5_tex(tex_coords, tex_attrs, palette, vram)),
            2 => Some(self.lookup_2bpp_tex(tex_coords, tex_attrs, palette, vram)),
            3 => Some(self.lookup_4bpp_tex(tex_coords, tex_attrs, palette, vram)),
            4 => Some(self.lookup_8bpp_tex(tex_coords, tex_attrs, palette, vram)),
            5 => Some(self.lookup_4x4_tex(tex_coords, tex_attrs, palette, vram)),
            6 => Some(self.lookup_a5i3_tex(tex_coords, tex_attrs, palette, vram)),
            7 => Some(self.lookup_dir_tex(tex_coords, tex_attrs, vram)),
            _ => None,
        }
    }

    /// Extract texture coordinates.
    fn get_tex_coords(tex_coords: TexCoords, tex_attrs: TextureAttrs) -> (u32, u32) {
        let width = tex_attrs.width();
        let base_tex_s = tex_coords.s.to_num::<i32>();

        let tex_s = if tex_attrs.contains(TextureAttrs::REPEAT_S) {
            let unsigned_tex_s = base_tex_s as u32;
            let mask = width - 1;
            if tex_attrs.contains(TextureAttrs::FLIP_S) && (unsigned_tex_s & width) != 0 {
                // Flip
                let s = unsigned_tex_s & mask;
                mask - s
            } else {
                unsigned_tex_s & mask
            }
        } else {
            // Clamp
            let max = (width - 1) as i32;
            std::cmp::min(max, std::cmp::max(base_tex_s, 0)) as u32
        };
        
        let height = tex_attrs.height();
        let base_tex_t = tex_coords.t.to_num::<i32>();

        let tex_t = if tex_attrs.contains(TextureAttrs::REPEAT_T) {
            let unsigned_tex_t = base_tex_t as u32;
            let mask = height - 1;
            if tex_attrs.contains(TextureAttrs::FLIP_T) && (unsigned_tex_t & height) != 0 {
                // Flip
                let t = unsigned_tex_t & mask;
                mask - t
            } else {
                unsigned_tex_t & mask
            }
        } else {
            // Clamp
            let max = (height - 1) as i32;
            std::cmp::min(max, std::cmp::max(base_tex_t, 0)) as u32
        };

        (tex_s, tex_t)
    }

    /// Lookup 2bpp texel colour.
    fn lookup_2bpp_tex(&self, tex_coords: TexCoords, tex_attrs: TextureAttrs, palette: u16, vram: &Engine3DVRAM) -> ColourAlpha {
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
            let palette_addr = (palette as u32) << 2;
            let colour = self.palette_cache.get_tex_colour(palette_addr + (data as u32));
            ColourAlpha {col: colour, alpha: 0x1F}
        }
    }
    
    /// Lookup 4bpp texel colour.
    fn lookup_4bpp_tex(&self, tex_coords: TexCoords, tex_attrs: TextureAttrs, palette: u16, vram: &Engine3DVRAM) -> ColourAlpha {
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
            let palette_addr = (palette as u32) << 3;
            let colour = self.palette_cache.get_tex_colour(palette_addr + (data as u32));
            ColourAlpha {col: colour, alpha: 0x1F}
        }
    }
    
    /// Lookup 8bpp texel colour.
    fn lookup_8bpp_tex(&self, tex_coords: TexCoords, tex_attrs: TextureAttrs, palette: u16, vram: &Engine3DVRAM) -> ColourAlpha {
        let (s, t) = Self::get_tex_coords(tex_coords, tex_attrs);
        let width = tex_attrs.width();
        let pos = (width * t) + s;
        let offset = pos;
        let addr = tex_attrs.addr();

        let data = vram.get_tex_byte(addr + offset);

        if (data == 0) && tex_attrs.contains(TextureAttrs::TRANSPARENT_0) {
            ColourAlpha::transparent()
        } else {
            let palette_addr = (palette as u32) << 3;
            let colour = self.palette_cache.get_tex_colour(palette_addr + (data as u32));
            ColourAlpha {col: colour, alpha: 0x1F}
        }
    }
    
    /// Lookup direct texel colour.
    fn lookup_dir_tex(&self, tex_coords: TexCoords, tex_attrs: TextureAttrs, vram: &Engine3DVRAM) -> ColourAlpha {
        let (s, t) = Self::get_tex_coords(tex_coords, tex_attrs);
        let width = tex_attrs.width();
        let pos = (width * t) + s;
        let offset = pos * 2;
        let addr = tex_attrs.addr();

        let data = vram.get_tex_halfword(addr + offset);
        ColourAlpha {col: Colour::from_555(data), alpha: ((data >> 15) as u8) * 0x1F}
    }

    fn lookup_a3i5_tex(&self, tex_coords: TexCoords, tex_attrs: TextureAttrs, palette: u16, vram: &Engine3DVRAM) -> ColourAlpha {
        let (s, t) = Self::get_tex_coords(tex_coords, tex_attrs);
        let width = tex_attrs.width();
        let pos = (width * t) + s;
        let offset = pos;
        let addr = tex_attrs.addr();

        let data = vram.get_tex_byte(addr + offset);
        let palette_data = data & 0x1F;
        let alpha_data = (data & 0xE0) >> 5;

        let palette_addr = (palette as u32) << 3;
        let colour = self.palette_cache.get_tex_colour(palette_addr + (palette_data as u32));
        let alpha = (alpha_data << 2) | (alpha_data >> 1);
        ColourAlpha {col: colour, alpha}
    }
    
    fn lookup_a5i3_tex(&self, tex_coords: TexCoords, tex_attrs: TextureAttrs, palette: u16, vram: &Engine3DVRAM) -> ColourAlpha {
        let (s, t) = Self::get_tex_coords(tex_coords, tex_attrs);
        let width = tex_attrs.width();
        let pos = (width * t) + s;
        let offset = pos;
        let addr = tex_attrs.addr();

        let data = vram.get_tex_byte(addr + offset);
        let palette_data = data & 0x7;
        let alpha_data = (data & 0xF8) >> 3;

        let palette_addr = (palette as u32) << 3;
        let colour = self.palette_cache.get_tex_colour(palette_addr + (palette_data as u32));
        ColourAlpha {col: colour, alpha: alpha_data}
    }
    
    fn lookup_4x4_tex(&self, tex_coords: TexCoords, tex_attrs: TextureAttrs, palette: u16, vram: &Engine3DVRAM) -> ColourAlpha {
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

        let base_palette_addr = (palette as u32) << 3;
        let block_palette_offset = ((block_palette_data & 0x3FFF) as u32) << 1;
        let palette_addr = base_palette_addr + block_palette_offset;
        
        let transparent_3 = !u16::test_bit(block_palette_data, 15);
        if u16::test_bit(block_palette_data, 14) {
            // Interpolation mode.
            match block_data {
                0 => {
                    let colour = self.palette_cache.get_tex_colour(palette_addr);
                    ColourAlpha {col: colour, alpha: 0x1F}
                },
                1 => {
                    let colour = self.palette_cache.get_tex_colour(palette_addr + 1);
                    ColourAlpha {col: colour, alpha: 0x1F}
                },
                2 if transparent_3 => {
                    let col_0 = self.palette_cache.get_tex_colour(palette_addr);
                    let col_1 = self.palette_cache.get_tex_colour(palette_addr + 1);
                    let r = (col_0.r as u16) + (col_1.r as u16);
                    let g = (col_0.g as u16) + (col_1.g as u16);
                    let b = (col_0.b as u16) + (col_1.b as u16);
                    ColourAlpha {
                        col: Colour {
                            r: (r >> 1) as u8,
                            g: (g >> 1) as u8,
                            b: (b >> 1) as u8,
                        }, alpha: 0x1F
                    }
                },
                2 => {
                    let col_0 = self.palette_cache.get_tex_colour(palette_addr);
                    let col_1 = self.palette_cache.get_tex_colour(palette_addr + 1);
                    let r = (col_0.r as u16) * 5 + (col_1.r as u16) * 3;
                    let g = (col_0.g as u16) * 5 + (col_1.g as u16) * 3;
                    let b = (col_0.b as u16) * 5 + (col_1.b as u16) * 3;
                    ColourAlpha {
                        col: Colour {
                            r: (r >> 3) as u8,
                            g: (g >> 3) as u8,
                            b: (b >> 3) as u8,
                        }, alpha: 0x1F
                    }
                },
                3 if transparent_3 => ColourAlpha::transparent(),
                3 => {
                    let col_0 = self.palette_cache.get_tex_colour(palette_addr);
                    let col_1 = self.palette_cache.get_tex_colour(palette_addr + 1);
                    let r = (col_0.r as u16) * 3 + (col_1.r as u16) * 5;
                    let g = (col_0.g as u16) * 3 + (col_1.g as u16) * 5;
                    let b = (col_0.b as u16) * 3 + (col_1.b as u16) * 5;
                    ColourAlpha {
                        col: Colour {
                            r: (r >> 3) as u8,
                            g: (g >> 3) as u8,
                            b: (b >> 3) as u8,
                        }, alpha: 0x1F
                    }
                },
                _ => unreachable!()
            }
        } else {
            let palette_offset = block_data as u32;
            if palette_offset == 3 && transparent_3 {
                ColourAlpha::transparent()
            } else {
                let colour = self.palette_cache.get_tex_colour(palette_addr + palette_offset);
                ColourAlpha {col: colour, alpha: 0x1F}
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
            let frag_alpha = (frag_colour.alpha + 1) as u16;
            let buffer_alpha = (0x1F - frag_colour.alpha) as u16;
            let r = ((frag_colour.col.r as u16) * frag_alpha) + ((buffer_colour.col.r as u16) * buffer_alpha);
            let g = ((frag_colour.col.g as u16) * frag_alpha) + ((buffer_colour.col.g as u16) * buffer_alpha);
            let b = ((frag_colour.col.b as u16) * frag_alpha) + ((buffer_colour.col.b as u16) * buffer_alpha);
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

    /// Returns true if the edge provided should be drawn.
    /// Provide an edge candidate with polygon ID and depth,
    /// plus all of the surrounding pixels.
    #[inline]
    fn check_edge(this: (u8, Depth), left: (u8, Depth), right: (u8, Depth), top: (u8, Depth), bottom: (u8, Depth)) -> bool {
        let compare_pix = |other: (u8, Depth)| {
            // Ensure polygon IDs differ, and depth value of edge is less than surrounding pixel.
            this.0 != other.0 && this.1 < other.1
        };
        compare_pix(left) || compare_pix(right) || compare_pix(top) || compare_pix(bottom)
    }
}
