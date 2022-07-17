/// Colour for general drawing.

/// A colour in R8G8B8 format.
#[derive(Clone, Copy, Default)]
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

    pub fn add(&mut self, other: &Self) {
        self.r = self.r.saturating_add(other.r);
        self.g = self.g.saturating_add(other.g);
        self.b = self.b.saturating_add(other.b);
    }

    pub fn mul(&self, other: &Self) -> Self {
        let r = (self.r as u16) * (other.r as u16);
        let g = (self.g as u16) * (other.g as u16);
        let b = (self.b as u16) * (other.b as u16);
        Self {
            r: (r >> 8) as u8,
            g: (g >> 8) as u8,
            b: (b >> 8) as u8,
        }
    }

    pub fn weight(&self, weight: u8) -> Self {
        let r = (self.r as u16) * (weight as u16);
        let g = (self.g as u16) * (weight as u16);
        let b = (self.b as u16) * (weight as u16);
        Self {
            r: (r >> 8) as u8,
            g: (g >> 8) as u8,
            b: (b >> 8) as u8,
        }
    }
}
