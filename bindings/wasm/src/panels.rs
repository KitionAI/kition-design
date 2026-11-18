//! Panel reads the Design panel's sections, the colour picker and the Stroke section need beyond
//! [`crate::query`]'s table, read the way the upstream panels read `app.session`
//! (`panels/right/*.rs`, `panels/layout_panel.rs`, `panels/component_panel.rs`,
//! `panels/variable_ui.rs`, `widgets/colorpicker.rs`):
//!
//! | name | params | answer |
//! | --- | --- | --- |
//! | `variable.modeRows` | `{ ids? }` (default the selection) | `{ show, rows: [ModeRow] }` (see [`mode_rows`]) |
//! | `documentColors` | `{ page?, limit? }` | `{ colors: [{ color, css, alpha, count }] }` (see [`document_colors`]) |
//! | `clipboard.paintState` | — | `{ fill, stroke, effect, properties, layers }`: what Paste fill, Paste stroke, Paste effect, Paste properties and Paste have to paste |
//! | `image.info` | `{ hash }` | `{ hash, width, height, bytes, format, profile, uses }` (see [`image_info`]) |
//! | `history` | `{ limit? }` | `{ undo: [label], redo: [label] }`, the next step first (at most `limit`, default 50) |
//!
//! [`node_json`] is the engine's `node_json` with the keys the panels read that the engine leaves
//! out; `query("node")` and `query("selection")` answer it.

use std::collections::BTreeSet;

use kition_design_doc::{Document, NodeId, NodeKind};
use serde_json::{Map, Value, json};

use crate::DesignDocument;

/// Most layers a [`mode_rows`] answer reads.
const MAX_IDS: usize = 500;

fn ser<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

/// The engine's `inspect::node_json` (to `depth`) plus, on every layer it lists:
///
/// - `constrainProportions` (the W/H link), `overflow` (`none`, `horizontal`, `vertical`, `both`:
///   prototype scrolling) and `scrollPosition` (`scrolls`, `fixed`, `sticky`), `startCap` and
///   `endCap` (stroke caps: `none`, `round`, `square`, `arrowLines`, …, as `node.set` takes them),
///   `varModes` (`{ collection: mode }`, the layer's own variable modes; Auto modes are absent),
///   `propRefs` (`{ field: property }`: which component property controls the layer's `visible`,
///   `characters` or `mainComponent`, or which slot property a frame is);
/// - the parent facts: `parent` (id or null), `parentAutoLayout`, `parentGrid` (the parent's auto
///   layout is a grid), `insideInstance` (a layer inside an instance, not the instance itself:
///   `components::inside_instance`), `hasStatus` (it can carry a handoff status:
///   `devinspect::has_status`);
/// - component sets: `description`;
/// - texts: `textAlignVertical` (`top`, `center`, `bottom`), `paragraphSpacing`,
///   `paragraphIndent` and `features` (`{ tag: on }`, the OpenType toggles set on the layer).
pub fn node_json(d: &Document, id: NodeId, depth: usize) -> Value {
    let mut v = kition_design_engine::inspect::node_json(d, id, depth);
    add_keys(d, &mut v, 0);
    v
}

fn add_keys(d: &Document, v: &mut Value, level: usize) {
    let Some(o) = v.as_object_mut() else { return };
    let Some(id) = o.get("id").and_then(kition_design_engine::params::id_of) else { return };
    let Some(n) = d.node(id) else { return };
    if !n.is_page() {
        let parent = n.parent.and_then(|p| d.node(p));
        let parent_layout = parent.and_then(|p| p.layout.as_ref());
        o.insert("constrainProportions".into(), json!(n.constrain_proportions));
        o.insert("overflow".into(), ser(&n.overflow));
        o.insert("scrollPosition".into(), ser(&n.scroll_position));
        o.insert("startCap".into(), ser(&n.stroke.cap_start));
        o.insert("endCap".into(), ser(&n.stroke.cap_end));
        o.insert("varModes".into(), json!(n.var_modes));
        o.insert("propRefs".into(), json!(n.prop_refs));
        o.insert("parent".into(), json!(parent.filter(|p| !p.is_page()).map(|p| p.id)));
        o.insert("parentAutoLayout".into(), json!(parent_layout.is_some()));
        o.insert("parentGrid".into(), json!(parent_layout.is_some_and(|l| l.mode == kition_design_doc::LayoutMode::Grid)));
        o.insert("insideInstance".into(), json!(kition_design_engine::components::inside_instance(d, id)));
        o.insert("hasStatus".into(), json!(kition_design_engine::devinspect::has_status(d, id)));
    }
    match &n.kind {
        NodeKind::ComponentSet { description } => {
            o.insert("description".into(), json!(description));
        }
        NodeKind::Text(t) => {
            o.insert("textAlignVertical".into(), ser(&t.align_v));
            o.insert("paragraphSpacing".into(), json!(t.paragraph_spacing));
            o.insert("paragraphIndent".into(), json!(t.paragraph_indent));
            o.insert("features".into(), json!(t.features));
        }
        _ => {}
    }
    if level < 64
        && let Some(Value::Array(children)) = o.get_mut("children")
    {
        for child in children {
            add_keys(d, child, level + 1);
        }
    }
}

/// The selection-wide facts of upstream's Design panel router (`panels/right.rs`,
/// `layout_panel.rs`, `section_panel.rs`, `component_panel.rs`), merged into `query("selection")`:
/// `parents` (each selected layer's parent, null at the top of a page), `allParentsAutoLayout`
/// (every parent has auto layout: constraints give way to auto layout sizing), `insideInstance`
/// (any selected layer is inside an instance), `hasStatus` (every selected layer can carry a
/// handoff status: the Status block shows), `canAddVariant` (a single component or component set
/// that "Add variant" works on) and `canCombineVariants` (two or more main components "Combine as
/// variants" takes).
pub fn selection_facts(d: &Document, ids: &[NodeId], out: &mut Map<String, Value>) {
    let parents: Vec<Option<NodeId>> =
        ids.iter().map(|i| d.node(*i).and_then(|n| n.parent).filter(|p| d.node(*p).is_some_and(|p| !p.is_page()))).collect();
    let all_al = !ids.is_empty() && ids.iter().all(|i| d.node(*i).and_then(|n| n.parent).and_then(|p| d.node(p)).is_some_and(|p| p.layout.is_some()));
    out.insert("parents".into(), json!(parents));
    out.insert("allParentsAutoLayout".into(), json!(all_al));
    out.insert("insideInstance".into(), json!(ids.iter().any(|i| kition_design_engine::components::inside_instance(d, *i))));
    out.insert("hasStatus".into(), json!(!ids.is_empty() && ids.iter().all(|i| kition_design_engine::devinspect::has_status(d, *i))));
    let single = match ids {
        [id] => kition_design_engine::cmd::component::can_add_variant(d, *id),
        _ => false,
    };
    out.insert("canAddVariant".into(), json!(single));
    out.insert("canCombineVariants".into(), json!(kition_design_engine::cmd::component::can_combine_variants(d, ids)));
}

fn ids_param(doc: &DesignDocument, params: &Value) -> Result<Vec<NodeId>, String> {
    match params.get("ids") {
        None | Some(Value::Null) => Ok(doc.ds()?.selection.clone()),
        Some(Value::Array(a)) => Ok(a.iter().filter_map(kition_design_engine::params::id_of).take(MAX_IDS).collect()),
        Some(other) => Err(format!("invalid-params: `ids` must be an array of layer ids, not {other}")),
    }
}

/// A mode row: the collection, its library (id, name), its id in the library, and its local id.
type ModeRow<'a> = (&'a kition_design_doc::Collection, Option<(&'a str, &'a str)>, Option<&'a str>, Option<&'a str>);

/// `variable.modeRows { ids? }`: the Design panel's Variable modes section (upstream
/// `panels/right/paints.rs` `variable_modes` with `variable_ui::mode_rows`). `show` is false
/// unless every layer is frame-like (frames, sections, components, instances) and a collection
/// has two or more modes. A row per such collection: this file's first, then each enabled
/// library's published ones (their copies once used here), then copies of collections from
/// libraries that can't be read:
///
/// ```text
/// { collection: null | id,     // the id frames keep the mode under: this file's collection or
///                              // a library collection's copy; null until a library's is used
///   key: null | id,            // a library collection's id in the library (while it is read)
///   name, library: null | { id, name },
///   modes: [{ id, name }],
///   own: null | mode id,       // the layers' own mode; null: Auto (or mixed)
///   mixed: bool,               // the layers' own modes differ
///   inherited: null | { id, name } }   // what Auto resolves to for the first layer
/// ```
///
/// Set a mode with `variable.setMode { ids, collection, mode }` (`mode: null` for Auto), or, for a
/// library's row with a `key`, `library.setVariableMode { library, collection: key, ids, mode }`,
/// which copies the collection in.
pub fn mode_rows(doc: &DesignDocument, params: &Value) -> Result<Value, String> {
    use kition_design_doc::Collection;
    use kition_design_engine::library;
    let ids = ids_param(doc, params)?;
    let d = &doc.ds()?.doc;
    let frames = !ids.is_empty() && ids.iter().all(|i| d.node(*i).is_some_and(|n| n.kind.is_frame_like()));
    let several = |c: &Collection| c.modes.len() > 1;
    let read: Vec<_> = crate::libraries::enabled(doc).into_iter().filter_map(|(_, lib)| lib.ok()).collect();
    let mut rows: Vec<ModeRow<'_>> = library::local_collections(d).filter(|c| several(c)).map(|c| (c, None, None, Some(c.id.as_str()))).collect();
    let copy = |lib: &str, key: &str| d.libraries.collections.iter().find(|r| r.library == lib && r.key == key).map(|r| r.local.as_str());
    for lib in &read {
        let (id, name) = (lib.manifest.id.as_str(), library::library_name(d, &lib.manifest.id));
        for c in library::published_collections(lib) {
            let local = copy(id, &c.id).filter(|l| d.variables.collection(l).is_some());
            let coll = local.and_then(|l| d.variables.collection(l)).unwrap_or(c);
            if several(coll) {
                rows.push((coll, Some((id, name)), Some(c.id.as_str()), local));
            }
        }
    }
    for r in &d.libraries.collections {
        if rows.iter().any(|x| x.3 == Some(r.local.as_str())) {
            continue;
        }
        if let Some(c) = d.variables.collection(&r.local).filter(|c| several(c)) {
            rows.push((c, Some((r.library.as_str(), library::library_name(d, &r.library))), None, Some(r.local.as_str())));
        }
    }
    let parent_modes = ids.first().map(|i| d.inherited_modes(*i)).unwrap_or_default();
    let answer: Vec<Value> = rows
        .iter()
        .map(|(c, lib, key, local)| {
            let mode = |m: &str| c.modes.iter().find(|x| x.id == m).map(|x| json!({ "id": x.id, "name": x.name }));
            let (own, mixed) = match local {
                Some(l) => {
                    let owns: BTreeSet<Option<&String>> = ids.iter().filter_map(|i| d.node(*i)).map(|n| n.var_modes.get(*l)).collect();
                    let mixed = owns.len() > 1;
                    (if mixed { None } else { owns.into_iter().next().flatten() }, mixed)
                }
                None => (None, false),
            };
            let inherited = local.and_then(|l| parent_modes.get(l)).or(c.default_mode()).and_then(|m| mode(m));
            json!({
                "collection": local,
                "key": key,
                "name": c.name,
                "library": lib.map(|(id, name)| json!({ "id": id, "name": name })),
                "modes": c.modes.iter().map(|m| json!({ "id": m.id, "name": m.name })).collect::<Vec<_>>(),
                "own": own,
                "mixed": mixed,
                "inherited": inherited,
            })
        })
        .collect();
    Ok(json!({ "show": frames && !answer.is_empty(), "rows": answer }))
}

/// `documentColors { page?, limit? }`: the colour picker's "On this page" swatches, the solid
/// colours the page's visible fills and strokes use, most used first (`paint.documentColors`
/// keeps them apart by their alpha too): `color` is `RRGGBB` as the engine command answers it,
/// `css` is `#RRGGBB` or `#RRGGBBAA` with the colour's own alpha, `alpha` 0–1.
pub fn document_colors(doc: &DesignDocument, params: &Value) -> Result<Value, String> {
    let ds = doc.ds()?;
    let page = kition_design_engine::params::id(params, "page").unwrap_or(ds.page);
    if ds.doc.node(page).is_none_or(|p| !p.is_page()) {
        return Err(format!("not-found: no page {page}"));
    }
    let limit = params.get("limit").and_then(Value::as_f64).filter(|l| l.is_finite()).map_or(21, |l| l.clamp(1.0, 500.0) as usize);
    let colors: Vec<Value> = kition_design_engine::cmd::styles::document_colors(&ds.doc, page)
        .into_iter()
        .take(limit)
        .map(|(c, count)| json!({ "color": c.to_hex(), "css": c.to_css_hex(), "alpha": f64::from(c.a), "count": count }))
        .collect();
    Ok(json!({ "colors": colors }))
}

/// `clipboard.paintState`: what the engine's clipboard holds for the paste-paint menu items
/// (upstream greys them from `session.clipboard`): `fill` (a copied fill row: Paste fill),
/// `stroke` (a copied stroke row and its settings: Paste stroke), `effect` (Paste effect),
/// `properties` (Copy properties: Paste properties), `layers` (copied layers: Paste).
pub fn clipboard_state(doc: &DesignDocument) -> Value {
    let c = &doc.session().clipboard;
    json!({
        "fill": matches!(c.paint, Some((true, _))),
        "stroke": matches!(c.paint, Some((false, _))),
        "effect": c.effect.is_some(),
        "properties": c.props.is_some(),
        "layers": !c.nodes.is_empty(),
    })
}

/// The format of an image file from its first bytes.
fn image_format(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        "png"
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "jpeg"
    } else if bytes.starts_with(b"GIF8") {
        "gif"
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP".as_slice()) {
        "webp"
    } else if bytes.starts_with(b"BM") {
        "bmp"
    } else if bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*") {
        "tiff"
    } else {
        "unknown"
    }
}

/// `image.info { hash }`: one image the file holds (an image paint's `image`), for the colour
/// picker's image settings: `{ hash, width, height, bytes, format, profile, uses }` with `format`
/// `png`, `jpeg`, `gif`, `webp`, `bmp`, `tiff` or `unknown`, `profile` the embedded colour profile's
/// name (upstream's "Color profile" line; null: taken as sRGB), and `uses` how many paints of the
/// file show it.
pub fn image_info(doc: &DesignDocument, params: &Value) -> Result<Value, String> {
    let hash = params.get("hash").and_then(Value::as_str).filter(|h| !h.is_empty()).ok_or("invalid-params: `hash` is required")?;
    let d = &doc.ds()?.doc;
    let data = d.images.get(hash).ok_or_else(|| format!("not-found: the file holds no image {hash}"))?;
    let shows = |p: &kition_design_color::Paint| matches!(&p.kind, kition_design_color::PaintKind::Image(i) if i.image == hash);
    let uses: usize = d.nodes().map(|n| n.fills.iter().chain(n.strokes.iter()).filter(|p| shows(p)).count()).sum();
    Ok(json!({
        "hash": hash,
        "width": data.width,
        "height": data.height,
        "bytes": data.bytes.len(),
        "format": image_format(&data.bytes),
        "profile": kition_design_render::images::profile_name(hash, Some(data)),
        "uses": uses,
    }))
}

/// `history { limit?, scope? }`: the undo and redo steps' labels of one undo scope (`person`, the
/// default, or `agent`), the next one first, as that scope's undo and redo take them (an
/// interaction's step is named after its `beginInteraction` label).
pub fn history(doc: &DesignDocument, params: &Value) -> Result<Value, String> {
    use kition_design_kdm::kition_kit::kdm::UndoScope;
    let limit = params.get("limit").and_then(Value::as_u64).map_or(50, |l| l.clamp(1, 1000) as usize);
    let scope = match params.get("scope") {
        None | Some(Value::Null) => UndoScope::Person,
        Some(Value::String(s)) if s == "person" => UndoScope::Person,
        Some(Value::String(s)) if s == "agent" => UndoScope::Agent,
        Some(other) => return Err(format!("invalid-params: `scope` must be \"person\" or \"agent\", not {other}")),
    };
    doc.ds()?;
    let session = doc.session();
    Ok(json!({ "undo": session.undo_labels_in(scope, limit), "redo": session.redo_labels_in(scope, limit) }))
}

/// The longest brush art, in CSS pixels.
const MAX_ART: f64 = 1000.0;

/// `strokePreviewOutline(params)`: a brush's outline along a short horizontal line, as the Stroke
/// section's brush rows draw it (upstream `panels/right/stroke.rs` `brush_art`: the brush outline of
/// a line `width` long at weight `height`, each subpath flattened at 0.25 px and stroked as a
/// closed 1 px line). Answers SVG path data in CSS pixels, the line along `y = height / 2` from
/// `x = 0`, one `M … L … Z` per subpath. `params`: `preset` (a brush id,
/// `kition_design_pathops::brush::PRESETS`: `taper`, `ink`, `ribbon`, `bristle`, `dots`, `stars`,
/// `confetti`, `leaves`), `width?` (default 120) and `height?` (default 16, the stroke weight).
pub fn stroke_preview_outline(params: &Value) -> Result<String, String> {
    use kition_design_geom::kurbo::{self, PathEl};
    if !params.is_object() {
        return Err("invalid-params: params must be an object".into());
    }
    let number = |key: &str, default: f64, min: f64| {
        params.get(key).and_then(Value::as_f64).filter(|v| v.is_finite()).map_or(default, |v| v.clamp(min, MAX_ART))
    };
    let (w, h) = (number("width", 120.0, 4.0), number("height", 16.0, 1.0));
    let preset = match params.get("preset") {
        Some(Value::String(p)) if kition_design_pathops::brush::preset(p).is_some() => p.clone(),
        Some(other) => return Err(format!("invalid-params: `preset` {other} is not a brush")),
        None => return Err("invalid-params: `preset` is required".into()),
    };
    let mut line = kition_design_geom::BezPath::new();
    line.move_to((0.0, 0.0));
    line.line_to((w, 0.0));
    let brush = kition_design_doc::Brush::new(&preset);
    let props =
        kition_design_doc::StrokeProps { weight: h, align: kition_design_doc::StrokeAlign::Center, brush: Some(brush.clone()), ..Default::default() };
    let out = kition_design_pathops::brush::outline(&line, h, &props, &brush).ok_or("the brush draws nothing")?;
    let mut d = String::new();
    for sub in kition_design_geom::split_subpaths(&out) {
        let mut first = true;
        kurbo::flatten(sub.iter(), 0.25, |el| {
            if let PathEl::MoveTo(p) | PathEl::LineTo(p) = el
                && p.x.is_finite()
                && p.y.is_finite()
            {
                d.push_str(&format!("{}{:.2} {:.2} ", if first { "M" } else { "L" }, p.x, h / 2.0 + p.y));
                first = false;
            }
        });
        if !first {
            d.push_str("Z ");
        }
    }
    Ok(d.trim_end().to_owned())
}

/// Answers the queries this module owns, then [`crate::lists::query`]'s.
pub fn query(doc: &DesignDocument, name: &str, params: &Value) -> Result<Value, String> {
    match name {
        "variable.modeRows" => mode_rows(doc, params),
        "documentColors" => document_colors(doc, params),
        "clipboard.paintState" => Ok(clipboard_state(doc)),
        "image.info" => image_info(doc, params),
        "history" => history(doc, params),
        _ => crate::lists::query(doc, name, params),
    }
}
