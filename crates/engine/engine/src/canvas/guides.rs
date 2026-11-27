//! Ruler guides on the page. A frame keeps its guides in its own coordinates, so on a rotated frame
//! they run along the frame's rotated axes and span only the frame; a page's guides are page lines
//! across the whole canvas. Only top-level frames (on the page or in sections) have guides: a
//! frame moved into another keeps its guides, but they stop showing and snapping until it's back.

use kition_design_doc::{Document, NodeId};
use kition_design_geom::{Affine, Point};

/// Whether `owner`'s guides apply: it's a page or a top-level frame.
pub fn keeps_guides(doc: &Document, owner: NodeId) -> bool {
    doc.node(owner).is_some_and(|n| n.is_page() || (n.kind.is_frame_like() && doc.top_level_frame(owner) == Some(owner)))
}

/// The spacing (page pixels) of the rulers' labelled ticks at `zoom`: the first of 1, 2, 5, 10, 20, …
/// that leaves at least 50 screen pixels between labels.
pub fn ruler_step(zoom: f64) -> f64 {
    const STEPS: [f64; 12] = [1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0];
    STEPS.into_iter().find(|s| s * zoom >= 50.0).unwrap_or(10000.0)
}

/// Who gets a guide dragged out of a ruler and dropped at page point `p`: the top-level frame under
/// it (dropped over a nested frame, the frame holding it), else the page.
pub fn drop_owner(doc: &Document, page: NodeId, p: Point) -> NodeId {
    crate::cmd::create::frame_at(doc, page, p, &[]).and_then(|f| doc.top_level_frame(f)).unwrap_or(page)
}

/// Below this, a transform coefficient counts as zero (rotations that keep the axes).
const EPS: f64 = 1e-7;

/// The page segment guide `offset` of frame `owner` covers: along the frame's own y axis across it
/// for a vertical guide (its x axis for a horizontal one). `None` for a page (its guides cross the
/// whole canvas) or a missing node.
pub fn segment(doc: &Document, owner: NodeId, vertical: bool, offset: f64) -> Option<[Point; 2]> {
    let n = doc.node(owner).filter(|n| !n.is_page())?;
    let xf = doc.world_transform(owner);
    let (a, b) =
        if vertical { (Point::new(offset, 0.0), Point::new(offset, n.height)) } else { (Point::new(0.0, offset), Point::new(n.width, offset)) };
    let (a, b) = (xf * a, xf * b);
    (a.is_finite() && b.is_finite()).then_some([a, b])
}

/// The offset of a guide of `owner` through page point `p`, in the owner's coordinates (its x for a
/// vertical guide). A page's coordinates are the page's.
pub fn offset_at(doc: &Document, owner: NodeId, vertical: bool, p: Point) -> Option<f64> {
    let n = doc.node(owner)?;
    let local = if n.is_page() { p } else { doc.world_transform(owner).inverse() * p };
    let v = if vertical { local.x } else { local.y };
    v.is_finite().then_some(v)
}

/// Which of frame `owner`'s guides (vertical or not, in its coordinates) a guide dragged from a
/// page ruler as `vertical` becomes: the one whose axis is nearer the page's, so a frame turned
/// past 45° takes the other kind.
pub fn frame_axis(doc: &Document, owner: NodeId, vertical: bool) -> bool {
    // The frame's y axis on the page is (c, d): it is nearer the page's y axis when |d| ≥ |c|.
    let [_, _, c, d, _, _] = doc.world_transform(owner).as_coeffs();
    if d.abs() >= c.abs() { vertical } else { !vertical }
}

/// Whether `xf` keeps lines parallel to the axes (no rotation off a multiple of 180°, no skew).
pub fn axis_aligned(xf: Affine) -> bool {
    let [_, b, c, _, _, _] = xf.as_coeffs();
    b.abs() < EPS && c.abs() < EPS
}

/// The line `x = offset` (`vertical`) or `y = offset` mapped by `m`, when it stays parallel to an
/// axis: whether it is vertical then, and its offset. A quarter turn swaps the axes.
pub fn line_in(m: Affine, vertical: bool, offset: f64) -> Option<(bool, f64)> {
    let [a, b, c, d, e, f] = m.as_coeffs();
    let line = if vertical {
        // (offset, t) ↦ (a·offset + c·t + e, b·offset + d·t + f)
        if c.abs() < EPS {
            (true, a * offset + e)
        } else if d.abs() < EPS {
            (false, b * offset + f)
        } else {
            return None;
        }
    } else if b.abs() < EPS {
        // (t, offset) ↦ (a·t + c·offset + e, b·t + d·offset + f)
        (false, d * offset + f)
    } else if a.abs() < EPS {
        (true, c * offset + e)
    } else {
        return None;
    };
    line.1.is_finite().then_some(line)
}

/// How near (screen pixels) a guide being dragged comes to a layer's edge or centre before it
/// lands on it.
pub const SNAP_PX: f64 = 5.0;

/// The offset a guide of `owner` (a page or a top-level frame) dragged to `offset` (the owner's
/// coordinates) lands on: the nearest edge or centre line, along the guide's axis, of the layers
/// it can line up with, when one is within [`SNAP_PX`] screen pixels at `zoom`. A page's guide
/// lines up with the page's visible layers; a frame's with the frame itself and the visible
/// layers inside it, at any depth (a hidden layer hides its branch). `None`: nothing in reach.
pub fn snap_offset(doc: &Document, page: NodeId, owner: NodeId, vertical: bool, offset: f64, zoom: f64) -> Option<f64> {
    if !offset.is_finite() || !(zoom.is_finite() && zoom > 0.0) {
        return None;
    }
    let space = if owner == page { Affine::IDENTITY } else { doc.world_transform(owner) };
    let to = space.inverse();
    // Screen pixels into owner units along the guide's axis (a scaled frame stretches them).
    let [a, b, c, d, _, _] = space.as_coeffs();
    let axis = if vertical { a.hypot(b) } else { c.hypot(d) };
    let reach = SNAP_PX / (zoom * axis.max(1e-6));
    let mut best: Option<f64> = None;
    let mut consider = |id: NodeId| {
        let Some(n) = doc.node(id) else { return };
        let r = kition_design_geom::transform_rect_bbox(to * doc.world_transform(id), n.local_rect());
        if !r.is_finite() {
            return;
        }
        let lines = if vertical { [r.x0, r.center().x, r.x1] } else { [r.y0, r.center().y, r.y1] };
        for line in lines {
            let gap = line - offset;
            if gap.abs() <= reach && best.is_none_or(|was| gap.abs() < (was - offset).abs()) {
                best = Some(line);
            }
        }
    };
    let o = doc.node(owner)?;
    if owner == page {
        for child in &o.children {
            if doc.node(*child).is_some_and(|n| n.visible) {
                consider(*child);
            }
        }
        return best;
    }
    consider(owner);
    let mut stack: Vec<NodeId> = o.children.clone();
    let mut seen = 0usize;
    while let Some(id) = stack.pop() {
        // A bounded walk: a frame with very many layers stops looking after 20,000 of them.
        seen += 1;
        if seen > 20_000 {
            break;
        }
        let Some(n) = doc.node(id).filter(|n| n.visible) else { continue };
        consider(id);
        stack.extend(n.children.iter().copied());
    }
    best
}
