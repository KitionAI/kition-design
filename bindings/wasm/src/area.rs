//! The document frame: where document pixels lie on a page's infinite canvas.
//!
//! The frame is the page's content (the render bounds of its visible top-level layers, which
//! include shadows and strokes) grown by [`MARGIN`] on every side, then out to whole [`CHUNK`]s of
//! the canvas (so moving or growing a layer moves the frame only when it crosses a chunk edge, not
//! on every nudge), at least [`MIN_WIDTH`] × [`MIN_HEIGHT`] to the right and down; an empty page
//! is that size at the canvas origin. One document pixel is one canvas unit, so a rendered
//! document pixel is a canvas unit square.

use kition_design_doc::{Document, NodeId};
use kition_design_geom::{Point, Rect};
use serde_json::{Value, json};

/// Space around the content, in canvas units (room for frame labels and for drawing next to it).
pub const MARGIN: f64 = 100.0;
/// The canvas is cut into squares this many units wide; the frame's edges lie on their edges.
pub const CHUNK: f64 = 1000.0;
/// The smallest frame: one 1920 × 1080 view.
pub const MIN_WIDTH: f64 = 1920.0;
pub const MIN_HEIGHT: f64 = 1080.0;
/// The largest frame side: content past it is not part of the document pixels.
pub const MAX_SIDE: f64 = 1_000_000.0;

/// The document frame: canvas point (`x`, `y`) is document pixel (0, 0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
    pub x: f64,
    pub y: f64,
    pub width: u32,
    pub height: u32,
}

impl Area {
    /// The frame of an empty page.
    pub fn empty() -> Self {
        Self { x: 0.0, y: 0.0, width: MIN_WIDTH as u32, height: MIN_HEIGHT as u32 }
    }

    /// The frame of `page` in `doc`.
    pub fn of(doc: &Document, page: NodeId) -> Self {
        let content = doc
            .node(page)
            .into_iter()
            .flat_map(|p| p.children.iter())
            .filter(|c| doc.node(**c).is_some_and(|n| n.visible))
            .filter_map(|c| doc.world_render_bounds(*c).or_else(|| doc.world_bounds(*c)))
            .filter(|r| [r.x0, r.y0, r.x1, r.y1].iter().all(|v| v.is_finite() && v.abs() < MAX_SIDE))
            .reduce(|a, b| a.union(b));
        let Some(content) = content else { return Self::empty() };
        let down = |v: f64| (v / CHUNK).floor() * CHUNK;
        let up = |v: f64| (v / CHUNK).ceil() * CHUNK;
        let (x, y) = (down(content.x0 - MARGIN), down(content.y0 - MARGIN));
        let w = (up(content.x1 + MARGIN) - x).clamp(MIN_WIDTH, MAX_SIDE);
        let h = (up(content.y1 + MARGIN) - y).clamp(MIN_HEIGHT, MAX_SIDE);
        Self { x, y, width: w as u32, height: h as u32 }
    }

    /// Canvas point → document pixel.
    pub fn to_doc(&self, p: Point) -> (f64, f64) {
        (p.x - self.x, p.y - self.y)
    }

    /// Document pixel → canvas point.
    pub fn to_canvas(&self, x: f64, y: f64) -> Point {
        Point::new(x + self.x, y + self.y)
    }

    /// A canvas rectangle in document pixels, `{ x, y, width, height }`.
    pub fn rect_json(&self, r: Rect) -> Value {
        json!({ "x": r.x0 - self.x, "y": r.y0 - self.y, "width": r.width(), "height": r.height() })
    }

    /// `{ x, y, width, height }`: the frame's canvas corner and its size in document pixels.
    pub fn to_json(&self) -> Value {
        json!({ "x": self.x, "y": self.y, "width": self.width, "height": self.height })
    }
}
