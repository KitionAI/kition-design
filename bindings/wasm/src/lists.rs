//! Panel lists and canvas reads beyond [`crate::query`]'s table, read the way the upstream egui
//! panels read `app.session` (`panels/layers.rs`, `panels/assets_panel.rs`, `variable_ui.rs`,
//! `menus.rs`, `canvas/input.rs`, `panels/dev_mode.rs`, `panels/dev_compare.rs`,
//! `panels/proto_panel.rs`, `panels/proto_editor.rs`). None of them changes the document or its
//! revision. Positions are document pixels (`inspect().area` places them) unless marked "page
//! space".
//!
//! | name | params | answer |
//! | --- | --- | --- |
//! | `layers.dropInfo` | `{ ids?: [id], moving?: [id] }` | `{ page, rows: [DropRow], truncated }` ([`drop_info`]) |
//! | `assets.list` | `{ query? }` | `{ folders: [{ name, tiles: [Tile] }], styles: [{ id, name, type, folder, label }], libraries: [Library] }` ([`assets_list`]) |
//! | `drop.preview` | `{ x, y, id?, alt? }` | `{ parent, dropTarget, index, line, gridCell, swap }` ([`drop_preview`]) |
//! | `library.list` | `{ assets? }` | `library.list` (`{ libraries, published }`; a library whose file can't be read answers an `error`) |
//! | `library.updates` | — | `library.updates` |
//! | `library.files` | — | `{ files: [{ id, name, path }] }`: the library files to read and lend ([`crate::libraries::files`]) |
//! | `styles.libraries` | `{ type? }` | `{ copies, libraries: [{ id, name, error, styles }] }` ([`library_rows::styles`]) |
//! | `variables.libraries` | — | `{ copies, libraries: [{ id, name, error, collections, variables }] }` ([`library_rows::variables`]) |
//! | `variables.pickFor` | `{ ids?, field?, kind?, query?, target?, variable?, style? }` | `{ scope, groups: [Group] }` ([`pick_for`]) |
//! | `menu.context` | `{ ids?, at?: { x, y } }` | `{ ids, items: [Item], selectLayer }` ([`menu::context`]) |
//! | `hitTest` | `{ x, y, deep?, zoom? }` | `{ leaf, target, frame, selected, handle }` ([`hit_test`]) |
//! | `rename.preview` | `{ ids?, find?, regex?, replace?, name?, start? }` | `{ rows: [{ id, before, after, changed }], error }` ([`rename_preview`]) |
//! | `prefs.recentActions` | — | `{ actions: [key], max }` |
//! | `layout.gridGeometry` | `{ id? }` | `{ id, columns, rows, cells, width, height, unit }` or `null` ([`grid_geometry`]) |
//! | `arrange.spacingInfo` | `{ ids? }` | `{ horizontal: Axis or null, vertical: Axis or null }` ([`spacing_info`]) |
//! | `dev.focusState` | — | `{ id, name, status, changed, index, count, canStep }` or `null` ([`dev::focus_state`]) |
//! | `dev.compare` | `{ id?, version? }` | the comparison with change boxes ([`dev::compare`]) |
//! | `codeLinks` | `{ id? }` | `{ id, owner, links: [CodeLink] }` ([`code_links`]) |
//! | `annotation.callouts` | `{ page? }` | `{ callouts: [{ …annotation, bounds }] }` ([`callouts`]) |
//! | `version.previewState` | — | `{ id, name, time }` or `null` |
//! | `proto.destinations` | `{ id?, kind? }` | `{ kind, destinations: [{ id, name, type }] }` ([`destinations`]) |
//! | `proto.parseExpression` | `{ text, expect?, variable?, kind? }` | `{ ok, error, at, display, type, expr }` ([`parse_expression`]) |
//!
//! Extras: `devCompareRender(params)` and `playgroundPreview(params)` answer PNG bytes
//! ([`dev::compare_render`], [`dev::playground_preview`]); `recordAction(key)` remembers an action
//! run from the Actions palette for its Recents ([`DesignDocument::record_action`]).

mod dev;
mod library_rows;
mod menu;

use std::collections::{BTreeMap, HashMap, HashSet};

use kition_design_doc::{Document, NodeId, NodeKind, VariableType};
use kition_design_geom::Point;
use serde_json::{Value, json};

use crate::DesignDocument;

/// The most rows `layers.dropInfo` answers.
pub const MAX_DROP_ROWS: usize = 20_000;
/// The most tiles, styles or variables a list answers.
pub const MAX_LISTED: usize = 5_000;

fn ids_param(params: &Value, key: &str) -> Option<Vec<NodeId>> {
    params.get(key).and_then(Value::as_array).map(|a| a.iter().filter_map(kition_design_engine::params::id_of).collect())
}

/// `ids` when given, else the selection.
fn targets(doc: &DesignDocument, params: &Value) -> Result<Vec<NodeId>, String> {
    Ok(match ids_param(params, "ids") {
        Some(ids) => ids,
        None => doc.ds()?.selection.clone(),
    })
}

/// `id` when given, else the first selected layer.
fn target(doc: &DesignDocument, params: &Value) -> Result<NodeId, String> {
    match params.get("id") {
        None | Some(Value::Null) => doc.ds()?.selection.first().copied().ok_or_else(|| "invalid-params: pass `id` or select a layer".to_owned()),
        Some(v) => kition_design_engine::params::id_of(v).ok_or_else(|| format!("invalid-params: `id` {v} is not a layer id")),
    }
}

fn point_param(params: &Value) -> Result<(f64, f64), String> {
    match (params.get("x").and_then(Value::as_f64), params.get("y").and_then(Value::as_f64)) {
        (Some(x), Some(y)) if x.is_finite() && y.is_finite() => Ok((x, y)),
        _ => Err("invalid-params: `x` and `y` (document pixels) are required".into()),
    }
}

fn str_param<'a>(params: &'a Value, key: &str) -> Option<&'a str> {
    params.get(key).and_then(Value::as_str)
}

/// A page-space line `[x0, y0, x1, y1]` in document pixels.
fn line_json(area: &crate::Area, l: [f64; 4]) -> Value {
    json!([l[0] - area.x, l[1] - area.y, l[2] - area.x, l[3] - area.y])
}

/// Answers the queries this module owns; `unknown query` for the rest.
pub fn query(doc: &DesignDocument, name: &str, params: &Value) -> Result<Value, String> {
    match name {
        "layers.dropInfo" => drop_info(doc, params),
        "assets.list" => assets_list(doc, params),
        "drop.preview" => drop_preview(doc, params),
        "library.list" => crate::query::read(doc, "library.list", params),
        "library.updates" => crate::query::read(doc, "library.updates", &json!({})),
        "library.files" => crate::libraries::files(doc),
        "variables.pickFor" => pick_for(doc, params),
        "styles.libraries" => library_rows::styles(doc, params),
        "variables.libraries" => library_rows::variables(doc),
        "menu.context" => menu::context(doc, params),
        "hitTest" => hit_test(doc, params),
        "rename.preview" => rename_preview(doc, params),
        "prefs.recentActions" => Ok(json!({ "actions": doc.session().prefs.recent_actions, "max": kition_design_engine::MAX_RECENT_ACTIONS })),
        "layout.gridGeometry" => grid_geometry(doc, params),
        "arrange.spacingInfo" => spacing_info(doc, params),
        "dev.focusState" => dev::focus_state(doc),
        "dev.compare" => dev::compare(doc, params),
        "codeLinks" => code_links(doc, params),
        "annotation.callouts" => callouts(doc, params),
        "version.previewState" => Ok(doc.ds()?.preview.as_ref().map_or(Value::Null, |p| json!({ "id": p.id, "name": p.name, "time": p.time }))),
        "proto.destinations" => destinations(doc, params),
        "proto.parseExpression" => parse_expression(doc, params),
        _ => Err(format!("unknown query {name}")),
    }
}

impl DesignDocument {
    /// `recordAction(key)`: remembers an action run from the Actions palette for its Recents
    /// (newest first, at most `MAX_RECENT_ACTIONS`; the same key again moves to the front), as
    /// upstream's `menus.rs` `record_recent`. Kept in the session's preferences (not the file, not
    /// an undo step). An empty key is refused.
    pub fn record_action(&mut self, key: &str) -> Result<(), String> {
        let key = key.trim();
        if key.is_empty() || key.chars().count() > 200 {
            return Err("invalid-params: `key` must be a non-empty action key of at most 200 characters".into());
        }
        let r = &mut self.session.prefs.recent_actions;
        if r.first().is_some_and(|f| f == key) {
            return Ok(());
        }
        r.retain(|k| k != key);
        r.insert(0, key.to_owned());
        r.truncate(kition_design_engine::MAX_RECENT_ACTIONS);
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------
// Layers drag.

/// One parent's children as [`drop_info`] reads them.
struct Siblings {
    index: HashMap<NodeId, usize>,
    below: Vec<usize>,
}

/// `layers.dropInfo { ids?, moving? }`: what the Layers panel's drag needs per row (upstream
/// `panels/layers.rs`): `DropRow` = `{ id, parent, index, container, structureLocked,
/// childrenLocked }`, plus with `moving` (the dragged layers) `blocked` and `drop`:
///
/// - `index`: the row's place among its parent's children, bottom first (as the engine stacks
///   them; the panel lists them top first).
/// - `container`: the row takes a drop into it (the middle half of the row): a container that is
///   not an instance.
/// - `structureLocked`: the row's parent is locked (inside an instance, outside its slots): a drop
///   above or below the row is refused. `childrenLocked`: the same for a drop into the row.
/// - `blocked`: the row is one of the dragged layers or inside one (no drop at all).
/// - `drop`: `{ above: { parent, index }, below: { parent, index }, into: { parent } or null }`,
///   the `node.reparent` params for each zone (`index` already discounts the dragged layers below
///   the gap, as upstream computes it), or null where the drop is refused.
///
/// Rows default to every layer on the current page (at most [`MAX_DROP_ROWS`]; `truncated` says
/// when more were left out).
pub fn drop_info(doc: &DesignDocument, params: &Value) -> Result<Value, String> {
    let ds = doc.ds()?;
    let d: &Document = &ds.doc;
    let moving = ids_param(params, "moving");
    let ids: Vec<NodeId> = match ids_param(params, "ids") {
        Some(ids) => ids,
        None => {
            let mut out = Vec::new();
            let mut stack: Vec<NodeId> = d.node(ds.page).map(|p| p.children.iter().rev().copied().collect()).unwrap_or_default();
            while let Some(id) = stack.pop() {
                if out.len() > MAX_DROP_ROWS {
                    break;
                }
                out.push(id);
                if let Some(n) = d.node(id) {
                    stack.extend(n.children.iter().copied());
                }
            }
            out
        }
    };
    let truncated = ids.len() > MAX_DROP_ROWS;
    let locked = |id: NodeId| kition_design_engine::components::structure_locked(d, id);
    let moving: Option<HashSet<NodeId>> = moving.map(|m| m.into_iter().collect());
    // Each parent's children once: their indexes, and how many dragged children lie below each gap
    // (`below[gap]`), so a row costs no walk over its siblings.
    let mut siblings: HashMap<NodeId, Siblings> = HashMap::new();
    for parent in ids.iter().take(MAX_DROP_ROWS).filter_map(|id| d.node(*id)?.parent) {
        siblings.entry(parent).or_insert_with(|| {
            let children = d.node(parent).map_or(&[][..], |p| p.children.as_slice());
            let index = children.iter().enumerate().map(|(i, c)| (*c, i)).collect();
            let mut below = Vec::with_capacity(children.len() + 1);
            below.push(0);
            let mut count = 0;
            for c in children {
                count += usize::from(moving.as_ref().is_some_and(|m| m.contains(c)));
                below.push(count);
            }
            Siblings { index, below }
        });
    }
    let rows: Vec<Value> = ids
        .iter()
        .take(MAX_DROP_ROWS)
        .filter_map(|id| d.node(*id).filter(|n| !n.is_page()))
        .map(|n| {
            let container = n.kind.is_container() && !matches!(n.kind, NodeKind::Instance { .. });
            let parent_locked = n.parent.is_some_and(locked);
            let children_locked = locked(n.id) || matches!(n.kind, NodeKind::Instance { .. });
            let at = n.parent.and_then(|p| siblings.get(&p));
            let index = at.and_then(|s| s.index.get(&n.id).copied());
            let mut row = json!({
                "id": n.id,
                "parent": n.parent,
                "index": index,
                "container": container,
                "structureLocked": parent_locked,
                "childrenLocked": children_locked,
            });
            if let Some(moving) = &moving {
                let blocked = moving.contains(&n.id) || d.ancestors(n.id).iter().any(|a| moving.contains(a));
                let gap = |above: bool| -> Value {
                    let (Some(parent), Some(at)) = (n.parent, at) else { return Value::Null };
                    if blocked || parent_locked {
                        return Value::Null;
                    }
                    let idx = index.unwrap_or(0);
                    let gap = if above { idx + 1 } else { idx };
                    let below = at.below.get(gap).copied().unwrap_or(0);
                    json!({ "parent": parent, "index": gap.saturating_sub(below) })
                };
                let into = (container && !blocked && !children_locked).then(|| json!({ "parent": n.id }));
                row["blocked"] = json!(blocked);
                row["drop"] = json!({ "above": gap(true), "below": gap(false), "into": into });
            }
            row
        })
        .collect();
    Ok(json!({ "page": ds.page, "rows": rows, "truncated": truncated }))
}

// ---------------------------------------------------------------------------------------------
// Assets.

fn matches(name: &str, q: &str) -> bool {
    q.is_empty() || name.to_lowercase().contains(q)
}

/// `assets.list { query? }`: the Assets tab's listing (upstream `assets_panel.rs` `build_listing`): this
/// file's components and styles, then each enabled library's published ones, filtered by `query`
/// (any case):
///
/// ```text
/// { folders: [{ name,                   // the folders joined with " / " ("" first: no heading)
///               tiles: [{ id, name,     // the full name (the tooltip's)
///                         label,        // the name on the tile: its last slash part
///                         set,          // a component set
///                         hidden,       // kept out of publishing (name starts with . or _)
///                         defaultVariant, // a set's default variant (what its tile draws; nodeThumbnail it), else the id
///                         page, variants }] }],   // variants: a set's variant count
///   styles: [{ id, name, type: "paint"|"text"|"effect"|"grid", folder, label, description }],
///   libraries: [{ id, name, path,     // each enabled library, in the document's order
///                 error,              // why its file can't be read now, or null
///                 folders, styles }], // its published components and styles, as above; tile
///                                     // ids are the library's (libraryThumbnail draws them),
///                                     // paint styles add `paints` (their swatch)
///   count }                            // this file's tiles listed
/// ```
///
/// Folders and tiles are sorted as upstream sorts them (folder, then label, any case). Place a
/// library tile with `library.place { library, component, x, y }` (or `library.swapAt`), apply a
/// library style with `library.applyStyle { library, style }`. While searching, libraries without
/// a match are left out.
pub fn assets_list(doc: &DesignDocument, params: &Value) -> Result<Value, String> {
    use kition_design_engine::library;
    let d: &Document = &doc.ds()?.doc;
    let q = str_param(params, "query").unwrap_or("").trim().to_lowercase();
    let local: Vec<NodeId> = library::local_components(d).into_iter().filter(|id| d.node(*id).is_some_and(|n| matches(&n.name, &q))).collect();
    let (folders, count) = tile_folders(d, &local);
    let styles: Vec<Value> = library::local_styles(d).filter(|s| matches(&s.name, &q)).take(MAX_LISTED).map(|s| style_row(s, false)).collect();
    let mut libraries = Vec::new();
    for (link, read) in crate::libraries::enabled(doc) {
        let mut entry = json!({ "id": link.id, "name": link.name, "path": link.path, "error": null, "folders": [], "styles": [] });
        match read {
            Ok(lib) => {
                let m = &lib.manifest;
                let ld = lib.doc.as_ref();
                let ids: Vec<NodeId> =
                    m.components.iter().filter(|a| a.published && matches(&a.name, &q) && ld.node(a.id).is_some()).map(|a| a.id).collect();
                let (lf, _) = tile_folders(ld, &ids);
                let ls: Vec<Value> = ld
                    .styles
                    .iter()
                    .filter(|s| m.style(&s.id).is_some_and(|x| x.published) && matches(&s.name, &q))
                    .take(MAX_LISTED)
                    .map(|s| style_row(s, true))
                    .collect();
                if !q.is_empty() && lf.is_empty() && ls.is_empty() {
                    continue;
                }
                entry["name"] = json!(m.name);
                entry["folders"] = json!(lf);
                entry["styles"] = json!(ls);
            }
            Err(error) if q.is_empty() => entry["error"] = json!(error),
            Err(_) => continue,
        }
        libraries.push(entry);
    }
    Ok(json!({ "folders": folders, "styles": styles, "libraries": libraries, "count": count }))
}

/// Tiles of components and sets `ids` of `d` under their folders, sorted as upstream sorts them;
/// and how many there are.
fn tile_folders(d: &Document, ids: &[NodeId]) -> (Vec<Value>, usize) {
    use kition_design_engine::library;
    let mut tiles: Vec<(String, String, Value)> = ids
        .iter()
        .filter_map(|id| d.node(*id))
        .take(MAX_LISTED)
        .map(|n| {
            let set = matches!(n.kind, NodeKind::ComponentSet { .. });
            let (folders, label) = library::asset_folder(d, n.id);
            let label = if label.is_empty() { n.name.clone() } else { label };
            let default = if set { kition_design_engine::components::default_variant(d, n.id).unwrap_or(n.id) } else { n.id };
            let tile = json!({
                "id": n.id,
                "name": n.name,
                "label": label,
                "set": set,
                "hidden": library::hidden_name(&n.name),
                "defaultVariant": default,
                "page": d.page_of(n.id),
                "variants": if set { n.children.len() } else { 0 },
            });
            (folders.join(" / "), label, tile)
        })
        .collect();
    tiles.sort_by(|(fa, la, a), (fb, lb, b)| {
        fa.to_lowercase().cmp(&fb.to_lowercase()).then(la.to_lowercase().cmp(&lb.to_lowercase())).then(a["id"].to_string().cmp(&b["id"].to_string()))
    });
    let count = tiles.len();
    let mut folders: Vec<(String, Vec<Value>)> = Vec::new();
    for (folder, _, tile) in tiles {
        match folders.last_mut() {
            Some((name, list)) if *name == folder => list.push(tile),
            _ => folders.push((folder, vec![tile])),
        }
    }
    (folders.into_iter().map(|(name, tiles)| json!({ "name": name, "tiles": tiles })).collect(), count)
}

/// A style row of `assets.list` (with its paints for a library's paint style: the editor has no
/// other read of a library's styles).
fn style_row(s: &kition_design_doc::Style, paints: bool) -> Value {
    let (folder, label) = match s.name.rsplit_once('/') {
        Some((f, l)) => (f.trim().to_owned(), l.trim().to_owned()),
        None => (String::new(), s.name.trim().to_owned()),
    };
    let mut row = json!({ "id": s.id, "name": s.name, "type": s.kind.type_name(), "folder": folder, "label": label, "description": s.description });
    if paints && let kition_design_doc::StyleKind::Paint { paints } = &s.kind {
        row["paints"] = serde_json::to_value(paints).unwrap_or(Value::Null);
    }
    row
}

/// `drop.preview { x, y, id?, alt? }`: where an Assets drag of component `id` released at
/// document pixel (`x`, `y`) lands (upstream `assets_panel.rs` `drag_feedback`):
///
/// ```text
/// { parent,            // the frame (or the page) it drops into
///   dropTarget,        // that frame, null on the page itself (the canvas outlines it)
///   index,             // its slot among an auto layout parent's children, null: on top
///   line,              // the insertion line [x0, y0, x1, y1] in an auto layout parent, or null
///   gridCell,          // in a grid auto layout parent: { column, row, rect: [x0, y0, x1, y1] }, or null
///   swap }             // with `alt`: the instance a drop swaps (instance.swapAt), or null
/// ```
///
/// Place with `instance.place { component, x, y }` (page space: `inspect().area` + document
/// pixels) or, when `swap` is set, `instance.swapAt`.
pub fn drop_preview(doc: &DesignDocument, params: &Value) -> Result<Value, String> {
    use kition_design_engine::drop;
    let (x, y) = point_param(params)?;
    let ds = doc.ds()?;
    let d: &Document = &ds.doc;
    let area = doc.area();
    let p: Point = area.to_canvas(x, y);
    let component = match params.get("id") {
        None | Some(Value::Null) => None,
        Some(v) => Some(kition_design_engine::params::id_of(v).ok_or_else(|| format!("invalid-params: `id` {v} is not a layer id"))?),
    };
    let alt = params.get("alt").and_then(Value::as_bool).unwrap_or(false);
    let swap = if alt {
        let family = component.and_then(|c| match d.node(c).map(|n| &n.kind) {
            Some(NodeKind::ComponentSet { .. }) => d.node(c).map(|n| n.name.clone()),
            _ => drop::swap_family(d, c),
        });
        drop::swap_target(d, ds.page, p, family.as_deref(), component)
    } else {
        None
    };
    let slot = drop::slot(d, ds.page, p);
    let grid = drop::grid_cell_at(d, slot.parent, p).map(|(column, row, r)| json!({ "column": column, "row": row, "rect": line_json(&area, r) }));
    Ok(json!({
        "parent": slot.parent,
        "dropTarget": (slot.parent != ds.page).then_some(slot.parent),
        "index": slot.index,
        "line": slot.line.map(|l| line_json(&area, l)),
        "gridCell": grid,
        "swap": swap,
    }))
}

// ---------------------------------------------------------------------------------------------
// Variable picker.

fn kind_of(s: &str) -> Option<VariableType> {
    Some(match s {
        "color" | "COLOR" => VariableType::Color,
        "number" | "float" | "FLOAT" => VariableType::Number,
        "string" | "STRING" => VariableType::String,
        "boolean" | "BOOLEAN" => VariableType::Boolean,
        _ => return None,
    })
}

/// A variable picker row: `display` is the value in the default modes as upstream's row shows it
/// (empty for colours, which show `swatch`).
fn pick_row(vars: &kition_design_doc::Variables, v: &kition_design_doc::Variable) -> Value {
    use kition_design_doc::VariableValue;
    let (swatch, display) = match vars.resolve(&v.id, &BTreeMap::new()) {
        Some(VariableValue::Color(c)) => (Some(c.to_css_hex()), String::new()),
        Some(other) => (None, other.display()),
        None => (None, String::new()),
    };
    json!({ "id": v.id, "name": v.name, "kind": v.kind.id(), "swatch": swatch, "display": display })
}

/// `variables.pickFor { ids?, field?, kind?, query?, target?, variable?, style? }`: the variable
/// picker's list (upstream `variable_ui.rs` `picker`): this file's collections (never library
/// copies), each with the variables that fit, then (to bind or alias) each enabled library's
/// published collections:
///
/// - `kind` (`color`, `number`, `string`, `boolean`; omitted: any type); a text layer's
///   `characters` field takes numbers too.
/// - `target`: `bind` (default: binding `field` of `ids`, default the selection: only variables
///   scoped to the field on every one of them), `alias` (`variable`'s value aliased: only those
///   that would not loop and have its type), `style` (`field` of style `style`: scoped to one of
///   the properties the field sets) or `proto` (a prototype action's variable: any).
/// - `query`: names containing it (any case).
///
/// ```text
/// { scope: null or "FRAME_FILL" …,     // the scope the field asks for
///   groups: [{ collection, name,
///              library: null or { id, name },   // a library's: ids are the library's; pick with
///                                               // library.bindVariable / library.aliasVariable
///              variables: [{ id, name, kind, swatch: "#RRGGBB(AA)" or null, display }] }] }
/// ```
///
/// `display` is the value in the default modes as upstream's row shows it (empty for colours,
/// which show `swatch`). Empty groups are left out.
pub fn pick_for(doc: &DesignDocument, params: &Value) -> Result<Value, String> {
    let d: &Document = &doc.ds()?.doc;
    let kind = match str_param(params, "kind") {
        None => None,
        Some(k) => Some(kind_of(k).ok_or_else(|| format!("invalid-params: `kind` is color, number, string or boolean, not {k:?}"))?),
    };
    let q = str_param(params, "query").unwrap_or("").trim().to_lowercase();
    let field = str_param(params, "field");
    let target = str_param(params, "target").unwrap_or("bind");
    let ids = targets(doc, params)?;
    let mut scopes: Vec<&'static str> = Vec::new();
    let mut own = None;
    let mut style_scopes: Option<&'static [&'static str]> = None;
    match target {
        "bind" => {
            if let Some(field) = field {
                scopes = ids.iter().filter_map(|id| kition_design_engine::cmd::variables::field_scope(d, *id, field)).collect();
                scopes.dedup();
            }
        }
        "alias" => {
            let v = str_param(params, "variable").ok_or("invalid-params: `variable` is required for an alias")?;
            own = Some(d.variables.get(v).ok_or_else(|| format!("not-found: no variable {v:?}"))?);
        }
        "style" => {
            let s = str_param(params, "style").ok_or("invalid-params: `style` is required")?;
            let f = field.ok_or("invalid-params: `field` is required")?;
            let st = d.style(s).ok_or_else(|| format!("not-found: no style {s:?}"))?;
            style_scopes = kition_design_engine::style_vars::field_scopes(&st.kind, f);
        }
        "proto" => {}
        other => return Err(format!("invalid-params: `target` is bind, alias, style or proto, not {other:?}")),
    }
    let kind = kind.or(own.map(|o| o.kind));
    let keep = |v: &kition_design_doc::Variable| {
        kind.is_none_or(|k| v.kind == k || (field == Some("characters") && v.kind == VariableType::Number))
            && matches(&v.name, &q)
            && scopes.iter().all(|s| v.in_scope(s))
            && style_scopes.is_none_or(|list| list.iter().any(|s| v.in_scope(s)))
            && own.is_none_or(|o| kition_design_engine::cmd::variables::alias_target(&d.variables, o, &v.id).is_ok())
    };
    let mut listed = 0usize;
    let mut groups: Vec<Value> = kition_design_engine::library::local_collections(d)
        .filter_map(|c| {
            let vars: Vec<Value> = d
                .variables
                .variables
                .iter()
                .filter(|v| v.collection == c.id && !kition_design_engine::library::is_remote_variable(d, &v.id) && keep(v))
                .take(MAX_LISTED.saturating_sub(listed))
                .map(|v| pick_row(&d.variables, v))
                .collect();
            listed += vars.len();
            (!vars.is_empty()).then(|| json!({ "collection": c.id, "name": c.name, "library": null, "variables": vars }))
        })
        .collect();
    // Then each enabled library's published variables (upstream `variable_ui::groups`): picking one
    // copies it in (`library.bindVariable`, `library.aliasVariable`).
    if matches!(target, "bind" | "alias") {
        let fits = |v: &kition_design_doc::Variable| {
            kind.is_none_or(|k| v.kind == k || (field == Some("characters") && v.kind == VariableType::Number))
                && matches(&v.name, &q)
                && scopes.iter().all(|s| v.in_scope(s))
        };
        for (link, read) in crate::libraries::enabled(doc) {
            let Ok(lib) = read else { continue };
            let offered: Vec<&kition_design_doc::Variable> = kition_design_engine::library::published_variables(&lib).collect();
            for c in kition_design_engine::library::published_collections(&lib) {
                let vars: Vec<Value> = offered
                    .iter()
                    .filter(|v| v.collection == c.id && fits(v))
                    .take(MAX_LISTED.saturating_sub(listed))
                    .map(|v| pick_row(&lib.doc.variables, v))
                    .collect();
                listed += vars.len();
                if !vars.is_empty() {
                    groups.push(
                        json!({ "collection": c.id, "name": c.name, "library": { "id": link.id, "name": lib.manifest.name }, "variables": vars }),
                    );
                }
            }
        }
    }
    let scope = match scopes.as_slice() {
        [one] => json!(one),
        [] => Value::Null,
        many => json!(many),
    };
    Ok(json!({ "scope": scope, "groups": groups }))
}

// ---------------------------------------------------------------------------------------------
// Canvas hit test.

/// `hitTest { x, y, deep?, zoom? }`: what a click at document pixel (`x`, `y`) selects, as
/// upstream's canvas resolves it (`canvas/input.rs` `open_context_menu`: the session's `hit` with
/// a 2-screen-pixel tolerance, then `select_target` against the selection; `deep` is Cmd-click):
///
/// ```text
/// { leaf,       // the topmost layer under the point, or null
///   target,     // what the click selects (the top-level frame's child, a whole group, …), or null
///   frame,      // the target's top-level frame, or null
///   selected,   // the target is selected already
///   handle }    // a shape handle of the selection under the point: { id, kind: "radius", corner } or { id, kind: "ratio" }, or null
/// ```
///
/// `zoom` (default the last tool event's) sets the tolerance and handle sizes.
pub fn hit_test(doc: &DesignDocument, params: &Value) -> Result<Value, String> {
    let (x, y) = point_param(params)?;
    let deep = params.get("deep").and_then(Value::as_bool).unwrap_or(false);
    let zoom = params.get("zoom").and_then(Value::as_f64).filter(|z| z.is_finite() && *z > 0.0).unwrap_or(doc.zoom).max(1e-6);
    let ds = doc.ds()?;
    let d: &Document = &ds.doc;
    let p = doc.area().to_canvas(x, y);
    let leaf = doc.session().hit(d, ds.page, p, 2.0 / zoom);
    let target = leaf.map(|l| kition_design_doc::geometry::select_target(d, l, &ds.selection, deep));
    let handle = kition_design_engine::shape_handles::hit(d, &ds.selection, p, zoom).map(|(id, h)| {
        let mut v = serde_json::to_value(h).unwrap_or(Value::Null);
        v["id"] = json!(id);
        v
    });
    Ok(json!({
        "leaf": leaf,
        "target": target,
        "frame": target.and_then(|t| d.top_level_frame(t)),
        "selected": target.is_some_and(|t| ds.selection.contains(&t)),
        "handle": handle,
    }))
}

// ---------------------------------------------------------------------------------------------
// Rename layers.

/// `rename.preview { ids?, find?, regex?, replace?, name?, start? }`: what Rename layers
/// (`object.batchRename`, the same params) would name each layer, without renaming:
/// `{ rows: [{ id, before, after, changed }], error: null }`, in the order the command numbers
/// them (stacking order, bottom first), or `{ rows: [], error: "invalid regular expression: …" }`
/// while the settings cannot run (the dialog shows why). `ids` default to the selection.
pub fn rename_preview(doc: &DesignDocument, params: &Value) -> Result<Value, String> {
    let d: &Document = &doc.ds()?.doc;
    let ids: Vec<NodeId> = targets(doc, params)?.into_iter().filter(|i| d.node(*i).is_some()).take(MAX_LISTED).collect();
    let rename = kition_design_engine::rename::BatchRename::new(
        str_param(params, "find"),
        params.get("regex").and_then(Value::as_bool).unwrap_or(false),
        str_param(params, "replace").unwrap_or(""),
        str_param(params, "name"),
        params.get("start").and_then(Value::as_f64),
    );
    let rename = match rename {
        Ok(r) => r,
        Err(e) => return Ok(json!({ "rows": [], "error": e })),
    };
    let rows: Vec<Value> = rename
        .plan(d, &ids)
        .into_iter()
        .map(|(id, after)| {
            let before = d.node(id).map(|n| n.name.clone()).unwrap_or_default();
            json!({ "id": id, "changed": before != after, "before": before, "after": after })
        })
        .collect();
    Ok(json!({ "rows": rows, "error": null }))
}

// ---------------------------------------------------------------------------------------------
// Layout and arrange.

/// `layout.gridGeometry { id? }`: a grid auto layout frame's resolved tracks and cells (the
/// engine's `grid_geometry`, from the children's current sizes), or `null` for other layers:
///
/// ```text
/// { id, width, height,                       // the size the grid wants (frame space)
///   columns: [{ start, end, size }],         // frame space, left to right
///   rows:    [{ start, end, size }],         // frame space, top to bottom
///   cells:   [{ id, column, row, columnSpan, rowSpan, rect: [x0, y0, x1, y1] }],  // rect in frame space
///   unit }                                   // [a, b, c, d, e, f]: frame space → document pixels (x' = a·x + c·y + e, y' = b·x + d·y + f)
/// ```
pub fn grid_geometry(doc: &DesignDocument, params: &Value) -> Result<Value, String> {
    let id = target(doc, params)?;
    let d: &Document = &doc.ds()?.doc;
    if d.node(id).is_none() {
        return Err(format!("not-found: no layer {id}"));
    }
    let Some(g) = kition_design_layout::grid_geometry(d, id) else { return Ok(Value::Null) };
    let tracks = |t: &[(f64, f64)]| -> Vec<Value> { t.iter().map(|(a, b)| json!({ "start": a, "end": b, "size": b - a })).collect() };
    let cells: Vec<Value> = g
        .cells
        .iter()
        .map(|c| {
            let r = g.cell_rect(c);
            json!({ "id": c.id, "column": c.column, "row": c.row, "columnSpan": c.column_span, "rowSpan": c.row_span, "rect": [r.x0, r.y0, r.x1, r.y1] })
        })
        .collect();
    let area = doc.area();
    let w = d.world_transform(id).as_coeffs();
    let unit = [w[0], w[1], w[2], w[3], w[4] - area.x, w[5] - area.y];
    Ok(json!({
        "id": id,
        "width": g.width,
        "height": g.height,
        "columns": tracks(&g.columns),
        "rows": tracks(&g.rows),
        "cells": cells,
        "unit": unit.iter().all(|v| v.is_finite()).then_some(unit),
    }))
}

/// `arrange.spacingInfo { ids? }`: the Position section's Spacing row for the selection (the
/// engine's `selection_spacing`: siblings outside auto layout flow, in rows or columns):
/// `{ horizontal: Axis or null, vertical: Axis or null }`, `Axis` = `{ value: number or null
/// (null: the gaps differ, "Mixed"), tracks: [{ ids, rect: { x, y, width, height } }], gaps:
/// [number] }` (tracks in document pixels, left to right or top to bottom). Set a spacing with
/// `arrange.spacing`.
pub fn spacing_info(doc: &DesignDocument, params: &Value) -> Result<Value, String> {
    let ids = targets(doc, params)?;
    let d: &Document = &doc.ds()?.doc;
    let area = doc.area();
    let s = kition_design_engine::selection_spacing::selection_spacing(d, &ids);
    let axis = |a: Option<&kition_design_engine::selection_spacing::Axis>| -> Value {
        let Some(a) = a else { return Value::Null };
        let tracks: Vec<Value> = a.tracks.iter().map(|t| json!({ "ids": t.ids, "rect": area.rect_json(t.bounds) })).collect();
        let gaps: Vec<f64> = a.tracks.windows(2).filter_map(|p| p.first().zip(p.get(1)).map(|(x, y)| a.gap(x, y))).collect();
        json!({ "value": a.value, "tracks": tracks, "gaps": gaps })
    };
    Ok(json!({ "horizontal": axis(s.horizontal.as_ref()), "vertical": axis(s.vertical.as_ref()) }))
}

// ---------------------------------------------------------------------------------------------
// Dev Mode reads.

/// `codeLinks { id? }`: the code components a layer is linked to (Connect to code): `{ id, owner,
/// links }`, `owner` the main component or set holding the links (an instance reads its main's;
/// null: nothing links), `links` its `CodeLink`s as `node_json` gives them (camelCase:
/// `{ language, component, source, props, … }`).
pub fn code_links(doc: &DesignDocument, params: &Value) -> Result<Value, String> {
    let id = target(doc, params)?;
    let d: &Document = &doc.ds()?.doc;
    if d.node(id).is_none() {
        return Err(format!("not-found: no layer {id}"));
    }
    let owner = kition_design_engine::code_connect::owner(d, id);
    let links = owner
        .and_then(|o| d.node(o))
        .map(|n| kition_design_engine::inspect::camel(serde_json::to_value(&n.code_links).unwrap_or(Value::Null)))
        .unwrap_or_else(|| json!([]));
    Ok(json!({ "id": id, "owner": owner, "links": links }))
}

/// `annotation.callouts { page? }`: the annotations Dev Mode draws on the canvas, in layer order:
/// `{ callouts: [{ id, name, index, label, properties, category?, bounds: { x, y, width, height }
/// or null, visible }] }` (`annotation.list`'s rows with the annotated layer's bounds in document
/// pixels; `visible` false for a layer hidden itself or inside a hidden one). `page` defaults to
/// the current page (other pages' bounds are their own page space).
pub fn callouts(doc: &DesignDocument, params: &Value) -> Result<Value, String> {
    let ds = doc.ds()?;
    let d: &Document = &ds.doc;
    let page = match params.get("page") {
        None | Some(Value::Null) => ds.page,
        Some(v) => {
            kition_design_engine::params::id_of(v).filter(|p| d.pages.contains(p)).ok_or_else(|| format!("invalid-params: {v} is not a page"))?
        }
    };
    let area = doc.area_on(Some(page));
    let mut out = Vec::new();
    for id in kition_design_engine::annotations::on_page(d, page) {
        let Some(n) = d.node(id) else { continue };
        let visible = n.visible && d.ancestors(id).iter().all(|a| d.node(*a).is_none_or(|x| x.visible));
        let bounds = d.world_bounds(id).map(|b| area.rect_json(b));
        for (index, a) in n.annotations.iter().enumerate() {
            let mut v = kition_design_engine::annotations::annotation_json(d, id, index, a);
            v["bounds"] = bounds.clone().unwrap_or(Value::Null);
            v["visible"] = json!(visible);
            out.push(v);
        }
    }
    Ok(json!({ "callouts": out }))
}

// ---------------------------------------------------------------------------------------------
// Prototype.

/// `proto.destinations { id?, kind? }`: what an action of `kind` on layer `id` (default the first
/// selected layer) can point at, as upstream's interaction editor lists them
/// (`panels/proto_panel.rs`): `{ kind, destinations: [{ id, name, type }] }`.
///
/// - `navigate` (default): the page's top-level frames, then its sections.
/// - `overlay` (Open overlay, Swap overlay): the top-level frames.
/// - `changeTo`: the variants of the layer's component set (the layer's owning component, or an
///   instance's main), or that lone component; empty outside components.
/// - `scrollTo`: the layers inside the layer's top-level frame (but the layer itself).
pub fn destinations(doc: &DesignDocument, params: &Value) -> Result<Value, String> {
    let ds = doc.ds()?;
    let d: &Document = &ds.doc;
    let kind = str_param(params, "kind").unwrap_or("navigate");
    let node = match params.get("id") {
        None | Some(Value::Null) => ds.selection.first().copied(),
        Some(v) => Some(kition_design_engine::params::id_of(v).ok_or_else(|| format!("invalid-params: `id` {v} is not a layer id"))?),
    };
    let ids: Vec<NodeId> = match kind {
        "navigate" => {
            let mut v = d.top_frames(ds.page);
            v.extend(d.sections_in(ds.page));
            v
        }
        "overlay" | "openOverlay" | "swapOverlay" => d.top_frames(ds.page),
        "changeTo" => {
            let node = node.ok_or("invalid-params: `id` is required for changeTo")?;
            let comp = kition_design_engine::components::owning_component(d, node).or_else(|| match d.node(node).map(|n| &n.kind) {
                Some(NodeKind::Instance { component, .. }) => Some(*component),
                _ => None,
            });
            let set = comp.and_then(|c| kition_design_engine::components::variant_set(d, c));
            match set.and_then(|s| d.node(s)) {
                Some(s) => s.children.clone(),
                None => comp.into_iter().collect(),
            }
        }
        "scrollTo" => {
            let node = node.ok_or("invalid-params: `id` is required for scrollTo")?;
            d.top_level_frame(node).map(|root| d.subtree(root).into_iter().skip(1).filter(|i| *i != node).collect()).unwrap_or_default()
        }
        other => return Err(format!("invalid-params: `kind` is navigate, overlay, changeTo or scrollTo, not {other:?}")),
    };
    let list: Vec<Value> = ids
        .into_iter()
        .take(MAX_LISTED)
        .filter_map(|id| d.node(id).map(|n| json!({ "id": id, "name": n.name, "type": n.kind.type_name() })))
        .collect();
    Ok(json!({ "kind": kind, "destinations": list }))
}

/// `proto.parseExpression { text, expect?, variable?, kind? }`: an expression typed in the
/// interaction editor, read as upstream's expression fields read it (`Expr::parse`, then the
/// check for what the field holds):
///
/// - `expect`: `condition` (must give true or false), `value` (a set variable's value: fits
///   `variable`'s type, or `kind`), or `any` (default).
///
/// ```text
/// { ok, error: null or message, at: null or char offset of a parse error,
///   display: the expression written back as the editor shows it (null when it does not parse),
///   type: "color"|"number"|"string"|"boolean" or null,
///   expr: the expression JSON a prototype action stores (`proto.action.*`'s `expr`/`condition`), or null }
/// ```
pub fn parse_expression(doc: &DesignDocument, params: &Value) -> Result<Value, String> {
    use kition_design_doc::proto::Expr;
    let text = str_param(params, "text").ok_or("invalid-params: `text` is required")?;
    let vars = &doc.ds()?.doc.variables;
    let expect = str_param(params, "expect").unwrap_or("any");
    let kind = match (str_param(params, "variable"), str_param(params, "kind")) {
        (Some(v), _) => Some(vars.get(v).map(|v| v.kind).ok_or_else(|| format!("not-found: no variable {v:?}"))?),
        (None, Some(k)) => Some(kind_of(k).ok_or_else(|| format!("invalid-params: `kind` is color, number, string or boolean, not {k:?}"))?),
        (None, None) => None,
    };
    let e = match Expr::parse(text, vars) {
        Ok(e) => e,
        Err(err) => return Ok(json!({ "ok": false, "error": err.message, "at": err.at, "display": null, "type": null, "expr": null })),
    };
    let checked = match expect {
        "condition" => e.check_condition(vars),
        "value" => match kind {
            Some(k) => e.check_value(vars, k),
            None => return Err("invalid-params: `expect: value` needs `variable` or `kind`".into()),
        },
        "any" => e.type_of(vars).map(|_| ()),
        other => return Err(format!("invalid-params: `expect` is condition, value or any, not {other:?}")),
    };
    let ty = e.type_of(vars).ok().map(VariableType::id);
    Ok(json!({
        "ok": checked.is_ok(),
        "error": checked.err(),
        "at": null,
        "display": e.display(vars),
        "type": ty,
        "expr": serde_json::to_value(&e).unwrap_or(Value::Null),
    }))
}

/// The Design editor's list extras.
#[cfg(target_arch = "wasm32")]
mod web {
    use kition_kit::wasm_bindgen::prelude::*;

    use crate::__kition_kit_export::WebDocument;

    fn error(message: String) -> JsError {
        JsError::new(&message)
    }

    #[wasm_bindgen(js_class = "DesignDocument", wasm_bindgen = kition_kit::wasm_bindgen)]
    impl WebDocument {
        /// The compare view's picture of a ready-for-dev layer as PNG bytes; `params` is a JSON
        /// object `{ id, version?, side: "before" or "after", maxSide? }` (see `lists/dev.rs`).
        #[wasm_bindgen(js_name = devCompareRender)]
        pub fn dev_compare_render(&self, params: &str) -> Result<Vec<u8>, JsError> {
            let params = kition_kit::parse_params(params).map_err(error)?;
            self.0.dev_compare_render(&params).map_err(error)
        }

        /// The component playground's instance with the chosen property values as PNG bytes;
        /// `params` is a JSON object `{ id?, properties?, scale?, maxSide? }` (see `lists/dev.rs`).
        #[wasm_bindgen(js_name = playgroundPreview)]
        pub fn playground_preview(&self, params: &str) -> Result<Vec<u8>, JsError> {
            let params = kition_kit::parse_params(params).map_err(error)?;
            self.0.playground_preview(&params).map_err(error)
        }

        /// Remembers an action run from the Actions palette for its Recents
        /// (`query("prefs.recentActions")`).
        #[wasm_bindgen(js_name = recordAction)]
        pub fn record_action(&mut self, key: &str) -> Result<(), JsError> {
            self.0.record_action(key).map_err(error)
        }
    }
}
