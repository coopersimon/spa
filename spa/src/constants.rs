/// Constants for timing.

/// GBA timing, video, and audio constants.
pub mod gba {
    //pub const CYCLES_PER_SECOND: usize = 16 * 1024 * 1024;

    /// Cycles needed to draw a single pixel on-screen.
    pub const DOT_TIME: usize = 4;
    /// Visible horizontal resolution.
    pub const H_RES: usize = 240;
    /// Number of dots spent in H-blank.
    pub const H_BLANK_RES: usize = 68;
    /// Visible vertical resolution.
    pub const V_RES: usize = 160;
    // Number of dots spent in V-blank.
    pub const V_BLANK_RES: usize = 68;

    /// Width of bitmap in mode 5.
    pub const SMALL_BITMAP_WIDTH: u8 = 160;
    /// Height of bitmap in mode 5.
    pub const SMALL_BITMAP_HEIGHT: u8 = 128;
    pub const SMALL_BITMAP_LEFT: u8 = ((H_RES as u8) - SMALL_BITMAP_WIDTH) / 2;
    pub const SMALL_BITMAP_TOP: u8 = ((V_RES as u8) - SMALL_BITMAP_HEIGHT) / 2;

    /// Cycles needed for a whole frame.
    pub const FRAME_CYCLES: usize = DOT_TIME * (H_RES + H_BLANK_RES) * (V_RES + V_BLANK_RES);

    // Video state timing:

    /// Horizontal drawing time.
    pub const H_DRAW_CYCLES: usize = H_RES * DOT_TIME;
    /// Time before H-Blank, after drawing finishes.
    pub const POST_H_DRAW_CYCLES: usize = 46;
    /// Total time before H-Blank.
    pub const H_CYCLES: usize = H_DRAW_CYCLES + POST_H_DRAW_CYCLES;
    /// Time during H-Blank.
    pub const H_BLANK_CYCLES: usize = (H_BLANK_RES * DOT_TIME) - POST_H_DRAW_CYCLES;
    /// Max V-Count before V-blank
    pub const V_MAX: u8 = 159;
    /// Max V-Count before starting new frame
    pub const VBLANK_MAX: u8 = 227;
}