use std::{
    collections::BTreeSet,
    cmp::Ordering
};
use bitflags::bitflags;
use fixed::types::{I12F4, I23F9};
use crate::{
    utils::bits::u32,
    common::colour::Colour
};

bitflags! {
    #[derive(Default)]
    pub struct Display3DControl: u32 {
        const CLEAR_IMAGE       = u32::bit(14);
        const LIST_RAM_OVERFLOW = u32::bit(13);
        const LINE_UNDERFLOW    = u32::bit(12);
        const FOG_SHIFT         = u32::bits(8, 11);
        const FOG_ENABLE        = u32::bit(7);
        const FOG_MODE          = u32::bit(6);
        const EDGE_MARKING      = u32::bit(5);
        const ANTI_ALIAS        = u32::bit(4);
        const BLENDING_ENABLE   = u32::bit(3);
        const ALPHA_TEST_ENABLE = u32::bit(2);
        const HIGHLIGHT_SHADING = u32::bit(1);
        const TEX_MAP_ENABLE    = u32::bit(0);
    }
}
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

impl PolygonAttrs {
    pub fn alpha(self) -> u8 {
        ((self & PolygonAttrs::ALPHA).bits() >> 16) as u8
    }
    
    pub fn id(self) -> u8 {
        ((self & PolygonAttrs::POLYGON_ID).bits() >> 24) as u8
    }
    
    pub fn mode(self) -> u8 {
        ((self & PolygonAttrs::POLYGON_MODE).bits() >> 4) as u8
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

impl TextureAttrs {
    pub fn addr(self) -> u32 {
        (self & TextureAttrs::ADDR).bits() as u32
    }
    
    pub fn format(self) -> u8 {
        ((self & TextureAttrs::FORMAT).bits() >> 26) as u8
    }
}

#[derive(Eq)]
pub struct PolygonOrder {
    pub y_max:    I12F4, // In screen space (0: top, 191: bottom)
    pub y_min:    I12F4, // In screen space (0: top, 191: bottom)
    pub polygon_index:  usize,
}

impl PartialEq for PolygonOrder {
    fn eq(&self, other: &Self) -> bool {
        self.y_max == other.y_max &&
        self.y_min == other.y_min &&
        self.polygon_index == other.polygon_index
    }
}

impl PartialOrd for PolygonOrder {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PolygonOrder {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.y_max.cmp(&other.y_max) {
            Ordering::Equal => match self.y_min.cmp(&other.y_min) {
                Ordering::Equal => self.polygon_index.cmp(&other.polygon_index),
                o => o
            },
            o => o
        }
    }
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
    pub x_max:          I12F4,
    pub x_min:          I12F4,
    pub vertex_indices: Vec<usize>
}

/// Coordinates for a point in screen-space.
#[derive(Default, Clone, Copy)]
pub struct Coords {
    pub x: I12F4,   // also tex s
    pub y: I12F4    // also tex t
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
    pub screen_p:   Coords,
    pub depth:      I23F9,
    pub colour:     Colour,
    pub tex_coords: Coords,
}

/// Polygon and vertex RAM for a frame.
/// 
/// Contains polygon order, polygon metadata, and vertex data.
pub struct PolygonRAM {
    pub opaque_polygons:    BTreeSet<PolygonOrder>,
    pub trans_polygons:     BTreeSet<PolygonOrder>,
    pub polygons:           Vec<Polygon>,
    pub vertices:           Vec<Vertex>
}

impl PolygonRAM {
    pub fn new() -> Self {
        Self {
            opaque_polygons: BTreeSet::new(),
            trans_polygons: BTreeSet::new(),
            polygons: Vec::new(),
            vertices: Vec::new()
        }
    }

    /// Clear the polygon and vertex RAM for the next geometry engine write cycle.
    pub fn clear(&mut self) {
        self.opaque_polygons.clear();
        self.trans_polygons.clear();
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
    pub fn insert_polygon(&mut self, polygon: Polygon, y_max: I12F4, y_min: I12F4) {
        let alpha = polygon.attrs.alpha();
        let polygon_index = self.polygons.len();

        let polygon_order = PolygonOrder {
            y_max, y_min, polygon_index
        };
        
        self.polygons.push(polygon);

        if alpha == 31 || alpha == 0 {
            self.opaque_polygons.insert(polygon_order);
        } else {
            self.trans_polygons.insert(polygon_order);
        }
    }
}
