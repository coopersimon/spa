mod math;
mod matrix;
mod lighting;
mod clip;

pub use math::*;
use matrix::*;
use lighting::*;
use clip::*;

use fixed::{types::{I4F12, I20F12, I23F9, I13F3, I12F4, I40F24}, traits::ToFixed};
use crate::utils::{
    bits, bits::u32, bytes
};
use super::types::*;

#[derive(Clone, Copy)]
enum Primitive {
    Triangle(usize),
    /// The first polygon of a triangle strip.
    TriangleStripFirst(usize),
    /// Subsequent polygons of a triangle strip.
    TriangleStrip,
    Quad(usize),
    /// The first polygon of a quad strip.
    QuadStripFirst(usize),
    /// The vertex after emitting a subsequent quad strip polygon.
    QuadStrip(usize),
}

const TRI_ORDER: [usize; 3] = [0, 1, 2];
const TRI_STRIP_ORDER_A: [usize; 3] = [0, 1, 2];
const TRI_STRIP_ORDER_B: [usize; 3] = [1, 0, 2];

const QUAD_ORDER: [usize; 4] = [0, 1, 2, 3];
const QUAD_STRIP_ORDER: [usize; 4] = [0, 1, 3, 2];
//const QUAD_STRIP_ORDER_B: [usize; 4] = [2, 3, 1, 0];

pub struct GeometryEngine {
    pub clipping_unit:  ClippingUnit,
    /// Use W or Z value for depth-buffering.
    w_buffer:       bool,
    /// Test w against this value for 1-dot polygons.
    dot_polygon_w:  I13F3,

    pub matrices:   Box<MatrixUnit>,
    lighting:       Box<LightingUnit>,

    /// Result of the box test:
    /// FALSE if ALL of the box faces are OUTSIDE view frustrum.
    pub box_test_res:   bool,
    /// Result of the position test.
    pub pos_test_res:   [u32; 4],
    /// Result of the direction (vector) test.
    pub dir_test_res:   [u16; 3],

    /// Current polygon attributes.
    polygon_attrs:  PolygonAttrs,
    /// Current texture attributes.
    texture_attrs:  TextureAttrs,
    /// Current texture palette address.
    tex_palette:    u16,
    /// Current texture coords.
    tex_coords:         TexCoords,
    /// Current transformed texture coords.
    trans_tex_coords:   TexCoords,
    /// Currently inputting vertices.
    current_vertex: [I4F12; 3],

    /// Current polygon vertices for outputting to Vertex RAM.
    /// Will only be written if it passes the W-test.
    staged_polygon:     Vec<StagedVertex>,
    staged_index:       usize,
    stage_size:         usize,
    output_order:       &'static [usize],
    primitive:          Option<Primitive>,
}

impl GeometryEngine {
    pub fn new() -> Self {
        Self {
            clipping_unit:  ClippingUnit::new(),
            w_buffer:       false,
            dot_polygon_w:  I13F3::from_bits(0x7FFF),

            matrices:       Box::new(MatrixUnit::new()),
            lighting:       Box::new(LightingUnit::new()),

            box_test_res:   false,
            pos_test_res:   [0; 4],
            dir_test_res:   [0; 3],

            polygon_attrs:  PolygonAttrs::default(),
            texture_attrs:  TextureAttrs::default(),
            tex_palette:    0,
            tex_coords:         TexCoords::default(),
            trans_tex_coords:   TexCoords::default(),
            current_vertex:     [I4F12::ZERO; 3],

            staged_polygon:     vec![StagedVertex::default(); 4],
            staged_index:       0,
            stage_size:         3,
            output_order:       &TRI_ORDER,
            primitive:          None,
        }
    }

    pub fn set_dot_polygon_depth(&mut self, data: u16) {
        let bits = (data & 0x7FFF) as i16;
        self.dot_polygon_w = I13F3::from_bits(bits);
    }
}

// GPU commands
impl GeometryEngine {
    pub fn set_viewport(&mut self, data: u32) -> isize {
        self.clipping_unit.set_viewport(data);
        1
    }

    /// Set values for next frame.
    /// Actual swapping of polygon/vertex buffers happens outside.
    pub fn swap_buffers(&mut self, data: u32) {
        self.w_buffer = u32::test_bit(data, 0);
    }

    pub fn set_vertex_colour(&mut self, data: u32) -> isize {
        self.lighting.set_vertex_colour(data);
        1
    }

    pub fn set_normal(&mut self, data: u32) -> isize {
        let x_bits = (data & 0x3FF) as u16;
        let y_bits = ((data >> 10) & 0x3FF) as u16;
        let z_bits = ((data >> 20) & 0x3FF) as u16;
        let v = Vector::new([
            N::from_bits(bits::u16::sign_extend(x_bits << 3, 13).into()),
            N::from_bits(bits::u16::sign_extend(y_bits << 3, 13).into()),
            N::from_bits(bits::u16::sign_extend(z_bits << 3, 13).into()),
        ]);
        let normal = self.matrices.dir_matrix().mul_vector_3(&v);
        let tex_cycles = if self.texture_attrs.transform_mode() == 2 {
            let s = self.tex_coords.s.to_fixed::<N>();
            let t = self.tex_coords.t.to_fixed::<N>();
            let m = &self.matrices.tex_matrix();
            let s0 = v.x() * m.elements[0] + v.y() * m.elements[4] + v.z() * m.elements[8] + s;
            let t0 = v.x() * m.elements[1] + v.y() * m.elements[5] + v.z() * m.elements[9] + t;
            self.trans_tex_coords.s = s0.to_fixed();
            self.trans_tex_coords.t = t0.to_fixed();
            2
        } else {
            0
        };
        // Calculate colour.
        self.lighting.set_normal(normal) + tex_cycles
    }

    pub fn set_dif_amb_colour(&mut self, data: u32) -> isize {
        self.lighting.set_dif_amb_colour(data);
        4
    }
    
    pub fn set_spe_emi_colour(&mut self, data: u32) -> isize {
        self.lighting.set_spe_emi_colour(data);
        4
    }
    
    pub fn set_specular_table(&mut self, data: impl Iterator<Item = u32>) -> isize {
        for d in data {
            self.lighting.set_specular_table(d);
        }
        32
    }
    
    pub fn set_light_direction(&mut self, data: u32) -> isize {
        let x_bits = (data & 0x3FF) as u16;
        let y_bits = ((data >> 10) & 0x3FF) as u16;
        let z_bits = ((data >> 20) & 0x3FF) as u16;
        let v = Vector::new([
            N::from_bits(bits::u16::sign_extend(x_bits << 3, 13).into()),
            N::from_bits(bits::u16::sign_extend(y_bits << 3, 13).into()),
            N::from_bits(bits::u16::sign_extend(z_bits << 3, 13).into()),
        ]);
        let direction = self.matrices.dir_matrix().mul_vector_3(&v);
        let light = (data >> 30) as usize;
        self.lighting.set_light_direction(light, direction);
        6
    }

    pub fn set_light_colour(&mut self, data: u32) -> isize {
        self.lighting.set_light_colour(data);
        1
    }

    pub fn set_tex_attrs(&mut self, data: u32) -> isize {
        self.texture_attrs = TextureAttrs::from_bits_truncate(data);
        1
    }
    
    pub fn set_tex_palette(&mut self, data: u32) -> isize {
        self.tex_palette = (data & 0x1FFF) as u16;
        1
    }
    
    pub fn set_tex_coords(&mut self, data: u32) -> isize {
        self.tex_coords.s = I12F4::from_bits(bytes::u32::lo(data) as i16);
        self.tex_coords.t = I12F4::from_bits(bytes::u32::hi(data) as i16);
        match self.texture_attrs.transform_mode() {
            0 => {
                self.trans_tex_coords = self.tex_coords.clone();
                1
            },
            1 => {
                let s = self.tex_coords.s.to_fixed::<N>();
                let t = self.tex_coords.t.to_fixed::<N>();
                let m = &self.matrices.tex_matrix();
                let s0 = s * m.elements[0] + t * m.elements[4] + (m.elements[8] / 16) + (m.elements[12] / 16);
                let t0 = s * m.elements[1] + t * m.elements[5] + (m.elements[9] / 16) + (m.elements[13] / 16);
                self.trans_tex_coords.s = s0.to_fixed();
                self.trans_tex_coords.t = t0.to_fixed();
                2
            },
            _ => 1,    // Transformed later.
        }
    }

    /// Called before vertex data is input.
    /// 
    /// Also decides which primitive type to use.
    pub fn begin_vertex_list(&mut self, data: u32) -> isize {
        let primitive = match data & 0b11 {
            0b00 => {
                self.stage_size = 3;
                self.output_order = &TRI_ORDER;
                Primitive::Triangle(0)
            },
            0b01 => {
                self.stage_size = 4;
                self.output_order = &QUAD_ORDER;
                Primitive::Quad(0)
            },
            0b10 => {
                self.stage_size = 3;
                self.output_order = &TRI_STRIP_ORDER_A;
                Primitive::TriangleStripFirst(0)
            },
            0b11 => {
                self.stage_size = 4;
                self.output_order = &QUAD_STRIP_ORDER;
                Primitive::QuadStripFirst(0)
            },
            _ => unreachable!()
        };
        self.primitive = Some(primitive);
        1
    }

    pub fn end_vertex_list(&mut self) -> isize {
        self.primitive = None;
        1
    }

    /// Set vertex coordinates. Uses 2 parameter words. I4F12 format.
    /// 
    /// First param: X in lower half, Y in upper half.
    /// 
    /// Second param: Z in lower half.
    pub fn set_vertex_coords_16(&mut self, lo: u32, hi: u32) -> isize {
        self.current_vertex[0] = I4F12::from_bits(bytes::u32::lo(lo) as i16);
        self.current_vertex[1] = I4F12::from_bits(bytes::u32::hi(lo) as i16);
        self.current_vertex[2] = I4F12::from_bits(bytes::u32::lo(hi) as i16);
        self.process_vertex() + 1
    }
    
    /// Set vertex coordinates. I4F6 format.
    /// 
    /// Param: X, Y, Z, each 10 bits.
    pub fn set_vertex_coords_10(&mut self, data: u32) -> isize {
        let x = (data & 0x3FF) << 6;
        let y = ((data >> 10) & 0x3FF) << 6;
        let z = ((data >> 20) & 0x3FF) << 6;
        self.current_vertex[0] = I4F12::from_bits(x as i16);
        self.current_vertex[1] = I4F12::from_bits(y as i16);
        self.current_vertex[2] = I4F12::from_bits(z as i16);
        self.process_vertex()
    }
    
    /// Set vertex coordinates X and Y. I4F12 format. Keep old Z.
    /// 
    /// Param: X in lower half, Y in upper half.
    pub fn set_vertex_coords_xy(&mut self, data: u32) -> isize {
        self.current_vertex[0] = I4F12::from_bits(bytes::u32::lo(data) as i16);
        self.current_vertex[1] = I4F12::from_bits(bytes::u32::hi(data) as i16);
        self.process_vertex()
    }
    
    /// Set vertex coordinates X and Z. I4F12 format. Keep old Y.
    /// 
    /// Param: X in lower half, Z in upper half.
    pub fn set_vertex_coords_xz(&mut self, data: u32) -> isize {
        self.current_vertex[0] = I4F12::from_bits(bytes::u32::lo(data) as i16);
        self.current_vertex[2] = I4F12::from_bits(bytes::u32::hi(data) as i16);
        self.process_vertex()
    }
    
    /// Set vertex coordinates Y and Z. I4F12 format. Keep old X.
    /// 
    /// Param: Y in lower half, Z in upper half.
    pub fn set_vertex_coords_yz(&mut self, data: u32) -> isize {
        self.current_vertex[1] = I4F12::from_bits(bytes::u32::lo(data) as i16);
        self.current_vertex[2] = I4F12::from_bits(bytes::u32::hi(data) as i16);
        self.process_vertex()
    }
    
    /// Set vertex coordinates as a diff of current. F10 format.
    /// 
    /// Param: X, Y, Z, each 10 bits.
    pub fn diff_vertex_coords(&mut self, data: u32) -> isize {
        let x_diff = data & 0x3FF;
        let y_diff = (data >> 10) & 0x3FF;
        let z_diff = (data >> 20) & 0x3FF;
        self.current_vertex[0] += I4F12::from_bits(bits::u16::sign_extend(x_diff as u16, 10));
        self.current_vertex[1] += I4F12::from_bits(bits::u16::sign_extend(y_diff as u16, 10));
        self.current_vertex[2] += I4F12::from_bits(bits::u16::sign_extend(z_diff as u16, 10));
        self.process_vertex()
    }

    pub fn set_polygon_attrs(&mut self, data: u32) -> isize {
        self.polygon_attrs = PolygonAttrs::from_bits_truncate(data);
        self.lighting.set_enabled(self.polygon_attrs);
        1
    }

    pub fn box_test(&mut self, args: &[u32]) -> isize {
        use crate::utils::bits::u8;

        let base_x = I4F12::from_bits(bytes::u32::lo(args[0]) as i16);
        let base_y = I4F12::from_bits(bytes::u32::hi(args[0]) as i16);
        let base_z = I4F12::from_bits(bytes::u32::lo(args[1]) as i16);
        
        let len_x = I4F12::from_bits(bytes::u32::hi(args[1]) as i16);
        let len_y = I4F12::from_bits(bytes::u32::lo(args[2]) as i16);
        let len_z = I4F12::from_bits(bytes::u32::hi(args[2]) as i16);

        // We fail the test if ALL of the corners of the box are OUTSIDE the view.
        let mut fail_test = true;

        // Test to see if each vertex is outside view space.
        for vertex in 0..8_u8 {
            let x = if u8::test_bit(vertex, 0) { base_x + len_x } else { base_x };
            let y = if u8::test_bit(vertex, 1) { base_y + len_y } else { base_y };
            let z = if u8::test_bit(vertex, 2) { base_z + len_z } else { base_z };

            let vertex = Vector::new([
                x.into(),
                y.into(),
                z.into(),
                N::ONE
            ]);
    
            let transformed_vertex = self.matrices.clip_matrix().mul_vector_4(&vertex);
            let w = transformed_vertex.w();
            let w2 = w * 2;
            let normal_x = (transformed_vertex.x() + w) / w2;
            let normal_y = (transformed_vertex.y() + w) / w2;
            let normal_z = (transformed_vertex.z() + w) / w2;

            let outside_x = (normal_x > N::ONE) || (normal_x < N::ZERO);
            let outside_y = (normal_y > N::ONE) || (normal_y < N::ZERO);
            let outside_z = (normal_z > N::ONE) || (normal_z < N::ZERO);

            fail_test = fail_test && (outside_x && outside_y && outside_z);
        }

        self.box_test_res = !fail_test;

        103
    }

    pub fn position_test(&mut self, args: &[u32]) -> isize {
        self.current_vertex[0] = I4F12::from_bits(bytes::u32::lo(args[0]) as i16);
        self.current_vertex[1] = I4F12::from_bits(bytes::u32::hi(args[0]) as i16);
        self.current_vertex[2] = I4F12::from_bits(bytes::u32::lo(args[1]) as i16);
        let vertex = Vector::new([
            self.current_vertex[0].into(),
            self.current_vertex[1].into(),
            self.current_vertex[2].into(),
            N::ONE
        ]);

        let transformed_vertex = self.matrices.clip_matrix().mul_vector_4(&vertex);
        self.pos_test_res[0] = transformed_vertex.x().to_bits() as u32;
        self.pos_test_res[1] = transformed_vertex.y().to_bits() as u32;
        self.pos_test_res[2] = transformed_vertex.z().to_bits() as u32;
        self.pos_test_res[3] = transformed_vertex.w().to_bits() as u32;

        9
    }

    pub fn direction_test(&mut self, data: u32) -> isize {
        let x_bits = (data & 0x3FF) as u16;
        let y_bits = ((data >> 10) & 0x3FF) as u16;
        let z_bits = ((data >> 20) & 0x3FF) as u16;
        let v = Vector::new([
            N::from_bits(bits::u16::sign_extend(x_bits << 3, 13).into()),
            N::from_bits(bits::u16::sign_extend(y_bits << 3, 13).into()),
            N::from_bits(bits::u16::sign_extend(z_bits << 3, 13).into()),
        ]);

        let direction = self.matrices.dir_matrix().mul_vector_3(&v);
        self.dir_test_res[0] = direction.x().to_bits() as u16;
        self.dir_test_res[1] = direction.y().to_bits() as u16;
        self.dir_test_res[2] = direction.z().to_bits() as u16;

        5
    }
}

// Internal processing
impl GeometryEngine {
    fn process_vertex(&mut self) -> isize {
        let vertex = Vector::new([
            self.current_vertex[0].to_fixed::<N>(),
            self.current_vertex[1].to_fixed::<N>(),
            self.current_vertex[2].to_fixed::<N>(),
            N::ONE
        ]);

        if self.texture_attrs.transform_mode() == 3 {
            let s = self.tex_coords.s.to_fixed::<N>();
            let t = self.tex_coords.t.to_fixed::<N>();
            let m = &self.matrices.tex_matrix();
            let s0 = vertex.x() * m.elements[0] + vertex.y() * m.elements[4] + vertex.z() * m.elements[8] + s;
            let t0 = vertex.x() * m.elements[1] + vertex.y() * m.elements[5] + vertex.z() * m.elements[9] + t;
            self.trans_tex_coords.s = s0.to_fixed();
            self.trans_tex_coords.t = t0.to_fixed();
        }

        // Transform the vertex.
        // Result is a I12F12.
        let transformed_vertex = self.matrices.clip_matrix().mul_vector_4(&vertex);
        let w = transformed_vertex.w().to_fixed::<I40F24>();
        let w2 = I40F24::ONE.checked_div(w * 2).unwrap_or(I40F24::MAX);
        let x = (transformed_vertex.x().to_fixed::<I40F24>() + w) * w2;
        let y = I40F24::ONE - (transformed_vertex.y().to_fixed::<I40F24>() + w) * w2;
        let z = (transformed_vertex.z().to_fixed::<I40F24>() + w) * w2;

        let depth = if self.w_buffer {
            w.to_fixed::<Depth>()
        } else {
            z.to_fixed::<Depth>() * 0x7FFF
        };
        
        self.staged_polygon[self.staged_index] = StagedVertex {
            position: Vector::new([
                x.to_fixed::<N>(), y.to_fixed::<N>(), z.to_fixed::<N>(), w.to_fixed::<N>()
            ]),
            screen_p:   self.clipping_unit.get_screen_coords(x.to_fixed::<N>(), y.to_fixed::<N>()),
            colour:     self.lighting.get_vertex_colour(),
            tex_coords: self.trans_tex_coords.clone(),
            depth: depth,

            needs_clip: None,
            idx:        None
        };
        self.output_vertex();

        8
    }

    /// Advance the staging state machine and possibly output a polygon.
    fn output_vertex(&mut self) {
        use Primitive::*;
        
        self.staged_index = (self.staged_index + 1) % self.stage_size;

        // Advance the staging state machine.
        match self.primitive.unwrap() {
            Triangle(2) => {
                self.try_emit();
                self.primitive = Some(Triangle(0));
            },
            Triangle(n) => {
                self.primitive = Some(Triangle(n+1));
            },

            TriangleStripFirst(2) | TriangleStrip => {
                self.try_emit();
                if self.output_order == &TRI_STRIP_ORDER_A {
                    self.output_order = &TRI_STRIP_ORDER_B;
                } else {
                    self.output_order = &TRI_STRIP_ORDER_A;
                }
                self.primitive = Some(TriangleStrip);
            },
            TriangleStripFirst(n) => {
                self.primitive = Some(TriangleStripFirst(n+1));
            },

            Quad(3) => {
                self.try_emit();
                self.primitive = Some(Quad(0));
            },
            Quad(n) => {
                self.primitive = Some(Quad(n+1));
            },
            
            QuadStripFirst(3) | QuadStrip(1) => {
                self.try_emit();
                self.primitive = Some(QuadStrip(0));
            },
            QuadStripFirst(n) => {
                self.primitive = Some(QuadStripFirst(n+1));
            },
            QuadStrip(n) => {
                self.primitive = Some(QuadStrip(n+1));
            },
        }
    }

    /// Test if this polygon should be output, and if so write it to buffers.
    /// 
    /// Test:
    /// - If all vertices are off-screen
    /// - Winding of vertices
    /// - One-dot display
    fn try_emit(&mut self) {
        if !self.test_winding() {
            return;
        }

        if !self.test_in_view() {
            return;
        }

        if !self.test_one_dot_display() {
            return;
        }
        
        self.clip_and_emit_polygon();
    }

    /// Test winding for the current polygon.
    /// This checks if the front or back face is showing,
    /// and if that face should be displayed.
    /// 
    /// Returns true if the polygon should be shown.
    fn test_winding(&self) -> bool {
        let size = (0..self.stage_size).fold(N::ZERO, |acc, n| {
            let current_index = self.output_order[n];
            let next_index = self.output_order[(n + 1) % self.stage_size];

            let stage_index_0 = (self.staged_index + current_index) % self.stage_size;
            let stage_index_1 = (self.staged_index + next_index) % self.stage_size;

            let v0 = &self.staged_polygon[stage_index_0];
            let v1 = &self.staged_polygon[stage_index_1];
            let segment_size = (v1.screen_p.x - v0.screen_p.x) * (v1.screen_p.y + v0.screen_p.y);
            acc + segment_size
        });

        if size > N::ZERO {
            self.polygon_attrs.contains(PolygonAttrs::RENDER_FRONT)
        } else if size < N::ZERO {
            self.polygon_attrs.contains(PolygonAttrs::RENDER_BACK)
        } else {
            // Always display line polygons.
            true
        }
    }
    
    /// If any vertices are outside the view, mark them for clipping.
    fn test_in_view(&mut self) -> bool {
        let mut intersects_far_plane = false;
        
        //let X_MAX = N::ONE - N::from_bits(1);
        //let Y_MAX = N::ONE - N::from_bits(1);
        
        for stage_idx in 0..self.stage_size {
            let vertex = &mut self.staged_polygon[stage_idx];
            if vertex.needs_clip.is_none() {
                // Calc if in view.
                let needs_clip = vertex.position.x() < I20F12::ZERO ||
                    vertex.position.x() > I20F12::ONE ||
                    vertex.position.y() < I20F12::ZERO ||
                    vertex.position.y() > I20F12::ONE/* ||
                    vertex.position.z() < I20F12::ZERO ||
                    vertex.position.z() >= I20F12::ONE*/;
                vertex.needs_clip = Some(needs_clip);
            };

            intersects_far_plane = intersects_far_plane || (
                vertex.position.z() >= I23F9::ONE
            );
        }

        // !(intersects_far_plane && !self.polygon_attrs.contains(PolygonAttrs::FAR_PLANE_CLIP))
        !intersects_far_plane
    }

    /// Test one-dot display for the current polygon.
    /// 
    /// If all of the vertices are within the same screen dot,
    /// optionally test against the one-dot depth value.
    fn test_one_dot_display(&self) -> bool {
        // Only do this test if the polygon attr is set to 0.
        if !self.polygon_attrs.contains(PolygonAttrs::RENDER_DOT) {
            let v0 = &self.staged_polygon[0];
            let screen_x = v0.screen_p.x.to_num::<i16>();
            let screen_y = v0.screen_p.y.to_num::<i16>();
            let mut dot_w = v0.position.w();
            for stage_idx in 1..self.stage_size {
                let v = &self.staged_polygon[stage_idx];
                // TODO: test if within a dot?
                if v.screen_p.x.to_num::<i16>() != screen_x ||
                    v.screen_p.y.to_num::<i16>() != screen_y {
                    // Not a one-dot polygon.
                    return true;
                }
                dot_w = std::cmp::min(dot_w, v.position.w());
            }
            // If the smallest dot w is larger than the test value,
            // DO NOT output this polygon.
            dot_w <= self.dot_polygon_w
        } else {
            true
        }
    }

    /// Clip the vertices, producing 1 or 2 new vertices per clip.
    /// 
    /// It will only output the polygon if some of the vertices are inside the view frustrum.
    fn clip_and_emit_polygon(&mut self) {
        let mut staged_polygon = StagedPolygon {
            polygon: Polygon {
                attrs:          self.polygon_attrs,
                tex:            self.texture_attrs,
                palette:        self.tex_palette,
                num_vertices:   0,
                vertex_indices: [0; 8],
            },
            max_y: N::ZERO,
            min_y: N::MAX
        };
        
        let mut out_clips = [None, None, None, None];
        let mut in_clips = [None, None, None, None];

        for n in 0..self.stage_size {
            let current_index = self.output_order[n];
            let stage_index = (self.staged_index + current_index) % self.stage_size;
            
            if self.staged_polygon[stage_index].needs_clip.unwrap() {
                let vertex = &self.staged_polygon[stage_index];

                let prev_index = self.output_order[(self.stage_size + n - 1) % self.stage_size];
                let stage_index_prev = (self.staged_index + prev_index) % self.stage_size;
                let prev_vtx = &self.staged_polygon[stage_index_prev];
                self.clipping_unit.clip(true, vertex, prev_vtx, &mut out_clips, &mut in_clips, &mut staged_polygon);
                
                let next_index = self.output_order[(n + 1) % self.stage_size];
                let stage_index_next = (self.staged_index + next_index) % self.stage_size;
                let next_vtx = &self.staged_polygon[stage_index_next];
                self.clipping_unit.clip(false, vertex, next_vtx, &mut out_clips, &mut in_clips, &mut staged_polygon);

            } else {
                // TODO: store vtx indexes in separate place
                let vertex = &mut self.staged_polygon[stage_index];

                staged_polygon.max_y = std::cmp::max(staged_polygon.max_y, vertex.screen_p.y);
                staged_polygon.min_y = std::cmp::min(staged_polygon.min_y, vertex.screen_p.y);

                if let Some(idx) = vertex.idx {
                    staged_polygon.polygon.add_vertex_index(idx);
                } else {
                    let idx = self.clipping_unit.polygon_ram.insert_vertex(Vertex {
                        screen_p:   vertex.screen_p.clone(),
                        depth:      vertex.depth,
                        colour:     vertex.colour,
                        tex_coords: vertex.tex_coords.clone()
                    });
                    vertex.idx = Some(idx);
                    staged_polygon.polygon.add_vertex_index(idx);
                }
            }
        }

        if !staged_polygon.polygon.vertex_indices.is_empty() && staged_polygon.polygon.vertex_indices.len() >= 3 {
            self.clipping_unit.polygon_ram.insert_polygon(staged_polygon.polygon, staged_polygon.max_y, staged_polygon.min_y);
        }
    }
}
