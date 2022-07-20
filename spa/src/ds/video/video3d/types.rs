
use bitflags::bitflags;
use crate::{
    utils::bits::u32,
    common::colour::Colour
};

pub enum PrimitiveType {
    Triangle,
    TriangleStrip,
    Quad,
    QuadStrip
}

bitflags! {
    #[derive(Default)]
    pub struct PolygonAttrs: u32 {
        const POLYGON_ID        = u32::bits(24, 29);
        const ALPHA             = u32::bits(16, 20);
        const FOG_BLEND_ENABLE  = u32::bit(15);
        const RENDER_EQ_DEPTH   = u32::bit(14);
        const RENDER_DOT        = u32::bit(13);
        const FAR_PLANE_CLIP    = u32::bit(12);
        const ALPHA_DEPTH       = u32::bit(11);
        const RENDER_FRONT      = u32::bit(7);
        const RENDER_BACK       = u32::bit(6);
        const POLYGON_MODE      = u32::bits(4, 5);
        const ENABLE_LIGHT_3    = u32::bit(3);
        const ENABLE_LIGHT_2    = u32::bit(2);
        const ENABLE_LIGHT_1    = u32::bit(1);
        const ENABLE_LIGHT_0    = u32::bit(0);
    }
}

bitflags! {
    #[derive(Default)]
    pub struct TextureAttrs: u32 {
        const TEX_COORD_TRANS   = u32::bits(30, 31);
        const TRANSPARENT_0     = u32::bit(29);
        const FORMAT            = u32::bits(26, 28);
        const SIZE_T            = u32::bits(23, 25);
        const SIZE_S            = u32::bits(20, 22);
        const FLIP_T            = u32::bit(19);
        const FLIP_S            = u32::bit(18);
        const REPEAT_T          = u32::bit(17);
        const REPEAT_S          = u32::bit(16);
        const ADDR              = u32::bits(0, 15);
    }
}

// TODO
pub struct PolygonOrder {

}

/// A polygon. 12 + 8/12 bytes.
/// 
/// Contains:
/// - Primitive type
/// - Polygon attributes
/// - Texture attributes
/// - Texture palette
/// - Vertex indices
pub struct Polygon {
    pub attrs:          PolygonAttrs,
    pub tex:            TextureAttrs,
    pub palette:        u16,
    pub use_quads:      bool,
    pub vertex_index:   usize,
}

/// A single vertex. 12 bytes.
/// 
/// Contains:
/// - Screenspace coords
/// - Depth
/// - Colour
/// - Texture coords
#[derive(Default, Clone)]
pub struct Vertex {
    pub screen_x:   u16,
    pub screen_y:   u16,
    pub depth:      u32,
    pub colour:     Colour,
    pub tex_s:      u16,
    pub tex_t:      u16
}

/// Polygon and vertex RAM for a frame.
/// 
/// Contains polygon order, polygon metadata, and vertex data.
pub struct PolygonRAM {
    pub order:      Vec<PolygonOrder>,
    pub polygons:   Vec<Polygon>,
    pub vertices:   Vec<Vertex>
}

impl PolygonRAM {
    pub fn new() -> Self {
        Self { order: Vec::new(), polygons: Vec::new(), vertices: Vec::new() }
    }

    /// Clear the polygon and vertex RAM for the next geometry engine write cycle.
    pub fn clear(&mut self) {
        self.order.clear();
        self.polygons.clear();
        self.vertices.clear();
    }

    /// Insert a vertex.
    /// 
    /// Returns the index to the vertex.
    pub fn insert_vertex(&mut self, vertex: Vertex) -> usize {
        let index = self.vertices.len();
        self.vertices.push(vertex);
        index
    }

    /// Insert a polygon.
    pub fn insert_polygon(&mut self, polygon: Polygon) {
        self.polygons.push(polygon);
    }
}
