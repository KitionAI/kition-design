//! What the upstream canvas draws over the rendered page, as engine overlays in document pixels.
//! Upstream drew these with its UI toolkit (`canvas/overlays.rs`, `outline.rs`,
//! `measure_overlay.rs`); this is the same logic with no toolkit, so any host that draws engine
//! overlays shows the canvas as the upstream editor does:
//!
//! - the hover outline, the outlines of a boolean group's operands, mask and frame outlines and
//!   slices (View menu);
//! - the drop target, the auto layout insertion line and grid cell, the instance a drop swaps;
//! - the selection's outlines (green, purple for components and instances), its transform box with
//!   its corner handles (two for a line), arc and corner radius handles, the Scale tool's pivot
//!   and the rotation origin;
//! - vector edit mode's segments, handles and points, the hatching of hovered or selected filled
//!   regions, the point a press would add, the box around several selected points, the lasso and
//!   the eraser;
//! - the text being edited: its box, the selected characters and the caret;
//! - smart guides and measurements with their labels, the box measured to;
//! - the marquee, the pen path being drawn with its handles and rubber band, the pencil stroke,
//!   the shape builder's regions.
//!
//! The size pill under the selection, frame labels and auto layout handles are canvas chrome
//! ([`crate::chrome`]), which the editor draws itself. Sizes given in screen pixels (anchors,
//! label offsets) are divided by the zoom where they place geometry.

use kition_design_doc::{Document, NodeId, NodeKind};
use kition_design_engine::Session;
use kition_design_engine::tools::ToolId;
use kition_design_geom::{Affine, BezPath, Point};
use kition_kit::tools::ToolOverlay;

use crate::Area;

/// The overlays' green on a light page (upstream `ACCENT`), and on a dark one (upstream `BRAND`).
pub const ACCENT: [u8; 3] = [0x00, 0x87, 0x5F];
pub const BRAND: [u8; 3] = [0x02, 0xDA, 0x96];
/// Components, component sets, instances and the layers in them.
pub const COMPONENT: [u8; 3] = [0x97, 0x47, 0xFF];
/// Smart guides and measurements.
pub const GUIDE: [u8; 3] = [0xF2, 0x48, 0x22];
/// The hatching of filled regions in vector edit mode, in screen pixels (upstream `REGION_HATCH_WIDTH`).
const REGION_HATCH_WIDTH: f32 = 1.4;

fn rgba([r, g, b]: [u8; 3], alpha: f32) -> [u8; 4] {
    [r, g, b, (alpha.clamp(0.0, 1.0) * 255.0).round() as u8]
}

/// The green that contrasts more with the page's background (upstream `set_canvas_page`).
pub fn accent_for(doc: &Document, page: NodeId) -> [u8; 3] {
    let background = crate::query::page_background(doc, page);
    let colour = |[r, g, b]: [u8; 3]| kition_design_color::Color::new(f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0, 1.0);
    if colour(BRAND).contrast(background) > colour(ACCENT).contrast(background) { BRAND } else { ACCENT }
}

/// The text colour on the page (the caret): dark on a light page, white on a dark one.
fn ink_for(doc: &Document, page: NodeId) -> [u8; 3] {
    let background = crate::query::page_background(doc, page);
    let white = kition_design_color::Color::new(1.0, 1.0, 1.0, 1.0);
    let dark = kition_design_color::Color::new(0x1E as f32 / 255.0, 0x1E as f32 / 255.0, 0x1E as f32 / 255.0, 1.0);
    if white.contrast(background) > dark.contrast(background) { [0xFF; 3] } else { [0x1E; 3] }
}

/// A layer's outline colour: purple for components, sets, instances and every layer in them, the
/// page's green otherwise (upstream `outline_colour`).
pub fn outline_colour(doc: &Document, id: NodeId, green: [u8; 3]) -> [u8; 3] {
    use kition_design_engine::components::{owning_component, owning_instance};
    let set = matches!(doc.node(id).map(|n| &n.kind), Some(NodeKind::ComponentSet { .. }));
    if set || owning_component(doc, id).is_some() || owning_instance(doc, id).is_some() { COMPONENT } else { green }
}

/// Builds overlays in document pixels.
struct Out<'a> {
    area: &'a Area,
    zoom: f64,
    list: Vec<ToolOverlay>,
}

impl Out<'_> {
    fn at(&self, p: Point) -> (f64, f64) {
        self.area.to_doc(p)
    }

    fn path(&mut self, path: &BezPath, color: [u8; 4], width: f32, dashed: bool) {
        if path.elements().is_empty() {
            return;
        }
        let moved = Affine::translate((-self.area.x, -self.area.y)) * path.clone();
        self.list.push(ToolOverlay::Path { d: moved.to_svg(), color, width, dashed });
    }

    fn polygon(&mut self, points: &[Point], closed: bool, color: [u8; 4], width: f32, dashed: bool) {
        let mut path = BezPath::new();
        let mut it = points.iter();
        let Some(first) = it.next() else { return };
        path.move_to(*first);
        for p in it {
            path.line_to(*p);
        }
        if closed {
            path.close_path();
        }
        self.path(&path, color, width, dashed);
    }

    fn quad(&mut self, q: [Point; 4], color: [u8; 4], width: f32, dashed: bool) {
        self.polygon(&q, true, color, width, dashed);
    }

    fn line(&mut self, a: Point, b: Point, color: [u8; 4], dashed: bool) {
        let ((ax, ay), (bx, by)) = (self.at(a), self.at(b));
        self.list.push(ToolOverlay::Line { ax, ay, bx, by, color, dashed });
    }

    fn anchor(&mut self, p: Point, color: [u8; 4], filled: bool, size: f32) {
        let (x, y) = self.at(p);
        self.list.push(ToolOverlay::Anchor { x, y, color, filled, size });
    }

    fn handle(&mut self, p: Point, color: [u8; 4]) {
        let (x, y) = self.at(p);
        self.list.push(ToolOverlay::Handle { x, y, color });
    }

    fn measure(&mut self, p: Point, text: String) {
        let (x, y) = self.at(p);
        self.list.push(ToolOverlay::Measure { x, y, text });
    }

    fn highlight(&mut self, q: [Point; 4], color: [u8; 4]) {
        let corner = |p: Point| [p.x - self.area.x, p.y - self.area.y];
        self.list.push(ToolOverlay::Highlight { quad: q.map(corner), color });
    }

    /// A circle of `radius` screen pixels at `p`.
    fn circle(&mut self, p: Point, radius: f64, color: [u8; 4], width: f32) {
        let r = radius / self.zoom.max(1e-6);
        let path = kition_design_geom::kurbo::Circle::new(p, r);
        self.path(&kition_design_geom::kurbo::Shape::to_path(&path, 0.1 / self.zoom.max(1e-6)), color, width, false);
    }
}

/// A layer's corners on the page (tl, tr, br, bl).
pub fn node_quad(doc: &Document, id: NodeId) -> Option<[Point; 4]> {
    let n = doc.node(id)?;
    let w = doc.world_transform(id);
    Some([Point::new(0.0, 0.0), Point::new(n.width, 0.0), Point::new(n.width, n.height), Point::new(0.0, n.height)].map(|c| w * c))
}

/// A layer's outline on the page: its shape for ellipses, vectors, polygons, stars and boolean
/// groups, its box otherwise (upstream `draw_outline`).
pub fn outline(doc: &Document, id: NodeId) -> Option<BezPath> {
    let n = doc.node(id)?;
    let shaped = matches!(
        n.kind,
        NodeKind::Ellipse { .. } | NodeKind::Vector { .. } | NodeKind::Polygon { .. } | NodeKind::Star { .. } | NodeKind::BooleanOp { .. }
    );
    if shaped && let Some((path, _)) = kition_design_render::node_outline(doc, n) {
        return Some(doc.world_transform(id) * path);
    }
    let q = node_quad(doc, id)?;
    let mut path = BezPath::new();
    path.move_to(q[0]);
    for p in &q[1..] {
        path.line_to(*p);
    }
    path.close_path();
    Some(path)
}

/// Every overlay the canvas shows now, at `zoom` screen pixels per document pixel.
pub fn overlays(session: &Session, area: &Area, zoom: f64) -> Vec<ToolOverlay> {
    let mut out = Out { area, zoom: if zoom.is_finite() && zoom > 0.0 { zoom } else { 1.0 }, list: Vec::new() };
    let Ok(ds) = session.ds() else { return out.list };
    let preedit = kition_design_engine::textedit::preedit_doc(session);
    let doc: &Document = &ds.doc;
    let page = ds.page;
    let sel = &ds.selection;
    let view = &ds.view;
    let green = accent_for(doc, page);
    let tool = session.tool.tool;
    let ov = &session.tool.overlay;

    // Hover outline (measuring outlines the measured box instead).
    if let Some(h) = ds.hover
        && !sel.contains(&h)
        && matches!(tool, ToolId::Move | ToolId::Scale)
        && session.prefs.highlight_on_hover
        && ov.measure_box.is_none()
        && let Some(path) = outline(doc, h)
    {
        out.path(&path, rgba(outline_colour(doc, h, green), 1.0), 1.5, false);
    }
    // A boolean group's operands while one is selected or edited.
    for id in kition_design_engine::booleans::outlined_operands(doc, sel, ds.vector_edit.as_ref().map(|ve| ve.id)) {
        if !sel.contains(&id)
            && let Some(path) = outline(doc, id)
        {
            out.path(&path, rgba(outline_colour(doc, id, green), 0.6), 1.0, false);
        }
    }
    // Mask outlines, frame outlines and slices, as the View menu shows them.
    if view.mask_outlines || view.frame_outlines || view.show_slices {
        for id in doc.subtree(page) {
            let Some(n) = doc.node(id).filter(|n| n.visible) else { continue };
            if view.mask_outlines && n.is_mask {
                if let Some(path) = outline(doc, id) {
                    out.path(&path, rgba(green, 1.0), 1.0, false);
                }
            } else if view.frame_outlines && n.kind.is_frame_like() {
                if let Some(path) = outline(doc, id) {
                    out.path(&path, [0x75, 0x75, 0x75, 0xFF], 1.0, false);
                }
            } else if view.show_slices
                && matches!(n.kind, NodeKind::Slice)
                && let Some(q) = node_quad(doc, id)
            {
                out.quad(q, [0x75, 0x75, 0x75, 0xFF], 1.0, true);
            }
        }
    }
    // Drop target, insertion line, grid cell, swap target.
    if let Some(d) = ov.drop_target
        && let Some(q) = node_quad(doc, d)
    {
        let colour = if kition_design_engine::components::is_slot(doc, d) { COMPONENT } else { green };
        out.quad(q, rgba(colour, 1.0), 1.0, false);
    }
    if let Some([x0, y0, x1, y1]) = ov.insert_line {
        out.line(Point::new(x0, y0), Point::new(x1, y1), rgba(green, 1.0), false);
    }
    if let Some([x0, y0, x1, y1]) = ov.drop_cell {
        let q = [Point::new(x0, y0), Point::new(x1, y0), Point::new(x1, y1), Point::new(x0, y1)];
        out.highlight(q, rgba(green, 0.12));
        out.quad(q, rgba(green, 1.0), 1.5, true);
    }
    if let Some(t) = ov.swap_target
        && let Some(q) = node_quad(doc, t)
    {
        out.highlight(q, rgba(COMPONENT, 0.12));
        out.quad(q, rgba(COMPONENT, 1.0), 2.0, false);
    }
    // Vector edit mode.
    if let Some(ve) = &ds.vector_edit {
        let width_tool = ve.tool == kition_design_engine::vectoredit::VTool::Width;
        let hover_on = |id: NodeId| ov.vector_hover.filter(|_| ov.vector_hover_on.unwrap_or(ve.id) == id);
        for part in &ve.others {
            if let Some(o) = kition_design_engine::vectoredit::overlay(doc, &ve.view(part)) {
                vector_overlay(&mut out, &o, !width_tool, hover_on(part.id), green, None);
            }
        }
        // The hovered or selected filled regions, hatched (upstream `region_hatch`): the engine
        // spaces the lines in screen pixels, so it hatches at the zoom and they come back here.
        let zoom = out.zoom;
        let hatch = session.vector_region_hatch(Affine::scale(zoom), None);
        if !hatch.is_empty() {
            let mut path = BezPath::new();
            for [a, b] in hatch {
                path.move_to(Point::new(a.x / zoom, a.y / zoom));
                path.line_to(Point::new(b.x / zoom, b.y / zoom));
            }
            out.path(&path, rgba(green, 1.0), REGION_HATCH_WIDTH, false);
        }
        if let Some(o) = kition_design_engine::vectoredit::overlay(doc, ve) {
            vector_overlay(&mut out, &o, !width_tool, hover_on(ve.id), green, ov.vector_add);
        }
        if !width_tool && let Some(c) = session.vector_edit_box() {
            out.quad(c, rgba(green, 1.0), 1.0, false);
        }
        if !ov.face.is_empty() {
            for poly in &ov.face {
                let points: Vec<Point> = poly.iter().map(|[x, y]| Point::new(*x, *y)).collect();
                out.polygon(&points, true, rgba(green, 1.0), 1.0, true);
            }
        }
        if let Some([x0, y0, x1, y1]) = ov.rubber {
            out.line(Point::new(x0, y0), Point::new(x1, y1), rgba(green, 1.0), true);
        }
    }
    // The selection.
    if !sel.is_empty() && ds.text_edit.is_none() && ds.vector_edit.is_none() {
        selection(&mut out, session, doc, sel, green);
    }
    // The shape builder's regions under the pointer or collected by the drag.
    if tool == ToolId::ShapeBuilder
        && let Some(arrangement) = kition_design_engine::shapebuilder::current(session)
    {
        let colour = if ov.builder_remove { GUIDE } else { green };
        for region in arrangement.regions.iter() {
            for poly in &region.polys {
                out.polygon(poly, true, rgba(colour, 0.5), 1.0, true);
            }
        }
        for i in &ov.builder_regions {
            if let Some(region) = arrangement.regions.get(*i) {
                out.path(&region.path, rgba(colour, 1.0), 2.0, false);
            }
        }
    }
    // Text editing: the box, the selected characters and the caret.
    if let Some(te) = &ds.text_edit {
        let edoc: &Document = preedit.as_ref().map_or(doc, |(d, _, _)| d);
        text_overlay(&mut out, edoc, te, preedit.as_ref().map(|(_, range, cursor)| (*range, *cursor)), green, ink_for(doc, page));
    }
    // Smart guides and measurements.
    if let Some([x0, y0, x1, y1]) = ov.measure_box {
        out.quad([Point::new(x0, y0), Point::new(x1, y0), Point::new(x1, y1), Point::new(x0, y1)], rgba(GUIDE, 1.0), 1.0, false);
    }
    for g in &ov.guides {
        let (a, b) = (Point::new(g.x0, g.y0), Point::new(g.x1, g.y1));
        if a == b {
            // A marker on a point snapped onto: a small cross.
            let d = 4.0 / out.zoom;
            out.line(Point::new(a.x - d, a.y - d), Point::new(a.x + d, a.y + d), rgba(GUIDE, 1.0), false);
            out.line(Point::new(a.x - d, a.y + d), Point::new(a.x + d, a.y - d), rgba(GUIDE, 1.0), false);
        } else {
            out.line(a, b, rgba(GUIDE, 1.0), g.dashed);
        }
        if let Some(label) = &g.label {
            out.measure(Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0), label.clone());
        }
    }
    // The marquee.
    if let Some([x0, y0, x1, y1]) = ov.marquee {
        let ((ax, ay), (bx, by)) = (out.at(Point::new(x0.min(x1), y0.min(y1))), out.at(Point::new(x0.max(x1), y0.max(y1))));
        out.list.push(ToolOverlay::Marquee { x0: ax, y0: ay, x1: bx, y1: by });
    }
    // The pen path being drawn, or the pencil stroke.
    if !ov.pen.is_empty() {
        let points: Vec<Point> = ov.pen.iter().map(|[x, y]| Point::new(*x, *y)).collect();
        if tool == ToolId::Pencil {
            let (colour, weight) = kition_design_engine::tools::PENCIL_STROKE;
            let [r, g, b, a] = colour.to_rgba8();
            out.polygon(&points, false, [r, g, b, a], ((weight * out.zoom) as f32).max(1.0), false);
        } else {
            for [x0, y0, x1, y1] in &ov.pen_handles {
                out.line(Point::new(*x0, *y0), Point::new(*x1, *y1), rgba(green, 1.0), false);
                out.handle(Point::new(*x1, *y1), rgba(green, 1.0));
            }
            for p in &points {
                out.anchor(*p, rgba(green, 1.0), false, 7.0);
            }
            if let Some([x0, y0, x1, y1]) = ov.rubber {
                let (a, b) = (Point::new(x0, y0), Point::new(x1, y1));
                match ov.rubber_handle {
                    Some([hx, hy]) => {
                        let mut path = BezPath::new();
                        path.move_to(a);
                        path.curve_to(Point::new(hx, hy), b, b);
                        out.path(&path, rgba(green, 1.0), 1.0, false);
                    }
                    None => out.line(a, b, rgba(green, 1.0), false),
                }
            }
        }
    } else if ds.vector_edit.is_none()
        && let Some([x0, y0, x1, y1]) = ov.rubber
    {
        out.line(Point::new(x0, y0), Point::new(x1, y1), rgba(green, 1.0), false);
    }
    // The eraser brush and the lasso (vector edit mode).
    if let Some([x, y, r]) = ov.brush {
        out.circle(Point::new(x, y), r * out.zoom, rgba(green, 1.0), 1.5);
    }
    if ov.lasso.len() > 1 {
        let points: Vec<Point> = ov.lasso.iter().map(|[x, y]| Point::new(*x, *y)).collect();
        out.polygon(&points, true, rgba(green, 1.0), 1.0, true);
    }
    out.list
}

/// The selection's outlines, transform box and handles (upstream `overlays`' selection part).
fn selection(out: &mut Out<'_>, session: &Session, doc: &Document, sel: &[NodeId], green: [u8; 3]) {
    let col = if sel.iter().any(|i| outline_colour(doc, *i, green) == COMPONENT) { COMPONENT } else { green };
    for id in sel {
        if let Some(path) = outline(doc, *id) {
            out.path(&path, rgba(col, 1.0), 1.0, false);
        }
    }
    let tool = session.tool.tool;
    if tool == ToolId::ShapeBuilder {
        return;
    }
    let Some(b) = kition_design_engine::tools::selection_box(doc, sel) else { return };
    let is_line = sel.len() == 1 && matches!(sel.first().and_then(|i| doc.node(*i)).map(|n| &n.kind), Some(NodeKind::Line));
    let tiny = kition_design_engine::tools::box_sides_px(&b, out.zoom).1 < kition_design_engine::tools::SMALL_BOX_PX;
    let measuring = session.tool.overlay.measure_box.is_some();
    let connector = kition_design_engine::board::selected_connector_ends(doc, sel).map(|(_, ends)| ends);
    let handles = if measuring || session.tool.dev_mode || connector.is_some() || sel.iter().any(|i| kition_design_engine::tools::locked_in(doc, *i))
    {
        0
    } else if is_line {
        2
    } else if tiny {
        0
    } else {
        4
    };
    if sel.len() > 1 {
        out.quad(b, rgba(col, 1.0), 1.0, false);
    }
    for c in b.iter().take(handles) {
        out.anchor(*c, rgba(col, 1.0), false, 8.0);
    }
    for end in connector.into_iter().flatten().filter(|_| !session.tool.dev_mode) {
        out.handle(end, rgba(col, 1.0));
    }
    if measuring {
        return;
    }
    // Arc handles of a selected ellipse, corner radius and star ratio handles.
    if let Some(hs) = session.arc_handles() {
        use kition_design_engine::arc_handles::ArcHandle;
        for h in [ArcHandle::Ratio, ArcHandle::Start, ArcHandle::Sweep] {
            if let Some([x, y]) = hs.at(h) {
                out.handle(Point::new(x, y), rgba(col, 1.0));
            }
        }
    }
    if let Some(hint) = &session.tool.overlay.arc {
        out.measure(Point::new(hint.at[0] + 12.0 / out.zoom, hint.at[1] + 12.0 / out.zoom), hint.label.clone());
    }
    for (_, [x, y]) in session.shape_handles() {
        out.handle(Point::new(x, y), rgba(col, 1.0));
    }
    if let Some(hint) = &session.tool.overlay.shape {
        let text = match hint.handle {
            kition_design_engine::shape_handles::ShapeHandle::Radius { .. } => format!("Radius {}", hint.value),
            kition_design_engine::shape_handles::ShapeHandle::Ratio => format!("Ratio {}%", hint.value),
        };
        out.measure(Point::new(hint.at[0] + 12.0 / out.zoom, hint.at[1] + 12.0 / out.zoom), text);
    }
    // The Scale tool's pivot.
    if tool == ToolId::Scale {
        let pivot = session
            .tool
            .overlay
            .scale
            .map(|h| Point::new(h.pivot[0], h.pivot[1]))
            .or_else(|| kition_design_engine::tools::scale_anchor_point(doc, sel, session.tool.scale_anchor));
        if let Some(p) = pivot {
            out.anchor(p, rgba(col, 1.0), true, 4.0);
            out.circle(p, 4.0, rgba(col, 1.0), 1.5);
        }
    }
    // The rotation origin while it shows (Alt+R): a ring with a cross through it.
    if let Ok(ds) = session.ds()
        && let Some(o) = ds.origin_handle()
    {
        out.circle(o, 5.0, rgba(col, 1.0), 1.5);
        let (near, far) = (5.0 / out.zoom, 10.0 / out.zoom);
        for (dx, dy) in [(1.0, 0.0), (0.0, 1.0)] {
            out.line(Point::new(o.x - dx * far, o.y - dy * far), Point::new(o.x - dx * near, o.y - dy * near), rgba(col, 1.0), false);
            out.line(Point::new(o.x + dx * near, o.y + dy * near), Point::new(o.x + dx * far, o.y + dy * far), rgba(col, 1.0), false);
        }
    }
}

/// Vector edit mode: a layer's segments, handles and points, its hovered part and the point a
/// press would add.
fn vector_overlay(
    out: &mut Out<'_>,
    o: &kition_design_engine::vectoredit::EditOverlay,
    points: bool,
    hover: Option<kition_design_engine::vectoredit::VHit>,
    green: [u8; 3],
    add: Option<kition_design_engine::tools::VectorAdd>,
) {
    use kition_design_engine::vectoredit::VHit;
    let pt = |p: &[f64; 2]| Point::new(p[0], p[1]);
    let hover_seg = match hover {
        Some(VHit::Segment { seg, .. }) => Some(seg),
        _ => None,
    };
    for (i, seg) in o.segments.iter().enumerate() {
        let mut path = BezPath::new();
        path.move_to(pt(&seg[0]));
        path.curve_to(pt(&seg[1]), pt(&seg[2]), pt(&seg[3]));
        let width = if o.selected_segments.contains(&i) {
            3.0
        } else if hover_seg == Some(i) {
            2.5
        } else {
            1.5
        };
        out.path(&path, rgba(green, 1.0), width, false);
    }
    if !points {
        return;
    }
    for h in &o.handles {
        out.line(pt(&h.vertex), pt(&h.handle), rgba(green, 1.0), false);
        let on = o.selected_handles.contains(&(h.seg, h.start));
        if on {
            out.anchor(pt(&h.handle), rgba(green, 1.0), true, 7.0);
        } else {
            out.handle(pt(&h.handle), rgba(green, 1.0));
        }
    }
    for (i, v) in o.vertices.iter().enumerate() {
        let hovered = matches!(hover, Some(VHit::Vertex { index }) if index == i);
        out.anchor(pt(v), rgba(green, 1.0), o.selected_vertices.contains(&i), if hovered { 9.0 } else { 7.0 });
        if o.anchor == Some(i) {
            out.circle(pt(v), 7.5, rgba(green, 1.0), 1.0);
        }
    }
    if let Some(a) = add {
        let p = Point::new(a.at[0], a.at[1]);
        if a.midpoint {
            out.anchor(p, rgba(green, if a.on { 1.0 } else { 0.7 }), a.on, if a.on { 11.0 } else { 7.0 });
        } else {
            out.anchor(p, rgba(green, 0.8), false, 7.0);
        }
    }
}

/// The text being edited: its box, the selected characters (or the IME composition, underlined)
/// and the caret.
fn text_overlay(
    out: &mut Out<'_>,
    doc: &Document,
    te: &kition_design_engine::TextEdit,
    preedit: Option<((usize, usize), usize)>,
    green: [u8; 3],
    ink: [u8; 3],
) {
    if let Some(q) = node_quad(doc, te.id) {
        out.quad(q, rgba(green, 1.0), 1.0, false);
    }
    let Some(n) = doc.node(te.id) else { return };
    let Some((layout, dy)) = kition_design_text::layout_node(n) else { return };
    let world = doc.world_transform(te.id);
    let at = |x: f64, y: f64| {
        let q = layout.warp_point(x, y);
        world * Point::new(q.x, q.y + dy)
    };
    let (a, b) = (te.caret.min(te.anchor), te.caret.max(te.anchor));
    if a != b && preedit.is_none() {
        for r in layout.selection_rects(a, b) {
            out.highlight([at(r.x0, r.y0), at(r.x1, r.y0), at(r.x1, r.y1), at(r.x0, r.y1)], rgba(green, 0.3));
        }
    }
    if let Some(((p0, p1), _)) = preedit {
        for r in layout.selection_rects(p0, p1) {
            out.line(at(r.x0, r.y1), at(r.x1, r.y1), rgba(ink, 1.0), false);
        }
    }
    if let Some((top, bottom)) = caret_line(doc, te, preedit) {
        out.line(top, bottom, rgba(ink, 1.0), false);
    }
}

/// The caret of the text being edited (at the input method's cursor while composing) as the canvas
/// draws it: its top and bottom on the page. `doc` is the document as shown (with the
/// composition, see [`kition_design_engine::textedit::preedit_doc`]).
pub(crate) fn caret_line(doc: &Document, te: &kition_design_engine::TextEdit, preedit: Option<((usize, usize), usize)>) -> Option<(Point, Point)> {
    let n = doc.node(te.id)?;
    let (layout, dy) = kition_design_text::layout_node(n)?;
    let world = doc.world_transform(te.id);
    let at = |x: f64, y: f64| {
        let q = layout.warp_point(x, y);
        world * Point::new(q.x, q.y + dy)
    };
    let caret = preedit.map_or(te.caret, |(_, cursor)| cursor);
    let (cx, li) = layout.caret(caret);
    Some(match layout.lines.get(li).or(layout.lines.last()) {
        Some(line) => (at(cx, line.top), at(cx, line.top + line.height)),
        None => {
            let p = at(0.0, 0.0);
            (p, Point::new(p.x, p.y + 14.0))
        }
    })
}
