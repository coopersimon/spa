mod types;
mod geometry;
mod render;
mod drawing;

use parking_lot::Mutex;
use std::{sync::Arc, collections::VecDeque};

use crate::utils::{
    meminterface::MemInterface32,
    bits::u32
};

use geometry::GeometryEngine;
pub use render::RenderingEngine;
pub use drawing::Software3DRenderer;

pub struct Video3D {
    command_fifo:           VecDeque<u32>,
    current_command_args:   usize,
    geometry_engine:        GeometryEngine,
    pub rendering_engine:   Arc<Mutex<RenderingEngine>>
}

impl Video3D {
    pub fn new() -> Self {
        Self {
            command_fifo:           VecDeque::with_capacity(256),
            current_command_args:   0,

            geometry_engine:    GeometryEngine::new(),
            rendering_engine:   Arc::new(Mutex::new(RenderingEngine::new()))
        }
    }

    pub fn clock(&mut self, cycles: usize) {
        // TODO: use cycles to process geom commands.
        // TODO: return interrupt if command buffer is empty enough
    }
}

impl MemInterface32 for Video3D {
    fn read_word(&mut self, addr: u32) -> u32 {
        match addr {
            0x0400_0060 => self.rendering_engine.lock().control.bits(),

            _ => panic!("reading invalid gpu address {:X}", addr)
        }
    }

    fn write_word(&mut self, addr: u32, data: u32) {
        match addr {
            0x0400_0060 => self.rendering_engine.lock().write_control(data),

            0x0400_0330..=0x0400_033F => self.rendering_engine.lock().set_edge_colour(((addr & 0xF) / 2) as usize, data),
            0x0400_0340 => self.rendering_engine.lock().set_alpha_test(data),
            0x0400_0350 => self.rendering_engine.lock().set_clear_colour_attr(data),
            0x0400_0354 => self.rendering_engine.lock().set_clear_depth_image(data),
            0x0400_0358 => self.rendering_engine.lock().set_fog_colour(data),
            0x0400_035C => self.rendering_engine.lock().set_fog_offset(data),
            0x0400_0360..=0x0400_037F => self.rendering_engine.lock().set_fog_table(((addr & 0x1F) / 4) as usize, data),
            0x0400_0380..=0x0400_03BF => self.rendering_engine.lock().set_toon_table(((addr & 0x3F) / 2) as usize, data),

            0x0400_0400..=0x0400_043F => self.push_command_buffer(data),              // Command buffer

            0x0400_0440 => self.push_command_cpu(data, 0x10, 1),    // Matrix Mode
            0x0400_0444 => self.push_command_cpu(data, 0x11, 0),    // Push Matrix
            0x0400_0448 => self.push_command_cpu(data, 0x12, 1),    // Pop Matrix
            0x0400_044C => self.push_command_cpu(data, 0x13, 1),    // Store Matrix
            0x0400_0450 => self.push_command_cpu(data, 0x14, 1),    // Restore Matrix
            0x0400_0454 => self.push_command_cpu(data, 0x15, 0),    // Set Identity Matrix
            0x0400_0458 => self.push_command_cpu(data, 0x16, 16),   // Set 4x4 Matrix
            0x0400_045C => self.push_command_cpu(data, 0x17, 12),   // Set 4x3 Matrix
            0x0400_0460 => self.push_command_cpu(data, 0x18, 16),   // Mul 4x4 Matrix
            0x0400_0464 => self.push_command_cpu(data, 0x19, 12),   // Mul 4x3 Matrix
            0x0400_0468 => self.push_command_cpu(data, 0x1A, 9),    // Mul 3x3 Matrix
            0x0400_046C => self.push_command_cpu(data, 0x1B, 3),    // Scale Matrix
            0x0400_0470 => self.push_command_cpu(data, 0x1C, 3),    // Trans Matrix

            0x0400_0480 => self.push_command_cpu(data, 0x20, 1),    // Set vertex colour
            0x0400_0484 => self.push_command_cpu(data, 0x21, 1),    // Set normal
            0x0400_0488 => self.push_command_cpu(data, 0x22, 1),    // Set tex coords

            0x0400_048C => self.push_command_cpu(data, 0x23, 2),    // Set vertex coords (16)
            0x0400_0490 => self.push_command_cpu(data, 0x24, 1),    // Set vertex coords (10)
            0x0400_0494 => self.push_command_cpu(data, 0x25, 1),    // Set vertex coords (xy)
            0x0400_0498 => self.push_command_cpu(data, 0x26, 1),    // Set vertex coords (xz)
            0x0400_049C => self.push_command_cpu(data, 0x27, 1),    // Set vertex coords (yz)
            0x0400_04A0 => self.push_command_cpu(data, 0x28, 1),    // Set vertex coords (diff)

            0x0400_04A4 => self.push_command_cpu(data, 0x29, 1),    // Set polygon attr
            0x0400_04A8 => self.push_command_cpu(data, 0x2A, 1),    // Set tex attr
            0x0400_04AC => self.push_command_cpu(data, 0x2B, 1),    // Set tex palette

            0x0400_04C0 => self.push_command_cpu(data, 0x30, 1),    // Set ambient+diffuse colour
            0x0400_04C4 => self.push_command_cpu(data, 0x31, 1),    // Set emission+specular colour
            0x0400_04C8 => self.push_command_cpu(data, 0x32, 1),    // Set light direction
            0x0400_04CC => self.push_command_cpu(data, 0x33, 1),    // Set light colour
            0x0400_04D0 => self.push_command_cpu(data, 0x34, 32),   // Set specular table

            0x0400_0500 => self.push_command_cpu(data, 0x40, 1),    // Begin vertex list
            0x0400_0504 => self.push_command_cpu(data, 0x41, 0),    // End vertex list

            0x0400_0540 => self.push_command_cpu(data, 0x50, 1),    // Swap buffers
            0x0400_0580 => self.push_command_cpu(data, 0x60, 1),    // Set viewport

            0x0400_05C0 => self.push_command_cpu(data, 0x70, 3),    // Box test
            0x0400_05C4 => self.push_command_cpu(data, 0x71, 2),    // Position test
            0x0400_05C8 => self.push_command_cpu(data, 0x72, 1),    // Vector test

            0x0400_0600 => {},  // TODO: status
            0x0400_0610 => {},  // TODO: 1-dot depth

            // TODO: tests
            _ => panic!("writing invalid gpu address {:X}", addr)
        }
    }
}

impl Video3D {
    fn push_command_buffer(&mut self, data: u32) {
        if self.command_fifo.len() == 256 {
            panic!("GPU command fifo full");   // TODO: handle
        }
        self.command_fifo.push_back(data);
    }

    fn push_command_cpu(&mut self, data: u32, command: u32, num_args: usize) {
        if self.current_command_args > 0 {
            self.push_command_buffer(data);
            self.current_command_args -= 1;
        } else {
            self.push_command_buffer(command);
            if num_args > 0 {
                self.push_command_buffer(data);
                self.current_command_args = num_args - 1;
            }
        }
    }

    fn swap_buffers(&mut self, data: u32) {
        std::mem::swap(
            &mut self.geometry_engine.polygon_ram,
            &mut self.rendering_engine.lock().polygon_ram
        );
        self.geometry_engine.polygon_ram.clear();
        self.geometry_engine.swap_buffers(data);
    }

    /// Do a single command, returning the number of cycles used in the process.
    fn process_command(&mut self) -> usize {
        // Pop command
        // match + process
        0
    }
}