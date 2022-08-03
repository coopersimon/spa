mod math;
mod matrix;
mod lighting;

pub use math::*;
use matrix::*;
use lighting::*;

use fixed::{types::{I4F12, I12F4, I23F9, I13F3}, traits::ToFixed};
use crate::{
    utils::{
        bits::u32,
        bytes, circbuffer::CircularBuffer
    },
};
use super::types::*;

#[derive(Clone, Copy)]
enum Primitive {
    Triangle,
    /// The first polygon of a triangle strip.
    TriangleStripFirst,
    /// Subsequent polygons of a triangle strip.
    TriangleStripReady,
    Quad,
    /// The first polygon of a quad strip.
    QuadStripFirst,
    /// The vertex after emitting a subsequent quad strip polygon.
    QuadStripBuffer,
    /// The vertex before emitting a subsequent quad strip polygon.
    QuadStripReady
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
    staged_polygon:     CircularBuffer<Vertex>,
    primitive:          Option<Primitive>,
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

            polygon_attrs:  PolygonAttrs::default(),
            texture_attrs:  TextureAttrs::default(),
            tex_palette:    0,
            current_vertex: [I4F12::ZERO; 3],

            staged_polygon:     CircularBuffer::new(),
            primitive:          None,
        }
    }

    pub fn set_dot_polygon_depth(&mut self, data: u32) {
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

    // TODO: tex

    /// Called before vertex data is input.
    /// 
    /// Also decides which primitive type to use.
    pub fn begin_vertex_list(&mut self, data: u32) -> isize {
        let primitive = match data & 0b11 {
            0b00 => {
                self.staged_polygon.resize(3);
                Primitive::Triangle
            },
            0b01 => {
                self.staged_polygon.resize(4);
                Primitive::Quad
            },
            0b10 => {
                self.staged_polygon.resize(3);
                Primitive::TriangleStripFirst
            },
            0b11 => {
                self.staged_polygon.resize(4);
                Primitive::QuadStripFirst
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
        let w = transformed_vertex.elements[3];
        let w2 = w * 2;
        let x = (transformed_vertex.elements[0]) + w / w2;
        let y = (transformed_vertex.elements[1]) + w / w2;
        let depth = if self.w_buffer {
            w.to_fixed::<I23F9>()
        } else {
            let z = (transformed_vertex.elements[2]) + w / w2;
            z.to_fixed::<I23F9>()
        };

        if x >= N::ONE || x < N::ZERO {
            // TODO: CLIP X
            println!("clip x");
        }
        if y >= N::ONE || y < N::ZERO {
            // TODO: CLIP Y
            println!("clip y");
        }

        // TODO: not sure about these calcs.
        let viewport_x = I12F4::from_num(self.viewport_x) + (x * N::from_num(self.viewport_width)).to_fixed::<I12F4>();
        let viewport_y = I12F4::from_num(self.viewport_y) + (y * N::from_num(self.viewport_height)).to_fixed::<I12F4>();

        self.staged_polygon.push(Vertex {
            screen_p: Coords{x: viewport_x, y: viewport_y},
            depth: depth,
            colour: self.lighting.get_vertex_colour(),
            tex_coords: Coords { x: I12F4::ZERO, y: I12F4::ZERO }   // TODO
        });

        if self.should_emit() {
            self.emit_polygon();
        }

        8
    }

    fn should_emit(&mut self) -> bool {
        if !self.staged_polygon.is_full() {
            return false;
        }
        match self.primitive.expect("trying to output vertex without calling begin") {
            Primitive::QuadStripBuffer => {
                // Only output a new polygon every second quad strip vertex.
                self.primitive = Some(Primitive::QuadStripReady);
                return false;
            }
            _ => ()
        }
        

        // TODO: check 1-dot display
        // For each vertex test if x&y are within 1 dot.

        // TODO: back/front face culling

        true
    }

    /// Output a polygon, and any associated new vertices.
    fn emit_polygon(&mut self) {
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
    }
}
