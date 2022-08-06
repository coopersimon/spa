mod math;
mod matrix;
mod lighting;

pub use math::*;
use matrix::*;
use lighting::*;

use fixed::{types::{I4F12, I12F4, I23F9, I13F3}, traits::ToFixed};
use crate::{
    utils::{
        bits::u32, bytes
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
    // The vertex before emitting a subsequent quad strip polygon.
    //QuadStripReady
}

pub struct GeometryEngine {
    pub polygon_ram:    Box<PolygonRAM>,

    viewport_x:     u8,
    viewport_y:     u8,
    viewport_width: u8,
    viewport_height:u8,

    /// Use W or Z value for depth-buffering.
    w_buffer:       bool,
    /// Manually sort translucent polygons.
    manual_sort:    bool,
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
    /// Currently inputting vertices.
    current_vertex: [I4F12; 3],

    /// Current polygon vertices for outputting to Vertex RAM.
    /// Will only be written if it passes the W-test.
    staged_polygon:     Vec<ClipVertex>,
    staged_index:       usize,
    stage_size:         usize,
    primitive:          Option<Primitive>,
}

#[derive(Default, Clone)]
struct ClipVertex {
    vtx:        Vertex,
    clip_vtx:   Option<Vertex>
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
            manual_sort:    false,
            dot_polygon_w:  I13F3::from_bits(0x7FFF),

            matrices:       Box::new(MatrixUnit::new()),
            lighting:       Box::new(LightingUnit::new()),

            box_test_res:   false,
            pos_test_res:   [0; 4],
            dir_test_res:   [0; 3],

            polygon_attrs:  PolygonAttrs::default(),
            texture_attrs:  TextureAttrs::default(),
            tex_palette:    0,
            current_vertex: [I4F12::ZERO; 3],

            staged_polygon:     vec![ClipVertex::default(); 4],
            staged_index:       0,
            stage_size:         3,
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
        self.manual_sort = u32::test_bit(data, 1);
    }

    pub fn set_vertex_colour(&mut self, data: u32) -> isize {
        self.lighting.set_vertex_colour(data);
        1
    }

    pub fn set_normal(&mut self, data: u32) -> isize {
        let x_bits = (data & 0x3FF) as i32;
        let y_bits = ((data >> 10) & 0x3FF) as i32;
        let z_bits = ((data >> 20) & 0x3FF) as i32;
        let v = Vector::new([
            N::from_bits(x_bits << 3),
            N::from_bits(y_bits << 3),
            N::from_bits(z_bits << 3),
        ]);
        let normal = self.matrices.dir_matrix().mul_vector_3(&v);
        // Calculate colour.
        self.lighting.set_normal(normal)
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
        let x_bits = (data & 0x3FF) as i32;
        let y_bits = ((data >> 10) & 0x3FF) as i32;
        let z_bits = ((data >> 20) & 0x3FF) as i32;
        let v = Vector::new([
            N::from_bits(x_bits << 3),
            N::from_bits(y_bits << 3),
            N::from_bits(z_bits << 3),
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
        // TODO
        1
    }

    /// Called before vertex data is input.
    /// 
    /// Also decides which primitive type to use.
    pub fn begin_vertex_list(&mut self, data: u32) -> isize {
        let primitive = match data & 0b11 {
            0b00 => {
                self.stage_size = 3;
                Primitive::Triangle(0)
            },
            0b01 => {
                self.stage_size = 4;
                Primitive::Quad(0)
            },
            0b10 => {
                self.stage_size = 3;
                Primitive::TriangleStripFirst(0)
            },
            0b11 => {
                self.stage_size = 4;
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
    
    /// Set vertex coordinates as a diff of current. I1F9 format.
    /// 
    /// Param: X, Y, Z, each 10 bits.
    pub fn diff_vertex_coords(&mut self, data: u32) -> isize {
        let x_diff = (data & 0x3FF) << 3;
        let y_diff = ((data >> 10) & 0x3FF) << 3;
        let z_diff = ((data >> 20) & 0x3FF) << 3;
        self.current_vertex[0] += I4F12::from_bits(x_diff as i16);
        self.current_vertex[1] += I4F12::from_bits(y_diff as i16);
        self.current_vertex[2] += I4F12::from_bits(z_diff as i16);
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
            self.current_vertex[0].into(),
            self.current_vertex[1].into(),
            self.current_vertex[2].into(),
            N::ONE
        ]);

        // Transform the vertex.
        // Result is a I12F12.
        let transformed_vertex = self.matrices.clip_matrix().mul_vector_4(&vertex);
        // TODO: mask?
        let w = transformed_vertex.w();
        let w2_recip = N::ONE.checked_div(w * 2).unwrap_or(N::MAX);
        let x = (transformed_vertex.x() + w) * w2_recip;
        let y = (transformed_vertex.y() + w) * w2_recip;
        let depth = if self.w_buffer {
            w.to_fixed::<I23F9>()
        } else {
            let z = (transformed_vertex.z() + w) * w2_recip;
            z.to_fixed::<I23F9>()
        };

        /*if x >= N::ONE || x < N::ZERO {
            // TODO: CLIP X
            println!("clip x");
        }
        if y >= N::ONE || y < N::ZERO {
            // TODO: CLIP Y
            println!("clip y");
        }*/

        // TODO: not sure about these calcs.
        let viewport_x = I12F4::from_num(self.viewport_x) + (x * N::from_num(self.viewport_width)).to_fixed::<I12F4>();
        let viewport_y = I12F4::from_num(self.viewport_y) + (y * N::from_num(self.viewport_height)).to_fixed::<I12F4>();

        self.output_vertex(Vertex {
            screen_p: Coords{x: viewport_x, y: viewport_y},
            depth: depth,
            colour: self.lighting.get_vertex_colour(),
            tex_coords: Coords { x: I12F4::ZERO, y: I12F4::ZERO }   // TODO
        });

        8
    }

    /// Advance the staging state machine and possibly output a polygon.
    fn output_vertex(&mut self, vertex: Vertex) {
        use Primitive::*;
        
        self.staged_polygon[self.staged_index] = ClipVertex {
            vtx: vertex,
            clip_vtx: None
        };
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
            QuadStripFirst(3) => {
                self.try_emit();
                self.primitive = Some(QuadStrip(0));
            },
            QuadStripFirst(n) => {
                self.primitive = Some(QuadStripFirst(n+1));
            },
            QuadStrip(1) => {
                self.try_emit();
                self.primitive = Some(QuadStrip(0));
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
        // Test winding

        // Clip + test

        // One-dot display test

        // Output
    }

    /// Clip the vertices, producing 1 or 2 new vertices per clip.
    /// 
    /// Test if this polygon should be output or not.
    fn clip_and_test(&mut self) -> bool {
        // Clip
        for n in 0..self.stage_size {
            let index = (self.staged_index + n) % self.stage_size;
            // If needs clipping...
                // Clip edge 0-1
                // Clip edge 2-0
            // If no output...
                // Skip triangle
        }
        true
    }
    
    /// Write a polygon + new vertices to list RAM.
    fn emit_polygon(&mut self, new_vertices: usize, old_vertices: usize) {
        let mut polygon = Polygon {
            attrs:      self.polygon_attrs,
            tex:        self.texture_attrs,
            palette:    self.tex_palette,
            x_max:      I12F4::ZERO,
            x_min:      I12F4::MAX,
            vertex_indices: Vec::new(),
        };

        let mut y_max = I12F4::ZERO;
        let mut y_min = I12F4::MAX;
        for n in 0..self.stage_size {
            let index = (self.staged_index + n) % self.stage_size;
            let vertex = &self.staged_polygon[index];
            //y_max = std::cmp::max(y_max, vertex.screen_p.y);
            //y_min = std::cmp::min(y_min, vertex.screen_p.y);
            //polygon.x_max = std::cmp::max(polygon.x_max, vertex.screen_p.x);
            //polygon.x_min = std::cmp::min(polygon.x_min, vertex.screen_p.x);
            // if output
            //let v_idx = self.polygon_ram.insert_vertex(vertex.clone());

            //polygon.vertex_indices.push(v_idx);
        }
    }

    // Output a polygon, and any associated new vertices.
    /*fn emit_polygon(&mut self) {
        let mut polygon = Polygon {
            attrs:      self.polygon_attrs,
            tex:        self.texture_attrs,
            palette:    self.tex_palette,
            x_max:      I12F4::ZERO,
            x_min:      I12F4::MAX,
            vertex_indices: Vec::new(),
        };
        
        match self.primitive.expect("trying to output vertex without calling begin") {
            Primitive::Triangle => {
                let mut y_max = I12F4::ZERO;
                let mut y_min = I12F4::MAX;
                for vertex in self.staged_polygon.iter() {
                    y_max = std::cmp::max(y_max, vertex.screen_p.y);
                    y_min = std::cmp::min(y_min, vertex.screen_p.y);
                    polygon.x_max = std::cmp::max(polygon.x_max, vertex.screen_p.x);
                    polygon.x_min = std::cmp::min(polygon.x_min, vertex.screen_p.x);
                    let v_idx = self.polygon_ram.insert_vertex(vertex.clone());
                    polygon.vertex_indices.push(v_idx);
                }
                self.polygon_ram.insert_polygon(polygon, y_max, y_min);
                self.staged_polygon.resize(3);
            },
            Primitive::TriangleStripFirst => {
                let mut y_max = I12F4::ZERO;
                let mut y_min = I12F4::MAX;
                for vertex in self.staged_polygon.iter() {
                    y_max = std::cmp::max(y_max, vertex.screen_p.y);
                    y_min = std::cmp::min(y_min, vertex.screen_p.y);
                    polygon.x_max = std::cmp::max(polygon.x_max, vertex.screen_p.x);
                    polygon.x_min = std::cmp::min(polygon.x_min, vertex.screen_p.x);
                    let v_idx = self.polygon_ram.insert_vertex(vertex.clone());
                    polygon.vertex_indices.push(v_idx);
                }
                self.polygon_ram.insert_polygon(polygon, y_max, y_min);
                self.primitive = Some(Primitive::TriangleStripReady);
            },
            Primitive::TriangleStripReady => {  // TODO
                let v_idx = self.polygon_ram.insert_vertex(self.staged_polygon.end().clone());
                polygon.vertex_indices.push(v_idx);
                self.polygon_ram.insert_polygon(polygon, I12F4::ZERO, I12F4::ZERO);
                // TODO: insert previous indices.
            },
            Primitive::Quad => {
                let mut y_max = I12F4::ZERO;
                let mut y_min = I12F4::MAX;
                for vertex in self.staged_polygon.iter() {
                    y_max = std::cmp::max(y_max, vertex.screen_p.y);
                    y_min = std::cmp::min(y_min, vertex.screen_p.y);
                    polygon.x_max = std::cmp::max(polygon.x_max, vertex.screen_p.x);
                    polygon.x_min = std::cmp::min(polygon.x_min, vertex.screen_p.x);
                    let v_idx = self.polygon_ram.insert_vertex(vertex.clone());
                    polygon.vertex_indices.push(v_idx);
                }
                self.polygon_ram.insert_polygon(polygon, y_max, y_min);
                self.staged_polygon.resize(4);
            },
            Primitive::QuadStripFirst => {
                let mut y_max = I12F4::ZERO;
                let mut y_min = I12F4::MAX;
                for vertex in self.staged_polygon.iter() {
                    y_max = std::cmp::max(y_max, vertex.screen_p.y);
                    y_min = std::cmp::min(y_min, vertex.screen_p.y);
                    polygon.x_max = std::cmp::max(polygon.x_max, vertex.screen_p.x);
                    polygon.x_min = std::cmp::min(polygon.x_min, vertex.screen_p.x);
                    let v_idx = self.polygon_ram.insert_vertex(vertex.clone());
                    polygon.vertex_indices.push(v_idx);
                }
                self.polygon_ram.insert_polygon(polygon, y_max, y_min);
                self.primitive = Some(Primitive::QuadStripBuffer);
            },
            Primitive::QuadStripBuffer => panic!("trying to emit a quad strip polygon when not ready"),
            Primitive::QuadStripReady => {
                let mut y_max = I12F4::ZERO;
                let mut y_min = I12F4::MAX;
                for vertex in self.staged_polygon.iter() {
                    y_max = std::cmp::max(y_max, vertex.screen_p.y);
                    y_min = std::cmp::min(y_min, vertex.screen_p.y);
                    polygon.x_max = std::cmp::max(polygon.x_max, vertex.screen_p.x);
                    polygon.x_min = std::cmp::min(polygon.x_min, vertex.screen_p.x);
                    let v_idx = self.polygon_ram.insert_vertex(vertex.clone());
                    polygon.vertex_indices.push(v_idx);
                    // TODO: insert previous indices.
                }
                self.polygon_ram.insert_polygon(polygon, y_max, y_min);
                self.primitive = Some(Primitive::QuadStripBuffer);
            }
        }
    }*/
}
