//! Dev Mode's focus bar, compare view and component playground picture (upstream
//! `panels/dev_mode.rs` `focus_bar`, `panels/dev_compare.rs`, the Inspect panel's playground).

use std::sync::Arc;

use kition_design_doc::{DevStatus, Document, NodeId};
use kition_design_engine::devcompare::{self, ChangeKind, Comparison};
use serde_json::{Map, Value, json};

use crate::DesignDocument;

/// The compare view's longest image side (upstream's texture side).
pub const COMPARE_SIDE: f64 = 1400.0;
/// The largest playground picture side.
pub const MAX_PLAYGROUND_SIDE: f64 = 2048.0;

/// `dev.focusState`: the focus view's bar, or `null` while no frame is focused (or a version
/// preview has the top of the canvas):
///
/// ```text
/// { id, name, status: "none"|"readyForDev"|"completed", changed,  // the focused frame
///   index: null or its place among the ready-for-dev frames (0-based),
///   count,          // ready-for-dev frames in the file
///   canStep,        // Previous / Next lead somewhere (dev.focus { direction: "previous" | "next" })
///   canCompare }    // Compare changes shows (ready for dev and changed)
/// ```
pub fn focus_state(doc: &DesignDocument) -> Result<Value, String> {
    let ds = doc.ds()?;
    let (Some(id), None) = (ds.focused(), ds.preview.as_ref()) else { return Ok(Value::Null) };
    let d: &Document = &ds.doc;
    let Some(n) = d.node(id) else { return Ok(Value::Null) };
    let list: Vec<NodeId> = kition_design_engine::devinspect::dev_items(d, Some(DevStatus::ReadyForDev), None).into_iter().map(|i| i.id).collect();
    let index = list.iter().position(|i| *i == id);
    Ok(json!({
        "id": id,
        "name": n.name,
        "status": n.dev_status.id(),
        "changed": n.dev_changed,
        "index": index,
        "count": list.len(),
        "canStep": list.len() > usize::from(index.is_some()),
        "canCompare": n.dev_status == DevStatus::ReadyForDev && n.dev_changed,
    }))
}

/// A compare view's layer, version, comparison, and its before and after documents.
type Compared = (NodeId, Option<String>, Comparison, Option<Arc<Document>>, Arc<Document>);

/// The two documents a compare view shows for layer `id` and `snapshot` (a saved version's KDM
/// snapshot, base64, as the host's checkpoint carries it; none: the snapshot from when it was
/// marked ready for dev), with the comparison and the replica version of the snapshot compared.
fn compared(doc: &DesignDocument, params: &Value) -> Result<Compared, String> {
    let ds = doc.ds()?;
    let id = super::target(doc, params)?;
    let now = ds.doc.clone();
    if now.node(id).is_none() {
        return Err(format!("not-found: no layer {id}"));
    }
    let snapshot = match params.get("snapshot") {
        None | Some(Value::Null) => None,
        Some(Value::String(v)) => Some(v.clone()),
        Some(other) => return Err(format!("invalid-params: `snapshot` must be a KDM snapshot (base64), not {other}")),
    };
    let (cmp, before, version) = match &snapshot {
        Some(v) => {
            let b = kition_design_engine::cmd::file::content_at("dev.compare", ds, v).map_err(crate::engine_error)?;
            let version = kition_design_engine::cmd::file::snapshot_version("dev.compare", v).map_err(crate::engine_error)?;
            (devcompare::compare_with(&b, &now, id), Some(b), Some(version))
        }
        None => (devcompare::compare(&now, id), devcompare::before_doc(&now, id).map(Arc::new), None),
    };
    Ok((id, version, cmp, before, now))
}

/// Where a side's picture is: its page-space area and the image's size at the picture's scale.
fn shot(d: &Document, id: NodeId, max_side: f64) -> Option<(kition_design_geom::Rect, f64)> {
    let area = kition_design_render::export_bounds(d, id)?;
    let side = area.width().max(area.height());
    (side.is_finite() && side > 0.0).then(|| (area, (max_side / side).min(2.0)))
}

/// `dev.compare { id?, snapshot?, maxSide? }`: what the compare view lists and outlines for a
/// ready-for-dev frame or section (`id`, default the first selected layer) against the snapshot
/// from when it was marked, or against the saved version whose KDM snapshot is `snapshot` (the
/// editor reads it from the host's checkpoint):
///
/// ```text
/// { …Comparison,          // the engine's dev.compare: id, name, changed, snapshot, changes: [{ id, name, type, change, depth, layers, properties }], added, removed, modified, truncated
///   version,              // the replica version of the snapshot compared with, or null
///   before: Side or null, // null: the before is not known (no snapshot)
///   after: Side or null }
/// Side = { area: { x, y, width, height } (page space), scale, width, height (the picture's pixels),
///          boxes: [{ id, change, rect: [x0, y0, x1, y1] (in the picture's pixels) }] }
/// ```
///
/// The before side outlines what was removed, the after side what was added or changed (depth
/// > 0), as upstream draws them. `devCompareRender` draws each side at the same scale.
pub fn compare(doc: &DesignDocument, params: &Value) -> Result<Value, String> {
    let (id, version, cmp, before, now) = compared(doc, params)?;
    let max_side = max_side(params, COMPARE_SIDE)?;
    let side = |d: &Document, old: bool| -> Value {
        let Some((area, scale)) = shot(d, id, max_side) else { return Value::Null };
        let boxes: Vec<Value> = cmp
            .changes
            .iter()
            .filter(|c| c.depth > 0 && (c.change == ChangeKind::Removed) == old)
            .filter_map(|c| {
                d.world_bounds(c.id).map(|b| {
                    let r = [(b.x0 - area.x0) * scale, (b.y0 - area.y0) * scale, (b.x1 - area.x0) * scale, (b.y1 - area.y0) * scale];
                    json!({ "id": c.id, "change": c.change, "rect": r })
                })
            })
            .collect();
        json!({
            "area": { "x": area.x0, "y": area.y0, "width": area.width(), "height": area.height() },
            "scale": scale,
            "width": (area.width() * scale).ceil(),
            "height": (area.height() * scale).ceil(),
            "boxes": boxes,
        })
    };
    let mut v = serde_json::to_value(&cmp).map_err(|e| e.to_string())?;
    if let Some(o) = v.as_object_mut() {
        o.insert("version".into(), json!(version));
        o.insert("before".into(), before.as_deref().map_or(Value::Null, |b| side(b, true)));
        o.insert("after".into(), side(&now, false));
    }
    Ok(v)
}

fn max_side(params: &Value, default: f64) -> Result<f64, String> {
    match params.get("maxSide") {
        None | Some(Value::Null) => Ok(default),
        Some(v) => v
            .as_f64()
            .filter(|s| s.is_finite() && *s >= 1.0)
            .map(|s| s.min(MAX_PLAYGROUND_SIDE))
            .ok_or_else(|| format!("invalid-params: `maxSide` {v} must be a positive number")),
    }
}

fn png(mut pix: kition_design_render::Pixmap, profile: kition_design_color::ColorProfile) -> Result<Vec<u8>, String> {
    kition_design_render::to_srgb(&mut pix, profile);
    kition_design_render::encode_png(&pix)
}

impl DesignDocument {
    /// `devCompareRender { id?, snapshot?, side: "before" | "after", maxSide? }`: one side of the
    /// compare view as a PNG, the size and scale `dev.compare` gives for that side (longest side
    /// `maxSide`, default 1400, at most 2x). An error when the side is not known (`before`
    /// without a snapshot).
    pub fn dev_compare_render(&self, params: &Value) -> Result<Vec<u8>, String> {
        let (id, _, _, before, now) = compared(self, params)?;
        let max_side = max_side(params, COMPARE_SIDE)?;
        let d: Arc<Document> = match params.get("side").and_then(Value::as_str) {
            Some("before") => before.ok_or_else(|| "not-found: what it changed from is not known".to_owned())?,
            Some("after") => now,
            _ => return Err("invalid-params: `side` is \"before\" or \"after\"".into()),
        };
        let (_, scale) = shot(&d, id, max_side).ok_or_else(|| format!("layer {id} draws nothing there"))?;
        let pix = kition_design_render::render_node(&d, id, scale).ok_or_else(|| format!("cannot draw layer {id}"))?;
        png(pix, d.color_profile)
    }

    /// `playgroundPreview { id?, properties?, scale?, maxSide? }`: the component playground's
    /// picture (Dev Mode's Inspect): a new instance of `id`'s component (an instance, a component
    /// or a component set; default the selection) with property values `properties` (`{ name:
    /// value }`, as `instance.setProperties` takes them; an instance starts from its own), drawn on
    /// a copy of the document as a transparent PNG at `scale` (default 2), fitted to `maxSide`
    /// (default 2048). The document never changes. `query("playground")` gives the same
    /// instance's properties and code.
    pub fn playground_preview(&self, params: &Value) -> Result<Vec<u8>, String> {
        let id = super::target(self, params)?;
        let values: Map<String, Value> = match params.get("properties") {
            None | Some(Value::Null) => Map::new(),
            Some(Value::Object(o)) => o.clone(),
            Some(other) => return Err(format!("invalid-params: `properties` must be an object, not {other}")),
        };
        let scale = match params.get("scale") {
            None | Some(Value::Null) => 2.0,
            Some(v) => v
                .as_f64()
                .filter(|s| s.is_finite() && *s > 0.0)
                .map(|s| s.min(8.0))
                .ok_or_else(|| format!("invalid-params: `scale` {v} must be a positive number"))?,
        };
        let max_side = max_side(params, MAX_PLAYGROUND_SIDE)?;
        let d: &Document = &self.ds()?.doc;
        let (copy, inst) = crate::no_files(|| kition_design_engine::playground::build(d, id, &values)).map_err(|e| e.to_string())?;
        let b = kition_design_render::export_bounds(&copy, inst).ok_or("the playground instance draws nothing")?;
        let side = b.width().max(b.height());
        if !(side.is_finite() && side > 0.0) {
            return Err("the playground instance draws nothing".into());
        }
        let scale = scale.min(max_side / side);
        let pix = kition_design_render::render_node(&copy, inst, scale).ok_or("cannot draw the playground instance")?;
        png(pix, copy.color_profile)
    }
}
