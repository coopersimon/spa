mod math;
mod matrix;
mod lighting;

pub use math::*;
use matrix::*;
use lighting::*;

use fixed::{types::{I4F12, I20F12, I23F9, I13F3, I12F4}, traits::ToFixed};
use crate::{
    common::colour::Colour,
    utils::{
        bits, bits::u32, bytes
    },
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
    pub polygon_ram:    Box<PolygonRAM>,

    viewport_x:     u8,
    viewport_y:     u8,
    viewport_width: u8,
    viewport_height:u8,

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

#[derive(Clone, Default)]
struct StagedVertex {
    position:   Vector<4>,
    screen_p:   Coords,
    colour:     Colour,
    tex_coords: TexCoords,

    needs_clip: Option<bool>,
    idx:        Option<usize>,
}

impl GeometryEngine {
    pub fn new() -> Self {
        Self {
            polygon_ram:    Box::new(PolygonRAM::new()),

            viewport_x:         0,
            viewport_y:         0,
            viewport_width:     0,
            viewport_height:    0,
            
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
        let bytes = u32::to_le_bytes(data);
        self.viewport_x = bytes[0];
        self.viewport_y = bytes[1];
        self.viewport_width = bytes[2] - self.viewport_x;
        self.viewport_height = bytes[3] - self.viewport_y;
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
            N::from_bits(bits::u16::sign_extend(x_bits << 3, 3).into()),
            N::from_bits(bits::u16::sign_extend(y_bits << 3, 3).into()),
            N::from_bits(bits::u16::sign_extend(z_bits << 3, 3).into()),
        ]);
        let normal = self.matrices.dir_matrix().mul_vector_3(&v);
        let tex_cycles = if self.texture_attrs.transform_mode() == 2 {
            let s = self.tex_coords.s.to_fixed::<N>();
            let t = self.tex_coords.t.to_fixed::<N>();
            let s0 = v.x() * self.matrices.tex_matrix().elements[0] + v.y() * self.matrices.tex_matrix().elements[4] + v.z() * self.matrices.tex_matrix().elements[8] + s;
            let t0 = v.x() * self.matrices.tex_matrix().elements[1] + v.y() * self.matrices.tex_matrix().elements[5] + v.z() * self.matrices.tex_matrix().elements[9] + t;
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
            N::from_bits(bits::u16::sign_extend(x_bits << 3, 3).into()),
            N::from_bits(bits::u16::sign_extend(y_bits << 3, 3).into()),
            N::from_bits(bits::u16::sign_extend(z_bits << 3, 3).into()),
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
                let s0 = s * self.matrices.tex_matrix().elements[0] + t * self.matrices.tex_matrix().elements[4] + self.matrices.tex_matrix().elements[8] + self.matrices.tex_matrix().elements[12];
                let t0 = s * self.matrices.tex_matrix().elements[1] + t * self.matrices.tex_matrix().elements[5] + self.matrices.tex_matrix().elements[9] + self.matrices.tex_matrix().elements[13];
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
            let w2_recip = N::ONE.checked_div(w * 2).unwrap_or(N::MAX);
            let normal_x = (transformed_vertex.x() + w) * w2_recip;
            let normal_y = (transformed_vertex.y() + w) * w2_recip;
            let normal_z = (transformed_vertex.z() + w) * w2_recip;

            let outside_x = (normal_x >= N::ONE) || (normal_x < N::ZERO);
            let outside_y = (normal_y >= N::ONE) || (normal_y < N::ZERO);
            let outside_z = (normal_z >= N::ONE) || (normal_z < N::ZERO);

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
        let x_bits = (data & 0x3FF) as i32;
        let y_bits = ((data >> 10) & 0x3FF) as i32;
        let z_bits = ((data >> 20) & 0x3FF) as i32;
        let v = Vector::new([
            N::from_bits(x_bits << 3),
            N::from_bits(y_bits << 3),
            N::from_bits(z_bits << 3),
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
            let s0 = vertex.x() * self.matrices.tex_matrix().elements[0] + vertex.y() * self.matrices.tex_matrix().elements[4] + vertex.z() * self.matrices.tex_matrix().elements[8] + s;
            let t0 = vertex.x() * self.matrices.tex_matrix().elements[1] + vertex.y() * self.matrices.tex_matrix().elements[5] + vertex.z() * self.matrices.tex_matrix().elements[9] + t;
            self.trans_tex_coords.s = s0.to_fixed();
            self.trans_tex_coords.t = t0.to_fixed();
        }

        // Transform the vertex.
        // Result is a I12F12.
        let transformed_vertex = self.matrices.clip_matrix().mul_vector_4(&vertex);
        let w = transformed_vertex.w();
        let w2_recip = N::ONE.checked_div(w * 2).unwrap_or(N::MAX);
        let x = (transformed_vertex.x() + w) * w2_recip;
        let y = N::ONE - (transformed_vertex.y() + w) * w2_recip;
        let z = (transformed_vertex.z() + w) * w2_recip;
        
        self.staged_polygon[self.staged_index] = StagedVertex {
            position: Vector::new([
                x, y, z, w
            ]),
            screen_p: self.get_screen_coords(x, y),
            colour: self.lighting.get_vertex_colour(),
            tex_coords: self.trans_tex_coords.clone(),

            needs_clip: None,
            idx:        None
        };
        self.output_vertex();

        8
    }

    fn get_screen_coords(&self, x: N, y: N) -> Coords {
        let screen_x = N::from_num(self.viewport_x) + (x * N::from_num(self.viewport_width));
        let screen_y = N::from_num(self.viewport_y) + (y * N::from_num(self.viewport_height));
        Coords { x: screen_x, y: screen_y }
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
    
    /// Test if current polygon is in view.
    /// 
    /// If any vertices are outside the view, mark them for clipping.
    /// If ALL vertices are outside the view, return FALSE.
    fn test_in_view(&mut self) -> bool {
        let mut all_outside_view = true;
        let mut intersects_far_plane = false;

        for stage_idx in 0..self.stage_size {
            let vertex = &mut self.staged_polygon[stage_idx];
            let v_outside_view = if let Some(needs_clip) = vertex.needs_clip {
                needs_clip
            } else {
                // Calc if in view.
                let needs_clip = vertex.position.x() < I20F12::ZERO ||
                    vertex.position.x() >= I20F12::ONE ||
                    vertex.position.y() < I20F12::ZERO ||
                    vertex.position.y() >= I20F12::ONE/* ||
                    vertex.position.z() < I20F12::ZERO ||
                    vertex.position.z() >= I20F12::ONE*/;
                vertex.needs_clip = Some(needs_clip);
                needs_clip
            };

            all_outside_view = all_outside_view && v_outside_view;

            intersects_far_plane = intersects_far_plane || (
                vertex.position.z() >= I23F9::ONE
            );
        }

        let hide = all_outside_view || (
            // If far plane clip is set to 0, and it touches the far plane,
            // ignore this polygon.
            intersects_far_plane// && !self.polygon_attrs.contains(PolygonAttrs::FAR_PLANE_CLIP)
        );

        !hide
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
        let mut polygon = Polygon {
            attrs:      self.polygon_attrs,
            tex:        self.texture_attrs,
            palette:    self.tex_palette,
            vertex_indices: Vec::new(),
        };
        
        let mut y_max = N::ZERO;
        let mut y_min = N::MAX;

        for n in 0..self.stage_size {
            let current_index = self.output_order[n];
            let stage_index = (self.staged_index + current_index) % self.stage_size;
            
            if self.staged_polygon[stage_index].needs_clip.unwrap() {
                let vertex = &self.staged_polygon[stage_index];

                let next_index = self.output_order[(n + 1) % self.stage_size];
                let stage_index_1 = (self.staged_index + next_index) % self.stage_size;
                let v1 = &self.staged_polygon[stage_index_1];
                if !v1.needs_clip.unwrap() {
                    let clipped_vtx = self.clip_and_interpolate(vertex, v1, self.w_buffer);
                    y_max = std::cmp::max(y_max, clipped_vtx.screen_p.y);
                    y_min = std::cmp::min(y_min, clipped_vtx.screen_p.y);
    
                    let idx = self.polygon_ram.insert_vertex(clipped_vtx);
                    polygon.vertex_indices.push(idx);
                }

                let next_index = self.output_order[(self.stage_size + n - 1) % self.stage_size];
                let stage_index_2 = (self.staged_index + next_index) % self.stage_size;
                let v2 = &self.staged_polygon[stage_index_2];
                if !v2.needs_clip.unwrap() {
                    let clipped_vtx = self.clip_and_interpolate(vertex, v2, self.w_buffer);
                    y_max = std::cmp::max(y_max, clipped_vtx.screen_p.y);
                    y_min = std::cmp::min(y_min, clipped_vtx.screen_p.y);
    
                    let idx = self.polygon_ram.insert_vertex(clipped_vtx);
                    polygon.vertex_indices.push(idx);
                }
            } else {
                // TODO: store vtx indexes in separate place
                let vertex = &mut self.staged_polygon[stage_index];

                y_max = std::cmp::max(y_max, vertex.screen_p.y);
                y_min = std::cmp::min(y_min, vertex.screen_p.y);

                if let Some(idx) = vertex.idx {
                    polygon.vertex_indices.push(idx);
                } else {
                    let idx = self.polygon_ram.insert_vertex(Vertex {
                        screen_p: vertex.screen_p.clone(),
                        depth: if self.w_buffer {
                            vertex.position.w().to_fixed::<I23F9>()
                        } else {
                            vertex.position.z().to_fixed::<I23F9>() * 0x7FFF
                        },
                        colour:     vertex.colour,
                        tex_coords: vertex.tex_coords.clone()
                    });
                    vertex.idx = Some(idx);
                    polygon.vertex_indices.push(idx);
                }
            }
        }
        
        self.polygon_ram.insert_polygon(polygon, y_max, y_min);
    }

    /// Clip point a, based on the line between a and b.
    fn clip_and_interpolate(&self, vtx_a: &StagedVertex, vtx_b: &StagedVertex, wbuffer: bool) -> Vertex {
        
        // TODO: ?
        let X_MAX = N::ONE - N::from_bits(1 << 4);
        if vtx_a.position.x() < N::ZERO {
            let factor_a = -vtx_a.position.x().checked_div(vtx_b.position.x() - vtx_a.position.x()).unwrap();
            //let gradient = (v1.position.y() - v0.position.y()).checked_div(v1.position.x() - v0.position.x()).unwrap_or(N::MAX);
            let y = (factor_a * (vtx_b.position.y() - vtx_a.position.y())) + vtx_a.position.y();
            //println!("try clip ({}, {}) => ({}, {}) : ({}, {})", vtx_a.position.x(), vtx_a.position.y(), vtx_b.position.x(), vtx_b.position.y(), N::ZERO, y);
            if y >= N::ZERO && y < N::ONE {
                let factor_b = N::ONE - factor_a;
                return Self::interpolate(vtx_a, vtx_b, wbuffer, factor_a, factor_b, self.get_screen_coords(N::ZERO, y));
            }
        } else if vtx_a.position.x() >= N::ONE {
            let factor_a = (X_MAX - vtx_a.position.x()).checked_div(vtx_b.position.x() - vtx_a.position.x()).unwrap();
            //let gradient = (v1.position.y() - v0.position.y()).checked_div(v1.position.x() - v0.position.x()).unwrap_or(N::MAX);
            let y = (factor_a * (vtx_b.position.y() - vtx_a.position.y())) + vtx_a.position.y();
            //println!("try clip ({}, {}) => ({}, {}) : ({}, {})", vtx_a.position.x(), vtx_a.position.y(), vtx_b.position.x(), vtx_b.position.y(), X_MAX, y);
            if y >= N::ZERO && y < N::ONE {
                let factor_b = N::ONE - factor_a;
                return Self::interpolate(vtx_a, vtx_b, wbuffer, factor_a, factor_b, self.get_screen_coords(X_MAX, y));
            }
        }
        
        let Y_MAX = N::ONE - N::from_bits(1 << 4);
        if vtx_a.position.y() < N::ZERO {
            //let gradient = (v1.position.x() - v0.position.x()).checked_div(v1.position.y() - v0.position.y()).unwrap_or(N::MAX);
            let factor_a = -vtx_a.position.y().checked_div(vtx_b.position.y() - vtx_a.position.y()).unwrap();
            let x = (factor_a * (vtx_b.position.x() - vtx_a.position.x())) + vtx_a.position.x();
            //println!("try clip ({}, {}) => ({}, {}) : ({}, {})", vtx_a.position.x(), vtx_a.position.y(), vtx_b.position.x(), vtx_b.position.y(), x, N::ZERO);
            if x >= N::ZERO && x < N::ONE {
                let factor_b = N::ONE - factor_a;
                return Self::interpolate(vtx_a, vtx_b, wbuffer, factor_a, factor_b, self.get_screen_coords(x, N::ZERO));
            }
        } else if vtx_a.position.y() >= N::ONE {
            let factor_a = (Y_MAX - vtx_a.position.y()).checked_div(vtx_b.position.y() - vtx_a.position.y()).unwrap();
            let x = (factor_a * (vtx_b.position.x() - vtx_a.position.x())) + vtx_a.position.x();
            //println!("try clip ({}, {}) => ({}, {}) : ({}, {})", vtx_a.position.x(), vtx_a.position.y(), vtx_b.position.x(), vtx_b.position.y(), x, Y_MAX);
            if x >= N::ZERO && x < N::ONE {
                let factor_b = N::ONE - factor_a;
                return Self::interpolate(vtx_a, vtx_b, wbuffer, factor_a, factor_b, self.get_screen_coords(x, Y_MAX));
            }
        }

        panic!("cannot find intersection!");
    }

    fn interpolate(vtx_a: &StagedVertex, vtx_b: &StagedVertex, wbuffer: bool, factor_a: N, factor_b: N, screen_p: Coords) -> Vertex {
        let depth_a = if wbuffer {
            vtx_a.position.w().to_fixed::<I23F9>()
        } else {
            vtx_a.position.z().to_fixed::<I23F9>() * 0x7FFF
        };
        let depth_b = if wbuffer {
            vtx_b.position.w().to_fixed::<I23F9>()
        } else {
            vtx_b.position.z().to_fixed::<I23F9>() * 0x7FFF
        };
        Vertex {
            screen_p: screen_p,
            colour: Self::interpolate_vertex_colour(vtx_a.colour, vtx_b.colour, factor_a, factor_b),
            tex_coords: Self::interpolate_tex_coords(vtx_a.tex_coords, vtx_b.tex_coords, factor_a, factor_b),
            depth: Self::interpolate_depth(depth_a, depth_b, factor_a, factor_b),
        }
    }
    
    // TODO: unify these and in 3d renderer
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

}
