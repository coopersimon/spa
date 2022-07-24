use crate::common::colour::Colour;
use crate::utils::{
    bytes, bits
};
use super::types::*;

pub struct RenderingEngine {
    pub polygon_ram:    Box<PolygonRAM>,

    clear_colour:   Colour,
    clear_alpha:    u8,
    clear_poly_id:  u8,
    clear_depth:    u32,

    clear_image_x:  u8,
    clear_image_y:  u8,

    alpha_test:     u8,
    
    fog_enabled:    bool,
    fog_colour:     Colour,
    fog_alpha:      u8,
    fog_offset:     u32,
    fog_table:      Vec<u8>,

    toon_table:     Vec<Colour>,
    edge_colour:    Vec<Colour>
}

impl RenderingEngine {
    pub fn new() -> Self {
        Self {
            polygon_ram:    Box::new(PolygonRAM::new()),
            
            clear_colour:   Colour::default(),
            clear_alpha:    0,
            clear_poly_id:  0,
            clear_depth:    0,

            clear_image_x:  0,
            clear_image_y:  0,

            alpha_test:     0,
    
            fog_enabled:    false,
            fog_colour:     Colour::default(),
            fog_alpha:      0,
            fog_offset:     0,
            fog_table:      vec![0; 32],

            toon_table:     vec![Colour::default(); 32],
            edge_colour:    vec![Colour::default(); 8]
        }
    }
}

// GPU Commands
impl RenderingEngine {
    pub fn set_clear_colour_attr(&mut self, data: u32) {
        self.clear_colour = Colour::from_555(bytes::u32::lo(data));
        self.fog_enabled = bits::u32::test_bit(data, 15);
        self.clear_alpha = ((data >> 16) & 0x1F) as u8;
        self.clear_poly_id = ((data >> 24) & 0x3F) as u8;
    }

    /// Set clear depth value, and image offset.
    pub fn set_clear_depth_image(&mut self, data: u32) {
        let depth = data & 0x7FFF;
        let depth_low = if depth == 0x7FFF {
            0x1FF
        } else {
            0
        };
        self.clear_depth = (depth << 9) | depth_low;

        let clear_image_coords = bytes::u32::hi(data);
        self.clear_image_x = bytes::u16::lo(clear_image_coords);
        self.clear_image_y = bytes::u16::hi(clear_image_coords);
    }

    pub fn set_toon_table(&mut self, index: usize, data: u32) {
        self.toon_table[index * 2] = Colour::from_555(bytes::u32::lo(data));
        self.toon_table[(index * 2) + 1] = Colour::from_555(bytes::u32::hi(data));
    }

    pub fn set_alpha_test(&mut self, data: u32) {
        self.alpha_test = (data & 0x1F) as u8;
    }

    pub fn set_edge_colour(&mut self, index: usize, data: u32) {
        self.edge_colour[index * 2] = Colour::from_555(bytes::u32::lo(data));
        self.edge_colour[(index * 2) + 1] = Colour::from_555(bytes::u32::hi(data));
    }

    pub fn set_fog_colour(&mut self, data: u32) {
        self.fog_colour = Colour::from_555(bytes::u32::lo(data));
        self.fog_alpha = (bytes::u32::hi(data) & 0x1F) as u8;
    }

    pub fn set_fog_offset(&mut self, data: u32) {
        self.fog_offset = data & 0x7FFF;
    }

    pub fn set_fog_table(&mut self, index: usize, data: u32) {
        let bytes = u32::to_le_bytes(data);
        for (byte, table_val) in bytes.iter().zip(self.fog_table.iter_mut().skip(index * 4).take(4)) {
            *table_val = *byte;
        }
    }
}

// Drawing
// TODO: separate data from impl?
impl RenderingEngine {
    pub fn draw_line(&mut self, line: u8) {
        // Clear stencil, depth, colour, attr buffers

        // Draw opaque polygons (sorted)
        for p in self.polygon_ram.opaque_polygons.iter()
            .skip_while(|el| el.y_max < line || el.y_min > line)
            .take_while(|el| el.y_max >= line && el.y_min <= line) {
            let polygon = &self.polygon_ram.polygons[p.polygon_index];
            // TODO: from x_min to x_max
            for x in 0..=255_u8 {
                // Check if inside
                // Interpolate depth, test depth
                // Stencil ??
                // Find fragment colour, tex colour, blend
                // Alpha blend with buffer colour
            }
        }

        // Edge marking in buffer
        // Fog in buffer

        // Anti-aliasing (after 2d-blend?)
    }
}