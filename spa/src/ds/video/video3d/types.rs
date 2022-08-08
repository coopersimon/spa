use std::{
    collections::BTreeSet,
    cmp::Ordering
};
use bitflags::bitflags;
use fixed::types::{I12F4, I23F9};
use super::geometry::N;
use crate::{
    utils::bits::{u16, u32},
    common::colour::Colour
};

bitflags! {
    #[derive(Default)]
    pub struct Display3DControl: u16 {
        const CLEAR_IMAGE       = u16::bit(14);
        const LIST_RAM_OVERFLOW = u16::bit(13);
        const LINE_UNDERFLOW    = u16::bit(12);
        const FOG_SHIFT         = u16::bits(8, 11);
        const FOG_ENABLE        = u16::bit(7);
        const FOG_MODE          = u16::bit(6);
        const EDGE_MARKING      = u16::bit(5);
        const ANTI_ALIAS        = u16::bit(4);
        const BLENDING_ENABLE   = u16::bit(3);
        const ALPHA_TEST_ENABLE = u16::bit(2);
        const HIGHLIGHT_SHADING = u16::bit(1);
        const TEX_MAP_ENABLE    = u16::bit(0);
    }
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
    pub y_max:    N, // In screen space (0: top, 191: bottom)
    pub y_min:    N, // In screen space (0: top, 191: bottom)
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
    pub vertex_indices: Vec<usize>
}

impl Polygon {
    /// Check if polygon is opaque.
    /// 
    /// A polygon is opaque if it is wireframe (alpha 0),
    /// or alpha is max, and it must not have a translucent
    /// texture type (A3I5 or A5I3).
    pub fn is_opaque(&self) -> bool {
        let alpha = self.attrs.alpha();
        let tex_format = self.tex.format();
        (alpha == 0 || alpha == 31) && (tex_format != 1 && tex_format != 6)
    }
}

/// Coordinates for a point in screen-space.
#[derive(Default, Clone, Copy)]
pub struct Coords {
    pub x: N,   // also tex s
    pub y: N    // also tex t
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

    pub use_manual_mode:        bool,   // Insert transparent polys in order.
    pub trans_polygon_auto:     BTreeSet<PolygonOrder>,
    pub trans_polygon_manual:   Vec<PolygonOrder>,

    pub polygons:           Vec<Polygon>,
    pub vertices:           Vec<Vertex>
}

impl PolygonRAM {
    pub fn new() -> Self {
        Self {
            opaque_polygons:        BTreeSet::new(),
            use_manual_mode:        false,
            trans_polygon_auto:     BTreeSet::new(),
            trans_polygon_manual:   Vec::new(),
            polygons:   Vec::new(),
            vertices:   Vec::new()
        }
    }

    /// Clear the polygon and vertex RAM for the next geometry engine write cycle.
    /// 
    /// Also set if polygons should be inserted in manual or automatic order.
    pub fn clear(&mut self, use_manual_mode: bool) {
        self.opaque_polygons.clear();
        self.use_manual_mode = use_manual_mode;
        self.trans_polygon_auto.clear();
        self.trans_polygon_manual.clear();
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
    pub fn insert_polygon(&mut self, polygon: Polygon, y_max: N, y_min: N) {
        let polygon_index = self.polygons.len();

        let polygon_order = PolygonOrder {
            y_max, y_min, polygon_index
        };

        if polygon.is_opaque() {
            self.opaque_polygons.insert(polygon_order);
        } else if self.use_manual_mode {
            self.trans_polygon_manual.push(polygon_order);
        } else {
            self.trans_polygon_auto.insert(polygon_order);
        }
        
        self.polygons.push(polygon);
    }
}
