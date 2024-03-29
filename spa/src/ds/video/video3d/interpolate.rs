use fixed::traits::ToFixed;
use fixed::types::I40F24;
use super::types::{
    Depth, TexCoords
};
use super::geometry::N;
use crate::common::video::colour::Colour;

// Interpolation helpers.

#[inline]
pub fn interpolate_depth(depth_a: Depth, depth_b: Depth, factor_a: N, factor_b: N) -> Depth {
    (depth_a.to_fixed::<N>() * factor_a + depth_b.to_fixed::<N>() * factor_b).to_fixed()
}

#[inline]
/// Perspective-correct depth interpolation.
pub fn interpolate_depth_p(depth_a: Depth, depth_b: Depth, factor_a: N, factor_b: N) -> Depth {
    ((depth_a.to_fixed::<I40F24>().recip() * factor_a.to_fixed::<I40F24>()) + (depth_b.to_fixed::<I40F24>().recip() * factor_b.to_fixed::<I40F24>())).recip().to_fixed()
}

#[inline]
pub fn interpolate_vertex_colour(colour_a: Colour, colour_b: Colour, factor_a: N, factor_b: N) -> Colour {
    let r = (N::from_num(colour_a.r) * factor_a) + (N::from_num(colour_b.r) * factor_b);
    let g = (N::from_num(colour_a.g) * factor_a) + (N::from_num(colour_b.g) * factor_b);
    let b = (N::from_num(colour_a.b) * factor_a) + (N::from_num(colour_b.b) * factor_b);
    Colour {
        r: r.to_num::<u8>(),
        g: g.to_num::<u8>(),
        b: b.to_num::<u8>()
    }
}

#[inline]
pub fn interpolate_tex_coords(tex_coords_a: TexCoords, tex_coords_b: TexCoords, factor_a: N, factor_b: N) -> TexCoords {
    let s = (tex_coords_a.s.to_fixed::<N>() * factor_a) + (tex_coords_b.s.to_fixed::<N>() * factor_b);
    let t = (tex_coords_a.t.to_fixed::<N>() * factor_a) + (tex_coords_b.t.to_fixed::<N>() * factor_b);
    TexCoords { s: s.to_fixed(), t: t.to_fixed() }
}

#[inline]
/// Perspective-correct texture interpolation.
pub fn interpolate_tex_coords_p(tex_coords_a: TexCoords, tex_coords_b: TexCoords, factor_a: N, factor_b: N, depth_a: Depth, depth_b: Depth) -> TexCoords {
    let s_a = tex_coords_a.s.to_fixed::<I40F24>().checked_div(depth_a.to_fixed::<I40F24>()).unwrap_or(N::MAX.to_fixed());
    let s_b = tex_coords_b.s.to_fixed::<I40F24>().checked_div(depth_b.to_fixed::<I40F24>()).unwrap_or(N::MAX.to_fixed());
    let t_a = tex_coords_a.t.to_fixed::<I40F24>().checked_div(depth_a.to_fixed::<I40F24>()).unwrap_or(N::MAX.to_fixed());
    let t_b = tex_coords_b.t.to_fixed::<I40F24>().checked_div(depth_b.to_fixed::<I40F24>()).unwrap_or(N::MAX.to_fixed());
    let s = (s_a * factor_a.to_fixed::<I40F24>()) + (s_b * factor_b.to_fixed::<I40F24>());
    let t = (t_a * factor_a.to_fixed::<I40F24>()) + (t_b * factor_b.to_fixed::<I40F24>());
    let d = factor_a.to_fixed::<I40F24>().checked_div(depth_a.to_fixed::<I40F24>()).unwrap_or(N::MAX.to_fixed()) +
        factor_b.to_fixed::<I40F24>().checked_div(depth_b.to_fixed::<I40F24>()).unwrap_or(N::MAX.to_fixed());
    let div = (d).recip();
    TexCoords { s: (s * div).to_fixed(), t: (t * div).to_fixed() }
}