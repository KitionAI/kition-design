//! `query(name, params)`: what the editor's panels show, read from the engine the way the upstream
//! panels read it (`panels/**` in the upstream UI read `Session` state and the engine's read
//! commands). Queries never change the document: read commands run on a scratch copy of the
//! document's state ([`read`]). Layer ids are the engine's numeric ids; positions in the Design
//! panel's numbers are canvas units, as upstream shows them (`x`, `y` relative to the parent frame).
//!
//! | name | params | answer |
//! | --- | --- | --- |
//! | `pages` | — | `{ current, pages: [{ id, name, current, divider, layers }] }` |
//! | `layers` | `{ expanded?: [id], all?: bool }` | `{ page, rows: [Row] }`, `Row` = `{ id, name, type, icon, visible, locked, lockedAbove, dimmed, component, mask, selected, hasChildren, expanded, children?: [Row] }`; children are listed for `expanded` ids, the ancestors of the selection, and everything with `all` |
//! | `selection` | — | the Design panel's selection: `{ ids, count, kind, types, nodes, common, mixed, parentAutoLayout, canAlign, booleanOperation, text, component, page, parents, allParentsAutoLayout, insideInstance, hasStatus, canAddVariant, canCombineVariants }` (see [`selection`], [`crate::panels::selection_facts`]) |
//! | `node` | `{ id, depth? }` | the engine's `node_json` for one layer with the panels' keys ([`crate::panels::node_json`]) |
//! | `document` | `{ depth?, allPages? }` | the engine's document summary (`inspect::document`) |
//! | `selectionColors` | `{ ids? }` | `selection.colors` |
//! | `componentProperties` | `{ id? }` | `component.properties` |
//! | `instanceExposed` | `{ id? }` | `instance.exposed` |
//! | `prototype` | — | `{ interactions: [{ id, name, interactions }], flows, settings, frames: [{ id, name }] }` |
//! | `components` | — | `component.list` |
//! | `styles` | `{ type? }` | `style.list` |
//! | `variables` | — | `variable.list` |
//! | `devInspect` | `{ id?, colorFormat? }` | `inspect.properties` |
//! | `code` | `{ id?, language?, unit?, rootFontSize? }` | `inspect.code` |
//! | `componentCode` | `{ id?, … }` | `inspect.componentCode` |
//! | `devAssets` | `{ ids? }` | `inspect.assets` |
//! | `readyForDev` | `{ … }` | `inspect.readyForDev` |
//! | `annotations` | `{ ids?, page?, category? }` | `annotation.list` |
//! | `annotationCategories` | — | `annotation.categories` |
//! | `measurements` | `{ … }` | `devMode.measurements` |
//! | `devResources` | `{ id? }` | `devResource.list` |
//! | `comments` | `{ … }` | `comment.list` |
//! | `framePresets` | — | `frame.presets` |
//! | `fonts` | `{ … }` | `text.fonts` |
//! | `documentColors` | `{ page?, limit? }` | `paint.documentColors` with each colour's alpha ([`crate::panels::document_colors`]) |
//! | `imagePresets` | — | `image.presets` |
//! | `vectorEdit` | — | `{ state: vector.edit.state or null, tools: [{ id, label, shortcut }] }` |
//! | `textSelection` | — | `text.edit.selection` (an error while no text is edited) |
//! | `menus` | — | the upstream main menu as catalog rows `[{ path, label, command }]`, separators `---` |
//! | `checked` | — | the View and Preferences toggles that are on: `{ ids: [command id], pixelPreview: { on, scale: 1 or 2 } }` (`view.pixelPreview { scale }` sets the scale) |
//! | `exportSettings` | — | `{ jobs: [{ id, setting, file }] }` for the selection (the Export section) |
//! | `fontFamilies` | `{ query? }` | `{ families: [{ family, styles: [style] }] }`: every family the engine draws (embedded, added with `addFont`), in its order; `query` keeps the names containing it (any case) |
//! | `font` | `{ family, style? }` | `{ family, style, found, styles, features, axes, instances }` for the Type settings popover (see [`font`]) |
//! | `missingFonts` | — | `text.missingFonts` |
//! | `textStyles` | `{ query? }` | `{ styles: [{ id, name, folder, label, description, style }] }`: local text styles with their typography (`style`, `node.set`'s keys), for the style picker and `textPreview` |
//! | `recentColors` | — | `{ colors: ["#rrggbb" or "#rrggbbaa"], picked: null or colour }`: the colour picker's recent swatches, newest first, and what the eyedropper picked for it (`edit.pickColor { apply: false }`) |
//! | `prefs` | — | `{ nudgeSmall, nudgeBig, keepTool, renameDuplicates, highlightOnHover, numberKeysOpacity, showDimensions, reduceMotion, userName }` |
//! | `keymap` | `{ platform?: "mac" or "other" }` | `{ bindings: [{ group, id, title, keys, shown, context }], gestures: [{ group, label, keys }] }`: the default shortcuts (`keys` as kits write them, `shown` the engine's display text, `context` `canvas`, `text` or `both`) |
//! | `toolbar` | — | `{ tool, devMode, readOnly, isBoard, vectorEdit: null or { tool }, shapeBuilder: bool, interaction }` (see [`toolbar`]) |
//! | `layersAt` | `{ x, y }` (document pixels) | `select.layersAt`: `{ layers: [{ id, name, type, depth }] }`, as the engine lists them (the frame, then the layers in it) |
//! | `distances` | `{ ids?, to? }` | `measure.distances` in document pixels: `{ lines: [{ x0, y0, x1, y1, label, dashed }] }` |
//! | `paddingLines` | `{ id? }` | `measure.padding` in document pixels: `{ frame, lines }` |
//! | `redlines` | `{ page?, id? }` | `devMode.measurements` in document pixels: `[{ from, fromName, index, to, toName, side, labels, lines }]` |
//! | `commentPeople` | — | `comment.people`: `{ people, you }` |
//! | `commentPins` | `{ zoom?, filter?, open? }` | pins and clusters in document pixels ([`crate::canvas::comment_pins`]) |
//! | `protoConnections` | `{ zoom?, x?, y? }` | prototype connections, "+" handles and hit tests ([`crate::canvas::proto_connections`]) |
//! | `paintHandles` | `{ id, index, fills? }` | gradient or crop handles ([`crate::canvas::paint_handles`]) |
//! | `blurHandles` | `{ id, index }` | a progressive blur's points ([`crate::canvas::blur_handles`]) |
//! | `playground` | `{ … }` | `component.playground` |
//! | `evaluate` | `{ expression, id? }` | `proto.evaluate` |
//! | `vectorBounds` | — | `vector.edit.bounds` (page space, as the engine gives it) |
//! | `shapeBuilderRegions` | `{ ids? }` | `shapeBuilder.regions` (page space) |
//!
//! Where the engine's read command answers in page space, the queries named for it here
//! (`distances`, `paddingLines`, `redlines`, `layersAt`) answer in document pixels; the older
//! `measurements` query passes `devMode.measurements` through in page space.

use std::collections::BTreeSet;

use kition_design_doc::{Document, NodeId, NodeKind};
use kition_design_engine::Session;
use serde_json::{Map, Value, json};

use crate::{DesignDocument, engine_error, no_files};

/// The page's background colour (the default page colour for a page without one).
pub fn page_background(doc: &Document, page: NodeId) -> kition_design_color::Color {
    match doc.node(page).map(|p| &p.kind) {
        Some(NodeKind::Page { background, .. }) => *background,
        _ => kition_design_color::Color::DEFAULT_PAGE,
    }
}

/// Runs a read command on a scratch copy of the document's state, so it neither changes the
/// document nor moves its revision.
pub fn read(doc: &DesignDocument, command: &str, params: &Value) -> Result<Value, String> {
    let ds = doc.ds()?.clone();
    let mut scratch = Session::empty();
    scratch.prefs = doc.session().prefs.clone();
    scratch.tool.dev_mode = doc.session().tool.dev_mode;
    // The same library files (and what was parsed of them, shared).
    scratch.libraries = doc.session().libraries.clone();
    scratch.docs.push(ds);
    no_files(|| scratch.execute(command, params)).map_err(engine_error)
}

fn id_param(params: &Value, key: &str) -> Option<NodeId> {
    kition_design_engine::params::id(params, key)
}

/// The upstream layers panel's icon for a layer (`layers.rs` `node_icon`).
pub fn icon(doc: &Document, id: NodeId) -> &'static str {
    use kition_design_doc::{BoolOp, LayoutMode};
    let Some(n) = doc.node(id) else { return "frame" };
    let op = |op: BoolOp| match op {
        BoolOp::Union => "union",
        BoolOp::Subtract => "subtract",
        BoolOp::Intersect => "intersect",
        BoolOp::Exclude => "exclude",
    };
    if let Some(o) = kition_design_engine::booleans::distinct_op(doc, id) {
        return op(o);
    }
    match &n.kind {
        NodeKind::Page { .. } => "page",
        NodeKind::Frame if kition_design_engine::components::is_slot(doc, id) => "slot",
        NodeKind::Frame => match n.layout.as_ref().map(|l| l.mode) {
            Some(LayoutMode::Vertical) => "layoutVertical",
            Some(LayoutMode::Grid) => "layoutGrid",
            Some(_) => "layoutHorizontal",
            None => "frame",
        },
        NodeKind::Group => "group",
        NodeKind::Section { .. } => "section",
        NodeKind::Rectangle if n.fills.iter().any(|p| matches!(p.kind, kition_design_color::PaintKind::Image(_))) => "image",
        NodeKind::Rectangle => "rectangle",
        NodeKind::Ellipse { .. } => "ellipse",
        NodeKind::Polygon { .. } => "polygon",
        NodeKind::Star { .. } => "star",
        NodeKind::Line if n.stroke.cap_end.is_arrow() => "arrow",
        NodeKind::Line => "line",
        NodeKind::Vector { .. } => "vector",
        NodeKind::Text(_) => "text",
        NodeKind::BooleanOp { op: o } => op(*o),
        NodeKind::Component { .. } => "component",
        NodeKind::ComponentSet { .. } => "componentSet",
        NodeKind::Instance { .. } => "instance",
        NodeKind::Slice => "slice",
    }
}

fn pages(doc: &DesignDocument) -> Result<Value, String> {
    let ds = doc.ds()?;
    let d = &ds.doc;
    let pages: Vec<Value> = d
        .pages
        .iter()
        .filter_map(|id| {
            d.node(*id)
                .map(|p| json!({ "id": id, "name": p.name, "current": *id == ds.page, "divider": d.is_divider(*id), "layers": p.children.len() }))
        })
        .collect();
    Ok(json!({ "current": ds.page, "pages": pages }))
}

fn layers(doc: &DesignDocument, params: &Value) -> Result<Value, String> {
    let ds = doc.ds()?;
    let d: &Document = &ds.doc;
    let all = params.get("all").and_then(Value::as_bool).unwrap_or(false);
    let mut expanded: BTreeSet<NodeId> =
        params.get("expanded").and_then(Value::as_array).into_iter().flatten().filter_map(kition_design_engine::params::id_of).collect();
    for id in &ds.selection {
        expanded.extend(d.ancestors(*id));
    }
    let selected: BTreeSet<NodeId> = ds.selection.iter().copied().collect();
    struct Walk<'a> {
        d: &'a Document,
        all: bool,
        expanded: &'a BTreeSet<NodeId>,
        selected: &'a BTreeSet<NodeId>,
        budget: usize,
    }
    fn row(w: &mut Walk<'_>, id: NodeId, dimmed: bool, locked_above: bool, depth: usize) -> Option<Value> {
        let n = w.d.node(id)?;
        w.budget = w.budget.checked_sub(1)?;
        let component = matches!(n.kind, NodeKind::Component { .. } | NodeKind::ComponentSet { .. } | NodeKind::Instance { .. });
        let open = (w.all || w.expanded.contains(&id)) && !n.children.is_empty() && depth < 64;
        let dim_children = dimmed || !n.visible || n.hides_contents();
        let mut v = json!({
            "id": id,
            "name": n.name,
            "type": n.kind.type_name(),
            "icon": icon(w.d, id),
            "visible": n.visible,
            "locked": n.locked,
            "lockedAbove": locked_above,
            "dimmed": dimmed || !n.visible,
            "component": component,
            "mask": n.is_mask,
            "selected": w.selected.contains(&id),
            "hasChildren": !n.children.is_empty(),
            "expanded": open,
        });
        if open && let Some(o) = v.as_object_mut() {
            // Listed top-most first, as the panel shows them.
            let kids: Vec<Value> = n.children.iter().rev().filter_map(|c| row(w, *c, dim_children, locked_above || n.locked, depth + 1)).collect();
            o.insert("children".into(), Value::Array(kids));
        }
        Some(v)
    }
    let mut walk = Walk { d, all, expanded: &expanded, selected: &selected, budget: 20_000 };
    let rows: Vec<Value> = d
        .node(ds.page)
        .map(|p| p.children.iter().rev().copied().collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|c| row(&mut walk, c, false, false, 0))
        .collect();
    Ok(json!({ "page": ds.page, "rows": rows, "truncated": walk.budget == 0 }))
}

/// The Design panel's view of the selection: every selected layer's properties as the engine's
/// `node_json` gives them (the keys `node.set` takes), the values they share (`common`) and the
/// keys whose values differ (`mixed`, upstream's "Mixed"), with what the panel's sections decide
/// on: `kind` (the type label, or "Mixed"), whether a parent has auto layout (`parentAutoLayout`:
/// constraints give way to auto layout sizing), `canAlign`, the boolean operation shown,
/// `text` (the character styles of a single text layer or of the edited text's selected range:
/// `{ id, runs: [{ start, end, style }] }`), `component` (for a layer in a component or an
/// instance: `{ owningComponent, owningInstance, variantSet, variantOptions: [[property,
/// [values]]], variantProps }`) and, with nothing selected, `page` (`{ id, background,
/// colorProfile, localStyles, localVariables }`).
fn selection(doc: &DesignDocument) -> Result<Value, String> {
    let ds = doc.ds()?;
    let d: &Document = ds.text_preview.as_deref().unwrap_or(&ds.doc);
    let ids = &ds.selection;
    if ids.is_empty() {
        let page = json!({
            "id": ds.page,
            "background": page_background(d, ds.page).to_css_hex(),
            "colorProfile": d.color_profile.id(),
            "localStyles": kition_design_engine::library::local_styles(d).count(),
            "localVariables": d.variables.variables.iter().filter(|v| !kition_design_engine::library::is_remote_variable(d, &v.id)).count(),
        });
        return Ok(json!({ "ids": [], "count": 0, "kind": null, "nodes": [], "common": {}, "mixed": [], "page": page }));
    }
    const MAX_NODES: usize = 200;
    let nodes: Vec<Value> = ids.iter().take(MAX_NODES).map(|id| crate::panels::node_json(d, *id, 0)).collect();
    let mut common = Map::new();
    let mut mixed: Vec<String> = Vec::new();
    let keys: BTreeSet<String> = nodes.iter().filter_map(Value::as_object).flat_map(|o| o.keys().cloned()).collect();
    for key in keys.into_iter().filter(|k| !matches!(k.as_str(), "id" | "name" | "absoluteBounds" | "childCount" | "children" | "parent")) {
        let mut values = nodes.iter().map(|n| n.get(&key));
        let first = values.next().flatten();
        if values.all(|v| v == first) {
            if let Some(v) = first {
                common.insert(key, v.clone());
            }
        } else {
            mixed.push(key);
        }
    }
    let labels: BTreeSet<&str> = ids.iter().filter_map(|i| d.node(*i)).map(|n| n.kind.label()).collect();
    let kind = match labels.len() {
        1 => labels.into_iter().next().map_or(Value::Null, |l| json!(l)),
        _ => json!("Mixed"),
    };
    let types: BTreeSet<&str> = ids.iter().filter_map(|i| d.node(*i)).map(|n| n.kind.type_name()).collect();
    let parent_al = ids.iter().filter_map(|i| d.node(*i)).any(|n| n.parent.and_then(|p| d.node(p)).is_some_and(|p| p.layout.is_some()));
    let text = text_runs(doc.session(), d, ids);
    let component = ids.first().copied().filter(|_| ids.len() == 1).map(|id| {
        use kition_design_engine::components as c;
        let owner = c::owning_component(d, id);
        let set = owner
            .and_then(|o| c::variant_set(d, o))
            .or_else(|| matches!(d.node(id).map(|n| &n.kind), Some(NodeKind::ComponentSet { .. })).then_some(id));
        json!({
            "owningComponent": owner,
            "owningInstance": c::owning_instance(d, id),
            "variantSet": set,
            "variantOptions": set.map(|s| c::variant_options(d, s)).unwrap_or_default(),
            "variantProps": owner.and_then(|o| d.node(o)).map(|n| c::variant_props(&n.name)).unwrap_or_default(),
        })
    });
    let mut answer = json!({
        "ids": ids,
        "count": ids.len(),
        "kind": kind,
        "types": types,
        "nodes": nodes,
        "common": common,
        "mixed": mixed,
        "parentAutoLayout": parent_al,
        "canAlign": kition_design_engine::cmd::arrange::can_align(d, ids),
        "booleanOperation": kition_design_engine::booleans::shown_op(d, ids),
        "text": text,
        "component": component,
        "page": null,
    });
    if let Some(o) = answer.as_object_mut() {
        crate::panels::selection_facts(d, ids, o);
    }
    Ok(answer)
}

/// The character styles of the edited text's selected range, or of a single selected text layer.
fn text_runs(session: &Session, d: &Document, ids: &[NodeId]) -> Value {
    let (id, range) = match kition_design_engine::textedit::selected_range(session) {
        Some((id, a, b)) => (id, Some((a, b))),
        None => match ids {
            [id] => (*id, None),
            _ => return Value::Null,
        },
    };
    let Some(t) = d.node(id).and_then(|n| n.kind.text()) else { return Value::Null };
    let runs: Vec<Value> = t
        .spans()
        .into_iter()
        .filter(|(a, b, _)| range.is_none_or(|(s, e)| if s == e { *a <= s && s <= *b } else { *a < e && *b > s }))
        .map(|(a, b, style)| json!({ "start": a, "end": b, "style": kition_design_engine::inspect::camel(serde_json::to_value(style).unwrap_or(Value::Null)) }))
        .collect();
    json!({ "id": id, "range": range.map(|(a, b)| json!([a, b])), "runs": runs })
}

fn prototype(doc: &DesignDocument) -> Result<Value, String> {
    let ds = doc.ds()?;
    let d: &Document = &ds.doc;
    let interactions: Vec<Value> =
        ds.selection.iter().filter_map(|id| d.node(*id).map(|n| json!({ "id": id, "name": n.name, "interactions": n.interactions }))).collect();
    let (flows, settings) = match d.node(ds.page).map(|p| &p.kind) {
        Some(NodeKind::Page { flows, prototype, .. }) => (json!(flows), json!(prototype)),
        _ => (json!([]), Value::Null),
    };
    let frames: Vec<Value> = d.top_frames(ds.page).into_iter().filter_map(|id| d.node(id).map(|n| json!({ "id": id, "name": n.name }))).collect();
    Ok(json!({ "interactions": interactions, "flows": flows, "settings": settings, "frames": frames }))
}

fn vector_edit(doc: &DesignDocument) -> Result<Value, String> {
    use kition_design_engine::vectoredit::VTool;
    let editing = doc.ds()?.vector_edit.is_some();
    let state = if editing { read(doc, "vector.edit.state", &json!({}))? } else { Value::Null };
    let tools: Vec<Value> = VTool::ALL.iter().map(|t| json!({ "id": t.id(), "label": t.label(), "shortcut": t.shortcut() })).collect();
    Ok(json!({ "state": state, "tools": tools }))
}

fn checked(doc: &DesignDocument) -> Value {
    let ids: Vec<&str> = kition_design_engine::catalog::items()
        .map(|m| m.id)
        .chain(["view.devMode", "pref.keepTool", "pref.highlightHover", "pref.renameDuplicates", "pref.showDimensions", "pref.numberKeysOpacity"])
        .filter(|id| kition_design_engine::cmd::view::checked(doc.session(), id).unwrap_or(false))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let pixel = doc.ds().ok().map(|ds| (ds.view.pixel_preview, ds.view.pixel_scale));
    json!({
        "ids": ids,
        "pixelPreview": pixel.map(|(on, scale)| json!({ "on": on, "scale": scale })),
    })
}

fn export_settings(doc: &DesignDocument) -> Result<Value, String> {
    let ds = doc.ds()?;
    let jobs = kition_design_engine::cmd::file::export_jobs(&ds.doc, &ds.selection);
    let jobs: Vec<Value> = jobs.iter().map(|j| json!({ "id": j.id, "setting": j.setting, "file": j.file })).collect();
    Ok(json!({ "jobs": jobs }))
}

/// `font { family, style? }`: one family as the Type settings popover and the style dropdown read
/// it: `found` (the engine has the family itself, not a fallback), its `styles`, and for `style`
/// (default Regular, else the first style) the face's OpenType `features` (tags) and variation
/// `axes` (`[{ tag, name, min, default, max, hidden }]`) with its named `instances`
/// (`[{ name, variations: { tag: value } }]`). `text.fonts` answers one of these per call, and
/// `features` wins when both flags are given (the `[]` the UI port spec saw was a face without
/// features).
fn font(params: &Value) -> Result<Value, String> {
    let family = params.get("family").and_then(Value::as_str).filter(|f| !f.trim().is_empty()).ok_or("invalid-params: `family` is required")?;
    let styles = kition_design_text::family_styles(family);
    let style = match params.get("style").and_then(Value::as_str) {
        Some(s) => s.to_owned(),
        None => styles.iter().find(|s| s.eq_ignore_ascii_case("Regular")).or(styles.first()).cloned().unwrap_or_else(|| "Regular".into()),
    };
    let face = kition_design_text::find_face(family, &style);
    let found = kition_design_text::has_family(family) && face.as_ref().is_some_and(|f| f.family.eq_ignore_ascii_case(family));
    let (features, axes, instances) = match face.filter(|_| found) {
        Some(face) => (
            json!(face.feature_tags()),
            json!(
                face.axes
                    .iter()
                    .map(|a| json!({ "tag": a.tag, "name": a.name, "min": a.min, "default": a.default, "max": a.max, "hidden": a.hidden }))
                    .collect::<Vec<_>>()
            ),
            json!(
                face.instances
                    .iter()
                    .map(|i| json!({
                        "name": i.name,
                        "variations": face.axes.iter().zip(&i.coords).map(|(a, c)| (a.tag.clone(), json!(c))).collect::<Map<String, Value>>(),
                    }))
                    .collect::<Vec<_>>()
            ),
        ),
        None => (json!([]), json!([]), json!([])),
    };
    Ok(json!({ "family": family, "style": style, "found": found, "styles": styles, "features": features, "axes": axes, "instances": instances }))
}

fn font_families(params: &Value) -> Value {
    let q = params.get("query").and_then(Value::as_str).unwrap_or("").trim().to_lowercase();
    let families: Vec<Value> = kition_design_text::family_names()
        .into_iter()
        .filter(|f| q.is_empty() || f.to_lowercase().contains(&q))
        .map(|f| {
            let styles = kition_design_text::family_styles(&f);
            json!({ "family": f, "styles": styles })
        })
        .collect();
    json!({ "families": families })
}

fn text_styles(doc: &DesignDocument, params: &Value) -> Result<Value, String> {
    let q = params.get("query").and_then(Value::as_str).unwrap_or("").trim().to_lowercase();
    let d = &doc.ds()?.doc;
    let styles: Vec<Value> = kition_design_engine::library::local_styles(d)
        .filter_map(|s| match &s.kind {
            kition_design_doc::StyleKind::Text { style } => Some((s, style)),
            _ => None,
        })
        .filter(|(s, _)| q.is_empty() || s.name.to_lowercase().contains(&q))
        .map(|(s, style)| {
            let (folder, label) = match s.name.rsplit_once('/') {
                Some((folder, label)) => (folder.trim().to_owned(), label.trim().to_owned()),
                None => (String::new(), s.name.trim().to_owned()),
            };
            json!({
                "id": s.id,
                "name": s.name,
                "folder": folder,
                "label": label,
                "description": s.description,
                "style": kition_design_engine::inspect::camel(serde_json::to_value(style).unwrap_or(Value::Null)),
            })
        })
        .collect();
    Ok(json!({ "styles": styles }))
}

fn recent_colors(doc: &DesignDocument) -> Value {
    let s = doc.session();
    json!({
        "colors": s.recent_colors.iter().map(|c| c.to_css_hex()).collect::<Vec<_>>(),
        "picked": s.tool.picked.map(kition_design_color::Color::to_css_hex),
    })
}

fn prefs(doc: &DesignDocument) -> Value {
    let p = &doc.session().prefs;
    json!({
        "nudgeSmall": p.nudge_small,
        "nudgeBig": p.nudge_big,
        "keepTool": p.keep_tool,
        "renameDuplicates": p.rename_duplicates,
        "highlightOnHover": p.highlight_on_hover,
        "numberKeysOpacity": p.number_keys_opacity,
        "showDimensions": p.show_dimensions,
        "reduceMotion": p.reduce_motion,
        "userName": kition_design_engine::cmd::comment::user_name(doc.session()),
    })
}

fn keymap(params: &Value) -> Result<Value, String> {
    use kition_design_engine::keymap::{Context, GESTURES, KEYS, Platform};
    let platform = match params.get("platform").and_then(Value::as_str) {
        None | Some("mac") => Platform::Mac,
        Some("other") => Platform::Other,
        Some(other) => return Err(format!("invalid-params: `platform` is mac or other, not {other:?}")),
    };
    let bindings: Vec<Value> = KEYS
        .iter()
        .filter(|b| crate::commands::offered(b.id))
        .map(|b| {
            let keys = b.keys(platform);
            json!({
                "group": b.group.label(),
                "id": b.id,
                "title": b.title(),
                "keys": keys.iter().filter_map(|k| crate::commands::kit_shortcut(k)).collect::<Vec<_>>(),
                "shown": keys.iter().map(|k| kition_design_engine::keymap::display(k, platform)).collect::<Vec<_>>(),
                "context": match b.context { Context::Canvas => "canvas", Context::Text => "text", Context::Both => "both" },
            })
        })
        .collect();
    let gestures: Vec<Value> = GESTURES
        .iter()
        .map(|(group, label, mac, other)| json!({ "group": group.label(), "label": label, "keys": if platform == Platform::Mac { mac } else { other } }))
        .collect();
    Ok(json!({ "bindings": bindings, "gestures": gestures }))
}

/// `toolbar`: what the toolbar pill and the mode switcher show: the `tool`, Dev Mode, `readOnly`
/// (why the file cannot be edited now, or null), whether the file is a board, vector edit mode's
/// tool, whether the shape builder is on, and the editor's open interaction's label.
fn toolbar(doc: &DesignDocument) -> Result<Value, String> {
    let s = doc.session();
    let ds = doc.ds()?;
    let read_only =
        s.active_read_only().map(str::to_owned).or_else(|| ds.preview.as_ref().map(|_| kition_design_engine::PREVIEW_READ_ONLY.to_owned()));
    Ok(json!({
        "tool": s.tool.tool.id(),
        "devMode": s.tool.dev_mode,
        "readOnly": read_only,
        "isBoard": kition_design_engine::board::is_board(&ds.doc),
        "vectorEdit": ds.vector_edit.as_ref().map(|ve| json!({ "tool": ve.tool.id() })),
        "shapeBuilder": s.tool.tool == kition_design_engine::ToolId::ShapeBuilder,
        "interaction": doc.interaction_label(),
    }))
}

/// Moves every `{ x0, y0, x1, y1 }` line in `value` from page space to document pixels.
fn lines_to_doc(value: &mut Value, area: &crate::Area, depth: usize) {
    if depth > 16 {
        return;
    }
    match value {
        Value::Object(o) => {
            let line = ["x0", "y0", "x1", "y1"].iter().all(|k| o.get(*k).is_some_and(Value::is_number));
            if line {
                for (k, shift) in [("x0", area.x), ("x1", area.x), ("y0", area.y), ("y1", area.y)] {
                    if let Some(v) = o.get(k).and_then(Value::as_f64) {
                        o.insert(k.into(), json!(v - shift));
                    }
                }
            }
            for v in o.values_mut() {
                lines_to_doc(v, area, depth + 1);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|v| lines_to_doc(v, area, depth + 1)),
        _ => {}
    }
}

/// Answers query `name` (the table in this module's documentation).
pub fn query(doc: &DesignDocument, name: &str, params: &Value) -> Result<Value, String> {
    let params = if params.is_null() { json!({}) } else { params.clone() };
    if !params.is_object() {
        return Err("invalid-params: params must be an object".into());
    }
    let via = |command: &str| read(doc, command, &params);
    match name {
        "pages" => pages(doc),
        "layers" => layers(doc, &params),
        "selection" => selection(doc),
        "node" => {
            let id = id_param(&params, "id").ok_or("invalid-params: `id` must be a layer id")?;
            let depth = params.get("depth").and_then(Value::as_u64).unwrap_or(0).min(64) as usize;
            let ds = doc.ds()?;
            if ds.doc.node(id).is_none() {
                return Err(format!("not-found: no layer {id}"));
            }
            Ok(crate::panels::node_json(&ds.doc, id, depth))
        }
        "document" => {
            let depth = params.get("depth").and_then(Value::as_u64).unwrap_or(1).min(64) as usize;
            let all = params.get("allPages").and_then(Value::as_bool).unwrap_or(false);
            Ok(kition_design_engine::inspect::document(doc.session(), depth, all))
        }
        "selectionColors" => via("selection.colors"),
        "componentProperties" => via("component.properties"),
        "instanceExposed" => via("instance.exposed"),
        "prototype" => prototype(doc),
        "components" => via("component.list"),
        "styles" => via("style.list"),
        "variables" => via("variable.list"),
        "devInspect" => via("inspect.properties"),
        "code" => via("inspect.code"),
        "componentCode" => via("inspect.componentCode"),
        "devAssets" => via("inspect.assets"),
        "readyForDev" => via("inspect.readyForDev"),
        "annotations" => via("annotation.list"),
        "annotationCategories" => via("annotation.categories"),
        "measurements" => via("devMode.measurements"),
        "devResources" => via("devResource.list"),
        "comments" => via("comment.list"),
        "framePresets" => via("frame.presets"),
        "fonts" => via("text.fonts"),
        "documentColors" => crate::panels::document_colors(doc, &params),
        "imagePresets" => via("image.presets"),
        "vectorEdit" => vector_edit(doc),
        "textSelection" => via("text.edit.selection"),
        "menus" => Ok(Value::Array(crate::commands::menu_rows())),
        "checked" => Ok(checked(doc)),
        "exportSettings" => export_settings(doc),
        "fontFamilies" => Ok(font_families(&params)),
        "font" => font(&params),
        "missingFonts" => via("text.missingFonts"),
        "textStyles" => text_styles(doc, &params),
        "recentColors" => Ok(recent_colors(doc)),
        "prefs" => Ok(prefs(doc)),
        "keymap" => keymap(&params),
        "toolbar" => toolbar(doc),
        "layersAt" => {
            let (Some(x), Some(y)) = (params.get("x").and_then(Value::as_f64), params.get("y").and_then(Value::as_f64)) else {
                return Err("invalid-params: `x` and `y` (document pixels) are required".into());
            };
            let at = doc.area().to_canvas(x, y);
            read(doc, "select.layersAt", &json!({ "x": at.x, "y": at.y }))
        }
        "distances" | "paddingLines" | "redlines" => {
            let command = match name {
                "distances" => "measure.distances",
                "paddingLines" => "measure.padding",
                _ => "devMode.measurements",
            };
            let mut value = via(command)?;
            let page = params.get("page").and_then(kition_design_engine::params::id_of);
            lines_to_doc(&mut value, &doc.area_on(page), 0);
            Ok(value)
        }
        "commentPeople" => via("comment.people"),
        "commentPins" => crate::canvas::comment_pins(doc, &params),
        "protoConnections" => crate::canvas::proto_connections(doc, &params),
        "paintHandles" => crate::canvas::paint_handles(doc, &params),
        "blurHandles" => crate::canvas::blur_handles(doc, &params),
        "playground" => via("component.playground"),
        "evaluate" => via("proto.evaluate"),
        "vectorBounds" => via("vector.edit.bounds"),
        "shapeBuilderRegions" => via("shapeBuilder.regions"),
        _ => crate::panels::query(doc, name, &params),
    }
}

/// The largest export scale.
const MAX_EXPORT_SCALE: f64 = 32.0;

/// One layer exported with an export setting (`exportNodeWith`), as the Export section's preview
/// and its Export button make it: the file bytes. `params` (JSON):
///
/// - `index?`: start from the layer's export setting at `index` (default the engine's default
///   setting: PNG at 1x);
/// - `setting?`: export setting keys over it, as the layer's `exports` list holds them: `format`
///   (`png`, `jpg`, `webp`, `svg`, `pdf`, `xml`), `size` (`{ by: "scale" | "width" | "height",
///   value }`), `contentsOnly` (only the layer, not what overlaps it; default true),
///   `includeBoundingBox`, `svgOutlineText` (default true), `svgIdAttribute`, `svgSimplifyStroke`
///   (default true), `print` (`{ bleed, cropMarks, bleedMarks, cmyk }`, PDF), `vdCompat`;
/// - `format?`: overrides the setting's format; `scale?`: overrides the scale its `size` gives
///   (at most 32).
///
/// Never changes the document.
pub fn export_node_with(doc: &DesignDocument, id: &str, params: &Value) -> Result<Vec<u8>, String> {
    use kition_design_doc::ExportSetting;
    let id: NodeId = id.parse().map_err(|_| format!("invalid-params: {id:?} is not a layer id"))?;
    if !params.is_object() && !params.is_null() {
        return Err("invalid-params: params must be an object".into());
    }
    let ds = doc.ds()?;
    let n = ds.doc.node(id).ok_or_else(|| format!("not-found: no layer {id}"))?;
    let mut setting = match params.get("index") {
        None | Some(Value::Null) => ExportSetting::default(),
        Some(v) => {
            let i = v.as_u64().ok_or("invalid-params: `index` must be a whole number")?;
            n.exports
                .get(usize::try_from(i).unwrap_or(usize::MAX))
                .cloned()
                .ok_or_else(|| format!("invalid-params: layer {id} has no export setting {i}"))?
        }
    };
    match params.get("setting") {
        None | Some(Value::Null) => {}
        Some(Value::Object(over)) => {
            let mut merged = serde_json::to_value(&setting).map_err(|e| e.to_string())?;
            if let Some(o) = merged.as_object_mut() {
                for (k, v) in over {
                    o.insert(k.clone(), v.clone());
                }
            }
            setting = serde_json::from_value(merged).map_err(|e| format!("invalid-params: `setting`: {e}"))?;
        }
        Some(other) => return Err(format!("invalid-params: `setting` must be an object, not {other}")),
    }
    if let Some(f) = params.get("format").filter(|v| !v.is_null()) {
        let text = f.as_str().ok_or("invalid-params: `format` must be a string")?;
        setting.format = kition_design_doc::ExportFormat::from_ext(text).ok_or_else(|| format!("invalid-params: cannot export as {text:?}"))?;
    }
    let scale = match params.get("scale").filter(|v| !v.is_null()) {
        Some(v) => v.as_f64().filter(|s| s.is_finite() && *s > 0.0).ok_or("invalid-params: `scale` must be a positive number")?,
        None => kition_design_engine::cmd::file::export_scale(&ds.doc, &[id], &setting),
    };
    let scale = if scale.is_finite() && scale > 0.0 { scale.min(MAX_EXPORT_SCALE) } else { 1.0 };
    no_files(|| kition_design_engine::cmd::file::export_bytes_with(&ds.doc, &[id], &setting, scale)).map_err(engine_error)
}

/// One layer exported as `format` at `scale` (`exportNode`): the file bytes.
pub fn export_node(doc: &DesignDocument, id: &str, format: &str, scale: f64) -> Result<Vec<u8>, String> {
    let id: NodeId = id.parse().map_err(|_| format!("invalid-params: {id:?} is not a layer id"))?;
    let format = kition_design_doc::ExportFormat::from_ext(format).ok_or_else(|| format!("invalid-params: cannot export as {format:?}"))?;
    let scale = if scale.is_finite() && scale > 0.0 { scale.min(32.0) } else { 1.0 };
    let ds = doc.ds()?;
    if ds.doc.node(id).is_none() {
        return Err(format!("not-found: no layer {id}"));
    }
    no_files(|| kition_design_engine::cmd::file::export_bytes(&ds.doc, &[id], format, scale)).map_err(engine_error)
}
