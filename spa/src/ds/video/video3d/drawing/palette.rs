use crate::common::video::colour::Colour;

/// The current palette of colours for textures.
pub struct TexPaletteCache {
    /// 96kB of potential colours.
    tex_palette:     Vec<Colour>,
}

impl TexPaletteCache {
    pub fn new() -> Self {
        Self {
            tex_palette:    vec![Colour::black(); 48 * 1024],
        }
    }

    /// Update the tex palette cache.
    pub fn update_tex(&mut self, tex_palette_ram: &[Option<&[u8]>]) {
        for (palette_cache, palette_ram) in self.tex_palette.chunks_exact_mut(8 * 1024).zip(tex_palette_ram) {
            if let Some(palette_ram) = palette_ram.as_ref() {
                for (cache_colour, bytes) in palette_cache.iter_mut().zip(palette_ram.chunks_exact(2)) {
                    *cache_colour = Colour::from_555(u16::from_le_bytes(bytes.try_into().unwrap()));
                }
            }
        }
    }

    pub fn get_tex_colour(&self, index: u32) -> Colour {
        self.tex_palette[index as usize].clone()
    }
}
