use crate::utils::bits::u32;
use super::math::*;

// Matrix modes
const PROJ_MODE: u32    = 0b00;
const POS_MODE: u32     = 0b01;
const POS_DIR_MODE: u32 = 0b10;
const TEX_MODE: u32     = 0b11;

#[derive(Default)]
pub struct MatrixUnit {
    old_mode:       u32,
    mode:           u32,
    /// Set when over/underflow occurs
    stack_error:    bool,

    pub current_projection: Matrix,
    projection_stack:   Matrix,
    proj_pointer:       usize,

    current_clip:       Matrix,

    pub current_position:   Matrix,
    current_direction:  Matrix,
    position_stack:     [Matrix; 31],
    direction_stack:    [Matrix; 31],
    pos_dir_pointer:    usize,

    current_texture:    Matrix,
    tex_stack:          Matrix,

    pub capture: bool,
}

impl MatrixUnit {
    pub fn new() -> Self {
        Self::default()
    }

    // Status

    pub fn has_stack_error(&self) -> bool {
        self.stack_error
    }

    pub fn clear_stack_error(&mut self) {
        self.stack_error = false;
    }

    pub fn proj_stack_level(&self) -> u32 {
        self.proj_pointer as u32
    }

    pub fn pos_dir_stack_level(&self) -> u32 {
        self.pos_dir_pointer as u32
    }

    // Get current matrices

    pub fn dir_matrix<'a>(&'a self) -> &'a Matrix {
        &self.current_direction
    }
    
    pub fn clip_matrix<'a>(&'a self) -> &'a Matrix {
        &self.current_clip
    }
    
    pub fn tex_matrix<'a>(&'a self) -> &'a Matrix {
        &self.current_texture
    }
}

// Commands
impl MatrixUnit {
    pub fn set_matrix_mode(&mut self, mode: u32) -> isize {
        self.old_mode = self.mode;
        self.mode = mode & 0b11;
        1
    }
    
    pub fn push_matrix(&mut self) -> isize {
        match self.mode {
            PROJ_MODE => {
                self.projection_stack = self.current_projection.clone();
                self.proj_pointer = 1;
                if self.capture {
                    println!("pushed proj {}", self.proj_pointer);
                }
            },
            POS_MODE | POS_DIR_MODE => {
                self.position_stack[self.pos_dir_pointer] = self.current_position.clone();
                self.direction_stack[self.pos_dir_pointer] = self.current_direction.clone();
                self.pos_dir_pointer += 1;
                if self.capture {
                    println!("pushed posdir {}", self.pos_dir_pointer);
                }
            },
            TEX_MODE => println!("cannot push texture matrix"),   // TODO: probably shouldn't panic
            _ => unreachable!()
        }
        17
    }
    
    pub fn pop_matrix(&mut self, pops: u32) -> isize {
        let signed_pops = u32::sign_extend(pops, 6);
        match self.mode {
            PROJ_MODE => {
                self.proj_pointer = 0;
                self.current_projection = self.projection_stack.clone();
                self.current_clip = self.current_position.mul(&self.current_projection);
                if self.capture {
                    println!("pop proj {} => {}", pops, self.proj_pointer);
                }
            },
            POS_MODE | POS_DIR_MODE => {
                let new_pointer = (self.pos_dir_pointer as isize) - (signed_pops as isize);
                self.pos_dir_pointer = (new_pointer as usize) % 31; // TODO: unsure
                self.current_position = self.position_stack[self.pos_dir_pointer].clone();
                self.current_direction = self.direction_stack[self.pos_dir_pointer].clone();
                self.current_clip = self.current_position.mul(&self.current_projection);
                if self.capture {
                    println!("pop posdir {} => {}", pops, new_pointer);
                }
            },
            TEX_MODE => println!("cannot pop texture matrix"),   // TODO: probably shouldn't panic
            _ => unreachable!()
        }
        36
    }
    
    pub fn store_matrix(&mut self, pos: u32) -> isize {
        match self.mode {
            PROJ_MODE => {
                if self.capture {
                    println!("store proj");
                }
                self.projection_stack = self.current_projection.clone()
            },
            POS_MODE | POS_DIR_MODE => {
                if self.capture {
                    println!("store POS {}", pos);
                }
                self.position_stack[pos as usize] = self.current_position.clone();
                self.direction_stack[pos as usize] = self.current_direction.clone();
            },
            TEX_MODE => self.current_texture = self.current_direction.clone(),//println!("cannot store texture matrix {}", pos),   // TODO: probably shouldn't panic
            _ => unreachable!()
        }
        17
    }
    
    pub fn restore_matrix(&mut self, pos: u32) -> isize {
        match self.mode {
            PROJ_MODE => {
                if self.capture {
                    println!("restore proj");
                }
                self.current_projection = self.projection_stack.clone();
                self.current_clip = self.current_position.mul(&self.current_projection);
            },
            POS_MODE | POS_DIR_MODE => {
                if self.capture {
                    println!("restore POS {}", pos);
                }
                self.current_position = self.position_stack[pos as usize].clone();
                self.current_direction = self.direction_stack[pos as usize].clone();
                self.current_clip = self.current_position.mul(&self.current_projection);
            },
            TEX_MODE => (),//println!("cannot restore texture matrix {}", pos),   // TODO: probably shouldn't panic
            _ => unreachable!()
        }
        36
    }

    pub fn set_identity(&mut self) -> isize {
        match self.mode {
            PROJ_MODE => {
                self.current_projection = Matrix::identity();
                self.current_clip = self.current_position.mul(&self.current_projection);
            },
            POS_MODE => {
                self.current_position = Matrix::identity();
                self.current_clip = self.current_position.mul(&self.current_projection);
            },
            POS_DIR_MODE => {
                self.current_position = Matrix::identity();
                self.current_direction = Matrix::identity();
                self.current_clip = self.current_position.mul(&self.current_projection);
            },
            TEX_MODE => {
                self.current_texture = Matrix::identity();
            },
            _ => unreachable!()
        }
        19
    }
    
    pub fn set_4x4(&mut self, value: &[N]) -> isize {
        match self.mode {
            PROJ_MODE => {
                self.current_projection = Matrix::from_4x4(value);
                self.current_clip = self.current_position.mul(&self.current_projection);
            },
            POS_MODE => {
                self.current_position = Matrix::from_4x4(value);
                self.current_clip = self.current_position.mul(&self.current_projection);
            },
            POS_DIR_MODE => {
                self.current_position = Matrix::from_4x4(value);
                self.current_direction = Matrix::from_4x4(value);
                self.current_clip = self.current_position.mul(&self.current_projection);
            },
            TEX_MODE => {
                self.current_texture = Matrix::from_4x4(value);
            },
            _ => unreachable!()
        }
        34
    }
    
    pub fn set_4x3(&mut self, value: &[N]) -> isize {
        match self.mode {
            PROJ_MODE => {
                self.current_projection = Matrix::from_4x3(value);
                self.current_clip = self.current_position.mul(&self.current_projection);
            },
            POS_MODE => {
                self.current_position = Matrix::from_4x3(value);
                self.current_clip = self.current_position.mul(&self.current_projection);
            },
            POS_DIR_MODE => {
                self.current_position = Matrix::from_4x3(value);
                self.current_direction = Matrix::from_4x3(value);
                self.current_clip = self.current_position.mul(&self.current_projection);
            },
            TEX_MODE => {
                self.current_texture = Matrix::from_4x3(value);
            },
            _ => unreachable!()
        }
        30
    }

    pub fn mul_4x4(&mut self, value: &[N]) -> isize {
        match self.mode {
            PROJ_MODE => {
                self.current_projection.mul_4x4(value);
                self.current_clip = self.current_position.mul(&self.current_projection);
                35
            },
            POS_MODE => {
                self.current_position.mul_4x4(value);
                self.current_clip = self.current_position.mul(&self.current_projection);
                35
            },
            POS_DIR_MODE => {
                self.current_position.mul_4x4(value);
                self.current_direction.mul_4x4(value);
                self.current_clip = self.current_position.mul(&self.current_projection);
                65
            },
            TEX_MODE => {
                self.current_texture.mul_4x4(value);
                35
            },
            _ => unreachable!()
        }
    }
    
    pub fn mul_4x3(&mut self, value: &[N]) -> isize {
        match self.mode {
            PROJ_MODE => {
                self.current_projection.mul_4x3(value);
                self.current_clip = self.current_position.mul(&self.current_projection);
                31
            },
            POS_MODE => {
                self.current_position.mul_4x3(value);
                self.current_clip = self.current_position.mul(&self.current_projection);
                31
            },
            POS_DIR_MODE => {
                self.current_position.mul_4x3(value);
                self.current_direction.mul_4x3(value);
                self.current_clip = self.current_position.mul(&self.current_projection);
                61
            },
            TEX_MODE => {
                self.current_texture.mul_4x3(value);
                31
            },
            _ => unreachable!()
        }
    }

    pub fn mul_3x3(&mut self, value: &[N]) -> isize {
        match self.mode {
            PROJ_MODE => {
                self.current_projection.mul_3x3(value);
                self.current_clip = self.current_position.mul(&self.current_projection);
                28
            },
            POS_MODE => {
                self.current_position.mul_3x3(value);
                self.current_clip = self.current_position.mul(&self.current_projection);
                28
            },
            POS_DIR_MODE => {
                self.current_position.mul_3x3(value);
                self.current_direction.mul_3x3(value);
                self.current_clip = self.current_position.mul(&self.current_projection);
                58
            },
            TEX_MODE => {
                self.current_texture.mul_3x3(value);
                28
            },
            _ => unreachable!()
        }
    }

    pub fn mul_scale(&mut self, value: &[N]) -> isize {
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
        22
    }

    pub fn mul_trans(&mut self, value: &[N]) -> isize {
        match self.mode {
            PROJ_MODE => {
                self.current_projection.mul_trans(value);
                self.current_clip = self.current_position.mul(&self.current_projection);
                22
            },
            POS_MODE => {
                self.current_position.mul_trans(value);
                self.current_clip = self.current_position.mul(&self.current_projection);
                22
            },
            POS_DIR_MODE => {
                self.current_position.mul_trans(value);
                self.current_direction.mul_trans(value);
                self.current_clip = self.current_position.mul(&self.current_projection);
                52
            },
            TEX_MODE => {
                self.current_texture.mul_trans(value);
                22
            },
            _ => unreachable!()
        }
    }
}
