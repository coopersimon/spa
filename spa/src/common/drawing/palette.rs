/// Palette and object types for rendering.

use crate::common::colour::Colour;

/// An object pixel.
#[derive(Clone, Copy)]
pub struct ObjectPixel {
    pub colour:     ColType,
    pub priority:   u8,
    pub obj_type:   BlendType,
}

/// A variable colour.
#[derive(Clone, Copy)]
pub enum ColType {
    /// One of the 256 normal colours
    Palette(u8),
    /// One of the 4096 extended colours
    Extended(u16),
    /// A direct colour (in 5,5,5 format)
    Direct(u16)
}

/// Type of pixel for blending.
#[derive(Clone, Copy)]
pub enum BlendType {
    /// Maybe for blending. Used for 2D BGs, normal Objects, and backdrop.
    None,
    /// Object used for alpha blending.
    SemiTransparent,
    /// NDS bitmap object, with alpha coefficient.
    Bitmap(u16),
    /// NDS 3D pixel, with alpha coefficient.
    BG3D(u16)
}

/// The current palette of colours.
pub struct PaletteCache {
    bg_palette:     Vec<Colour>,
    obj_palette:    Vec<Colour>,

    ext_bg_palette:     [Vec<Colour>; 4],
    ext_obj_palette:    Vec<Colour>,
}

impl PaletteCache {
    pub fn new() -> Self {
        Self {
            bg_palette:     vec![Colour::black(); 256],
            obj_palette:    vec![Colour::black(); 256],

            ext_bg_palette:     [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
            ext_obj_palette:    Vec::new(),
        }
    }

    /// Update the bg palette cache, using 555 colour format.
    pub fn update_bg_555(&mut self, palette_ram: &[u16]) {
        for (raw, colour) in palette_ram.iter().zip(self.bg_palette.iter_mut()) {
            *colour = Colour::from_555(*raw);
        }
    }

    /// Update the obj palette cache, using 555 colour format.
    pub fn update_obj_555(&mut self, palette_ram: &[u16]) {
        for (raw, colour) in palette_ram.iter().zip(self.obj_palette.iter_mut()) {
            *colour = Colour::from_555(*raw);
        }
    }

    /// Update the bg palette cache, using 565 colour format.
    pub fn update_bg_565(&mut self, palette_ram: &[u16]) {
        for (raw, colour) in palette_ram.iter().zip(self.bg_palette.iter_mut()) {
            *colour = Colour::from_565(*raw);
        }
    }

    /// Update the obj palette cache, using 565 colour format.
    pub fn update_obj_565(&mut self, palette_ram: &[u16]) {
        for (raw, colour) in palette_ram.iter().zip(self.obj_palette.iter_mut()) {
            *colour = Colour::from_565(*raw);
        }
    }

    /// Update the extended bg palette cache.
    pub fn update_ext_bg(&mut self, slot: usize, ext_palette_ram: &[u8]) {
        self.ext_bg_palette[slot] = ext_palette_ram.chunks_exact(2)
            .map(|bytes| u16::from_le_bytes(bytes.try_into().unwrap()))
            .map(|raw| Colour::from_565(raw))
            .collect::<Vec<_>>();
    }

    /// Update the extended obj palette cache.
    pub fn update_ext_obj(&mut self, ext_palette_ram: &[u8]) {
        self.ext_obj_palette = ext_palette_ram.chunks_exact(2)
            .map(|bytes| u16::from_le_bytes(bytes.try_into().unwrap()))
            .map(|raw| Colour::from_565(raw))
            .collect::<Vec<_>>();
    }

    pub fn get_backdrop(&self) -> Colour {
        self.bg_palette[0].clone()
    }

    pub fn get_bg(&self, index: u8) -> Colour {
        self.bg_palette[index as usize].clone()
    }

    pub fn get_obj(&self, index: u8) -> Colour {
        self.obj_palette[index as usize].clone()
    }

    pub fn get_ext_bg(&self, slot: usize, index: u16) -> Colour {
        self.ext_bg_palette[slot][index as usize].clone()
    }

    pub fn get_ext_obj(&self, index: u16) -> Colour {
        self.ext_obj_palette[index as usize].clone()
    }
}