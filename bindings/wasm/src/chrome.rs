//! The canvas chrome the engine's overlays cannot express, as JSON in document pixels, for the
//! editor to draw on its overlay canvas in the upstream editor's style. Upstream drew it with its
//! UI toolkit (`canvas/frame_labels.rs`, `overlays.rs` `transform_box`, `handles.rs` `al_zones`,
//! `rulers.rs`, `guides.rs`, the layout grids and pixel grid in `overlays.rs`).
//!
//! `chrome(zoom)` answers (all positions in document pixels, colours `#rrggbb` or `#rrggbbaa`,
//! angles in radians clockwise, sizes marked "px" in screen pixels):
//!
//! ```text
//! {
//!   zoom,                                   // as given
//!   colors: { accent, component, guide },   // the page's overlay green, component purple, guide red
//!   labels: [{                              // name labels of top-level frames and sections (and of
//!     id, name,                             //   frames in sections whose contents show), in z-order
//!     kind: "frame"|"component"|"componentSet"|"instance"|"section",
//!     x, y,                                 // the corner the label starts at (the frame's top-left
//!     angle,                                //   as seen), and the direction it reads along
//!     maxWidth,                             // the frame's edge length (px): longer names end in "…"
//!     color,                                // component purple, the accent when selected/hovered,
//!                                           //   else null (the editor's secondary text colour)
//!     selected, hovered,
//!     status: null | { label: "Ready for dev"|"Changed"|"Completed", color },
//!     contentsHidden,                       // a section hiding its contents (eye in its pill)
//!     quad: [[x, y] × 4]                    // the area the engine treats as the label (estimated
//!   }],                                     //   from the name's length: 11 px type)
//!   sizeLabel: null | { text: "393 × 766.5", x, y, color },
//!                                           // under the selection: (x, y) is the box's bottom
//!                                           //   centre; the pill hangs 9 px below it
//!   autoLayout: null | {                    // one selected, unrotated auto layout frame
//!     id,
//!     padding: [{ side: 0..3 (top, right, bottom, left), key: "paddingTop"|…, value, rect }],
//!     gaps: [{ key: "gap"|"crossGap", value, vertical, rect }],
//!     tracks: [[[x, y], [x, y]]]            // grid layout: the tracks' edges
//!   },                                      // rect: { x, y, width, height }; dragging one runs
//!                                           //   node.set { ids: [id], props: { [key]: value } }
//!   grids: [{ frame, color, lines: bool, rects: [rect] }],
//!                                           // layout grids (View > Layout grids): grid lines
//!                                           //   (lines: true, rects are 0-wide/high lines) or
//!                                           //   column/row bands to fill
//!   pixelGrid: bool,                        // View > Pixel grid at 800 %+: 1-unit lines
//!   rulers: null | { step, originX, originY, selection: null | rect },
//!                                           // View > Rulers: ticks every `step` canvas units;
//!                                           //   canvas 0 is document pixel (originX, originY)
//!   guides: [{ owner, page, index, vertical, offset, a: [x, y], b: [x, y], selected }],
//!                                           // ruler guides of the page and its top-level frames
//! }
//! ```

use kition_design_doc::{ContentAlign, DevStatus, Document, GridKind, LayoutMode, NodeId, NodeKind};
use kition_design_geom::{Point, Rect};
use serde_json::{Value, json};

use crate::overlays::{COMPONENT, GUIDE, accent_for};
use crate::{Area, DesignDocument};

/// Upstream's label type size, and the width estimate per character it measures to.
const LABEL_PX: f64 = 11.0;
const CHAR_PX: f64 = 6.2;

fn hex([r, g, b]: [u8; 3]) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// A number as the canvas writes it (upstream `rulers::fmt`): up to two decimals.
pub fn fmt(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    if (r - r.round()).abs() < 1e-9 { format!("{}", r.round() as i64) } else { format!("{r}") }
}

/// The layers that get canvas labels and layout grids, in z-order (upstream `rulers::boards`).
pub fn boards(doc: &Document, page: NodeId) -> Vec<NodeId> {
    fn walk(doc: &Document, id: NodeId, depth: usize, out: &mut Vec<NodeId>) {
        let Some(n) = doc.node(id) else { return };
        for c in &n.children {
            let Some(cn) = doc.node(*c) else { continue };
            if !cn.visible {
                continue;
            }
            if matches!(cn.kind, NodeKind::Section { contents_hidden: false }) && depth < 32 {
                walk(doc, *c, depth + 1, out);
            }
            out.push(*c);
        }
    }
    let mut out = Vec::new();
    walk(doc, page, 0, &mut out);
    out
}

/// Where a label goes: the corner it starts at, the angle it reads along and the length of the
/// frame's edge along it (upstream `frame_labels::label_edge`, on the page: the view does not
/// rotate, so angles are the same on screen).
fn label_edge(p00: Point, p10: Point, p01: Point) -> (Point, f64, f64) {
    let (ex, ey) = (p10 - p00, p01 - p00);
    let len = ex.hypot();
    if !len.is_finite() || len <= 1e-6 {
        return (p00, 0.0, 0.0);
    }
    let mirrored = ex.x * ey.y - ex.y * ey.x < 0.0;
    let reversed = mirrored && ex.x < 0.0;
    let d = if reversed { -ex } else { ex };
    let up = kition_design_geom::Vec2::new(d.y, -d.x);
    let bottom = up.dot(ey) > 0.0;
    let mut corner = p00;
    if reversed {
        corner += ex;
    }
    if bottom {
        corner += ey;
    }
    (corner, d.y.atan2(d.x), len)
}

fn status(n: &kition_design_doc::Node) -> Value {
    match n.dev_status {
        DevStatus::None => Value::Null,
        DevStatus::ReadyForDev if n.dev_changed => json!({ "label": "Changed", "color": "#a75b00" }),
        DevStatus::ReadyForDev => json!({ "label": "Ready for dev", "color": "#087d4f" }),
        DevStatus::Completed => json!({ "label": "Completed", "color": "#6e6e73" }),
    }
}

/// The frame and section labels, and the areas the engine treats as them (page coordinates).
fn labels(doc: &Document, ds: &kition_design_engine::DocState, area: &Area, zoom: f64, green: [u8; 3]) -> (Vec<Value>, Vec<(NodeId, [Point; 4])>) {
    let (mut shown, mut hits) = (Vec::new(), Vec::new());
    let focus = ds.focused();
    if focus.is_some() {
        return (shown, hits);
    }
    for id in boards(doc, ds.page) {
        let Some(n) = doc.node(id) else { continue };
        if !(n.kind.is_frame_like() || matches!(n.kind, NodeKind::Section { .. })) || n.is_board_atom() {
            continue;
        }
        let w = doc.world_transform(id);
        let (corner, angle, len) = label_edge(w * Point::new(0.0, 0.0), w * Point::new(n.width, 0.0), w * Point::new(0.0, n.height));
        let section = matches!(n.kind, NodeKind::Section { .. });
        let component = matches!(n.kind, NodeKind::Component { .. } | NodeKind::ComponentSet { .. });
        let (selected, hovered) = (ds.selection.contains(&id), ds.hover == Some(id));
        let kind = match n.kind {
            NodeKind::Section { .. } => "section",
            NodeKind::Component { .. } => "component",
            NodeKind::ComponentSet { .. } => "componentSet",
            NodeKind::Instance { .. } => "instance",
            _ => "frame",
        };
        let color = if component {
            Some(COMPONENT)
        } else if selected || hovered {
            Some(green)
        } else {
            None
        };
        // The label's area in label space (x along the edge, y up from it negative), screen px.
        let chars = n.name.chars().count() as f64;
        let edge_px = len * zoom;
        let (x0, y0, x1, y1) = if section {
            let eye = if n.hides_contents() { 16.0 } else { 0.0 };
            let text = (chars * CHAR_PX).clamp(20.0, (edge_px - 16.0 - eye).clamp(20.0, 480.0));
            (0.0, 0.0, text + 16.0 + eye, LABEL_PX + 8.0 + 3.0)
        } else {
            let icon = if component { 14.0 } else { 0.0 };
            let text = (chars * CHAR_PX).min((edge_px - icon).max(20.0));
            (0.0, -18.0, icon + text, -16.0 + LABEL_PX + 3.0)
        };
        let (cos, sin) = (angle.cos(), angle.sin());
        let at = |x: f64, y: f64| Point::new(corner.x + (x * cos - y * sin) / zoom, corner.y + (x * sin + y * cos) / zoom);
        let quad = [at(x0, y0), at(x1, y0), at(x1, y1), at(x0, y1)];
        let (cx, cy) = area.to_doc(corner);
        shown.push(json!({
            "id": id,
            "name": n.name,
            "kind": kind,
            "x": cx,
            "y": cy,
            "angle": angle,
            "maxWidth": edge_px,
            "color": color.map(hex),
            "selected": selected,
            "hovered": hovered,
            "status": status(n),
            "contentsHidden": n.hides_contents(),
            "quad": quad.iter().map(|p| { let (x, y) = area.to_doc(*p); json!([x, y]) }).collect::<Vec<_>>(),
        }));
        hits.push((id, quad));
    }
    (shown, hits)
}

/// The size pill under the selection (upstream `transform_box`'s label).
fn size_label(session: &kition_design_engine::Session, doc: &Document, area: &Area, green: [u8; 3]) -> Value {
    let Ok(ds) = session.ds() else { return Value::Null };
    if !session.prefs.show_dimensions || session.tool.overlay.measure_box.is_some() || ds.text_edit.is_some() {
        return Value::Null;
    }
    let sel = &ds.selection;
    let (corners, text) = if ds.vector_edit.is_some() {
        let Some(c) = session.vector_edit_box() else { return Value::Null };
        (c, format!("{} × {}", fmt((c[1] - c[0]).hypot()), fmt((c[3] - c[0]).hypot())))
    } else {
        if sel.is_empty() || kition_design_engine::board::selected_connector_ends(doc, sel).is_some() {
            return Value::Null;
        }
        let (Some(b), Some((_, w, h))) =
            (kition_design_engine::tools::selection_box(doc, sel), kition_design_engine::tools::selection_frame(doc, sel))
        else {
            return Value::Null;
        };
        let is_line = sel.len() == 1 && matches!(sel.first().and_then(|i| doc.node(*i)).map(|n| &n.kind), Some(NodeKind::Line));
        let mut text = if is_line { fmt(w) } else { format!("{} × {}", fmt(w), fmt(h)) };
        if let Some(hint) = session.tool.overlay.scale {
            text = format!("{text} · {}x", fmt(hint.factor.abs()));
        }
        (b, text)
    };
    // The points' box in vector edit mode is the page's green (upstream `transform_box` with
    // `accent`), whatever layer the points are in.
    let col =
        if ds.vector_edit.is_none() && sel.iter().any(|i| crate::overlays::outline_colour(doc, *i, green) == COMPONENT) { COMPONENT } else { green };
    let bottom = corners.iter().map(|p| p.y).fold(f64::MIN, f64::max);
    let cx = corners.iter().map(|p| p.x).sum::<f64>() / 4.0;
    let (x, y) = area.to_doc(Point::new(cx, bottom));
    json!({ "text": text, "x": x, "y": y, "color": hex(if col == crate::overlays::BRAND { crate::overlays::ACCENT } else { col }) })
}

/// The padding bands, gap bands and grid tracks of one selected auto layout frame (upstream
/// `handles::al_zones`, in document pixels).
fn auto_layout(doc: &Document, ds: &kition_design_engine::DocState, area: &Area) -> Value {
    if ds.selection.len() != 1 || ds.text_edit.is_some() || ds.vector_edit.is_some() {
        return Value::Null;
    }
    let Some(id) = ds.selection.first().copied() else { return Value::Null };
    let Some(n) = doc.node(id) else { return Value::Null };
    let Some(al) = n.layout.as_ref() else { return Value::Null };
    let w = doc.world_transform(id);
    if kition_design_geom::rotation_degrees(w).abs() > 0.01 {
        return Value::Null;
    }
    let to = |x: f64, y: f64| w * Point::new(x, y);
    let band = |x0: f64, y0: f64, x1: f64, y1: f64| {
        let (a, b) = (to(x0, y0), to(x1, y1));
        Rect::new(a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y))
    };
    let rect = |r: Rect| area.rect_json(r);
    let [pt, pr, pb, pl] = al.padding;
    let (fw, fh) = (n.width, n.height);
    let keys = ["paddingTop", "paddingRight", "paddingBottom", "paddingLeft"];
    let bands =
        [band(0.0, 0.0, fw, pt.max(4.0)), band(fw - pr.max(4.0), 0.0, fw, fh), band(0.0, fh - pb.max(4.0), fw, fh), band(0.0, 0.0, pl.max(4.0), fh)];
    let padding: Vec<Value> = bands
        .iter()
        .zip([pt, pr, pb, pl])
        .enumerate()
        .map(|(side, (r, value))| json!({ "side": side, "key": keys.get(side), "value": value, "rect": rect(*r) }))
        .collect();
    let auto_main = al.main_align.is_auto_gap() && al.mode != LayoutMode::Grid;
    let auto_cross = al.wrap && al.content_align == ContentAlign::SpaceBetween;
    let mut gaps: Vec<(Rect, &str, f64, bool)> = Vec::new();
    let mut tracks: Vec<[Point; 2]> = Vec::new();
    if let Some(g) = kition_design_layout::grid_geometry(doc, id) {
        let (top, bottom) = (g.rows.first().map_or(pt, |r| r.0), g.rows.last().map_or(fh - pb, |r| r.1));
        let (left, right) = (g.columns.first().map_or(pl, |c| c.0), g.columns.last().map_or(fw - pr, |c| c.1));
        for pair in g.columns.windows(2) {
            if let [a, b] = pair {
                gaps.push((band(a.1, top, b.0, bottom), "gap", al.gap, false));
            }
        }
        for pair in g.rows.windows(2) {
            if let [a, b] = pair {
                gaps.push((band(left, a.1, right, b.0), "crossGap", al.cross_gap, true));
            }
        }
        for c in &g.columns {
            tracks.push([to(c.0, top), to(c.0, bottom)]);
            tracks.push([to(c.1, top), to(c.1, bottom)]);
        }
        for r in &g.rows {
            tracks.push([to(left, r.0), to(right, r.0)]);
            tracks.push([to(left, r.1), to(right, r.1)]);
        }
    } else {
        let vertical = al.mode == LayoutMode::Vertical;
        let flip = |r: Rect| if vertical { Rect::new(r.y0, r.x0, r.y1, r.x1) } else { r };
        let fband = |x0: f64, y0: f64, x1: f64, y1: f64| if vertical { band(y0, x0, y1, x1) } else { band(x0, y0, x1, y1) };
        let (mw, mp0, mp1, cw, cp0, cp1) = if vertical { (fh, pt, pb, fw, pl, pr) } else { (fw, pl, pr, fh, pt, pb) };
        let boxes: Vec<Rect> = kition_design_layout::flow_children(doc, id)
            .into_iter()
            .filter_map(|c| doc.node(c))
            .map(|c| flip(kition_design_geom::transform_rect_bbox(c.transform, c.local_rect())))
            .collect();
        let mut lines: Vec<Vec<Rect>> = Vec::new();
        for b in boxes {
            match lines.last_mut() {
                Some(line) if !al.wrap || line.last().is_some_and(|a| b.x0 >= a.x1 - 1e-6) => line.push(b),
                _ => lines.push(vec![b]),
            }
        }
        let span = |line: &[Rect]| line.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), r| (a.min(r.y0), b.max(r.y1)));
        for line in &lines {
            let (y0, y1) = if al.wrap { span(line) } else { (cp0, cw - cp1) };
            for pair in line.windows(2) {
                if let [ra, rb] = pair {
                    gaps.push((fband(ra.x1, y0, rb.x0, y1), "gap", if auto_main { rb.x0 - ra.x1 } else { al.gap }, vertical));
                }
            }
        }
        for pair in lines.windows(2) {
            if let [a, b] = pair {
                let dist = span(b).0 - span(a).1;
                gaps.push((fband(mp0, span(a).1, mw - mp1, span(b).0), "crossGap", if auto_cross { dist } else { al.cross_gap }, !vertical));
            }
        }
    }
    gaps.retain(|(r, ..)| r.width() > 0.5 && r.height() > 0.5);
    let point = |p: Point| {
        let (x, y) = area.to_doc(p);
        json!([x, y])
    };
    json!({
        "id": id,
        "padding": padding,
        "gaps": gaps.iter().map(|(r, key, value, vertical)| json!({ "key": key, "value": value, "vertical": vertical, "rect": rect(*r) })).collect::<Vec<_>>(),
        "tracks": tracks.iter().map(|[a, b]| json!([point(*a), point(*b)])).collect::<Vec<_>>(),
    })
}

/// Layout grids of the labelled frames (View ▸ Layout grids).
fn grids(doc: &Document, ds: &kition_design_engine::DocState, area: &Area) -> Vec<Value> {
    if !ds.view.layout_grids {
        return Vec::new();
    }
    let mut out = Vec::new();
    for id in boards(doc, ds.page) {
        let Some(n) = doc.node(id) else { continue };
        if n.grids.is_empty() || !kition_design_layout::grids_shown(doc, id) {
            continue;
        }
        let w = doc.world_transform(id);
        for g in n.grids.iter().filter(|g| g.visible) {
            let rects: Vec<Value> = kition_design_layout::grid_lines(g, n.width, n.height)
                .into_iter()
                .map(|r| {
                    let (a, b) = (w * Point::new(r.x0, r.y0), w * Point::new(r.x1, r.y1));
                    area.rect_json(Rect::new(a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y)))
                })
                .collect();
            out.push(json!({ "frame": id, "color": format!("#{}", g.color.to_hex()), "lines": g.kind == GridKind::Grid, "rects": rects }));
        }
    }
    out
}

/// How far a page guide's line reaches each way (page pixels): across any canvas there is.
const PAGE_GUIDE_REACH: f64 = 1e7;

/// Ruler guides of the page and its top-level frames: a frame's across the frame, the page's
/// (`page: true`) across the whole canvas.
fn guides(doc: &Document, ds: &kition_design_engine::DocState, area: &Area) -> Vec<Value> {
    let owners = std::iter::once(ds.page).chain(doc.node(ds.page).map(|p| p.children.clone()).unwrap_or_default());
    let mut out = Vec::new();
    for owner in owners {
        let Some(n) = doc.node(owner) else { continue };
        let page = owner == ds.page;
        if !page && !n.visible {
            continue;
        }
        for (index, g) in n.guides().iter().enumerate() {
            let ends = if page {
                let (a, b) = if g.vertical {
                    (Point::new(g.offset, -PAGE_GUIDE_REACH), Point::new(g.offset, PAGE_GUIDE_REACH))
                } else {
                    (Point::new(-PAGE_GUIDE_REACH, g.offset), Point::new(PAGE_GUIDE_REACH, g.offset))
                };
                Some([a, b])
            } else {
                kition_design_engine::guides::segment(doc, owner, g.vertical, g.offset)
            };
            let Some([a, b]) = ends else { continue };
            let (a, b) = (area.to_doc(a), area.to_doc(b));
            out.push(json!({
                "owner": owner, "page": page, "index": index, "vertical": g.vertical, "offset": g.offset,
                "a": [a.0, a.1], "b": [b.0, b.1], "selected": ds.guide == Some((owner, index)),
            }));
        }
    }
    out
}

/// The chrome at `zoom`, and the label areas for the engine's hit tests.
pub fn chrome(doc: &DesignDocument, zoom: f64) -> (Value, Vec<(NodeId, [Point; 4])>) {
    let Ok(ds) = doc.ds() else { return (json!({ "zoom": zoom }), Vec::new()) };
    let area = doc.area();
    let d: &Document = &ds.doc;
    let green = accent_for(d, ds.page);
    let (labels, hits) = labels(d, ds, &area, zoom, green);
    let selection = d.bounds_of(&ds.selection).map(|b| area.rect_json(b));
    let rulers = ds
        .view
        .rulers
        .then(|| json!({ "step": kition_design_engine::guides::ruler_step(zoom), "originX": -area.x, "originY": -area.y, "selection": selection }));
    let value = json!({
        "zoom": zoom,
        "colors": { "accent": hex(green), "component": hex(COMPONENT), "guide": hex(GUIDE) },
        "labels": labels,
        "sizeLabel": size_label(doc.session(), d, &area, green),
        "autoLayout": auto_layout(d, ds, &area),
        "grids": grids(d, ds, &area),
        "pixelGrid": ds.view.pixel_grid && zoom >= kition_design_engine::ViewState::PIXEL_GRID_ZOOM - 1e-9,
        "rulers": rulers,
        "guides": guides(d, ds, &area),
    });
    (value, hits)
}
