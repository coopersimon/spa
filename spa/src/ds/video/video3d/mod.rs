mod matrix;

use matrix::*;

use bitflags::bitflags;
use crate::{
    utils::{
        meminterface::MemInterface32,
        bits::u32,
        bytes
    },
    common::colour::Colour
};

bitflags! {
    #[derive(Default)]
    struct PolygonAttrs: u32 {
        const POLYGON_ID        = u32::bits(24, 29);
        const ALPHA             = u32::bits(16, 20);
        const FOG_BLEND_ENABLE  = u32::bit(15);
        const RENDER_EQ_DEPTH   = u32::bit(14);
        const RENDER_DOT        = u32::bit(13);
        const FAR_PLANE_CLIP    = u32::bit(12);
        const ALPHA_DEPTH       = u32::bit(11);
        const RENDER_FRONT      = u32::bit(7);
        const RENDER_BACK       = u32::bit(6);
        const POLYGON_MODE      = u32::bits(4, 5);
        const ENABLE_LIGHT_3    = u32::bit(3);
        const ENABLE_LIGHT_2    = u32::bit(2);
        const ENABLE_LIGHT_1    = u32::bit(1);
        const ENABLE_LIGHT_0    = u32::bit(0);
    }
}

pub struct GeometryEngine {
    input_buffer:   Vec<N>,
    matrices:       Box<MatrixUnit>,
    lighting:       Box<LightingUnit>,
    polygon_attrs:  PolygonAttrs,
}

impl GeometryEngine {
    pub fn new() -> Self {
        Self {
            input_buffer:   Vec::new(),
            matrices:       Box::new(MatrixUnit::new()),
            lighting:       Box::new(LightingUnit::new()),
            polygon_attrs:  PolygonAttrs::default()
        }
    }
}

impl MemInterface32 for GeometryEngine {
    fn read_word(&mut self, addr: u32) -> u32 {
        match addr {
            _ => panic!("reading invalid gpu address {:X}", addr)
        }
    }

    fn write_word(&mut self, addr: u32, data: u32) {
        match addr {
            0x0400_0440 => self.matrices.set_matrix_mode(data),
            0x0400_0444 => self.matrices.push_matrix(),
            0x0400_0448 => self.matrices.pop_matrix(data & 0x3F),
            0x0400_044C => self.matrices.store_matrix(data & 0x1F),
            0x0400_0450 => self.matrices.restore_matrix(data & 0x1F),
            0x0400_0454 => self.set_identity_matrix(),
            0x0400_0458 => self.set_4x4_matrix(data),
            0x0400_045C => self.set_4x3_matrix(data),
            0x0400_0460 => self.mul_4x4(data),
            0x0400_0464 => self.mul_4x3(data),
            0x0400_0468 => self.mul_3x3(data),
            0x0400_046C => self.mul_scale(data),
            0x0400_0470 => self.mul_trans(data),

            0x0400_0480 => self.set_vertex_colour(data),
            0x0400_0484 => self.set_normal(data),

            0x0400_04A4 => self.set_polygon_attrs(data),

            0x0400_04C0 => self.lighting.set_dif_amb_colour(data),
            0x0400_04C4 => self.lighting.set_spe_emi_colour(data),
            0x0400_04C8 => self.set_light_direction(data),
            0x0400_04CC => self.lighting.set_light_colour(data),
            0x0400_04D0 => self.lighting.set_specular_table(data),

            0x0400_0540 => {},  // TODO: swap buffers
            0x0400_0580 => {},  // TODO: viewport
            _ => panic!("writing invalid gpu address {:X}", addr)
        }
    }
}

// GPU commands
impl GeometryEngine {
    fn set_identity_matrix(&mut self) {
        self.matrices.set_current_matrix(&Matrix::identity());
    }

    fn set_4x4_matrix(&mut self, data: u32) {
        self.input_buffer.push(N::from_bits(data as i32));
        if self.input_buffer.len() == 16 {
            self.matrices.set_current_matrix(&Matrix::from_4x4(&self.input_buffer));
            self.input_buffer.clear();
        }
    }
    
    fn set_4x3_matrix(&mut self, data: u32) {
        self.input_buffer.push(N::from_bits(data as i32));
        if self.input_buffer.len() == 12 {
            self.matrices.set_current_matrix(&Matrix::from_4x3(&self.input_buffer));
            self.input_buffer.clear();
        }
    }

    fn mul_4x4(&mut self, data: u32) {
        self.input_buffer.push(N::from_bits(data as i32));
        if self.input_buffer.len() == 16 {
            self.matrices.mul_4x4(&self.input_buffer);
            self.input_buffer.clear();
        }
    }
    
    fn mul_4x3(&mut self, data: u32) {
        self.input_buffer.push(N::from_bits(data as i32));
        if self.input_buffer.len() == 12 {
            self.matrices.mul_4x3(&self.input_buffer);
            self.input_buffer.clear();
        }
    }
    
    fn mul_3x3(&mut self, data: u32) {
        self.input_buffer.push(N::from_bits(data as i32));
        if self.input_buffer.len() == 9 {
            self.matrices.mul_3x3(&self.input_buffer);
            self.input_buffer.clear();
        }
    }
    
    fn mul_scale(&mut self, data: u32) {
        self.input_buffer.push(N::from_bits(data as i32));
        if self.input_buffer.len() == 3 {
            self.matrices.mul_scale(&self.input_buffer);
            self.input_buffer.clear();
        }
    }
    
    fn mul_trans(&mut self, data: u32) {
        self.input_buffer.push(N::from_bits(data as i32));
        if self.input_buffer.len() == 3 {
            self.matrices.mul_trans(&self.input_buffer);
            self.input_buffer.clear();
        }
    }

    fn set_vertex_colour(&mut self, data: u32) {
        self.lighting.set_vertex_colour(data);
    }

    fn set_light_direction(&mut self, data: u32) {
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

    fn set_normal(&mut self, data: u32) {
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

    fn set_polygon_attrs(&mut self, data: u32) {
        self.polygon_attrs = PolygonAttrs::from_bits_truncate(data);
        self.lighting.set_enabled(self.polygon_attrs);
    }
}

// Matrix modes
const PROJ_MODE: u32    = 0b00;
const POS_MODE: u32     = 0b01;
const POS_DIR_MODE: u32 = 0b10;
const TEX_MODE: u32     = 0b11;

#[derive(Default)]
struct MatrixUnit {
    mode:   u32,
    current_projection: Matrix,
    projection_stack:   Matrix,
    current_position:   Matrix,
    current_direction:  Matrix,
    position_stack:     [Matrix; 31],
    direction_stack:    [Matrix; 31],
    pos_dir_pointer:    usize,
    current_texture:    Matrix,
}

impl MatrixUnit {
    fn new() -> Self {
        Self::default()
    }

    fn set_matrix_mode(&mut self, mode: u32) {
        self.mode = mode & 0b11;
    }
    
    fn push_matrix(&mut self) {
        match self.mode {
            PROJ_MODE => self.projection_stack = self.current_projection.clone(),
            POS_MODE | POS_DIR_MODE => {
                self.position_stack[self.pos_dir_pointer] = self.current_position.clone();
                self.direction_stack[self.pos_dir_pointer] = self.current_direction.clone();
                self.pos_dir_pointer += 1;
            },
            TEX_MODE => panic!("cannot push texture matrix"),   // TODO: probably shouldn't panic
            _ => unreachable!()
        }
    }
    
    fn pop_matrix(&mut self, pops: u32) {
        match self.mode {
            PROJ_MODE => self.current_projection = self.projection_stack.clone(),
            POS_MODE | POS_DIR_MODE => {
                self.pos_dir_pointer - (pops as usize);
                self.current_position = self.position_stack[self.pos_dir_pointer].clone();
                self.current_direction = self.direction_stack[self.pos_dir_pointer].clone();
            },
            TEX_MODE => panic!("cannot pop texture matrix"),   // TODO: probably shouldn't panic
            _ => unreachable!()
        }
    }
    
    fn store_matrix(&mut self, pos: u32) {
        match self.mode {
            PROJ_MODE => self.projection_stack = self.current_projection.clone(),
            POS_MODE | POS_DIR_MODE => {
                self.position_stack[pos as usize] = self.current_position.clone();
                self.direction_stack[pos as usize] = self.current_direction.clone();
            },
            TEX_MODE => panic!("cannot store texture matrix"),   // TODO: probably shouldn't panic
            _ => unreachable!()
        }
    }
    
    fn restore_matrix(&mut self, pos: u32) {
        match self.mode {
            PROJ_MODE => self.current_projection = self.projection_stack.clone(),
            POS_MODE | POS_DIR_MODE => {
                self.current_position = self.position_stack[pos as usize].clone();
                self.current_direction = self.direction_stack[pos as usize].clone();
            },
            TEX_MODE => panic!("cannot restore texture matrix"),   // TODO: probably shouldn't panic
            _ => unreachable!()
        }
    }

    fn set_current_matrix(&mut self, value: &Matrix) {
        match self.mode {
            PROJ_MODE => self.current_projection = value.clone(),
            POS_MODE => self.current_position = value.clone(),
            POS_DIR_MODE => {
                self.current_position = value.clone();
                self.current_direction = value.clone();
            },
            TEX_MODE => self.current_texture = value.clone(),
            _ => unreachable!()
        }
    }

    fn mul_4x4(&mut self, value: &[N]) {
        match self.mode {
            PROJ_MODE => self.current_projection.mul_4x4(value),
            POS_MODE => self.current_position.mul_4x4(value),
            POS_DIR_MODE => {
                self.current_position.mul_4x4(value);
                self.current_direction.mul_4x4(value);
            },
            TEX_MODE => self.current_texture.mul_4x4(value),
            _ => unreachable!()
        }
    }
    
    fn mul_4x3(&mut self, value: &[N]) {
        match self.mode {
            PROJ_MODE => self.current_projection.mul_4x3(value),
            POS_MODE => self.current_position.mul_4x3(value),
            POS_DIR_MODE => {
                self.current_position.mul_4x3(value);
                self.current_direction.mul_4x3(value);
            },
            TEX_MODE => self.current_texture.mul_4x3(value),
            _ => unreachable!()
        }
    }

    fn mul_3x3(&mut self, value: &[N]) {
        match self.mode {
            PROJ_MODE => self.current_projection.mul_3x3(value),
            POS_MODE => self.current_position.mul_3x3(value),
            POS_DIR_MODE => {
                self.current_position.mul_3x3(value);
                self.current_direction.mul_3x3(value);
            },
            TEX_MODE => self.current_texture.mul_3x3(value),
            _ => unreachable!()
        }
    }

    fn mul_scale(&mut self, value: &[N]) {
        match self.mode {
            PROJ_MODE => self.current_projection.mul_scale(value),
            POS_MODE | POS_DIR_MODE => self.current_position.mul_scale(value),
            TEX_MODE => self.current_texture.mul_scale(value),
            _ => unreachable!()
        }
    }

    fn mul_trans(&mut self, value: &[N]) {
        match self.mode {
            PROJ_MODE => self.current_projection.mul_trans(value),
            POS_MODE => self.current_position.mul_trans(value),
            POS_DIR_MODE => {
                self.current_position.mul_trans(value);
                self.current_direction.mul_trans(value);
            },
            TEX_MODE => self.current_texture.mul_trans(value),
            _ => unreachable!()
        }
    }
}

#[derive(Default)]
struct Light {
    direction:  Vector<3>,
    half_angle: Vector<3>,
    colour:     Colour,
    enabled:    bool,
}

#[derive(Default)]
struct LightingUnit {
    lights:             [Light; 4],

    /// Current vertex colour.
    vertex_colour:      Colour,

    diffuse_colour:     Colour,
    ambient_colour:     Colour,
    specular_colour:    Colour,
    emission_colour:    Colour,

    enable_table:       bool,
    specular_table:     Vec<u8>,
    specular_index:     usize
}

impl LightingUnit {
    fn new() -> Self {
        Self {
            specular_table: vec![0; 128],
            ..Default::default()
        }
    }

    /// Calculate colour.
    fn set_normal(&mut self, normal: Vector<3>) {
        self.vertex_colour = self.emission_colour;

        for light in &self.lights {
            if !light.enabled {
                continue;
            }
            let diffuse = N::max(N::ZERO, -normal.dot_product(&light.direction));
            let diffuse_weight = diffuse.to_num::<i32>() as u8;
            let diffuse_colour = light.colour.mul(&self.diffuse_colour).weight(diffuse_weight);

            let ambient_colour = light.colour.mul(&self.ambient_colour);

            let specular_angle_cos = N::max(N::ZERO, normal.dot_product(&light.half_angle));
            let specular_weight = if self.enable_table {
                let table_idx = (specular_angle_cos.to_num::<i32>() % 128) as usize;
                self.specular_table[table_idx]
            } else {
                specular_angle_cos.to_num::<i32>() as u8
            };
            let specular_colour = light.colour.mul(&self.specular_colour).weight(specular_weight);

            self.vertex_colour.add(&diffuse_colour);
            self.vertex_colour.add(&ambient_colour);
            self.vertex_colour.add(&specular_colour);
        }
    }

    fn set_vertex_colour(&mut self, colour: u32) {
        self.vertex_colour = Colour::from_555(bytes::u32::lo(colour));
    }

    fn set_light_direction(&mut self, light: usize, direction: Vector<3>) {
        self.lights[light].direction = direction.clone();
        self.lights[light].half_angle = Vector::new([
            -direction.elements[0] >> 1,
            -direction.elements[1] >> 1,
            -(direction.elements[2] - N::ONE) >> 1
        ]);
    }

    fn set_light_colour(&mut self, data: u32) {
        let light = (data >> 30) as usize;
        self.lights[light].colour = Colour::from_555(bytes::u32::lo(data));
    }

    fn set_dif_amb_colour(&mut self, data: u32) {
        self.diffuse_colour = Colour::from_555(bytes::u32::lo(data));
        self.ambient_colour = Colour::from_555(bytes::u32::hi(data));
        if u32::test_bit(data, 15) {
            self.vertex_colour = self.diffuse_colour;
        }
    }
    
    fn set_spe_emi_colour(&mut self, data: u32) {
        self.specular_colour = Colour::from_555(bytes::u32::lo(data));
        self.emission_colour = Colour::from_555(bytes::u32::hi(data));
        self.enable_table = u32::test_bit(data, 15);
    }

    fn set_specular_table(&mut self, data: u32) {
        for (table, input) in self.specular_table.iter_mut().skip(self.specular_index).zip(&data.to_le_bytes()) {
            *table = *input;
        }
        self.specular_index = (self.specular_index + 4) % 128;
    }

    fn set_enabled(&mut self, attrs: PolygonAttrs) {
        self.lights[0].enabled = attrs.contains(PolygonAttrs::ENABLE_LIGHT_0);
        self.lights[1].enabled = attrs.contains(PolygonAttrs::ENABLE_LIGHT_1);
        self.lights[2].enabled = attrs.contains(PolygonAttrs::ENABLE_LIGHT_2);
        self.lights[3].enabled = attrs.contains(PolygonAttrs::ENABLE_LIGHT_3);
    }
}
