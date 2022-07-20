mod types;
mod geometry;

use crate::{
    utils::{
        meminterface::MemInterface32,
        bits::u32
    },
};

use geometry::GeometryEngine;

pub struct Video3D {
    geometry_engine:    GeometryEngine,
}

impl Video3D {
    pub fn new() -> Self {
        Self {
            geometry_engine:    GeometryEngine::new(),
        }
    }
}

impl MemInterface32 for Video3D {
    fn read_word(&mut self, addr: u32) -> u32 {
        match addr {
            _ => panic!("reading invalid gpu address {:X}", addr)
        }
    }

    fn write_word(&mut self, addr: u32, data: u32) {
        match addr {
            0x0400_0400..=0x0400_043F => {},    // Command buffer

            0x0400_0440 => self.geometry_engine.matrices.set_matrix_mode(data),
            0x0400_0444 => self.geometry_engine.matrices.push_matrix(),
            0x0400_0448 => self.geometry_engine.matrices.pop_matrix(data & 0x3F),
            0x0400_044C => self.geometry_engine.matrices.store_matrix(data & 0x1F),
            0x0400_0450 => self.geometry_engine.matrices.restore_matrix(data & 0x1F),
            0x0400_0454 => self.geometry_engine.set_identity_matrix(),
            0x0400_0458 => self.geometry_engine.set_4x4_matrix(data),
            0x0400_045C => self.geometry_engine.set_4x3_matrix(data),
            0x0400_0460 => self.geometry_engine.mul_4x4(data),
            0x0400_0464 => self.geometry_engine.mul_4x3(data),
            0x0400_0468 => self.geometry_engine.mul_3x3(data),
            0x0400_046C => self.geometry_engine.mul_scale(data),
            0x0400_0470 => self.geometry_engine.mul_trans(data),

            0x0400_0480 => self.geometry_engine.set_vertex_colour(data),
            0x0400_0484 => self.geometry_engine.set_normal(data),
            0x0400_0488 => {},  // SET TEX coords

            0x0400_048C => self.geometry_engine.set_vertex_coords_16(data),
            0x0400_0490 => self.geometry_engine.set_vertex_coords_10(data),
            0x0400_0494 => self.geometry_engine.set_vertex_coords_xy(data),
            0x0400_0498 => self.geometry_engine.set_vertex_coords_xz(data),
            0x0400_049C => self.geometry_engine.set_vertex_coords_yz(data),
            0x0400_04A0 => self.geometry_engine.diff_vertex_coords(data),

            0x0400_04A4 => self.geometry_engine.set_polygon_attrs(data),
            0x0400_04A8 => {},  // SET TEX params
            0x0400_04AC => {},  // Set tex palette

            0x0400_04C0 => self.geometry_engine.lighting.set_dif_amb_colour(data),
            0x0400_04C4 => self.geometry_engine.lighting.set_spe_emi_colour(data),
            0x0400_04C8 => self.geometry_engine.set_light_direction(data),
            0x0400_04CC => self.geometry_engine.lighting.set_light_colour(data),
            0x0400_04D0 => self.geometry_engine.lighting.set_specular_table(data),

            0x0400_0500 => {},  // BEGIN
            0x0400_0504 => {},  // END

            0x0400_0540 => {},  // TODO: swap buffers
            0x0400_0580 => {},  // TODO: viewport

            // TODO: tests
            _ => panic!("writing invalid gpu address {:X}", addr)
        }
    }
}
