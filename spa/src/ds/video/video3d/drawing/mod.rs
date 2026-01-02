mod palette;

use super::{
    render::RenderingEngine,
    types::*
};
use fixed::types::{I12F4, I16F0, I23F9, I24F8};
use fixed::traits::ToFixed;
use crate::{
    ds::video::memory::Engine3DVRAM,
    common::video::colour::*,
    utils::bits::u16, utils::bytes
};

use palette::TexPaletteCache;

#[derive(Copy, Clone, Default)]
struct Attributes {
    opaque_id:  u8,
    // It appears we store a "none" value here at init
    trans_id:   Option<u8>,
    fog:        bool,
    edge:       bool,
}

/*#[derive(Clone)]
struct VertexStep {
    x:          I12F4,
    depth:      Depth,
    colour_r:   I12F4,
    colour_g:   I12F4,
    colour_b:   I12F4,
    tex_s:      I24F8,
    tex_t:      I24F8,
}

impl VertexStep {
    fn add(&mut self, other: &Self) {
        self.x += other.x;
        self.depth += other.depth;
        self.colour_r += other.colour_r;
        self.colour_g += other.colour_g;
        self.colour_b += other.colour_b;
        self.tex_s += other.tex_s;
        self.tex_t += other.tex_t;
    }
}

struct InterpolatedLine {
    current_values: VertexStep,
    step: VertexStep
}*/

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
                        trans_id:   None,
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
                trans_id:   None,
                fog:        render_engine.fog_enabled,
                edge:       false,
            };
            self.attr_buffer.fill(clear_attrs);
            self.depth_buffer.fill(render_engine.clear_depth);
            target.fill(ColourAlpha { col: render_engine.clear_colour, alpha: render_engine.clear_alpha });
        }
    }
    
    fn draw_opaque_polygons(&mut self, render_engine: &RenderingEngine, vram: &Engine3DVRAM, target: &mut [ColourAlpha]) {
        //use std::hash::{Hash, Hasher};
        for p in render_engine.polygon_ram.opaque_polygons.iter() {
            /*let mut hash_state = std::hash::DefaultHasher::new();
            (n as u32).hash(&mut hash_state);
            let hash = hash_state.finish();
            let r = (hash & 0xFF) as u8;
            let g = ((hash >> 8) & 0xFF) as u8;
            let b = ((hash >> 16) & 0xFF) as u8;
            let poly_colour = ColourAlpha { col: Colour { r, g, b }, alpha: 0x1F };*/

            let polygon = &render_engine.polygon_ram.polygons[p.polygon_index];
            let mode = polygon.attrs.mode();

            /*for vtx_idx in polygon.vertex_indices.iter().take(polygon.num_vertices as usize) {
                let vertex = &render_engine.polygon_ram.vertices[*vtx_idx as usize];
                println!("{} VTX: {:X}, {:X} TEX: {:X}, {:X}", n, vertex.screen_p.x, vertex.screen_p.y, vertex.tex_coords.s, vertex.tex_coords.t);
            }*/
            
            let (mut x_min_prev, mut x_max_prev) = (256, 0);
            let (y_min, y_max) = (p.y_min.to_num::<u8>(), p.y_max.to_num::<u8>());

            for y_idx in y_min..y_max {
                let Some([left, right]) = Self::find_intersect_points(render_engine, polygon, y_idx.to_fixed()) else {
                    continue;
                };

                //let half = I16F0::ONE / 2;
                //let y = I16F0::from_num(y_idx);// + half;
                let y_idx_base = (y_idx as usize) * 256;

                //println!("Draw line {:X}", y);
                if left.screen_p.x == right.screen_p.x {
                    // Should this ever happen?
                    continue;
                }

                /*let width = right.screen_p.x - left.screen_p.x;
                let depth_step = (right.depth - left.depth).to_fixed::<I46F18>() / width.to_fixed::<I46F18>();
                let r_step = (right.colour.r.to_fixed::<I24F8>() - left.colour.r.to_fixed::<I24F8>()) / width.to_fixed::<I24F8>();
                let g_step = (right.colour.g.to_fixed::<I24F8>() - left.colour.g.to_fixed::<I24F8>()) / width.to_fixed::<I24F8>();
                let b_step = (right.colour.b.to_fixed::<I24F8>() - left.colour.b.to_fixed::<I24F8>()) / width.to_fixed::<I24F8>();
                let tex_s_step = (right.tex_coords.s.to_fixed::<I24F8>() - left.tex_coords.s.to_fixed::<I24F8>()) / width.to_fixed::<I24F8>();
                let tex_t_step = (right.tex_coords.t.to_fixed::<I24F8>() - left.tex_coords.t.to_fixed::<I24F8>()) / width.to_fixed::<I24F8>();

                let step = VertexStep {
                    x: I12F4::ONE,
                    depth: depth_step.to_fixed(),
                    colour_r: r_step.to_fixed(),
                    colour_g: g_step.to_fixed(),
                    colour_b: b_step.to_fixed(),
                    tex_s: tex_s_step,
                    tex_t: tex_t_step,
                };
                let mut line = InterpolatedLine {
                    current_values: VertexStep {
                        x: I12F4::ONE,
                        depth: left.depth,
                        colour_r: left.colour.r.to_fixed(),
                        colour_g: left.colour.g.to_fixed(),
                        colour_b: left.colour.b.to_fixed(),
                        tex_s: left.tex_coords.s.to_fixed(),
                        tex_t: left.tex_coords.t.to_fixed()
                    },
                    step: step
                };*/

                let x_min = left.screen_p.x.round().to_num::<i16>();
                let x_max = right.screen_p.x.round().to_num::<i16>();

                if x_min == 0 && x_max == 256 {
                    //println!("Line {:X} | x: {:X} to {:X} | tex ({:X}, {:X}) to ({:X}, {:X})", y_idx, x_min, x_max, left.tex_coords.s, left.tex_coords.t, right.tex_coords.s, right.tex_coords.t/* , line.step.tex_s, line.step.tex_t*/);
                }

                for x_idx in x_min..x_max {

                    let factor = if !Self::similar_depth(left.depth, right.depth) {
                        let factor_over = (x_idx.to_fixed::<I16F0>() - left.screen_p.x).to_fixed::<I23F9>() * left.depth;
                        let factor_under = (right.screen_p.x - x_idx.to_fixed::<I16F0>()).to_fixed::<I23F9>() * right.depth + factor_over;
                        factor_over / factor_under
                    } else {
                        let factor_over = (x_idx.to_fixed::<I16F0>() - left.screen_p.x).to_fixed::<I23F9>();
                        let factor_under = (right.screen_p.x - x_idx.to_fixed::<I16F0>()).to_fixed::<I23F9>() + factor_over;
                        factor_over / factor_under
                    };

                    let depth_offset = (right.depth - left.depth) * factor;
                    let r_offset = (right.colour.r.to_fixed::<Depth>() - left.colour.r.to_fixed::<Depth>()) * factor;
                    let g_offset = (right.colour.g.to_fixed::<Depth>() - left.colour.g.to_fixed::<Depth>()) * factor;
                    let b_offset = (right.colour.b.to_fixed::<Depth>() - left.colour.b.to_fixed::<Depth>()) * factor;
                    let tex_s_offset = (right.tex_coords.s.to_fixed::<Depth>() - left.tex_coords.s.to_fixed::<Depth>()) * factor;
                    let tex_t_offset = (right.tex_coords.t.to_fixed::<Depth>() - left.tex_coords.t.to_fixed::<Depth>()) * factor;

                    let depth = left.depth + depth_offset;
                    let colour = Colour {
                        r: (left.colour.r.to_fixed::<Depth>() + r_offset).to_num(),
                        g: (left.colour.g.to_fixed::<Depth>() + g_offset).to_num(),
                        b: (left.colour.b.to_fixed::<Depth>() + b_offset).to_num(),
                    };
                    let tex_coords = TexCoords {
                        s: (left.tex_coords.s.to_fixed::<Depth>() + tex_s_offset).to_fixed(),
                        t: (left.tex_coords.t.to_fixed::<Depth>() + tex_t_offset).to_fixed()
                    };

                    /*let current = line.current_values.clone();
                    line.current_values.add(&line.step);
                    let depth = current.depth;
                    let colour = Colour { r: current.colour_r.to_num(), g: current.colour_g.to_num(), b: current.colour_b.to_num() };
                    let tex_coords = TexCoords { s: current.tex_s.to_fixed(), t: current.tex_t.to_fixed() };*/

                    let idx = y_idx_base + (x_idx as usize);
                    if !Self::test_depth(polygon.render_eq_depth(), self.depth_buffer[idx], depth) {
                        continue;
                    }

                    let top_edge = x_idx < x_min_prev || x_idx > x_max_prev;
                    let bottom_edge = y_idx == (y_max - 1); // TODO: compare with next line.
                    let edge = top_edge || bottom_edge || (x_idx == x_min) || (x_idx == (x_max - 1));
                    if !edge && polygon.is_wireframe() {
                        continue;
                    }

                    let vtx_colour = ColourAlpha {
                        col: colour,
                        alpha: 0x1F
                    };

                    let tex_colour = self.lookup_tex_colour(tex_coords, polygon.tex, polygon.palette, vram);
                    //let tex_colour = Some(poly_colour);

                    let frag_colour = if let Some(tex_colour) = tex_colour {
                        Self::blend_frag_tex_colour(render_engine, mode, vtx_colour, tex_colour)
                    } else {
                        Self::blend_fragment_colour(render_engine, mode, vtx_colour)
                    };
                    //let frag_colour = poly_colour;
                    if frag_colour.alpha > 0 {
                        target[idx] = frag_colour;
                        self.depth_buffer[idx] = depth;
                        self.attr_buffer[idx].opaque_id = polygon.attrs.id();
                        self.attr_buffer[idx].fog = polygon.attrs.contains(PolygonAttrs::FOG_BLEND_ENABLE);
                        self.attr_buffer[idx].edge = edge;
                    }
                }

                x_min_prev = x_min;
                x_max_prev = x_max;
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
        let id = polygon.attrs.id();
        
        /*println!("poly");
        for vtx_idx in polygon.vertex_indices.iter().take(polygon.num_vertices as usize) {
            let vertex = &render_engine.polygon_ram.vertices[*vtx_idx as usize];
            println!("  VTX: {:X}, {:X} TEX: {:X}, {:X}", vertex.screen_p.x, vertex.screen_p.y, vertex.tex_coords.s, vertex.tex_coords.t);
        }*/

        //let (mut x_min_prev, mut x_max_prev) = (256, 0);
        let (y_min, y_max) = (p.y_min.to_num::<u8>(), p.y_max.to_num::<u8>());

        for y_idx in y_min..y_max {

            let Some([left, right]) = Self::find_intersect_points(render_engine, polygon, y_idx.to_fixed()) else {
                continue;
            };

            //let half = I16F0::ONE / 2;
            //let y = I16F0::from_num(y_idx);// + half;
            let y_idx_base = (y_idx as usize) * 256;

            //println!("Draw line {:X}", y);

            if left.screen_p.x == right.screen_p.x {
                // Should this ever happen?
                continue;
            }

            /*let width = right.screen_p.x - left.screen_p.x;
            let depth_step = (right.depth - left.depth).to_fixed::<I46F18>() / width.to_fixed::<I46F18>();
            let r_step = (right.colour.r.to_fixed::<I24F8>() - left.colour.r.to_fixed::<I24F8>()) / width.to_fixed::<I24F8>();
            let g_step = (right.colour.g.to_fixed::<I24F8>() - left.colour.g.to_fixed::<I24F8>()) / width.to_fixed::<I24F8>();
            let b_step = (right.colour.b.to_fixed::<I24F8>() - left.colour.b.to_fixed::<I24F8>()) / width.to_fixed::<I24F8>();
            let tex_s_step = (right.tex_coords.s.to_fixed::<I24F8>() - left.tex_coords.s.to_fixed::<I24F8>()) / width.to_fixed::<I24F8>();
            let tex_t_step = (right.tex_coords.t.to_fixed::<I24F8>() - left.tex_coords.t.to_fixed::<I24F8>()) / width.to_fixed::<I24F8>();

            let step = VertexStep {
                x: I12F4::ONE,
                depth: depth_step.to_fixed(),
                colour_r: r_step.to_fixed(),
                colour_g: g_step.to_fixed(),
                colour_b: b_step.to_fixed(),
                tex_s: tex_s_step,
                tex_t: tex_t_step,
            };
            let mut line = InterpolatedLine {
                current_values: VertexStep {
                    x: I12F4::ONE,
                    depth: left.depth,
                    colour_r: left.colour.r.to_fixed(),
                    colour_g: left.colour.g.to_fixed(),
                    colour_b: left.colour.b.to_fixed(),
                    tex_s: left.tex_coords.s.to_fixed(),
                    tex_t: left.tex_coords.t.to_fixed()
                },
                step: step
            };*/

            let x_min = left.screen_p.x.round().to_num::<i16>();
            let x_max = right.screen_p.x.round().to_num::<i16>();

            //println!("Line {:X} | x: {:X} to {:X} | tex ({:X}, {:X}) to ({:X}, {:X})", y_idx, x_min, x_max, left.tex_coords.s, left.tex_coords.t, right.tex_coords.s, right.tex_coords.t);

            for x_idx in x_min..x_max {
                let idx = y_idx_base + (x_idx as usize);
                // TODO: only extract for shadow polygons?
                let stencil_mask = if mode == PolygonMode::Shadow && id != 0 {
                    std::mem::replace(&mut self.stencil_buffer[idx], false)
                } else {
                    false
                };

                let factor = if !Self::similar_depth(left.depth, right.depth) {
                    let factor_over = (x_idx.to_fixed::<I16F0>() - left.screen_p.x).to_fixed::<I23F9>() * left.depth;
                    let factor_under = (right.screen_p.x - x_idx.to_fixed::<I16F0>()).to_fixed::<I23F9>() * right.depth + factor_over;
                    factor_over / factor_under
                } else {
                    let factor_over = (x_idx.to_fixed::<I16F0>() - left.screen_p.x).to_fixed::<I23F9>();
                    let factor_under = (right.screen_p.x - x_idx.to_fixed::<I16F0>()).to_fixed::<I23F9>() + factor_over;
                    factor_over / factor_under
                };

                let depth_offset = (right.depth - left.depth) * factor;
                let r_offset = (right.colour.r.to_fixed::<Depth>() - left.colour.r.to_fixed::<Depth>()) * factor;
                let g_offset = (right.colour.g.to_fixed::<Depth>() - left.colour.g.to_fixed::<Depth>()) * factor;
                let b_offset = (right.colour.b.to_fixed::<Depth>() - left.colour.b.to_fixed::<Depth>()) * factor;
                let tex_s_offset = (right.tex_coords.s.to_fixed::<Depth>() - left.tex_coords.s.to_fixed::<Depth>()) * factor;
                let tex_t_offset = (right.tex_coords.t.to_fixed::<Depth>() - left.tex_coords.t.to_fixed::<Depth>()) * factor;

                let depth = left.depth + depth_offset;
                let colour = Colour {
                    r: (left.colour.r.to_fixed::<Depth>() + r_offset).to_num(),
                    g: (left.colour.g.to_fixed::<Depth>() + g_offset).to_num(),
                    b: (left.colour.b.to_fixed::<Depth>() + b_offset).to_num(),
                };
                let tex_coords = TexCoords {
                    s: (left.tex_coords.s.to_fixed::<Depth>() + tex_s_offset).to_fixed(),
                    t: (left.tex_coords.t.to_fixed::<Depth>() + tex_t_offset).to_fixed()
                };

                /*let current = line.current_values.clone();
                line.current_values.add(&line.step);
                let depth = current.depth;
                let colour = Colour { r: current.colour_r.to_num(), g: current.colour_g.to_num(), b: current.colour_b.to_num() };
                let tex_coords = TexCoords { s: current.tex_s.to_fixed(), t: current.tex_t.to_fixed() };*/

                if !Self::test_depth(polygon.render_eq_depth(), self.depth_buffer[idx], depth) {
                    if id == 0 && mode == PolygonMode::Shadow {
                        // Shadow polygon mask
                        self.stencil_buffer[idx] = true;
                    }
                    continue;
                }

                if mode == PolygonMode::Shadow {
                    if id == 0 {
                        // Ignore masks
                        continue;
                    }
                    if !stencil_mask || self.attr_buffer[idx].opaque_id == id {
                        // We only want to draw the shadow if it passes depth,
                        // is masked, and doesn't match the IDs
                        continue;
                    }
                }

                let vtx_colour = ColourAlpha {
                    col: colour,
                    alpha: polygon.attrs.alpha()
                };

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
                } else if frag_colour.alpha != 0x1F {
                    if let Some(existing_id) = self.attr_buffer[idx].trans_id {
                        if existing_id == id {
                            continue;
                        }
                    }
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
                    self.attr_buffer[idx].trans_id = Some(id);
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
    fn find_intersect_points(render_engine: &RenderingEngine, polygon: &Polygon, y: I16F0) -> Option<[Vertex; 2]> {
        // Find start and end points.
        let mut points = [None, None];
        let mut line_a_points = None;
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

            // Bottom = higher Y value
            let (top, bottom) = if vtx_a.screen_p.y > vtx_b.screen_p.y {
                (&vtx_b, &vtx_a)
            } else {
                (&vtx_a, &vtx_b)
            };

            if let Some((top_a, bottom_a)) = line_a_points {
                if top.screen_p.y == bottom_a || bottom.screen_p.y == top_a {
                    continue;
                }
            } else {
                line_a_points = Some((top.screen_p.y, bottom.screen_p.y));
            }

            // TODO: special case for single-pixel point...

            let x_factor = {
                let factor_over = (y - top.screen_p.y).to_fixed::<I23F9>();
                let factor_under = (bottom.screen_p.y - y).to_fixed::<I23F9>() + factor_over;
                factor_over / factor_under
            };
            let factor = if !Self::similar_depth(top.depth, bottom.depth) {
                let factor_over = (y - top.screen_p.y).to_fixed::<I23F9>() * top.depth;
                let factor_under = (bottom.screen_p.y - y).to_fixed::<I23F9>() * bottom.depth + factor_over;
                factor_over / factor_under
            } else {
                x_factor
            };

            // TODO: calculate this differently
            let x_offset = (bottom.screen_p.x - top.screen_p.x).to_fixed::<Depth>() * x_factor;
            let depth_offset = (bottom.depth - top.depth) * factor;
            let r_offset = (bottom.colour.r.to_fixed::<Depth>() - top.colour.r.to_fixed::<Depth>()) * factor;
            let g_offset = (bottom.colour.g.to_fixed::<Depth>() - top.colour.g.to_fixed::<Depth>()) * factor;
            let b_offset = (bottom.colour.b.to_fixed::<Depth>() - top.colour.b.to_fixed::<Depth>()) * factor;
            let tex_s_offset = (bottom.tex_coords.s.to_fixed::<Depth>() - top.tex_coords.s.to_fixed::<Depth>()) * factor;
            let tex_t_offset = (bottom.tex_coords.t.to_fixed::<Depth>() - top.tex_coords.t.to_fixed::<Depth>()) * factor;

            let vertex = Vertex {
                screen_p: Coords {
                    x: (top.screen_p.x.to_fixed::<Depth>() + x_offset).round().to_fixed(),
                    y
                },
                depth: top.depth + depth_offset,
                colour: Colour {
                    r: (top.colour.r.to_fixed::<Depth>() + r_offset).to_num(),
                    g: (top.colour.g.to_fixed::<Depth>() + g_offset).to_num(),
                    b: (top.colour.b.to_fixed::<Depth>() + b_offset).to_num(),
                },
                tex_coords: TexCoords {
                    s: (top.tex_coords.s.to_fixed::<Depth>() + tex_s_offset).to_fixed(),
                    t: (top.tex_coords.t.to_fixed::<Depth>() + tex_t_offset).to_fixed()
                },
            };

            if points[0].is_none() {
                // First line.
                points[0] = Some(vertex);
            } else if points[1].is_none() {
                // Second line - we are done.
                points[1] = Some(vertex);
                break;
            }
        }

        if let [Some(vtx_a), Some(vtx_b)] = points {
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
            (buffer_depth - Depth::ONE) <= frag_depth && (buffer_depth + Depth::ONE) >= frag_depth
        } else {
            buffer_depth > frag_depth
        }
    }

    /// Returns true if both depth values provided are nearly the same.
    /// Masks off lower-order bits.
    /// Also ensures neither depth value is 0.
    fn similar_depth(a: Depth, b: Depth) -> bool {
        let a = (a.to_bits() as u32) & 0xFFFF_FFC0;
        let b = (b.to_bits() as u32) & 0xFFFF_FFC0;
        a == b || a == 0 || b == 0
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
                let a = ((vtx_colour.alpha as u16) + 1) * ((tex_colour.alpha as u16) + 1) - 1;

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
                    ColourAlpha {
                        col: tex_colour.col,
                        alpha: vtx_colour.alpha
                    }
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
