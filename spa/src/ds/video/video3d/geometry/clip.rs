
use fixed::types::I40F24;
use fixed::traits::ToFixed;
use crate::common::colour::Colour;
use super::{
    super::interpolate::*,
    super::types::*,
    math::{N, Vector},
};

enum ClipPlane {
    Bottom = 0,
    Top = 1,
    Left = 2,
    Right = 3
}

#[derive(Clone, Default)]
pub struct StagedVertex {
    pub position:   Vector<4>,
    pub screen_p:   Coords,
    pub colour:     Colour,
    pub tex_coords: TexCoords,
    pub depth:      Depth,

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

    viewport_x:         u8,
    viewport_y:         u8,
    viewport_width:     u16,
    viewport_height:    u16,
}

impl ClippingUnit {
    pub fn new() -> Self {
        Self {
            polygon_ram:    Box::new(PolygonRAM::new()),

            viewport_x:         0,
            viewport_y:         0,
            viewport_width:     0,
            viewport_height:    0,
        }
    }

    pub fn set_viewport(&mut self, data: u32) {
        let bytes = u32::to_le_bytes(data);
        self.viewport_x = bytes[0];
        self.viewport_y = bytes[1];
        self.viewport_width = (bytes[2] - self.viewport_x) as u16;
        self.viewport_height = (bytes[3] - self.viewport_y) as u16;
    }
    
    pub fn get_screen_coords(&self, x: N, y: N) -> Coords {
        let screen_x = N::from_num(self.viewport_x) + (x * N::from_num(self.viewport_width));
        let screen_y = N::from_num(self.viewport_y) + (y * N::from_num(self.viewport_height));
        Coords { x: screen_x, y: screen_y }
    }

    /// Add a vertex to the vertex list RAM.
    /// 
    /// Also make a note of its index in the current polygon.
    pub fn add_vertex(&mut self, staged_polygon: &mut StagedPolygon, vertex: Vertex) {
        staged_polygon.max_y = std::cmp::max(staged_polygon.max_y, vertex.screen_p.y);
        staged_polygon.min_y = std::cmp::min(staged_polygon.min_y, vertex.screen_p.y);
        let idx = self.polygon_ram.insert_vertex(vertex);
        staged_polygon.polygon.add_vertex_index(idx);
    }
    
    /// Clip point a, based on the line between "a" and "b".
    /// 
    /// Find all points where the planes that "a" clips are intersected
    /// by the line a->b.
    /// 
    /// Each intersection point in view is added to the vertex list.
    /// Corner points can also be generated (in the corners of the view).
    /// 
    /// out_clips keeps track of clipping boundaries outside of the view,
    /// in_clips keeps track of clipping boundaries within the view.
    /// When a pair of out and in clip a boundary, a corner point can be
    /// generated.
    /// 
    /// If the line is formed with the previous point in the vertex list,
    /// i.e. "b" comes before "a" in the output order,
    /// then corners should be emitted after view edge intersections.
    /// Otherwise emit corners before edge intersections.
    /// This is important to preserve winding order.
    pub fn clip(
        &mut self, prev: bool,
        vtx_a: &StagedVertex, vtx_b: &StagedVertex,
        out_clips: &mut [Option<Vertex>; 4], in_clips: &mut [Option<Vertex>; 4],
        staged_polygon: &mut StagedPolygon
    ) {
        
        // TODO: const the below.
        let X_MAX = N::ONE;// - N::from_bits(0b1);
        let Y_MAX = N::ONE;// - N::from_bits(0b1);

        let mut corner_vtx: Option<Vertex> = None;
        let mut edge_vtx: Option<Vertex> = None;

        if vtx_a.position.x() < N::ZERO && vtx_b.position.x() >= N::ZERO {
            // Clip on left plane.
            let factor_a = (-vtx_b.position.x().to_fixed::<I40F24>() / (vtx_a.position.x() - vtx_b.position.x()).to_fixed::<I40F24>()).to_fixed::<N>();
            let y = (factor_a * (vtx_a.position.y() - vtx_b.position.y())) + vtx_b.position.y();
            
            let factor_b = N::ONE - factor_a;
            let vtx = Self::interpolate(vtx_a, vtx_b, factor_a, factor_b, self.get_screen_coords(N::ZERO, y));

            if y >= N::ZERO && y <= N::ONE {
                // Point clips in view.
                if let Some(out_vtx) = std::mem::take(&mut out_clips[ClipPlane::Left as usize]) {
                    let coords = if out_vtx.screen_p.y > vtx.screen_p.y {
                        self.get_screen_coords(N::ZERO, Y_MAX)
                    } else {
                        self.get_screen_coords(N::ZERO, N::ZERO)
                    };
                    corner_vtx = Some(Self::interpolate_corner_y(&vtx, &out_vtx, coords));
                } else {
                    in_clips[ClipPlane::Left as usize] = Some(vtx.clone());
                }
                edge_vtx = Some(vtx);
            } else {
                // Point clips out of view.
                if let Some(in_vtx) = std::mem::take(&mut in_clips[ClipPlane::Left as usize]) {
                    let coords = if vtx.screen_p.y > in_vtx.screen_p.y {
                        self.get_screen_coords(N::ZERO, Y_MAX)
                    } else {
                        self.get_screen_coords(N::ZERO, N::ZERO)
                    };
                    corner_vtx = Some(Self::interpolate_corner_y(&vtx, &in_vtx, coords));
                } else {
                    out_clips[ClipPlane::Left as usize] = Some(vtx);
                }
            }
        } else if vtx_a.position.x() > N::ONE && vtx_b.position.x() <= N::ONE {
            // Clip on right plane.
            let factor_a = ((X_MAX - vtx_b.position.x()).to_fixed::<I40F24>() / (vtx_a.position.x() - vtx_b.position.x()).to_fixed::<I40F24>()).to_fixed::<N>();
            let y = (factor_a * (vtx_a.position.y() - vtx_b.position.y())) + vtx_b.position.y();
            
            let factor_b = N::ONE - factor_a;
            let vtx = Self::interpolate(vtx_a, vtx_b, factor_a, factor_b, self.get_screen_coords(X_MAX, y));

            if y >= N::ZERO && y <= N::ONE {
                // Point clips in view.
                if let Some(out_vtx) = std::mem::take(&mut out_clips[ClipPlane::Right as usize]) {
                    let coords = if out_vtx.screen_p.y > vtx.screen_p.y {
                        self.get_screen_coords(X_MAX, Y_MAX)
                    } else {
                        self.get_screen_coords(X_MAX, N::ZERO)
                    };
                    corner_vtx = Some(Self::interpolate_corner_y(&vtx, &out_vtx, coords));
                } else {
                    in_clips[ClipPlane::Right as usize] = Some(vtx.clone());
                }
                edge_vtx = Some(vtx);
            } else {
                // Point clips out of view.
                if let Some(in_vtx) = std::mem::take(&mut in_clips[ClipPlane::Right as usize]) {
                    let coords = if vtx.screen_p.y > in_vtx.screen_p.y {
                        self.get_screen_coords(X_MAX, N::ONE)
                    } else {
                        self.get_screen_coords(X_MAX, N::ZERO)
                    };
                    corner_vtx = Some(Self::interpolate_corner_y(&vtx, &in_vtx, coords));
                } else {
                    out_clips[ClipPlane::Right as usize] = Some(vtx);
                }
            }
        }
        
        if vtx_a.position.y() < N::ZERO && vtx_b.position.y() >= N::ZERO {
            // Clip on top plane.
            let factor_a = (-vtx_b.position.y().to_fixed::<I40F24>() / (vtx_a.position.y() - vtx_b.position.y()).to_fixed::<I40F24>()).to_fixed::<N>();
            let x = (factor_a * (vtx_a.position.x() - vtx_b.position.x())) + vtx_b.position.x();
            
            let factor_b = N::ONE - factor_a;
            let vtx = Self::interpolate(vtx_a, vtx_b, factor_a, factor_b, self.get_screen_coords(x, N::ZERO));

            if x >= N::ZERO && x <= N::ONE {
                // Point clips in view.
                if let Some(out_vtx) = std::mem::take(&mut out_clips[ClipPlane::Top as usize]) {
                    let coords = if out_vtx.screen_p.x > vtx.screen_p.x {
                        self.get_screen_coords(X_MAX, N::ZERO)
                    } else {
                        self.get_screen_coords(N::ZERO, N::ZERO)
                    };
                    corner_vtx = Some(Self::interpolate_corner_x(&vtx, &out_vtx, coords));
                } else {
                    in_clips[ClipPlane::Top as usize] = Some(vtx.clone());
                }
                edge_vtx = Some(vtx);
            } else {
                // Point clips out of view.
                if let Some(in_vtx) = std::mem::take(&mut in_clips[ClipPlane::Top as usize]) {
                    let coords = if vtx.screen_p.x > in_vtx.screen_p.x {
                        self.get_screen_coords(X_MAX, N::ZERO)
                    } else {
                        self.get_screen_coords(N::ZERO, N::ZERO)
                    };
                    corner_vtx = Some(Self::interpolate_corner_x(&vtx, &in_vtx, coords));
                } else {
                    out_clips[ClipPlane::Top as usize] = Some(vtx);
                }
            }
        } else if vtx_a.position.y() > N::ONE && vtx_b.position.y() <= N::ONE {
            // Clip on bottom plane.
            let factor_a = ((Y_MAX - vtx_b.position.y()).to_fixed::<I40F24>() / (vtx_a.position.y() - vtx_b.position.y()).to_fixed::<I40F24>()).to_fixed::<N>();
            let x = (factor_a * (vtx_a.position.x() - vtx_b.position.x())) + vtx_b.position.x();
            
            let factor_b = N::ONE - factor_a;
            let vtx = Self::interpolate(vtx_a, vtx_b, factor_a, factor_b, self.get_screen_coords(x, Y_MAX));

            if x >= N::ZERO && x <= N::ONE {
                // Point clips in view.
                if let Some(out_vtx) = std::mem::take(&mut out_clips[ClipPlane::Bottom as usize]) {
                    let coords = if out_vtx.screen_p.x > vtx.screen_p.x {
                        self.get_screen_coords(X_MAX, Y_MAX)
                    } else {
                        self.get_screen_coords(N::ZERO, Y_MAX)
                    };
                    corner_vtx = Some(Self::interpolate_corner_x(&vtx, &out_vtx, coords));
                } else {
                    in_clips[ClipPlane::Bottom as usize] = Some(vtx.clone());
                }
                edge_vtx = Some(vtx);
            } else {
                // Point clips out of view.
                if let Some(in_vtx) = std::mem::take(&mut in_clips[ClipPlane::Bottom as usize]) {
                    let coords = if vtx.screen_p.x > in_vtx.screen_p.x {
                        self.get_screen_coords(X_MAX, Y_MAX)
                    } else {
                        self.get_screen_coords(N::ZERO, Y_MAX)
                    };
                    corner_vtx = Some(Self::interpolate_corner_x(&vtx, &in_vtx, coords));
                } else {
                    out_clips[ClipPlane::Bottom as usize] = Some(vtx);
                }
            }
        }

        if prev {
            // Emit edge before corner, when "b" appears before "a" in the output order.
            if let Some(vertex) = edge_vtx {
                self.add_vertex(staged_polygon, vertex);
            }
            if let Some(vertex) = corner_vtx {
                self.add_vertex(staged_polygon, vertex);
            }
        } else {
            if let Some(vertex) = corner_vtx {
                self.add_vertex(staged_polygon, vertex);
            }
            if let Some(vertex) = edge_vtx {
                self.add_vertex(staged_polygon, vertex);
            }
        }
    }
    
    fn interpolate(vtx_a: &StagedVertex, vtx_b: &StagedVertex, factor_a: N, factor_b: N, screen_p: Coords) -> Vertex {
        Vertex {
            screen_p: screen_p,
            colour: interpolate_vertex_colour(vtx_a.colour, vtx_b.colour, factor_a, factor_b),
            tex_coords: interpolate_tex_coords(vtx_a.tex_coords, vtx_b.tex_coords, factor_a, factor_b),
            depth: interpolate_depth(vtx_a.depth, vtx_b.depth, factor_a, factor_b),
        }
    }
    
    fn interpolate_corner_x(vtx_a: &Vertex, vtx_b: &Vertex, screen_p: Coords) -> Vertex {
        let factor_a = ((screen_p.x - vtx_b.screen_p.x).to_fixed::<I40F24>() / (vtx_a.screen_p.x - vtx_b.screen_p.x).to_fixed::<I40F24>()).to_fixed::<N>();
        let factor_b = N::ONE - factor_a;
        Vertex {
            screen_p: screen_p,
            colour: interpolate_vertex_colour(vtx_a.colour, vtx_b.colour, factor_a, factor_b),
            tex_coords: interpolate_tex_coords(vtx_a.tex_coords, vtx_b.tex_coords, factor_a, factor_b),
            depth: interpolate_depth(vtx_a.depth, vtx_b.depth, factor_a, factor_b),
        }
    }
    
    fn interpolate_corner_y(vtx_a: &Vertex, vtx_b: &Vertex, screen_p: Coords) -> Vertex {
        let factor_a = ((screen_p.y - vtx_b.screen_p.y).to_fixed::<I40F24>() / (vtx_a.screen_p.y - vtx_b.screen_p.y).to_fixed::<I40F24>()).to_fixed::<N>();
        let factor_b = N::ONE - factor_a;
        Vertex {
            screen_p: screen_p,
            colour: interpolate_vertex_colour(vtx_a.colour, vtx_b.colour, factor_a, factor_b),
            tex_coords: interpolate_tex_coords(vtx_a.tex_coords, vtx_b.tex_coords, factor_a, factor_b),
            depth: interpolate_depth(vtx_a.depth, vtx_b.depth, factor_a, factor_b),
        }
    }
}
