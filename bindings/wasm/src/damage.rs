//! What a tool event changed on the canvas (`damage`, kition-kit's `KitTools::take_damage`): the
//! area of the page a canvas showing it before the event must draw again, so a drag repaints the
//! dragged layer's old and new places instead of the whole view (stage 4 of the hand plan, gap R2
//! item 5). It is found by comparing what the canvas draws before and after the event
//! ([`kition_design_render::changed_area`]), so it holds for every tool, auto layout and instances
//! included; anything else that changes how the page draws (another page, outline mode, the focus
//! view, new fonts, the document frame moving) repaints everything, and so does every edit in
//! outline mode. A patch of a curved edge cut by the area's border can differ from the whole
//! render by a few levels of anti-aliasing (the renderer splits curves at a render's edge), as the
//! engine's own retained renderer's patches do; the next whole render (a zoom, a pan past the
//! frame, a command) draws it exactly.

use std::sync::Arc;

use kition_design_doc::{Document, NodeId};
use kition_kit::tools::ToolDamage;

use crate::{Area, DesignDocument};

/// What the canvas draws ([`crate::render`]): the document as shown (with the text being
/// composed), its page and frame, and the view options that change pixels.
pub(crate) struct Shown {
    doc: Arc<Document>,
    page: NodeId,
    area: Area,
    revision: u64,
    look: Look,
}

/// The view options [`crate::render`] draws with, and the fonts it has.
#[derive(PartialEq)]
struct Look {
    outlines: bool,
    hidden: bool,
    bounds: bool,
    outlined: Vec<NodeId>,
    solo: Option<NodeId>,
    fonts: u64,
}

impl DesignDocument {
    /// What the canvas draws now; none without a document.
    pub(crate) fn shown(&self) -> Option<Shown> {
        let ds = self.session.ds().ok()?;
        let doc = match kition_design_engine::textedit::preedit_doc(&self.session) {
            Some((doc, _, _)) => Arc::new(doc),
            None => ds.doc.clone(),
        };
        let look = Look {
            outlines: ds.view.outlines,
            hidden: ds.view.outline_hidden,
            bounds: ds.view.outline_bounds,
            outlined: if ds.view.outlines_selection { ds.selection.clone() } else { Vec::new() },
            solo: ds.focused(),
            fonts: kition_design_text::layout_generation(),
        };
        Some(Shown { doc, page: ds.page, area: self.area(), revision: ds.revision, look })
    }

    /// Keeps what the event that found the canvas at `before` changed, for the answer's `damage`.
    pub(crate) fn note_damage(&mut self, before: Option<Shown>, zoom: f64) {
        self.damage = before.zip(self.shown()).and_then(|(before, after)| between(&before, &after, zoom));
    }
}

/// The document pixels (the frame's, as the canvas renders them) that differ between `before` and
/// `after` at `zoom`, or none: repaint everything (also when nothing changed: the canvas then
/// repaints nothing anyway).
fn between(before: &Shown, after: &Shown, zoom: f64) -> Option<ToolDamage> {
    if before.revision == after.revision || before.page != after.page || before.area != after.area || before.look != after.look {
        return None;
    }
    // Outlines are hairlines, whose anti-aliasing shifts with where a render starts: a patch would
    // leave half-covered rows the whole render does not have. Outline mode repaints everything.
    if after.look.outlines || !after.look.outlined.is_empty() {
        return None;
    }
    let zoom = if zoom.is_finite() && zoom > 0.0 { zoom } else { 1.0 };
    let changed =
        if Arc::ptr_eq(&before.doc, &after.doc) { None } else { kition_design_render::changed_area(&before.doc, &after.doc, after.page, zoom)? };
    let Some(r) = changed else {
        // No pixel changed (a click that only selects): the smallest area, next to nothing to draw.
        return Some(ToolDamage { x0: 0.0, y0: 0.0, x1: 1.0, y1: 1.0 });
    };
    // Anti-aliasing reaches past a shape's bounds: two device pixels, at any pixel ratio and in a
    // zoomed-out frame (whose pixels are at most 1 / zoom document pixels).
    let m = 2.0 / zoom;
    let a = after.area;
    let damage = ToolDamage { x0: r.x0 - a.x - m, y0: r.y0 - a.y - m, x1: r.x1 - a.x + m, y1: r.y1 - a.y + m };
    [damage.x0, damage.y0, damage.x1, damage.y1].iter().all(|v| v.is_finite()).then_some(damage)
}
