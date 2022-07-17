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



pub struct GeometryEngine {
    input_buffer: Vec<N>,
    matrices:   Box<MatrixUnit>,
    lighting:   Box<LightingUnit>
}

impl GeometryEngine {
    pub fn new() -> Self {
        Self {
            input_buffer:   Vec::new(),
            matrices:       Box::new(MatrixUnit::new()),
            lighting:       Box::new(LightingUnit::new())
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

            0x0400_04C0 => self.lighting.set_dif_amb_colour(data),
            0x0400_04C4 => self.lighting.set_spe_emi_colour(data),
            0x0400_04C8 => self.lighting.set_light_direction(data),
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
    x: u16,
    y: u16,
    z: u16,
    // TODO: Colour param?
    colour: Colour
}

#[derive(Default)]
struct LightingUnit {
    lights:             [Light; 4],

    diffuse_colour:     Colour,
    ambient_colour:     Colour,
    specular_colour:    Colour,
    emission_colour:    Colour,

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

    fn set_light_direction(&mut self, data: u32) {
        let light = (data >> 30) as usize;
        self.lights[light].x = (data & 0x3FF) as u16;
        self.lights[light].y = ((data >> 10) & 0x3FF) as u16;
        self.lights[light].z = ((data >> 20) & 0x3FF) as u16;
    }

    fn set_light_colour(&mut self, data: u32) {
        let light = (data >> 30) as usize;
        self.lights[light].colour = Colour::from_555(bytes::u32::lo(data));
    }

    fn set_dif_amb_colour(&mut self, data: u32) {
        self.diffuse_colour = Colour::from_555(bytes::u32::lo(data));
        self.ambient_colour = Colour::from_555(bytes::u32::hi(data));
    }
    
    fn set_spe_emi_colour(&mut self, data: u32) {
        self.specular_colour = Colour::from_555(bytes::u32::lo(data));
        self.emission_colour = Colour::from_555(bytes::u32::hi(data));
    }

    fn set_specular_table(&mut self, data: u32) {
        for (table, input) in self.specular_table.iter_mut().skip(self.specular_index).zip(&data.to_le_bytes()) {
            *table = *input;
        }
        self.specular_index = (self.specular_index + 4) % 128;
    }
}
