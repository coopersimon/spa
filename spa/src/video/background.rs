use fixed::types::I24F8;

#[derive(Clone)]
/// Background data for use by renderers
pub enum BackgroundData {
    Tiled(TiledBackgroundData),
    Affine(AffineBackgroundData),
    Bitmap(BitmapBackgroundData)
}

impl BackgroundData {
    pub fn priority(&self) -> u8 {
        use BackgroundData::*;
        match self {
            Tiled(t) => t.priority,
            Affine(a) => a.priority,
            Bitmap(b) => b.priority
        }
    }
}

#[derive(Clone)]
pub enum BackgroundMapLayout {
    Small,  // 1x1 map
    Wide,   // 2x1 map
    Tall,   // 1x2 map
    Large   // 2x2 map
}

/// Data for a tiled background.
#[derive(Clone)]
pub struct TiledBackgroundData {
    pub priority:       u8,
    pub tile_map_addr:  u32,
    pub tile_data_addr: u32,
    pub use_8bpp:       bool,
    pub mosaic:         bool,

    pub scroll_x:   u16,
    pub scroll_y:   u16,
    pub layout:     BackgroundMapLayout,
}

/// Data for a tiled background.
#[derive(Clone)]
pub struct AffineBackgroundData {
    pub priority:       u8,
    pub tile_map_addr:  u32,
    pub tile_data_addr: u32,
    pub mosaic:         bool,

    pub bg_ref_point_x: I24F8,
    pub bg_ref_point_y: I24F8,
    pub matrix_a:       I24F8,
    pub matrix_b:       I24F8,
    pub matrix_c:       I24F8,
    pub matrix_d:       I24F8,
    pub wrap:           bool,
    pub size:           u32,
}

/// Data for a bitmap background.
#[derive(Clone)]
pub struct BitmapBackgroundData {
    pub priority:       u8,
    pub data_addr:      u32,
    pub use_15bpp:      bool,
    pub mosaic:         bool,
    pub small:          bool,
}
