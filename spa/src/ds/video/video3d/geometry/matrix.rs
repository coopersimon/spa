use super::math::*;

// Matrix modes
const PROJ_MODE: u32    = 0b00;
const POS_MODE: u32     = 0b01;
const POS_DIR_MODE: u32 = 0b10;
const TEX_MODE: u32     = 0b11;

#[derive(Default)]
pub struct MatrixUnit {
    mode:   u32,

    current_projection:     Matrix,
    projection_stack:       Matrix,

    pub current_clip:       Matrix,

    current_position:       Matrix,
    pub current_direction:  Matrix,
    position_stack:         [Matrix; 31],
    direction_stack:        [Matrix; 31],
    pos_dir_pointer:        usize,

    pub current_texture:    Matrix,
}

impl MatrixUnit {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_matrix_mode(&mut self, mode: u32) {
        self.mode = mode & 0b11;
    }
    
    pub fn push_matrix(&mut self) {
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
    
    pub fn pop_matrix(&mut self, pops: u32) {
        match self.mode {
            PROJ_MODE => {
                self.current_projection = self.projection_stack.clone();
                self.current_clip = self.current_position.mul(&self.current_projection);
            }
            POS_MODE | POS_DIR_MODE => {
                self.pos_dir_pointer -= pops as usize;  // TODO: signed
                self.current_position = self.position_stack[self.pos_dir_pointer].clone();
                self.current_direction = self.direction_stack[self.pos_dir_pointer].clone();
                self.current_clip = self.current_position.mul(&self.current_projection);
            },
            TEX_MODE => panic!("cannot pop texture matrix"),   // TODO: probably shouldn't panic
            _ => unreachable!()
        }
    }
    
    pub fn store_matrix(&mut self, pos: u32) {
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
    
    pub fn restore_matrix(&mut self, pos: u32) {
        match self.mode {
            PROJ_MODE => {
                self.current_projection = self.projection_stack.clone();
                self.current_clip = self.current_position.mul(&self.current_projection);
            },
            POS_MODE | POS_DIR_MODE => {
                self.current_position = self.position_stack[pos as usize].clone();
                self.current_direction = self.direction_stack[pos as usize].clone();
                self.current_clip = self.current_position.mul(&self.current_projection);
            },
            TEX_MODE => panic!("cannot restore texture matrix"),   // TODO: probably shouldn't panic
            _ => unreachable!()
        }
    }

    pub fn set_current_matrix(&mut self, value: &Matrix) {
        match self.mode {
            PROJ_MODE => {
                self.current_projection = value.clone();
                self.current_clip = self.current_position.mul(&self.current_projection);
            },
            POS_MODE => self.current_position = value.clone(),
            POS_DIR_MODE => {
                self.current_position = value.clone();
                self.current_direction = value.clone();
                self.current_clip = self.current_position.mul(&self.current_projection);
            },
            TEX_MODE => self.current_texture = value.clone(),
            _ => unreachable!()
        }
    }

    pub fn mul_4x4(&mut self, value: &[N]) {
        match self.mode {
            PROJ_MODE => {
                self.current_projection.mul_4x4(value);
                self.current_clip = self.current_position.mul(&self.current_projection);
            },
            POS_MODE => self.current_position.mul_4x4(value),
            POS_DIR_MODE => {
                self.current_position.mul_4x4(value);
                self.current_direction.mul_4x4(value);
                self.current_clip = self.current_position.mul(&self.current_projection);
            },
            TEX_MODE => self.current_texture.mul_4x4(value),
            _ => unreachable!()
        }
    }
    
    pub fn mul_4x3(&mut self, value: &[N]) {
        match self.mode {
            PROJ_MODE => {
                self.current_projection.mul_4x3(value);
                self.current_clip = self.current_position.mul(&self.current_projection);
            },
            POS_MODE => self.current_position.mul_4x3(value),
            POS_DIR_MODE => {
                self.current_position.mul_4x3(value);
                self.current_direction.mul_4x3(value);
                self.current_clip = self.current_position.mul(&self.current_projection);
            },
            TEX_MODE => self.current_texture.mul_4x3(value),
            _ => unreachable!()
        }
    }

    pub fn mul_3x3(&mut self, value: &[N]) {
        match self.mode {
            PROJ_MODE => {
                self.current_projection.mul_3x3(value);
                self.current_clip = self.current_position.mul(&self.current_projection);
            },
            POS_MODE => self.current_position.mul_3x3(value),
            POS_DIR_MODE => {
                self.current_position.mul_3x3(value);
                self.current_direction.mul_3x3(value);
                self.current_clip = self.current_position.mul(&self.current_projection);
            },
            TEX_MODE => self.current_texture.mul_3x3(value),
            _ => unreachable!()
        }
    }

    pub fn mul_scale(&mut self, value: &[N]) {
        match self.mode {
            PROJ_MODE => {
                self.current_projection.mul_scale(value);
                self.current_clip = self.current_position.mul(&self.current_projection);
            }
            POS_MODE | POS_DIR_MODE => {
                self.current_position.mul_scale(value);
                self.current_clip = self.current_position.mul(&self.current_projection);
            },
            TEX_MODE => self.current_texture.mul_scale(value),
            _ => unreachable!()
        }
    }

    pub fn mul_trans(&mut self, value: &[N]) {
        match self.mode {
            PROJ_MODE => {
                self.current_projection.mul_trans(value);
                self.current_clip = self.current_position.mul(&self.current_projection);
            },
            POS_MODE => self.current_position.mul_trans(value),
            POS_DIR_MODE => {
                self.current_position.mul_trans(value);
                self.current_direction.mul_trans(value);
                self.current_clip = self.current_position.mul(&self.current_projection);
            },
            TEX_MODE => self.current_texture.mul_trans(value),
            _ => unreachable!()
        }
    }
}
