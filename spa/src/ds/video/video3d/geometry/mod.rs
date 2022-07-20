mod math;
mod matrix;
mod lighting;

use math::*;
use matrix::*;
use lighting::*;

use fixed::types::I4F12;
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
    polygon_ram:    Box<PolygonRAM>,

    /// Matrix input buffer
    input_buffer:   Vec<N>,
    pub matrices:   Box<MatrixUnit>,

    pub lighting:   Box<LightingUnit>,

    /// Current polygon attributes.
    polygon_attrs:  PolygonAttrs,
    /// Current texture attributes.
    texture_attrs:  TextureAttrs,
    /// Current texture palette address.
    tex_palette:    u16,
    /// Currently inputting vertices.
    current_vertex: [I4F12; 3],
    /// For VTX_16_XYZ: indicates whether high or low input word.
    current_hi:     bool,

    /// Current polygon vertices for outputting to Vertex RAM.
    /// Will only be written if it passes the W-test.
    staged_polygon:     CircularBuffer<Vertex>,
    primitive:          Option<Primitive>,
}

impl GeometryEngine {
    pub fn new() -> Self {
        Self {
            polygon_ram:    Box::new(PolygonRAM::new()),

            input_buffer:   Vec::new(),
            matrices:       Box::new(MatrixUnit::new()),
            lighting:       Box::new(LightingUnit::new()),

            polygon_attrs:  PolygonAttrs::default(),
            texture_attrs:  TextureAttrs::default(),
            tex_palette:    0,
            current_vertex: [I4F12::ZERO; 3],
            current_hi:     false,

            staged_polygon:     CircularBuffer::new(),
            primitive:          None,
        }
    }
}

// GPU commands
impl GeometryEngine {
    pub fn set_identity_matrix(&mut self) {
        self.matrices.set_current_matrix(&Matrix::identity());
    }

    pub fn set_4x4_matrix(&mut self, data: u32) {
        self.input_buffer.push(N::from_bits(data as i32));
        if self.input_buffer.len() == 16 {
            self.matrices.set_current_matrix(&Matrix::from_4x4(&self.input_buffer));
            self.input_buffer.clear();
        }
    }
    
    pub fn set_4x3_matrix(&mut self, data: u32) {
        self.input_buffer.push(N::from_bits(data as i32));
        if self.input_buffer.len() == 12 {
            self.matrices.set_current_matrix(&Matrix::from_4x3(&self.input_buffer));
            self.input_buffer.clear();
        }
    }

    pub fn mul_4x4(&mut self, data: u32) {
        self.input_buffer.push(N::from_bits(data as i32));
        if self.input_buffer.len() == 16 {
            self.matrices.mul_4x4(&self.input_buffer);
            self.input_buffer.clear();
        }
    }
    
    pub fn mul_4x3(&mut self, data: u32) {
        self.input_buffer.push(N::from_bits(data as i32));
        if self.input_buffer.len() == 12 {
            self.matrices.mul_4x3(&self.input_buffer);
            self.input_buffer.clear();
        }
    }
    
    pub fn mul_3x3(&mut self, data: u32) {
        self.input_buffer.push(N::from_bits(data as i32));
        if self.input_buffer.len() == 9 {
            self.matrices.mul_3x3(&self.input_buffer);
            self.input_buffer.clear();
        }
    }
    
    pub fn mul_scale(&mut self, data: u32) {
        self.input_buffer.push(N::from_bits(data as i32));
        if self.input_buffer.len() == 3 {
            self.matrices.mul_scale(&self.input_buffer);
            self.input_buffer.clear();
        }
    }
    
    pub fn mul_trans(&mut self, data: u32) {
        self.input_buffer.push(N::from_bits(data as i32));
        if self.input_buffer.len() == 3 {
            self.matrices.mul_trans(&self.input_buffer);
            self.input_buffer.clear();
        }
    }

    pub fn set_vertex_colour(&mut self, data: u32) {
        self.lighting.set_vertex_colour(data);
    }

    pub fn set_light_direction(&mut self, data: u32) {
        let x_bits = (data & 0x3FF) as i32;
        let y_bits = ((data >> 10) & 0x3FF) as i32;
        let z_bits = ((data >> 20) & 0x3FF) as i32;
        let v = Vector::new([
            N::from_bits(x_bits << 3),
            N::from_bits(y_bits << 3),
            N::from_bits(z_bits << 3),
        ]);
        let direction = self.matrices.current_direction.mul_vector_3(&v);
        let light = (data >> 30) as usize;
        self.lighting.set_light_direction(light, direction);
    }

    pub fn set_normal(&mut self, data: u32) {
        let x_bits = (data & 0x3FF) as i32;
        let y_bits = ((data >> 10) & 0x3FF) as i32;
        let z_bits = ((data >> 20) & 0x3FF) as i32;
        let v = Vector::new([
            N::from_bits(x_bits << 3),
            N::from_bits(y_bits << 3),
            N::from_bits(z_bits << 3),
        ]);
        let normal = self.matrices.current_direction.mul_vector_3(&v);
        // Calculate colour.
        self.lighting.set_normal(normal);
    }

    // TODO: tex

    /// Called before vertex data is input.
    /// 
    /// Also decides which primitive type to use.
    pub fn begin_vertex_list(&mut self, data: u32) {
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
    }

    pub fn end_vertex_list(&mut self) {
        self.primitive = None;
    }

    /// Set vertex coordinates. Uses 2 parameter words. I4F12 format.
    /// 
    /// First param: X in lower half, Y in upper half.
    /// 
    /// Second param: Z in lower half.
    pub fn set_vertex_coords_16(&mut self, data: u32) {
        if self.current_hi {
            self.current_vertex[2] = I4F12::from_bits(bytes::u32::lo(data) as i16);
            self.current_hi = false;
            self.process_vertex();
        } else {
            self.current_vertex[0] = I4F12::from_bits(bytes::u32::lo(data) as i16);
            self.current_vertex[1] = I4F12::from_bits(bytes::u32::hi(data) as i16);
            self.current_hi = true;
        }
    }
    
    /// Set vertex coordinates. I4F6 format.
    /// 
    /// Param: X, Y, Z, each 10 bits.
    pub fn set_vertex_coords_10(&mut self, data: u32) {
        let x = (data & 0x3FF) << 6;
        let y = ((data >> 10) & 0x3FF) << 6;
        let z = ((data >> 20) & 0x3FF) << 6;
        self.current_vertex[0] = I4F12::from_bits(x as i16);
        self.current_vertex[1] = I4F12::from_bits(y as i16);
        self.current_vertex[2] = I4F12::from_bits(z as i16);
        self.process_vertex();
    }
    
    /// Set vertex coordinates X and Y. I4F12 format. Keep old Z.
    /// 
    /// Param: X in lower half, Y in upper half.
    pub fn set_vertex_coords_xy(&mut self, data: u32) {
        self.current_vertex[0] = I4F12::from_bits(bytes::u32::lo(data) as i16);
        self.current_vertex[1] = I4F12::from_bits(bytes::u32::hi(data) as i16);
        self.process_vertex();
    }
    
    /// Set vertex coordinates X and Z. I4F12 format. Keep old Y.
    /// 
    /// Param: X in lower half, Z in upper half.
    pub fn set_vertex_coords_xz(&mut self, data: u32) {
        self.current_vertex[0] = I4F12::from_bits(bytes::u32::lo(data) as i16);
        self.current_vertex[2] = I4F12::from_bits(bytes::u32::hi(data) as i16);
        self.process_vertex();
    }
    
    /// Set vertex coordinates Y and Z. I4F12 format. Keep old X.
    /// 
    /// Param: Y in lower half, Z in upper half.
    pub fn set_vertex_coords_yz(&mut self, data: u32) {
        self.current_vertex[1] = I4F12::from_bits(bytes::u32::lo(data) as i16);
        self.current_vertex[2] = I4F12::from_bits(bytes::u32::hi(data) as i16);
        self.process_vertex();
    }
    
    /// Set vertex coordinates as a diff of current. I1F9 format.
    /// 
    /// Param: X, Y, Z, each 10 bits.
    pub fn diff_vertex_coords(&mut self, data: u32) {
        let x_diff = (data & 0x3FF) << 3;
        let y_diff = ((data >> 10) & 0x3FF) << 3;
        let z_diff = ((data >> 20) & 0x3FF) << 3;
        self.current_vertex[0] += I4F12::from_bits(x_diff as i16);
        self.current_vertex[1] += I4F12::from_bits(y_diff as i16);
        self.current_vertex[2] += I4F12::from_bits(z_diff as i16);
        self.process_vertex();
    }

    pub fn set_polygon_attrs(&mut self, data: u32) {
        self.polygon_attrs = PolygonAttrs::from_bits_truncate(data);
        self.lighting.set_enabled(self.polygon_attrs);
    }

}

// Internal processing
impl GeometryEngine {
    fn process_vertex(&mut self) {
        let vertex = Vector::new([
            self.current_vertex[0].into(),
            self.current_vertex[1].into(),
            self.current_vertex[2].into(),
            N::ONE
        ]);

        // Transform the vertex.
        // Result is a I12F12.
        let transformed_vertex = self.matrices.current_clip.mul_vector_4(&vertex);
        // TODO: mask?
        let w = transformed_vertex.elements[3];
        let w2 = w * 2;
        let x = transformed_vertex.elements[0] + w / w2;
        let y = transformed_vertex.elements[1] + w / w2;
        let depth = if true {   // TODO: z-buffer
            let z = transformed_vertex.elements[2] + w / w2;
            z.to_bits()
        } else {
            w.to_bits()
        } as u32;

        // TODO: test clipping?

        self.staged_polygon.push(Vertex {
            screen_x: x.to_num(),
            screen_y: y.to_num(),
            depth: depth,
            colour: self.lighting.get_vertex_colour(),
            tex_s: 0,   // TODO
            tex_t: 0,   // TODO
        });

        if self.should_emit() {
            self.emit_polygon();
        }
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

        true
    }

    /// Output a polygon, and any associated new vertices.
    fn emit_polygon(&mut self) {
        let mut polygon = Polygon {
            attrs:      self.polygon_attrs,
            tex:        self.texture_attrs,
            palette:    self.tex_palette,
            use_quads:  false,
            vertex_index:   0,
        };
        
        match self.primitive.expect("trying to output vertex without calling begin") {
            Primitive::Triangle => {
                for vertex in self.staged_polygon.iter() {
                    let v_idx = self.polygon_ram.insert_vertex(vertex.clone());
                    // TODO: only first
                    polygon.vertex_index = v_idx;
                }
                self.polygon_ram.insert_polygon(polygon);
                self.staged_polygon.resize(3);
            },
            Primitive::TriangleStripFirst => {
                for vertex in self.staged_polygon.iter() {
                    let v_idx = self.polygon_ram.insert_vertex(vertex.clone());
                    // TODO: only first
                    polygon.vertex_index = v_idx;
                }
                self.polygon_ram.insert_polygon(polygon);
                self.primitive = Some(Primitive::TriangleStripReady);
            },
            Primitive::TriangleStripReady => {
                let v_idx = self.polygon_ram.insert_vertex(self.staged_polygon.end().clone());
                polygon.vertex_index = v_idx - 2;
                self.polygon_ram.insert_polygon(polygon);
            },
            Primitive::Quad => {
                polygon.use_quads = true;
                for vertex in self.staged_polygon.iter() {
                    let v_idx = self.polygon_ram.insert_vertex(vertex.clone());
                    // TODO: only first
                    polygon.vertex_index = v_idx;
                }
                self.polygon_ram.insert_polygon(polygon);
                self.staged_polygon.resize(4);
            },
            Primitive::QuadStripFirst => {
                polygon.use_quads = true;
                for vertex in self.staged_polygon.iter() {
                    let v_idx = self.polygon_ram.insert_vertex(vertex.clone());
                    // TODO: only first
                    polygon.vertex_index = v_idx;
                }
                self.polygon_ram.insert_polygon(polygon);
                self.primitive = Some(Primitive::QuadStripBuffer);
            },
            Primitive::QuadStripBuffer => panic!("trying to emit a quad strip polygon when not ready"),
            Primitive::QuadStripReady => {
                polygon.use_quads = true;
                for vertex in self.staged_polygon.iter().skip(2) {
                    let v_idx = self.polygon_ram.insert_vertex(vertex.clone());
                    // TODO: only first
                    polygon.vertex_index = v_idx - 2;
                }
                self.polygon_ram.insert_polygon(polygon);
                self.primitive = Some(Primitive::QuadStripBuffer);
            }
        }
    }
}
