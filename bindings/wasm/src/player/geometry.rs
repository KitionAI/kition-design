//! Scrolling, device screens and overlay placement for the player, ported from upstream's
//! `present/scroll.rs`, `present/layout.rs` and `present/transition.rs` (`overlay_rect`), with
//! screen points in place of egui's (the player works at scale 1: one screen unit is one canvas
//! unit; the editor scales the pictures it draws).

use std::collections::HashMap;

use kition_design_doc::proto::{Overflow, OverlayAnchor, OverlayPosition, PrototypeSettings, ScrollPosition};
use kition_design_doc::{Document, HConstraint, NodeId, NodeKind, VConstraint};
use kition_design_geom::{Affine, Rect};

/// The prototype settings of the page holding `id` (defaults when there's none).
pub fn page_settings(doc: &Document, id: NodeId) -> PrototypeSettings {
    doc.page_of(id)
        .and_then(|pg| doc.node(pg))
        .and_then(|n| match &n.kind {
            NodeKind::Page { prototype, .. } => Some(prototype.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

/// The part of top-level `frame` its page's device shows (canvas units): the frame clipped to the
/// device's screen. None without a device: the frame then shows whole.
pub fn viewport(doc: &Document, frame: NodeId) -> Option<(f64, f64)> {
    let d = page_settings(doc, frame).device()?;
    let n = doc.node(frame)?;
    Some((n.width.min(d.width), n.height.min(d.height)))
}

/// How far `frame`'s content can scroll when `viewport` of it shows (see upstream
/// `scroll_range_in`): a frame bigger than a device's screen scrolls through its whole size;
/// content past the frame's own edges scrolls only on the axes its overflow allows.
pub fn scroll_range_in(doc: &Document, frame: NodeId, viewport: Option<(f64, f64)>) -> (f64, f64) {
    let Some(n) = doc.node(frame) else { return (0.0, 0.0) };
    if n.overflow == Overflow::None && viewport.is_none() {
        return (0.0, 0.0);
    }
    let (mut x1, mut y1) = (0.0f64, 0.0f64);
    for c in &n.children {
        let Some(cn) = doc.node(*c) else { continue };
        if !cn.visible || cn.scroll_position == ScrollPosition::Fixed {
            continue;
        }
        let b = kition_design_geom::transform_rect_bbox(cn.transform, cn.local_rect());
        x1 = x1.max(b.x1);
        y1 = y1.max(b.y1);
    }
    if let Some(l) = &n.layout {
        x1 += l.padding[1];
        y1 += l.padding[2];
    }
    if !matches!(n.overflow, Overflow::Horizontal | Overflow::Both) {
        x1 = 0.0;
    }
    if !matches!(n.overflow, Overflow::Vertical | Overflow::Both) {
        y1 = 0.0;
    }
    let (vw, vh) = match viewport {
        Some(v) => {
            x1 = x1.max(n.width);
            y1 = y1.max(n.height);
            v
        }
        None => (n.width, n.height),
    };
    let (rx, ry) = ((x1 - vw).max(0.0), (y1 - vh).max(0.0));
    (if rx.is_finite() { rx } else { 0.0 }, if ry.is_finite() { ry } else { 0.0 })
}

/// `(x, y)` kept within `0..=range` on each axis (0 for non-finite values).
pub fn clamp_scroll((x, y): (f64, f64), (rx, ry): (f64, f64)) -> (f64, f64) {
    let c = |v: f64, r: f64| if v.is_finite() && r.is_finite() { v.clamp(0.0, r.max(0.0)) } else { 0.0 };
    (c(x, rx), c(y, ry))
}

/// Size `frame` to the `(w, h)` of it a screen shows; fixed layers follow their constraints to the
/// screen's edges.
pub fn fit_viewport(doc: &mut Document, frame: NodeId, (w, h): (f64, f64)) {
    let Some(n) = doc.node(frame) else { return };
    let (dw, dh) = (w - n.width, h - n.height);
    if !dw.is_finite() || !dh.is_finite() || (dw == 0.0 && dh == 0.0) {
        return;
    }
    let fixed: Vec<NodeId> =
        n.children.iter().copied().filter(|c| doc.node(*c).is_some_and(|c| c.scroll_position == ScrollPosition::Fixed)).collect();
    for c in fixed {
        if let Some(m) = doc.node_mut(c) {
            let dx = match m.constraints.horizontal {
                HConstraint::Right => dw,
                HConstraint::Center => dw / 2.0,
                _ => 0.0,
            };
            let dy = match m.constraints.vertical {
                VConstraint::Bottom => dh,
                VConstraint::Center => dh / 2.0,
                _ => 0.0,
            };
            m.transform = Affine::translate((dx, dy)) * m.transform;
        }
    }
    if let Some(m) = doc.node_mut(frame) {
        m.width = w;
        m.height = h;
    }
}

/// Shift scrolling frames' children by their scroll offsets: fixed children stay put, sticky ones
/// scroll until they reach the frame's top or left edge.
pub fn apply_scroll(doc: &Document, scroll: &HashMap<NodeId, (f64, f64)>) -> Document {
    let mut out = doc.clone();
    let sticky = |pos: f64, s: f64| if pos >= 0.0 && pos - s < 0.0 { -pos } else { -s };
    for (&frame, &(sx, sy)) in scroll {
        if !sx.is_finite() || !sy.is_finite() || (sx == 0.0 && sy == 0.0) {
            continue;
        }
        let Some(n) = doc.node(frame) else { continue };
        for &c in &n.children {
            let Some(cn) = doc.node(c) else { continue };
            let (dx, dy) = match cn.scroll_position {
                ScrollPosition::Fixed => continue,
                ScrollPosition::Scrolls => (-sx, -sy),
                ScrollPosition::Sticky => {
                    let b = kition_design_geom::transform_rect_bbox(cn.transform, cn.local_rect());
                    (sticky(b.x0, sx), sticky(b.y0, sy))
                }
            };
            if let Some(m) = out.node_mut(c) {
                m.transform = Affine::translate((dx, dy)) * m.transform;
            }
        }
    }
    out
}

/// Fixed layers draw (and take clicks) above the content that scrolls under them.
pub fn raise_fixed(doc: &mut Document) {
    let fixed = |c: &NodeId| doc.node(*c).is_some_and(|c| c.scroll_position == ScrollPosition::Fixed);
    let moves: Vec<(NodeId, Vec<NodeId>)> = doc
        .nodes()
        .filter(|n| !matches!(n.kind, NodeKind::Page { .. }) && n.children.iter().any(fixed))
        .filter_map(|n| {
            let (top, mut order): (Vec<NodeId>, Vec<NodeId>) = n.children.iter().partition(|c| fixed(c));
            order.extend(top);
            (order != n.children).then_some((n.id, order))
        })
        .collect();
    for (id, order) in moves {
        if let Some(n) = doc.node_mut(id) {
            n.children = order;
        }
    }
}

/// Screen rect of an overlay of `size` over the screen `base`. A manual one sits `offset` from the
/// top-left of `anchor` (the layer that opened it); without an offset, right below it.
pub fn overlay_rect(pos: OverlayPosition, base: Rect, size: (f64, f64), anchor: Option<Rect>, offset: Option<[f64; 2]>) -> Rect {
    let cx = base.center().x - size.0 / 2.0;
    let cy = base.center().y - size.1 / 2.0;
    let (x, y) = match pos {
        OverlayPosition::Center => (cx, cy),
        OverlayPosition::TopLeft => (base.x0, base.y0),
        OverlayPosition::TopCenter => (cx, base.y0),
        OverlayPosition::TopRight => (base.x1 - size.0, base.y0),
        OverlayPosition::BottomLeft => (base.x0, base.y1 - size.1),
        OverlayPosition::BottomCenter => (cx, base.y1 - size.1),
        OverlayPosition::BottomRight => (base.x1 - size.0, base.y1 - size.1),
        OverlayPosition::Manual => match (anchor, offset) {
            (Some(a), Some(o)) => (a.x0 + o[0], a.y0 + o[1]),
            (Some(a), None) => (a.x0, a.y1),
            (None, _) => (cx, cy),
        },
    };
    Rect::new(x, y, x + size.0, y + size.1)
}

/// [`overlay_rect`], placed on the layer that opened the overlay when its anchor says so.
pub fn anchored_overlay_rect(
    pos: OverlayPosition,
    anchor_to: OverlayAnchor,
    base: Rect,
    size: (f64, f64),
    anchor: Option<Rect>,
    offset: Option<[f64; 2]>,
) -> Rect {
    match anchor.and_then(|a| anchor_to.offset(pos, [a.width(), a.height()], [size.0, size.1])) {
        Some(o) => overlay_rect(OverlayPosition::Manual, base, size, anchor, Some(o)),
        None => overlay_rect(pos, base, size, anchor, offset),
    }
}
