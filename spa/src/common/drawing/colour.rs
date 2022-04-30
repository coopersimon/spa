/// Colours for rendering.

/// An object pixel.
#[derive(Clone, Copy)]
pub struct ObjectPixel {
    pub colour:             ColType,
    pub priority:           u8,
    pub semi_transparent:   bool,
}

/// A variable colour.
#[derive(Clone, Copy)]
pub enum ColType {
    /// One of the 256 normal colours
    Palette(u8),
    /// One of the 4096 extended colours
    Extended(u16),
    /// A direct colour (in 5,5,5 format)
    Direct(u16) // TODO: alpha
}

#[derive(Clone)]
pub struct Colour {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Colour {
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

    pub fn black() -> Self {
        Self {
            r: 0, g: 0, b: 0
        }
    }
}

/// The current palette of colours.
pub struct PaletteCache {
    bg_palette:     Vec<Colour>,
    obj_palette:    Vec<Colour>,
}

impl PaletteCache {
    pub fn new() -> Self {
        Self {
            bg_palette:     vec![Colour::black(); 256],
            obj_palette:    vec![Colour::black(); 256],
        }
    }

    /// Update the bg palette cache.
    pub fn update_bg(&mut self, palette_ram: &[u16]) {
        for (raw, colour) in palette_ram.iter().zip(self.bg_palette.iter_mut()) {
            *colour = Colour::from_555(*raw);
        }
    }

    /// Update the obj palette cache.
    pub fn update_obj(&mut self, palette_ram: &[u16]) {
        for (raw, colour) in palette_ram.iter().zip(self.obj_palette.iter_mut()) {
            *colour = Colour::from_555(*raw);
        }
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
}