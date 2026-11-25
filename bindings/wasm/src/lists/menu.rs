//! `menu.context`: the canvas and layer context menu with per-command availability, as upstream's
//! `menus.rs` builds it (the entries, then the section, convert, combine and main component
//! substitutions). As upstream's `available`, a row is enabled when the kit offers its command:
//! running one that has nothing to act on is a no-op or answers its own error.

use kition_design_doc::{Document, NodeId, NodeKind};
use kition_design_engine::keymap::{self, Platform};
use serde_json::{Value, json};

use crate::DesignDocument;

/// A separator row.
const SEP: (&str, &str) = ("-", "");
/// The Copy as submenu's marker.
const COPY_AS: &str = "copyAs";

/// Commands the kit's canvas or the editor runs itself (not in the kit's catalog): always
/// enabled, marked `ui: true`.
const UI_ONLY: &[&str] = &["view.toggleUi", "view.zoomFit"];

fn entries(d: &Document, ids: &[NodeId], selection: &[NodeId], guide: bool) -> Vec<(&'static str, &'static str)> {
    let has_sel = !ids.is_empty();
    let nodes: Vec<_> = ids.iter().filter_map(|i| d.node(*i)).collect();
    let mut e: Vec<(&str, &str)> = if has_sel {
        vec![
            ("Copy", "edit.copy"),
            ("Paste here", "edit.paste"),
            ("Paste to replace", "edit.pasteReplace"),
            ("Copy as", COPY_AS),
            SEP,
            ("Bring to front", "arrange.bringToFront"),
            ("Bring forward", "arrange.bringForward"),
            ("Send backward", "arrange.sendBackward"),
            ("Send to back", "arrange.sendToBack"),
            SEP,
            ("Group selection", "arrange.group"),
            ("Frame selection", "object.frameSelection"),
            ("Wrap in new section", "object.wrapInSection"),
            ("Ungroup", "arrange.ungroup"),
            ("Flatten", "vector.flatten"),
            ("Outline stroke", "vector.outlineStroke"),
            ("Use as mask", "object.mask"),
            SEP,
            ("Add auto layout", "layout.add"),
            ("Create component", "component.add"),
            ("Detach instance", "component.detach"),
            SEP,
            ("Show/Hide", "object.toggleVisible"),
            ("Lock/Unlock", "object.toggleLock"),
            SEP,
            ("Flip horizontal", "object.flipH"),
            ("Flip vertical", "object.flipV"),
            SEP,
            ("Delete", "edit.delete"),
        ]
    } else {
        vec![
            ("Paste here", "edit.paste"),
            SEP,
            ("Select all", "edit.selectAll"),
            SEP,
            ("Show/Hide UI", "view.toggleUi"),
            ("Zoom to fit", "view.zoomFit"),
            ("Rulers", "view.rulers"),
            ("Pixel grid", "view.pixelGrid"),
        ]
    };
    // Sections in the selection add a contents toggle naming what it does.
    let secs: Vec<bool> = nodes.iter().filter(|n| matches!(n.kind, NodeKind::Section { .. })).map(|n| n.hides_contents()).collect();
    if !secs.is_empty()
        && let Some(at) = e.iter().position(|x| x.1 == "object.toggleLock")
    {
        let label = if secs.iter().all(|h| *h) { "Show section contents" } else { "Hide section contents" };
        e.insert(at + 1, (label, "section.toggleContents"));
    }
    // Frames on the page (or in a section) convert to sections, and sections to frames.
    let on_board =
        |n: &kition_design_doc::Node| n.parent.and_then(|p| d.node(p)).is_some_and(|p| p.is_page() || matches!(p.kind, NodeKind::Section { .. }));
    let convert = if nodes.is_empty() {
        None
    } else if nodes.iter().all(|n| matches!(n.kind, NodeKind::Section { .. })) {
        Some(("Convert to frame", "object.convertToFrame"))
    } else if nodes.iter().all(|n| matches!(n.kind, NodeKind::Frame) && on_board(n)) {
        Some(("Convert to section", "object.convertToSection"))
    } else {
        None
    };
    if let Some(entry) = convert
        && let Some(at) = e.iter().position(|x| x.1 == "object.wrapInSection")
    {
        e.insert(at + 1, entry);
    }
    // Judged on the selection the command acts on, as upstream does.
    if kition_design_engine::cmd::component::can_combine_variants(d, selection)
        && let Some(at) = e.iter().position(|x| x.1 == "component.add")
    {
        e.insert(at + 1, ("Combine as variants", "component.combineVariants"));
    }
    if has_sel
        && nodes.len() == ids.len()
        && nodes.iter().all(|n| matches!(n.kind, NodeKind::Instance { .. }))
        && let Some(at) = e.iter().position(|x| x.1 == "component.add")
        && let Some(slot) = e.get_mut(at)
    {
        *slot = ("Go to main component", "component.goToMain");
    }
    if !has_sel && guide {
        e.splice(0..0, [("Remove guide", "guide.remove"), SEP]);
    }
    e
}

fn shortcut(id: &str) -> Option<String> {
    keymap::primary(id, Platform::Mac).and_then(crate::commands::kit_shortcut)
}

/// `menu.context { ids?, at?: { x, y } }`: the context menu for `ids` (default the selection; a
/// layer row right-clicked outside the selection passes just itself) opened at document pixel
/// `at`:
///
/// ```text
/// { ids,
///   items: [{ command, label, shortcut: "Cmd+C" or null, enabled, checked, ui,
///             params?,                 // what to run it with (Paste here: { x, y } in page space at `at`)
///             submenu?: [Item] }       // Copy as: Copy as CSS, SVG, PNG, Copy link
///           or { separator: true }],
///   selectLayer: [{ id, name, type, depth }] or null }   // the layers under `at` (upstream's "Select layer" submenu)
/// ```
///
/// `enabled`: the kit offers the command and the host has not disabled it (upstream's `available`:
/// Paste with nothing copied, Ungroup on a rectangle or Detach on a frame stay enabled and do
/// nothing or say why). `checked`: a View toggle that is on (Rulers, Pixel grid). `ui`:
/// the editor runs it itself (Show/Hide UI, Zoom to fit). Labels are upstream's English text.
pub fn context(doc: &DesignDocument, params: &Value) -> Result<Value, String> {
    let ds = doc.ds()?;
    let d: &Document = &ds.doc;
    let ids: Vec<NodeId> = match params.get("ids").and_then(Value::as_array) {
        Some(a) => a.iter().filter_map(kition_design_engine::params::id_of).filter(|i| d.node(*i).is_some_and(|n| !n.is_page())).collect(),
        None => ds.selection.clone(),
    };
    let at = match params.get("at") {
        None | Some(Value::Null) => None,
        Some(v) => match (v.get("x").and_then(Value::as_f64), v.get("y").and_then(Value::as_f64)) {
            (Some(x), Some(y)) if x.is_finite() && y.is_finite() => Some(doc.area().to_canvas(x, y)),
            _ => return Err("invalid-params: `at` must be { x, y } in document pixels".into()),
        },
    };
    let disabled = crate::commands::disabled(doc.session());
    let item = |label: &str, id: &str| -> Result<Value, String> {
        let ui = UI_ONLY.contains(&id);
        let mut run_params = json!({});
        if id == "edit.paste"
            && let Some(p) = at
        {
            run_params = json!({ "x": p.x, "y": p.y });
        }
        let enabled = ui || (crate::commands::offered(id) && !disabled.iter().any(|x| x == id));
        let checked = kition_design_engine::cmd::view::checked(doc.session(), id).unwrap_or(false);
        let mut v = json!({ "command": id, "label": label, "shortcut": shortcut(id), "enabled": enabled, "checked": checked, "ui": ui });
        if id == "edit.paste" && at.is_some() {
            v["params"] = run_params;
        }
        Ok(v)
    };
    let mut items = Vec::new();
    for (label, id) in entries(d, &ids, &ds.selection, ds.selected_guide().is_some()) {
        if label == "-" {
            items.push(json!({ "separator": true }));
            continue;
        }
        if id == COPY_AS {
            let sub = [
                ("Copy as CSS", "edit.copyAsCss"),
                ("Copy as SVG", "edit.copyAsSvg"),
                ("Copy as PNG", "edit.copyAsPng"),
                ("Copy link to selection", "edit.copyLink"),
            ]
            .iter()
            .map(|(l, i)| item(l, i))
            .collect::<Result<Vec<_>, _>>()?;
            let enabled = sub.iter().any(|s| s["enabled"] == true);
            items.push(
                json!({ "command": COPY_AS, "label": label, "shortcut": null, "enabled": enabled, "checked": false, "ui": false, "submenu": sub }),
            );
            continue;
        }
        items.push(item(label, id)?);
    }
    let select_layer = match at {
        Some(p) => {
            let mut v = crate::query::read(doc, "select.layersAt", &json!({ "x": p.x, "y": p.y }))?;
            v.get_mut("layers").map(Value::take).unwrap_or(Value::Null)
        }
        None => Value::Null,
    };
    Ok(json!({ "ids": ids, "items": items, "selectLayer": select_layer }))
}
