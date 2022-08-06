use bitflags::bitflags;
use fixed::types::I24F8;

use crate::utils::bits::u8;

/// Colour special effect
pub enum ColourEffect {
    None,
    AlphaBlend,
    Brighten,
    Darken
}

pub struct BackgroundData {
    pub priority:       u8,
    pub window_mask:    WindowMask,
    pub blend_mask:     BlendMask,
    pub mosaic:         bool,
    pub type_data:      BackgroundTypeData,
}

#[derive(Clone)]
/// Background data for use by renderers
pub enum BackgroundTypeData {
    Render3D(Render3DBackgroundData),
    Tiled(TiledBackgroundData),
    Affine(AffineBackgroundData),
    Bitmap(BitmapBackgroundData),
    ExtTiledAffine(AffineBackgroundData),
    ExtBitmapAffine(BitmapAffineBackgroundData),
}

bitflags! {
    #[derive(Default)]
    pub struct WindowMask: u8 {
        const OUT_WIN   = u8::bit(3);
        const OBJ_WIN   = u8::bit(2);
        const WINDOW_1  = u8::bit(1);
        const WINDOW_0  = u8::bit(0);
    }
}

impl WindowMask {
    pub fn make(win0: bool, win1: bool, obj_win: bool, out_win: bool) -> Self {
        let mut ret = WindowMask::default();
        ret.set(WindowMask::WINDOW_0, win0);
        ret.set(WindowMask::WINDOW_1, win1);
        ret.set(WindowMask::OBJ_WIN, obj_win);
        ret.set(WindowMask::OUT_WIN, out_win);
        ret
    }
}

bitflags! {
    #[derive(Default)]
    pub struct BlendMask: u8 {
        const LAYER_2   = u8::bit(1);
        const LAYER_1   = u8::bit(0);
    }
}

impl BlendMask {
    pub fn make(layer_1: bool, layer_2: bool) -> Self {
        let mut ret = BlendMask::default();
        ret.set(BlendMask::LAYER_1, layer_1);
        ret.set(BlendMask::LAYER_2, layer_2);
        ret
    }
}

#[derive(Clone)]
pub enum BackgroundMapLayout {
    Small,  // 1x1 map
    Wide,   // 2x1 map
    Tall,   // 1x2 map
    Large   // 2x2 map
}

/// Data for 3D background.
#[derive(Clone)]
pub struct Render3DBackgroundData {
    pub scroll_x:   u16
}

/// Data for a tiled background.
#[derive(Clone)]
pub struct TiledBackgroundData {
    pub tile_map_addr:  u32,
    pub tile_data_addr: u32,
    pub use_8bpp:       bool,

    pub scroll_x:   u16,
    pub scroll_y:   u16,
    pub layout:     BackgroundMapLayout,

    pub ext_palette:    Option<usize>
}

/// Data for a tiled background.
#[derive(Clone)]
pub struct AffineBackgroundData {
    pub tile_map_addr:  u32,
    pub tile_data_addr: u32,

    pub bg_ref_point_x: I24F8,
    pub bg_ref_point_y: I24F8,
    pub matrix_a:       I24F8,
    pub matrix_b:       I24F8,
    pub matrix_c:       I24F8,
    pub matrix_d:       I24F8,
    pub wrap:           bool,
    pub size:           u32,

    pub ext_palette:    Option<usize>
}

/// Data for a bitmap background.
#[derive(Clone)]
pub struct BitmapBackgroundData {
    pub data_addr:      u32,
    pub use_15bpp:      bool,
    pub small:          bool,
}

/// NDS: Data for an extended background,
/// in bitmap-affine mode.
#[derive(Clone)]
pub struct BitmapAffineBackgroundData {
    pub data_addr:      u32,
    pub use_15bpp:      bool,

    pub bg_ref_point_x: I24F8,
    pub bg_ref_point_y: I24F8,
    pub matrix_a:       I24F8,
    pub matrix_b:       I24F8,
    pub matrix_c:       I24F8,
    pub matrix_d:       I24F8,
    pub wrap:           bool,
    pub size:           (u32, u32),
}
