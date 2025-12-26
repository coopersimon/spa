
use fixed::types::{I16F0, I40F24};
use fixed::traits::{Fixed, ToFixed};
use crate::common::video::colour::Colour;
use super::{
    super::types::*,
    math::{N, Vector},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClipPlane {
    Bottom = 0,
    Top = 1,
    Left = 2,
    Right = 3,
    Near = 4,
    Far = 5
}
impl ClipPlane {
    pub fn all() -> &'static[Self] {
        use ClipPlane::*;
        const ALL: &[ClipPlane] = &[Near, Far, Left, Right, Bottom, Top];
        ALL
    }
}

#[derive(Clone, Default)]
pub struct StagedVertex {
    pub position:   Vector<4>,
    pub colour:     Colour,
    pub tex_coords: TexCoords,
}

pub struct ClippingUnit {
    pub polygon_ram:    Box<PolygonRAM>,

    /// Use W or Z value for depth-buffering.
    w_buffer:           bool,

    /// Test w against this value for 1-dot polygons.
    dot_polygon_w:      Depth,

    viewport_x:         N,
    viewport_y:         N,
    viewport_width:     N,
    viewport_height:    N,
}

impl ClippingUnit {
    pub fn new() -> Self {
        Self {
            polygon_ram:    Box::new(PolygonRAM::new()),

            w_buffer:           false,

            dot_polygon_w:      Depth::from_bits(0x7FFF << 6),

            viewport_x:         N::ZERO,
            viewport_y:         N::ZERO,
            viewport_width:     N::ZERO,
            viewport_height:    N::ZERO,
        }
    }

    pub fn set_viewport(&mut self, data: u32) {
        let bytes = u32::to_le_bytes(data);
        self.viewport_x = N::from_num(bytes[0]);
        self.viewport_y = N::from_num(bytes[1]);
        self.viewport_width = N::ONE + N::from_num(bytes[2] as u16) - self.viewport_x;
        self.viewport_height = N::ONE + N::from_num(bytes[3] as u16) - self.viewport_y;
    }

    pub fn set_w_buffer(&mut self, w_buffer: bool) {
        self.w_buffer = w_buffer;
    }

    pub fn set_dot_polygon_depth(&mut self, data: u16) {
        let bits = (data & 0x7FFF) as i32;
        self.dot_polygon_w = Depth::from_bits(bits << 6);
    }

    /// Add a polygon and vertices to the vertex list RAM.
    /// 
    /// Also make a note of its index in the current polygon.
    pub fn add_polygon(&mut self, mut staged_polygon: Polygon, vertices: &mut [StagedVertex], capture: bool) {
        let (mut min_y, mut max_y) = (I16F0::MAX, I16F0::ZERO);

        let mut one_dot_w = N::MAX;
        let vertices_out = vertices.iter().map(|vertex| {

            let w = vertex.position.w().to_fixed::<I40F24>();
            let w2 = (w * 2).checked_recip().unwrap_or(I40F24::MAX);
            let x = (vertex.position.x().to_fixed::<I40F24>() + w) * w2;
            let y = (vertex.position.y().to_fixed::<I40F24>() + w) * w2;
            let z = (vertex.position.z().to_fixed::<I40F24>() + w) * w2;

            let screen_p = self.get_screen_coords(x.to_fixed(), N::ONE - y.to_fixed::<N>());
            max_y = std::cmp::max(max_y, screen_p.y);
            min_y = std::cmp::min(min_y, screen_p.y);
            one_dot_w = std::cmp::min(vertex.position.w(), one_dot_w);

            // TODO: re-use vertices...
            /*let idx = if let Some(idx) = vertex.idx {
                idx
            } else {*/
                let depth = if self.w_buffer {
                    w.to_fixed::<Depth>()
                } else {
                    (z * 0x7FFF).to_fixed::<Depth>()
                };
                if capture {
                    println!("W: {:X} W2: {:X}", w, w2);
                    println!("Emit vtx: Screen: {:X}, {:X} | XYZ: {:X}, {:X}, {:X}", screen_p.x, screen_p.y, x, y, z);
                    println!("Input: {:X}, {:X}, {:X}, {:X}", vertex.position.x(), vertex.position.y(), vertex.position.z(), vertex.position.w());
                    println!("Depth: {:X}", depth);
                }
                let out_vertex = Vertex {
                    screen_p,
                    depth: depth,
                    colour: vertex.colour,
                    tex_coords: vertex.tex_coords
                };
                let idx = self.polygon_ram.insert_vertex(out_vertex.clone());
                //vertex.idx = Some(idx);
                //idx
            //};
            staged_polygon.add_vertex_index(idx);
            out_vertex
        }).collect::<Vec<_>>();

        if test_winding(&staged_polygon, &vertices_out)/* && self.test_one_dot_display(&staged_polygon.attrs, &vertices_out, one_dot_w.to_fixed())*/ {
            self.polygon_ram.insert_polygon(staged_polygon, max_y, min_y);
        }
    }
    
    pub fn get_screen_coords(&self, x: N, y: N) -> Coords {
        let clamped_x = x.clamp(N::ZERO, N::ONE);
        let clamped_y = y.clamp(N::ZERO, N::ONE);
        let screen_x = self.viewport_x + (clamped_x * self.viewport_width);
        let screen_y = self.viewport_y + (clamped_y * self.viewport_height);
        Coords { x: screen_x.round().to_fixed(), y: screen_y.round().to_fixed() }
    }

    /// Returns true if clip occurred.
    pub fn clip(&mut self, plane: ClipPlane, in_polygon: &[StagedVertex], out_polygon: &mut Vec<StagedVertex>) -> bool {
        use ClipPlane::*;
        const X: usize = 0;
        const Y: usize = 1;
        const Z: usize = 2;
        match plane {
            Top     => self.clip_polygon(in_polygon, out_polygon, Y, -N::ONE, |n, w| n < -w),
            Bottom  => self.clip_polygon(in_polygon, out_polygon, Y, N::ONE, |n, w| n > w),
            Left    => self.clip_polygon(in_polygon, out_polygon, X, -N::ONE, |n, w| n < -w),
            Right   => self.clip_polygon(in_polygon, out_polygon, X, N::ONE, |n, w| n > w),
            Far     => self.clip_polygon(in_polygon, out_polygon, Z, N::ONE, |n, w| n > w),
            Near    => self.clip_polygon(in_polygon, out_polygon, Z, -N::ONE, |n, w| n < -w),
        }
    }

    fn clip_polygon(&mut self, in_polygon: &[StagedVertex], out_polygon: &mut Vec<StagedVertex>, dim: usize, val: N, clips: fn(N, N) -> bool) -> bool {
        let mut clip = false;
        for n in 0..in_polygon.len() {
            let vtx_a = &in_polygon[n];
            let vtx_b = &in_polygon[(n+1) % in_polygon.len()];

            let wa = vtx_a.position.w();
            let wb = vtx_b.position.w();

            if !clips(vtx_a.position.elements[dim], wa) && !clips(vtx_b.position.elements[dim], wb) {
                // No clip.
                out_polygon.push(vtx_a.clone());
            } else if !clips(vtx_a.position.elements[dim], wa) && clips(vtx_b.position.elements[dim], wb) {
                // B clips
                let over = vtx_a.position.elements[dim] - (wa * val);
                let under = (wb * val) - vtx_b.position.elements[dim] + over;
                let factor = over.to_fixed::<I40F24>() / under.to_fixed::<I40F24>();

                let clip_vtx_b = Self::interpolate(vtx_a, vtx_b, factor);
                out_polygon.push(vtx_a.clone());
                out_polygon.push(clip_vtx_b);
                clip = true;
            } else if clips(vtx_a.position.elements[dim], wa) && !clips(vtx_b.position.elements[dim], wb) {
                // A clips
                let over = vtx_a.position.elements[dim] - (wa * val);
                let under = (wb * val) - vtx_b.position.elements[dim] + over;
                let factor = over.to_fixed::<I40F24>() / under.to_fixed::<I40F24>();

                let clip_vtx_a = Self::interpolate(vtx_a, vtx_b, factor);
                out_polygon.push(clip_vtx_a);
                clip = true;
            } else {
                // both points clip.
                clip = true
            }
        }
        clip
    }

    fn interpolate(vtx_a: &StagedVertex, vtx_b: &StagedVertex, factor: I40F24) -> StagedVertex {
        StagedVertex {
            position: Vector::new([
                interpolate(vtx_a.position.x(), vtx_b.position.x(), factor),
                interpolate(vtx_a.position.y(), vtx_b.position.y(), factor),
                interpolate(vtx_a.position.z(), vtx_b.position.z(), factor),
                interpolate(vtx_a.position.w(), vtx_b.position.w(), factor),
            ]),
            colour: Colour {
                r: interpolate(vtx_a.colour.r.to_fixed::<I16F0>(), vtx_b.colour.r.to_fixed::<I16F0>(), factor).to_num(),
                g: interpolate(vtx_a.colour.g.to_fixed::<I16F0>(), vtx_b.colour.g.to_fixed::<I16F0>(), factor).to_num(),
                b: interpolate(vtx_a.colour.b.to_fixed::<I16F0>(), vtx_b.colour.b.to_fixed::<I16F0>(), factor).to_num(),
            },
            tex_coords: TexCoords {
                s: interpolate(vtx_a.tex_coords.s, vtx_b.tex_coords.s, factor),
                t: interpolate(vtx_a.tex_coords.t, vtx_b.tex_coords.t, factor),
            }
        }
    }

    fn _test_one_dot_display(&self, attrs: &PolygonAttrs, vertices: &[Vertex], dot_w: Depth) -> bool {
        if !attrs.contains(PolygonAttrs::RENDER_DOT) {
            let v0 = &vertices[0];
            let screen_x = v0.screen_p.x.to_num::<i16>();
            let screen_y = v0.screen_p.y.to_num::<i16>();
            for vn in vertices.iter().skip(1) {
                if vn.screen_p.x.to_num::<i16>() != screen_x ||
                    vn.screen_p.y.to_num::<i16>() != screen_y {
                    // Not a one-dot polygon.
                    return true;
                }
            }
            // If the smallest dot w is larger than the test value,
            // DO NOT output this polygon.
            dot_w <= self.dot_polygon_w
        } else {
            true
        }
    }
}

#[inline]
fn interpolate<T: Fixed>(a: T, b: T, factor: I40F24) -> T {
    if a == b {
        a
    } else {
        let offset = (b.to_fixed::<I40F24>() - a.to_fixed::<I40F24>()) * factor;
        (a.to_fixed::<I40F24>() + offset).to_fixed::<T>()
    }
}

/// Test winding for the current polygon.
/// This checks if the front or back face is showing,
/// and if that face should be displayed.
/// 
/// Returns true if the polygon should be shown.
fn test_winding(polygon: &Polygon, vertices: &[Vertex]) -> bool {
    let size = (0..vertices.len()).fold(0_i32, |acc, n| {
        let current_index = n;
        let next_index = (n + 1) % vertices.len();

        let v0 = &vertices[current_index];
        let v1 = &vertices[next_index];
        let segment_size = (v1.screen_p.x - v0.screen_p.x).to_num::<i32>() * (v1.screen_p.y + v0.screen_p.y).to_num::<i32>();
        acc + segment_size
    });

    if size > 0 {
        polygon.attrs.contains(PolygonAttrs::RENDER_FRONT)
    } else if size < 0 {
        polygon.attrs.contains(PolygonAttrs::RENDER_BACK)
    } else {
        // Always display line polygons.
        true
    }
}