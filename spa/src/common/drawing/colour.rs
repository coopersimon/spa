/// Colours for rendering.

/// An object pixel.
#[derive(Clone, Copy)]
pub struct ObjectPixel {
    pub colour:     ColType,
    pub priority:   u8,
    pub obj_type:   ObjType,
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

/// Type of object for blending.
#[derive(Clone, Copy)]
pub enum ObjType {
    /// Not an object, or not for blending.
    None,
    /// For blending.
    SemiTransparent,
    /// NDS bitmap object, with alpha coefficient.
    Bitmap(u16)
}

/// A colour in R8G8B8 format.
#[derive(Clone, Copy)]
pub struct Colour {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Colour {
    /// Deserialise from format:
    /// 0bbbbbgg gggrrrrr
    pub fn from_555(colour: u16) -> Self {
        let r = ((colour & 0x001F) << 3) as u8;
        let g = ((colour & 0x03E0) >> 2) as u8;
        let b = ((colour & 0x7C00) >> 7) as u8;
        Self {
            r: r | (r >> 5),
            g: g | (g >> 5),
            b: b | (b >> 5),
        }
    }

    /// Deserialise from format:
    /// Gbbbbbgg gggrrrrr
    pub fn from_565(colour: u16) -> Self {
        let r = ((colour & 0x001F) << 3) as u8;
        let g_hi = ((colour & 0x03E0) >> 2) as u8;
        let g_lo = ((colour & 0x8000) >> 13) as u8;
        let b = ((colour & 0x7C00) >> 7) as u8;
        Self {
            r: r | (r >> 6),
            g: g_hi | g_lo | (g_hi >> 6),
            b: b | (b >> 6),
        }
    }

    pub fn black() -> Self {
        Self {
            r: 0, g: 0, b: 0
        }
    }

    pub fn to_555(self) -> u16 {
        let r = (self.r >> 3) as u16;
        let g = (self.g >> 3) as u16;
        let b = (self.b >> 3) as u16;
        r | (g << 5) | (b << 10)
    }
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