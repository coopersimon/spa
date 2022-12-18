
use fixed::types::I40F24;
use fixed::traits::ToFixed;
use crate::common::video::colour::Colour;
use super::{
    super::interpolate::*,
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
        self.viewport_width = N::from_num(1 + (bytes[2] as u16)) - self.viewport_x;
        self.viewport_height = N::from_num(1 + (bytes[3] as u16)) - self.viewport_y;
    }

    pub fn set_w_buffer(&mut self, w_buffer: bool) {
        self.w_buffer = w_buffer;
    }

    /// Add a polygon and vertices to the vertex list RAM.
    /// 
    /// Also make a note of its index in the current polygon.
    pub fn add_polygon(&mut self, mut staged_polygon: Polygon, vertices: &mut [StagedVertex]) {
        let (mut min_y, mut max_y) = (N::MAX, N::ZERO);

        let vertices_out = vertices.iter().map(|vertex| {

            let w = vertex.position.w().to_fixed::<I40F24>();
            let w2 = (w * 2).checked_recip().unwrap_or(I40F24::MAX);
            let x = (vertex.position.x().to_fixed::<I40F24>() + w) * w2;
            let y = I40F24::ONE - (vertex.position.y().to_fixed::<I40F24>() + w) * w2;
            let z = (vertex.position.z().to_fixed::<I40F24>() + w) * w2;

            let screen_p = self.get_screen_coords(x.to_fixed(), y.to_fixed());
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
        Coords { x: screen_x, y: screen_y }
    }

    pub fn clip(&mut self, plane: ClipPlane, in_polygon: &[StagedVertex], out_polygon: &mut Vec<StagedVertex>) {
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

    fn clip_polygon(&mut self, in_polygon: &[StagedVertex], out_polygon: &mut Vec<StagedVertex>, dim: usize, val: N, clips: fn(N, N) -> bool) {
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
                let over = ((wb * val) - vtx_b.position.elements[dim]).to_fixed::<I40F24>();
                let under = (vtx_a.position.elements[dim] - vtx_b.position.elements[dim] - (wa * val) + (wb * val)).to_fixed::<I40F24>();
                let factor_a = (over / under).clamp(I40F24::ZERO, I40F24::ONE);

                let factor_b = I40F24::ONE - factor_a;
                let position = interpolate_position(&vtx_a.position, &vtx_b.position, factor_a, factor_b);
                
                let clip_vtx_b = Self::interpolate(vtx_a, vtx_b, factor_a.to_fixed(), factor_b.to_fixed(), position);

                out_polygon.push(vtx_a.clone());
                out_polygon.push(clip_vtx_b);
            } else if clips(vtx_a.position.elements[dim], wa) && !clips(vtx_b.position.elements[dim], wb) {
                // A clips
                let over = ((wb * val) - vtx_b.position.elements[dim]).to_fixed::<I40F24>();
                let under = (vtx_a.position.elements[dim] - vtx_b.position.elements[dim] - (wa * val) + (wb * val)).to_fixed::<I40F24>();
                let factor_a = (over / under).clamp(I40F24::ZERO, I40F24::ONE);

                let factor_b = I40F24::ONE - factor_a;
                let position = interpolate_position(&vtx_a.position, &vtx_b.position, factor_a, factor_b);
                
                let clip_vtx_a = Self::interpolate(vtx_a, vtx_b, factor_a.to_fixed(), factor_b.to_fixed(), position);

                out_polygon.push(clip_vtx_a);
            }
            // else: both points clip.
        }
    }

    fn interpolate(vtx_a: &StagedVertex, vtx_b: &StagedVertex, factor_a: N, factor_b: N, position: Vector<4>) -> StagedVertex {
        StagedVertex {
            position: position,
            colour: interpolate_vertex_colour(vtx_a.colour, vtx_b.colour, factor_a, factor_b),
            tex_coords: interpolate_tex_coords(vtx_a.tex_coords, vtx_b.tex_coords, factor_a, factor_b),
            needs_clip: None,
            idx: None
        }
    }
}

fn interpolate_position(position_a: &Vector<4>, position_b: &Vector<4>, factor_a: I40F24, factor_b: I40F24) -> Vector<4> {
    let x = factor_a * position_a.x().to_fixed::<I40F24>() + factor_b * position_b.x().to_fixed::<I40F24>();
    let y = factor_a * position_a.y().to_fixed::<I40F24>() + factor_b * position_b.y().to_fixed::<I40F24>();
    let z = factor_a * position_a.z().to_fixed::<I40F24>() + factor_b * position_b.z().to_fixed::<I40F24>();
    let w = factor_a * position_a.w().to_fixed::<I40F24>() + factor_b * position_b.w().to_fixed::<I40F24>();
    Vector::new([
        x.to_fixed(),
        y.to_fixed(),
        z.to_fixed(),
        w.to_fixed(),
    ])
}

/// Test winding for the current polygon.
/// This checks if the front or back face is showing,
/// and if that face should be displayed.
/// 
/// Returns true if the polygon should be shown.
fn test_winding(polygon: &Polygon, vertices: &[Vertex]) -> bool {
    let size = (0..vertices.len()).fold(N::ZERO, |acc, n| {
        let current_index = n;
        let next_index = (n + 1) % vertices.len();

        let v0 = &vertices[current_index];
        let v1 = &vertices[next_index];
        let segment_size = (v1.screen_p.x - v0.screen_p.x) * (v1.screen_p.y + v0.screen_p.y);
        acc + segment_size
    });

    if size > N::ZERO {
        polygon.attrs.contains(PolygonAttrs::RENDER_FRONT)
    } else if size < N::ZERO {
        polygon.attrs.contains(PolygonAttrs::RENDER_BACK)
    } else {
        // Always display line polygons.
        true
    }
}