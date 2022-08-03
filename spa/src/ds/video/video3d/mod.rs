mod types;
mod geometry;
mod render;
mod drawing;
mod commandfifo;

use bitflags::bitflags;
use parking_lot::Mutex;
use std::sync::Arc;

use crate::{utils::{
    meminterface::MemInterface32,
    bits::u32
}, ds::interrupt::Interrupts};

use commandfifo::GeomCommandFifo;
use geometry::{GeometryEngine, N};
pub use render::RenderingEngine;
pub use drawing::Software3DRenderer;

bitflags! {
    #[derive(Default)]
    pub struct GeometryEngineStatus: u32 {
        const CMD_FIFO_INT          = u32::bits(30, 31);
        const GEOM_BUSY             = u32::bit(27);
        const CMD_FIFO_EMPTY        = u32::bit(26);
        const CMD_FIFO_UNDER_HALF   = u32::bit(25);
        const CMD_FIFO_FULL         = u32::bit(24);
        const CMD_FIFO_COUNT        = u32::bits(16, 23);
        const MAT_STACK_ERROR       = u32::bit(15);
        const MAT_STACK_BUSY        = u32::bit(14);
        const PROJ_MAT_STACK_LEVEL  = u32::bit(13);
        const POS_MAT_STACK_LEVEL   = u32::bits(8, 12);
        const TEST_BOX_RESULT       = u32::bit(1);
        const TEST_BUSY             = u32::bit(0);
    }
}

pub struct Video3D {
    geom_command_fifo:      GeomCommandFifo,
    current_commands:       u32,
    pending_swap:           Option<u32>,

    geometry_engine:        GeometryEngine,
    cycle_count:            isize,

    pub rendering_engine:   Arc<Mutex<RenderingEngine>>
}

impl Video3D {
    pub fn new() -> Self {
        Self {
            geom_command_fifo:      GeomCommandFifo::new(),
            current_commands:       0,
            pending_swap:           None,

            geometry_engine:        GeometryEngine::new(),
            cycle_count:            0,

            rendering_engine:   Arc::new(Mutex::new(RenderingEngine::new()))
        }
    }

    pub fn clock(&mut self, cycles: usize) -> Interrupts {
        self.cycle_count += cycles as isize;
        while self.cycle_count > 0 {
            self.cycle_count -= self.process_command();
        }

        if self.geom_command_fifo.interrupt() {
            Interrupts::GEOM_FIFO
        } else {
            Interrupts::empty()
        }
    }

    pub fn on_vblank(&mut self) {
        if let Some(swap_data) = self.pending_swap {
            self.pending_swap = None;
            self.cycle_count -= self.swap_buffers(swap_data);
        }
    }
}

impl MemInterface32 for Video3D {
    fn read_word(&mut self, addr: u32) -> u32 {
        match addr {
            0x0400_0060 => self.rendering_engine.lock().control.bits(),

            0x0400_0320 => 46,   // TODO: Rendered line count

            0x0400_0600 => self.get_geom_engine_status().bits(),
            0x0400_0604 => 0,   // POLY+VTX COUNT
            0x0400_0620..=0x0400_0634 => 0, // test result

            0x0400_0640..=0x0400_067F => self.read_clip_matrix((addr / 4) % 16),
            0x0400_0680..=0x0400_068B => self.read_dir_matrix((addr / 4) % 4),      // 3x3 first row
            0x0400_068C..=0x0400_0697 => self.read_dir_matrix(4 + (addr / 4) % 4),  // 3x3 second row
            0x0400_0698..=0x0400_06A3 => self.read_dir_matrix(8 + (addr / 4) % 4),  // 3x3 third row

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

            0x0400_0400..=0x0400_043F => self.geom_command_fifo.push_command_buffer(data),              // Command buffer

            0x0400_0440 => self.geom_command_fifo.push_command_cpu(data, 0x10, 1),    // Matrix Mode
            0x0400_0444 => self.geom_command_fifo.push_command_cpu(data, 0x11, 0),    // Push Matrix
            0x0400_0448 => self.geom_command_fifo.push_command_cpu(data, 0x12, 1),    // Pop Matrix
            0x0400_044C => self.geom_command_fifo.push_command_cpu(data, 0x13, 1),    // Store Matrix
            0x0400_0450 => self.geom_command_fifo.push_command_cpu(data, 0x14, 1),    // Restore Matrix
            0x0400_0454 => self.geom_command_fifo.push_command_cpu(data, 0x15, 0),    // Set Identity Matrix
            0x0400_0458 => self.geom_command_fifo.push_command_cpu(data, 0x16, 16),   // Set 4x4 Matrix
            0x0400_045C => self.geom_command_fifo.push_command_cpu(data, 0x17, 12),   // Set 4x3 Matrix
            0x0400_0460 => self.geom_command_fifo.push_command_cpu(data, 0x18, 16),   // Mul 4x4 Matrix
            0x0400_0464 => self.geom_command_fifo.push_command_cpu(data, 0x19, 12),   // Mul 4x3 Matrix
            0x0400_0468 => self.geom_command_fifo.push_command_cpu(data, 0x1A, 9),    // Mul 3x3 Matrix
            0x0400_046C => self.geom_command_fifo.push_command_cpu(data, 0x1B, 3),    // Scale Matrix
            0x0400_0470 => self.geom_command_fifo.push_command_cpu(data, 0x1C, 3),    // Trans Matrix

            0x0400_0480 => self.geom_command_fifo.push_command_cpu(data, 0x20, 1),    // Set vertex colour
            0x0400_0484 => self.geom_command_fifo.push_command_cpu(data, 0x21, 1),    // Set normal
            0x0400_0488 => self.geom_command_fifo.push_command_cpu(data, 0x22, 1),    // Set tex coords

            0x0400_048C => self.geom_command_fifo.push_command_cpu(data, 0x23, 2),    // Set vertex coords (16)
            0x0400_0490 => self.geom_command_fifo.push_command_cpu(data, 0x24, 1),    // Set vertex coords (10)
            0x0400_0494 => self.geom_command_fifo.push_command_cpu(data, 0x25, 1),    // Set vertex coords (xy)
            0x0400_0498 => self.geom_command_fifo.push_command_cpu(data, 0x26, 1),    // Set vertex coords (xz)
            0x0400_049C => self.geom_command_fifo.push_command_cpu(data, 0x27, 1),    // Set vertex coords (yz)
            0x0400_04A0 => self.geom_command_fifo.push_command_cpu(data, 0x28, 1),    // Set vertex coords (diff)

            0x0400_04A4 => self.geom_command_fifo.push_command_cpu(data, 0x29, 1),    // Set polygon attr
            0x0400_04A8 => self.geom_command_fifo.push_command_cpu(data, 0x2A, 1),    // Set tex attr
            0x0400_04AC => self.geom_command_fifo.push_command_cpu(data, 0x2B, 1),    // Set tex palette

            0x0400_04C0 => self.geom_command_fifo.push_command_cpu(data, 0x30, 1),    // Set ambient+diffuse colour
            0x0400_04C4 => self.geom_command_fifo.push_command_cpu(data, 0x31, 1),    // Set emission+specular colour
            0x0400_04C8 => self.geom_command_fifo.push_command_cpu(data, 0x32, 1),    // Set light direction
            0x0400_04CC => self.geom_command_fifo.push_command_cpu(data, 0x33, 1),    // Set light colour
            0x0400_04D0 => self.geom_command_fifo.push_command_cpu(data, 0x34, 32),   // Set specular table

            0x0400_0500 => self.geom_command_fifo.push_command_cpu(data, 0x40, 1),    // Begin vertex list
            0x0400_0504 => self.geom_command_fifo.push_command_cpu(data, 0x41, 0),    // End vertex list

            0x0400_0540 => self.geom_command_fifo.push_command_cpu(data, 0x50, 1),    // Swap buffers
            0x0400_0580 => self.geom_command_fifo.push_command_cpu(data, 0x60, 1),    // Set viewport

            0x0400_05C0 => self.geom_command_fifo.push_command_cpu(data, 0x70, 3),    // Box test
            0x0400_05C4 => self.geom_command_fifo.push_command_cpu(data, 0x71, 2),    // Position test
            0x0400_05C8 => self.geom_command_fifo.push_command_cpu(data, 0x72, 1),    // Vector test

            0x0400_0600 => self.set_geom_engine_status(data),
            0x0400_0610 => self.geometry_engine.set_dot_polygon_depth(data),

            // TODO: tests
            _ => panic!("writing invalid gpu address {:X}", addr)
        }
    }
}

impl Video3D {
    fn swap_buffers(&mut self, data: u32) -> isize {
        std::mem::swap(
            &mut self.geometry_engine.polygon_ram,
            &mut self.rendering_engine.lock().polygon_ram
        );
        self.geometry_engine.polygon_ram.clear();
        self.geometry_engine.swap_buffers(data);
        392
    }

    /// Do a single command, returning the number of cycles used in the process.
    fn process_command(&mut self) -> isize {
        if self.current_commands == 0 {
            if let Some(commands) = self.geom_command_fifo.pop() {
                self.current_commands = commands;
            } else {
                // No commands queued.
                return 0;
            }
        }

        let command = (self.current_commands & 0xFF) as u8;
        self.current_commands >>= 8;

        match command {
            0x00 => 0,  // NOP

            0x10 => {
                self.geometry_engine.matrices.set_matrix_mode(self.geom_command_fifo.pop().unwrap());
                1
            },
            0x11 => self.geometry_engine.matrices.push_matrix(),
            0x12 => self.geometry_engine.matrices.pop_matrix(self.geom_command_fifo.pop().unwrap()),
            0x13 => self.geometry_engine.matrices.store_matrix(self.geom_command_fifo.pop().unwrap()),
            0x14 => self.geometry_engine.matrices.restore_matrix(self.geom_command_fifo.pop().unwrap()),
            // TODO: make these a bit nicer...
            0x15 => self.geometry_engine.matrices.set_identity(),
            0x16 => self.geometry_engine.matrices.set_4x4(&self.geom_command_fifo.pop_n(16).map(|n| N::from_bits(n as i32)).collect::<Vec<_>>()),
            0x17 => self.geometry_engine.matrices.set_4x3(&self.geom_command_fifo.pop_n(12).map(|n| N::from_bits(n as i32)).collect::<Vec<_>>()),
            0x18 => self.geometry_engine.matrices.mul_4x4(&self.geom_command_fifo.pop_n(16).map(|n| N::from_bits(n as i32)).collect::<Vec<_>>()),
            0x19 => self.geometry_engine.matrices.mul_4x3(&self.geom_command_fifo.pop_n(12).map(|n| N::from_bits(n as i32)).collect::<Vec<_>>()),
            0x1A => self.geometry_engine.matrices.mul_3x3(&self.geom_command_fifo.pop_n(9).map(|n| N::from_bits(n as i32)).collect::<Vec<_>>()),
            0x1B => self.geometry_engine.matrices.mul_scale(&self.geom_command_fifo.pop_n(3).map(|n| N::from_bits(n as i32)).collect::<Vec<_>>()),
            0x1C => self.geometry_engine.matrices.mul_trans(&self.geom_command_fifo.pop_n(3).map(|n| N::from_bits(n as i32)).collect::<Vec<_>>()),

            0x20 => self.geometry_engine.set_vertex_colour(self.geom_command_fifo.pop().unwrap()),
            0x21 => self.geometry_engine.set_normal(self.geom_command_fifo.pop().unwrap()),
            0x22 => 0,  // TEX COORD

            0x23 => self.geometry_engine.set_vertex_coords_16(self.geom_command_fifo.pop().unwrap(), self.geom_command_fifo.pop().unwrap()),
            0x24 => self.geometry_engine.set_vertex_coords_10(self.geom_command_fifo.pop().unwrap()),
            0x25 => self.geometry_engine.set_vertex_coords_xy(self.geom_command_fifo.pop().unwrap()),
            0x26 => self.geometry_engine.set_vertex_coords_xz(self.geom_command_fifo.pop().unwrap()),
            0x27 => self.geometry_engine.set_vertex_coords_yz(self.geom_command_fifo.pop().unwrap()),
            0x28 => self.geometry_engine.diff_vertex_coords(self.geom_command_fifo.pop().unwrap()),
            
            0x29 => self.geometry_engine.set_polygon_attrs(self.geom_command_fifo.pop().unwrap()),
            0x2A => 0,  // TEX PARAM
            0x2B => 0,  // TEX PALETTE BASE

            0x30 => self.geometry_engine.set_dif_amb_colour(self.geom_command_fifo.pop().unwrap()),
            0x31 => self.geometry_engine.set_spe_emi_colour(self.geom_command_fifo.pop().unwrap()),
            0x32 => self.geometry_engine.set_light_direction(self.geom_command_fifo.pop().unwrap()),
            0x33 => self.geometry_engine.set_light_colour(self.geom_command_fifo.pop().unwrap()),
            0x34 => self.geometry_engine.set_specular_table(self.geom_command_fifo.pop_n(32)),

            0x40 => self.geometry_engine.begin_vertex_list(self.geom_command_fifo.pop().unwrap()),
            0x41 => self.geometry_engine.end_vertex_list(),

            0x50 => {
                self.pending_swap = self.geom_command_fifo.pop();
                0
            },
            0x60 => self.geometry_engine.set_viewport(self.geom_command_fifo.pop().unwrap()),

            // TODO: test

            _ => 0, // Undefined
        }
    }

    fn is_busy(&self) -> bool {
        !self.geom_command_fifo.is_empty() || self.current_commands != 0 || self.cycle_count < 0
    }

    fn get_geom_engine_status(&self) -> GeometryEngineStatus {
        let cmd_buffer_count = self.geom_command_fifo.len();
        let proj_stack_level = self.geometry_engine.matrices.proj_stack_level();
        let pos_dir_stack_level = self.geometry_engine.matrices.pos_dir_stack_level();

        let mut status = GeometryEngineStatus::from_bits_truncate((cmd_buffer_count << 16) | (proj_stack_level << 13) | (pos_dir_stack_level << 8));
        status.set(GeometryEngineStatus::GEOM_BUSY, self.is_busy());
        status.set(GeometryEngineStatus::CMD_FIFO_EMPTY, self.geom_command_fifo.is_empty());
        status.set(GeometryEngineStatus::CMD_FIFO_UNDER_HALF, self.geom_command_fifo.under_half_full());
        status.set(GeometryEngineStatus::CMD_FIFO_FULL, self.geom_command_fifo.is_full());

        // TODO: mat stack busy
        status.set(GeometryEngineStatus::MAT_STACK_ERROR, self.geometry_engine.matrices.has_stack_error());
        // TODO: box test

        status
    }
    
    fn set_geom_engine_status(&mut self, data: u32) {
        let status_in = GeometryEngineStatus::from_bits_truncate(data);

        self.geom_command_fifo.set_interrupt_cond(status_in);

        if status_in.contains(GeometryEngineStatus::MAT_STACK_ERROR) {
            self.geometry_engine.matrices.clear_stack_error();
        }
    }

    fn read_clip_matrix(&self, index: u32) -> u32 {
        self.geometry_engine.matrices
            .clip_matrix()
            .elements[index as usize]
            .to_bits() as u32
    }
    
    fn read_dir_matrix(&self, index: u32) -> u32 {
        self.geometry_engine.matrices
            .dir_matrix()
            .elements[index as usize]
            .to_bits() as u32
    }
}