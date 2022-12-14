
use fixed::types::I40F24;
use fixed::traits::ToFixed;
use crate::common::video::colour::Colour;
use super::{
    super::interpolate::*,
    super::types::*,
    math::{N, Vector},
};

#[derive(Clone, Copy, Debug)]
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
        const ALL: [ClipPlane; 6] = [Far, Near, Bottom, Top, Left, Right];
        &ALL
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

pub struct StagedPolygon {
    pub polygon:    Polygon,
    pub max_y:      N,
    pub min_y:      N,
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
    
    pub fn get_screen_coords(&self, x: N, y: N) -> Coords {
        let screen_x = self.viewport_x + (x * self.viewport_width);
        let screen_y = self.viewport_y + (y * self.viewport_height);
        Coords { x: screen_x, y: screen_y }
    }

    /// Add a vertex to the vertex list RAM.
    /// 
    /// Also make a note of its index in the current polygon.
    pub fn add_vertex(&mut self, staged_polygon: &mut StagedPolygon, vertex: &mut StagedVertex) {
        let screen_p = self.get_screen_coords(vertex.position.x(), vertex.position.y());
        staged_polygon.max_y = std::cmp::max(staged_polygon.max_y, screen_p.y);
        staged_polygon.min_y = std::cmp::min(staged_polygon.min_y, screen_p.y);
        let idx = if let Some(idx) = vertex.idx {
            idx
        } else {
            let depth = if self.w_buffer {
                vertex.position.w().to_fixed::<Depth>()
            } else {
                (vertex.position.z() * 0x7FFF).to_fixed::<Depth>()
            };
            let out_vertex = Vertex {
                screen_p,
                depth: depth,
                colour: vertex.colour,
                tex_coords: vertex.tex_coords
            };
            let idx = self.polygon_ram.insert_vertex(out_vertex);
            vertex.idx = Some(idx);
            idx
        };
        staged_polygon.polygon.add_vertex_index(idx);
    }
    
    pub fn clip(&mut self, plane: ClipPlane, in_polygon: &[StagedVertex], out_polygon: &mut Vec<StagedVertex>) {
        use ClipPlane::*;
        const X: usize = 0;
        const Y: usize = 1;
        const Z: usize = 2;
        match plane {
            Bottom  => self.clip_polygon(in_polygon, out_polygon, Y, N::ZERO, |n| n < N::ZERO),
            Top     => self.clip_polygon(in_polygon, out_polygon, Y, N::ONE, |n| n > N::ONE),
            Left    => self.clip_polygon(in_polygon, out_polygon, X, N::ZERO, |n| n < N::ZERO),
            Right   => self.clip_polygon(in_polygon, out_polygon, X, N::ONE, |n| n > N::ONE),
            Far     => self.clip_polygon(in_polygon, out_polygon, Z, N::ONE, |n| n > N::ONE),
            Near    => self.clip_polygon(in_polygon, out_polygon, Z, N::ZERO, |n| n < N::ZERO),
        }
    }

    fn clip_polygon(&mut self, in_polygon: &[StagedVertex], out_polygon: &mut Vec<StagedVertex>, dim: usize, val: N, clips: fn(N) -> bool) {
        for n in 0..in_polygon.len() {
            let vtx_a = &in_polygon[n];
            let vtx_b = &in_polygon[(n+1) % in_polygon.len()];

            if !clips(vtx_a.position.elements[dim]) && !clips(vtx_b.position.elements[dim]) {
                // No clip.
                out_polygon.push(vtx_a.clone());
            } else if !clips(vtx_a.position.elements[dim]) && clips(vtx_b.position.elements[dim]) {
                // B clips
                let factor_a = ((val-vtx_b.position.elements[dim]).to_fixed::<I40F24>() / (vtx_a.position.elements[dim] - vtx_b.position.elements[dim]).to_fixed::<I40F24>()).to_fixed::<N>();
                let mut position = interpolate_position(&vtx_a.position, &vtx_b.position, factor_a);
                position.elements[dim] = val;
                
                let factor_b = N::ONE - factor_a;
                let clip_vtx_b = Self::interpolate(vtx_a, vtx_b, factor_a, factor_b, position);

                out_polygon.push(vtx_a.clone());
                out_polygon.push(clip_vtx_b);
            } else if clips(vtx_a.position.elements[dim]) && !clips(vtx_b.position.elements[dim]) {
                // A clips
                let factor_a = ((val-vtx_b.position.elements[dim]).to_fixed::<I40F24>() / (vtx_a.position.elements[dim] - vtx_b.position.elements[dim]).to_fixed::<I40F24>()).to_fixed::<N>();
                let mut position = interpolate_position(&vtx_a.position, &vtx_b.position, factor_a);
                position.elements[dim] = val;
                
                let factor_b = N::ONE - factor_a;
                let clip_vtx_a = Self::interpolate(vtx_a, vtx_b, factor_a, factor_b, position);

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

fn interpolate_position(position_a: &Vector<4>, position_b: &Vector<4>, factor_a: N) -> Vector<4> {
    Vector::new([
        (factor_a * (position_a.x() - position_b.x())) + position_b.x(),
        (factor_a * (position_a.y() - position_b.y())) + position_b.y(),
        (factor_a * (position_a.z() - position_b.z())) + position_b.z(),
        position_b.w()//(factor_a * (position_a.w() - position_b.w())) + position_b.w()
    ])
}
