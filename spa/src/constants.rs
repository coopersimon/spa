/// Constants for timing.

/// GBA timing, video, and audio constants.
pub mod gba {
    pub const CYCLES_PER_SECOND: usize = 16 * 1024 * 1024;

    /// Cycles needed to draw a single pixel on-screen.
    pub const DOT_TIME: usize = 4;
    /// Visible horizontal resolution.
    pub const H_RES: usize = 240;
    /// Number of dots spent in H-blank.
    pub const H_BLANK_RES: usize = 68;
    /// Visible vertical resolution.
    pub const V_RES: usize = 160;
    /// Number of dots spent in V-blank.
    pub const V_BLANK_RES: usize = 68;

    /// Cycles needed for a whole frame.
    pub const FRAME_CYCLES: usize = DOT_TIME * (H_RES + H_BLANK_RES) * (V_RES + V_BLANK_RES);
}