
use crate::{
    utils::{
        bits::u32,
        bytes
    },
    common::video::colour::Colour,
};
use super::math::*;
use super::super::types::PolygonAttrs;

#[derive(Default)]
struct Light {
    direction:  Vector<3>,
    half_angle: Vector<3>,
    colour:     Colour,
    enabled:    bool,
}

#[derive(Default)]
pub struct LightingUnit {
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
    pub fn new() -> Self {
        Self {
            specular_table: vec![0; 128],
            ..Default::default()
        }
    }

    /// Get the current vertex colour.
    pub fn get_vertex_colour(&self) -> Colour {
        self.vertex_colour
    }

    /// Calculate colour.
    pub fn set_normal(&mut self, normal: Vector<3>) -> isize {
        let mut cycles = 8;
        self.vertex_colour = self.emission_colour;

        for light in &self.lights {
            if !light.enabled {
                continue;
            }
            let diffuse = N::max(N::ZERO, -normal.dot_product(&light.direction));
            let diffuse_weight = (diffuse.to_bits() >> 4) as u8;
            let diffuse_colour = light.colour.mul(&self.diffuse_colour).weight(diffuse_weight);

            let ambient_colour = light.colour.mul(&self.ambient_colour);

            let specular_angle_cos = N::max(N::ZERO, normal.dot_product(&light.half_angle));
            let specular_angle_bits = specular_angle_cos.to_bits() >> 4;
            let specular_weight = if self.enable_table {
                let table_idx = specular_angle_bits >> 1;
                self.specular_table[table_idx as usize]
            } else {
                specular_angle_bits as u8
            };
            let specular_colour = light.colour.mul(&self.specular_colour).weight(specular_weight);

            self.vertex_colour.add(&diffuse_colour);
            self.vertex_colour.add(&ambient_colour);
            self.vertex_colour.add(&specular_colour);

            cycles += 1;
        }
        
        cycles
    }

    pub fn set_vertex_colour(&mut self, colour: u32) {
        self.vertex_colour = Colour::from_555(bytes::u32::lo(colour));
    }

    pub fn set_light_direction(&mut self, light: usize, direction: Vector<3>) {
        self.lights[light].direction = direction.clone();
        // Find normalised half-vector between light dir and line-of-sight (-Z)
        // Then negate it for specular calculations.
        self.lights[light].half_angle = Vector::new([
            -direction.elements[0] >> 1,
            -direction.elements[1] >> 1,
            (N::ONE - direction.elements[2]) >> 1
        ]);
    }

    pub fn set_light_colour(&mut self, data: u32) {
        let light = (data >> 30) as usize;
        self.lights[light].colour = Colour::from_555(bytes::u32::lo(data));
    }

    pub fn set_dif_amb_colour(&mut self, data: u32) {
        self.diffuse_colour = Colour::from_555(bytes::u32::lo(data));
        self.ambient_colour = Colour::from_555(bytes::u32::hi(data));
        if u32::test_bit(data, 15) {
            self.vertex_colour = self.diffuse_colour;
        }
    }
    
    pub fn set_spe_emi_colour(&mut self, data: u32) {
        self.specular_colour = Colour::from_555(bytes::u32::lo(data));
        self.emission_colour = Colour::from_555(bytes::u32::hi(data));
        self.enable_table = u32::test_bit(data, 15);
    }

    pub fn set_specular_table(&mut self, data: u32) {
        for (table, input) in self.specular_table.iter_mut().skip(self.specular_index).zip(&data.to_le_bytes()) {
            *table = *input;
        }
        self.specular_index = (self.specular_index + 4) % 128;
    }

    pub fn set_enabled(&mut self, attrs: PolygonAttrs) {
        self.lights[0].enabled = attrs.contains(PolygonAttrs::ENABLE_LIGHT_0);
        self.lights[1].enabled = attrs.contains(PolygonAttrs::ENABLE_LIGHT_1);
        self.lights[2].enabled = attrs.contains(PolygonAttrs::ENABLE_LIGHT_2);
        self.lights[3].enabled = attrs.contains(PolygonAttrs::ENABLE_LIGHT_3);
    }
}
