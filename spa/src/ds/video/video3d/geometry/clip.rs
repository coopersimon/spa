
use fixed::types::{I16F0, I40F24, I12F4};
use fixed::traits::ToFixed;
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

    pub needs_clip: Option<bool>,
    pub idx:        Option<u16>,
}

pub struct ClippingUnit {
    pub polygon_ram:    Box<PolygonRAM>,

    /// Use W or Z value for depth-buffering.
    w_buffer:           bool,

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

    /// Add a polygon and vertices to the vertex list RAM.
    /// 
    /// Also make a note of its index in the current polygon.
    pub fn add_polygon(&mut self, mut staged_polygon: Polygon, vertices: &mut [StagedVertex]) {
        let (mut min_y, mut max_y) = (I16F0::MAX, I16F0::ZERO);

        let vertices_out = vertices.iter().map(|vertex| {

            let w = vertex.position.w().to_fixed::<I40F24>();
            let w2 = (w * 2).checked_recip().unwrap_or(I40F24::MAX);
            let x = (vertex.position.x().to_fixed::<I40F24>() + w) * w2;
            let y = (vertex.position.y().to_fixed::<I40F24>() + w) * w2;
            let z = (vertex.position.z().to_fixed::<I40F24>() + w) * w2;

            let screen_p = self.get_screen_coords(x.to_fixed(), N::ONE - y.to_fixed::<N>());
            max_y = std::cmp::max(max_y, screen_p.y);
            min_y = std::cmp::min(min_y, screen_p.y);
            // TODO: re-use vertices...
            /*let idx = if let Some(idx) = vertex.idx {
                idx
            } else {*/
                let depth = if self.w_buffer {
                    w.to_fixed::<Depth>()
                } else {
                    (z * 0x7FFF).to_fixed::<Depth>()
                };
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

        if test_winding(&staged_polygon, &vertices_out) {
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
        let x_offset = (vtx_b.position.x() - vtx_a.position.x()).to_fixed::<I40F24>() * factor;
        let y_offset = (vtx_b.position.y() - vtx_a.position.y()).to_fixed::<I40F24>() * factor;
        let z_offset = (vtx_b.position.z() - vtx_a.position.z()).to_fixed::<I40F24>() * factor;
        let w_offset = (vtx_b.position.w() - vtx_a.position.w()).to_fixed::<I40F24>() * factor;
        let r_offset = (vtx_b.colour.r.to_fixed::<I40F24>() - vtx_a.colour.r.to_fixed::<I40F24>()) * factor;
        let g_offset = (vtx_b.colour.g.to_fixed::<I40F24>() - vtx_a.colour.g.to_fixed::<I40F24>()) * factor;
        let b_offset = (vtx_b.colour.b.to_fixed::<I40F24>() - vtx_a.colour.b.to_fixed::<I40F24>()) * factor;
        let tex_s_offset = (vtx_b.tex_coords.s - vtx_a.tex_coords.s).to_fixed::<I40F24>() * factor;
        let tex_t_offset = (vtx_b.tex_coords.t - vtx_a.tex_coords.t).to_fixed::<I40F24>() * factor;
        StagedVertex {
            position: Vector::new([
                vtx_a.position.x() + x_offset.to_fixed::<N>(),
                vtx_a.position.y() + y_offset.to_fixed::<N>(),
                vtx_a.position.z() + z_offset.to_fixed::<N>(),
                vtx_a.position.w() + w_offset.to_fixed::<N>()
            ]),
            colour: Colour {
                r: (vtx_a.colour.r.to_fixed::<I40F24>() + r_offset).to_num(),
                g: (vtx_a.colour.g.to_fixed::<I40F24>() + g_offset).to_num(),
                b: (vtx_a.colour.b.to_fixed::<I40F24>() + b_offset).to_num()
            },
            tex_coords: TexCoords {
                s: vtx_a.tex_coords.s + tex_s_offset.to_fixed::<I12F4>(),
                t: vtx_a.tex_coords.t + tex_t_offset.to_fixed::<I12F4>()
            },
            needs_clip: None,
            idx: None
        }
    }
}

/// Test winding for the current polygon.
/// This checks if the front or back face is showing,
/// and if that face should be displayed.
/// 
/// Returns true if the polygon should be shown.
fn test_winding(polygon: &Polygon, vertices: &[Vertex]) -> bool {
    let size = (0..vertices.len()).fold(I16F0::ZERO, |acc, n| {
        let current_index = n;
        let next_index = (n + 1) % vertices.len();

        let v0 = &vertices[current_index];
        let v1 = &vertices[next_index];
        let segment_size = (v1.screen_p.x - v0.screen_p.x) * (v1.screen_p.y + v0.screen_p.y);
        acc + segment_size
    });

    if size > I16F0::ZERO {
        polygon.attrs.contains(PolygonAttrs::RENDER_FRONT)
    } else if size < I16F0::ZERO {
        polygon.attrs.contains(PolygonAttrs::RENDER_BACK)
    } else {
        // Always display line polygons.
        true
    }
}