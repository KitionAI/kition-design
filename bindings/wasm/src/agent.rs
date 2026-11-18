//! The binding's own commands for agents, beside the engine's catalog: `file.render`, the preview
//! KHP A39 asks for; `page.go` and `page.inspect`, which go to a page and list one without going
//! there (task card C4/C5/C6-design); and the screening of `command.batch` steps, so a batch runs only
//! commands the kit offers (never the upstream app's live sessions, servers (team libraries on a
//! server among them) or plug-in directory, which the kit leaves out; libraries from files are read
//! through the lent files, `crate::libraries`).
//!
//! `file.render` is a function of the design and the call: its view is `target` (a page by
//! `pageId` or `page` counted from 1, a layer by `objectId` or a diagnostic's `path`, a `frame`, or
//! an `area` of a page), `page` (from 1) or `slide` (from 0), the first page without any, never the
//! current page, the selection or the view. It draws on the calling thread, never in bands across
//! threads, so the bytes do not depend on the host's thread count, with the fonts built into the
//! kit. It never changes the document.

use base64::Engine as _;
use kition_design_doc::{Document, NodeId};
use kition_design_geom::Rect;
use kition_design_render::{Options, View};
use kition_kit::KitCommand;
use serde_json::{Value, json};

use crate::DesignDocument;

/// The preview command's id.
pub const RENDER: &str = "file.render";

/// The largest `scale` (device pixels per canvas unit) and `maxSize` (pixels on the longer side).
pub const MAX_SCALE: f64 = 2.0;
pub const MAX_SIZE: u32 = 4096;
/// `maxSize` when none is given.
pub const DEFAULT_SIZE: u32 = 2048;
/// What an empty page shows: one 1920 x 1080 view of its background at the canvas origin.
const EMPTY: (f64, f64) = (1920.0, 1080.0);

/// What `file.render` answers, as a `ValueSchemaSpec` (KHP A1), for hosts that carry output
/// schemas.
pub fn output() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "png": { "type": "string", "required": true, "description": "The picture as base64 PNG." },
            "mimeType": { "type": "string", "required": true },
            "width": { "type": "integer", "required": true },
            "height": { "type": "integer", "required": true },
            "scale": { "type": "number", "required": true, "description": "The scale drawn at, after the size budget." },
            "page": { "type": "integer", "required": true, "description": "The page drawn, counted from 1." },
            "pageId": { "type": "integer", "required": true, "description": "The page drawn, by its id." },
            "pageName": { "type": "string", "required": true },
            "drew": { "type": "string", "required": true, "description": "What the view is: `page` (its content), `layer` (one layer where it is on its page) or `area` (an area of the page)." },
            "objectId": { "type": "json", "required": true, "description": "The layer drawn (a layer or frame view), else null." },
            "bounds": { "type": "json", "required": true, "description": "The canvas area drawn, in page coordinates: {x, y, width, height}." },
        },
    })
}

/// The command that goes to a page (navigation: the current page moves), and the one that lists a
/// page's layers, or one layer's, without going there.
pub const GO: &str = "page.go";
pub const INSPECT: &str = "page.inspect";

/// The most layers [`INSPECT`] lists.
const MAX_LISTED: usize = 500;

/// Whether `id` is one of the binding's own commands.
pub fn owns(id: &str) -> bool {
    matches!(id, RENDER | GO | INSPECT)
}

/// The binding's commands, as the catalog lists them.
pub fn commands(open: bool) -> Vec<KitCommand> {
    let page = |what: &str| {
        json!({
            "index": { "type": "integer", "description": format!("The page {what}, counted from 0 (its `index` in inspect's `pages`).") },
            "id": { "oneOf": [{ "type": "integer" }, { "type": "string" }], "description": format!("The page {what}, by its id (inspect's `pages[].id`), which stays the same when pages are added, moved or renamed.") },
        })
    };
    let mut inspect_schema = page("to list");
    inspect_schema["id"]["description"] =
        json!("A page's id (inspect's `pages[].id`): that page's layers; or a layer's id: that layer and the layers in it.");
    vec![
        KitCommand {
            id: GO.to_owned(),
            title: "Go to page".to_owned(),
            description: "Makes a page the current one, by its id or its index, as clicking it in the Pages list does; answers its id, index and name. Agents name the page a step acts on with `at.part` instead, which goes there for that step only.".to_owned(),
            params: Some("{id? (page id) | index? (from 0)} → {id, index, name}".to_owned()),
            schema: Some(page("to go to")),
            output_schema: Some(json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "id": { "type": "integer", "required": true },
                    "index": { "type": "integer", "required": true, "description": "Counted from 0." },
                    "name": { "type": "string", "required": true },
                },
            })),
            mutates: false,
            enabled: open,
            ..KitCommand::default()
        },
        KitCommand {
            id: INSPECT.to_owned(),
            title: "List page layers".to_owned(),
            description: "Lists the layers of any page (by id or index), or one layer and the layers in it (by its id), without going to that page: each with its id, name, type, `index` (its place among its siblings, 0 the first child: the bottom unless its frame stacks the first on top) and `z` (its place in paint order, 0 at the bottom), topmost first as the Layers panel shows them.".to_owned(),
            params: Some("{id? (page or layer id) | index? (page, from 0)} → {id, index, name, page, layers: [{id, name, type, index, z, hidden?, children?, childCount?}], truncated}".to_owned()),
            schema: Some(inspect_schema),
            output_schema: Some(json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "id": { "type": "integer", "required": true, "description": "The page's id, or the layer's." },
                    "index": { "type": "integer", "required": true, "description": "The page's index, counted from 0." },
                    "name": { "type": "string", "required": true },
                    "page": { "type": "integer", "required": true, "description": "The id of the page listed, or of the layer's page." },
                    "layers": { "type": "array", "required": true, "items": { "type": "json" }, "description": "The page's top-level layers (a layer: itself), topmost first, each with its children." },
                    "truncated": { "type": "boolean", "required": true, "description": "Whether more layers than the 500 listed are there." },
                },
            })),
            mutates: false,
            enabled: open,
            ..KitCommand::default()
        },
        KitCommand {
        id: RENDER.to_owned(),
        title: "Render preview".to_owned(),
        description: "Draws a view of the design as a PNG preview, a function of the design and the call alone: a page by id or number, a layer, a frame or an area of a page, framed to it; without a target the first page. Never the current page, the selection or the view, and never a change. Use it to look at the result of an edit.".to_owned(),
        menu: None,
        shortcut: None,
        params: Some(
            "{target? {pageId? (a page's id) | page? (counted from 1), objectId? | path? (\"layer:<id>\", a diagnostic's) | frame? (a frame's id), area? {x, y, width, height} (page coordinates)}, page? (counted from 1), slide? (a page counted from 0), scale? (up to 2, default 1), maxSize? (pixels on the longer side, up to 4096, default 2048)} → {png (base64), mimeType, width, height, scale, page, pageId, pageName, drew, objectId, bounds}".to_owned(),
        ),
        schema: Some(json!({
            "target": {
                "type": "object",
                "additionalProperties": false,
                "description": "The view: a page (`pageId`, or `page` counted from 1), a layer (`objectId`, or a diagnostic's `path` `layer:<id>`), a frame (`frame`), or an `area` of a page; a layer names its page. Without one, the first page.",
                "properties": {
                    "pageId": { "oneOf": [{ "type": "integer" }, { "type": "string" }], "description": "A page's id, as inspect lists it." },
                    "page": { "type": "integer", "description": "A page counted from 1, as diagnostics give it." },
                    "objectId": { "oneOf": [{ "type": "integer" }, { "type": "string" }], "description": "A layer's id: that layer where it is on its page." },
                    "path": { "type": "string", "description": "A diagnostic's target, `layer:<id>`." },
                    "frame": { "oneOf": [{ "type": "integer" }, { "type": "string" }], "description": "A frame's (or section's, or component's) id." },
                    "area": { "type": "object", "additionalProperties": false, "description": "An area in page coordinates {x, y, width, height}, on the page named (the first without one).", "properties": {
                        "x": { "type": "number" }, "y": { "type": "number" }, "width": { "type": "number" }, "height": { "type": "number" }
                    } },
                },
            },
            "page": { "type": "integer", "description": "The page to draw, counted from 1." },
            "slide": { "type": "integer", "description": "The page to draw, counted from 0 (what hosts send for one part of a document)." },
            "scale": { "type": "number", "description": "Device pixels per canvas unit, above 0 and up to 2; default 1." },
            "maxSize": { "type": "integer", "description": "The most pixels on the longer side, 1 to 4096; default 2048. A larger view is drawn at a smaller scale." },
        })),
        // The answer's shape is in `params` and [`output`]: Kition's Kit Process Protocol schema has no
        // `output` member on a command yet, so the catalog carries none.
        output_schema: None,
        keywords: Vec::new(),
        mutates: false,
        enabled: open,
        ..KitCommand::default()
    },
    ]
}

/// Runs one of the binding's commands.
pub fn run(doc: &mut DesignDocument, command: &str, params: &Value) -> Result<Value, String> {
    match command {
        RENDER => render(doc, params),
        GO => go(doc, params),
        INSPECT => list(doc, params),
        other => Err(format!("unknown command {other}")),
    }
}

/// How deep `command.batch` steps may nest batches before [`screen_batch`] refuses them.
const MAX_BATCH_DEPTH: usize = 32;

/// Refuses a `command.batch` whose steps name commands the kit does not offer, in its nested
/// batches too (a step that is a `command.batch` runs its own steps).
pub fn screen_batch(params: &Value) -> Result<(), String> {
    screen_steps(params, "", 0)
}

fn screen_steps(params: &Value, at: &str, depth: usize) -> Result<(), String> {
    if depth > MAX_BATCH_DEPTH {
        return Err(format!("invalid-params: command.batch nests batches deeper than {MAX_BATCH_DEPTH} levels"));
    }
    let Some(steps) = params.get("commands").and_then(Value::as_array) else { return Ok(()) };
    for (index, step) in steps.iter().enumerate() {
        let Some(command) = step.get("command").and_then(Value::as_str) else { continue };
        let path = format!("{at}{index}");
        if !crate::commands::offered(command) {
            return Err(format!("invalid-params: step {path} of command.batch runs {command}, which this kit does not offer"));
        }
        if command == "command.batch"
            && let Some(inner) = step.get("params")
        {
            screen_steps(inner, &format!("{path}."), depth + 1)?;
        }
    }
    Ok(())
}

fn integer(params: &Value, key: &str) -> Result<Option<i64>, String> {
    match params.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_i64()
            .or_else(|| value.as_f64().filter(|v| v.fract() == 0.0 && v.abs() < 1e15).map(|v| v as i64))
            .map(Some)
            .ok_or_else(|| format!("invalid-params: `{key}` must be a whole number, not {value}")),
    }
}

/// The page id of page `number` (counted from 1).
fn page_numbered(doc: &Document, number: i64) -> Result<NodeId, String> {
    number
        .checked_sub(1)
        .and_then(|index| usize::try_from(index).ok())
        .and_then(|index| doc.pages.get(index).copied())
        .ok_or_else(|| format!("not-found: there is no page {number}; the design has {} page(s)", doc.pages.len()))
}

/// The members a view (`target`) takes.
const VIEW_MEMBERS: [&str; 6] = ["pageId", "page", "objectId", "path", "frame", "area"];

/// What to draw: a page's content, one layer where it is on its page, or an area of a page.
enum Subject {
    Page(NodeId),
    Layer(NodeId, NodeId),
    Area(NodeId, Rect),
}

fn as_id(value: &Value) -> Option<NodeId> {
    value.as_u64().or_else(|| value.as_str().and_then(|text| text.trim().parse().ok())).map(NodeId)
}

/// The page `target.pageId` names.
fn page_by_id(doc: &Document, value: &Value) -> Result<NodeId, String> {
    let id = as_id(value).ok_or_else(|| format!("invalid-params: `target.pageId` is {value}, not a page id"))?;
    if doc.pages.contains(&id) {
        Ok(id)
    } else if doc.contains(id) {
        Err(format!("invalid-params: {id} is a layer, not a page: give it as `objectId`, or a page's id as `pageId`"))
    } else {
        Err(format!("not-found: the design has no page {id}; inspect's `pages` lists their ids"))
    }
}

/// The layer `objectId`, `path` or `frame` names, with its page.
fn layer(doc: &Document, key: &str, value: &Value) -> Result<(NodeId, NodeId), String> {
    let text = match (key, value.as_str()) {
        ("path", Some(path)) => {
            path.strip_prefix("layer:").ok_or_else(|| format!("invalid-params: target.path {path:?} is not a layer (`layer:<id>`)"))?.to_owned()
        }
        ("path", None) => return Err(format!("invalid-params: target.path is {value}, not `layer:<id>`")),
        _ => value.as_str().map_or_else(|| value.to_string(), str::to_owned),
    };
    let id: NodeId = text.trim().parse().map_err(|_| format!("invalid-params: target.{key} {value} is not a layer id"))?;
    let node = doc.node(id).ok_or_else(|| format!("not-found: no layer {id}"))?;
    if doc.pages.contains(&id) {
        return Err(format!("invalid-params: {id} is a page: give it as `pageId`"));
    }
    if key == "frame" && !node.kind.is_frame_like() {
        return Err(format!("invalid-params: target.frame {id} is a {}, not a frame: give it as `objectId`", node.kind.type_name()));
    }
    let page = doc.page_of(id).filter(|page| doc.pages.contains(page)).ok_or_else(|| format!("not-found: layer {id} is on no page"))?;
    Ok((id, page))
}

/// An area of a page, `{x, y, width, height}` in page coordinates.
fn area(value: &Value) -> Result<Rect, String> {
    let side = |key: &str| value.get(key).and_then(Value::as_f64).filter(|v| v.is_finite() && v.abs() < crate::area::MAX_SIDE);
    match (side("x"), side("y"), side("width"), side("height")) {
        (Some(x), Some(y), Some(w), Some(h)) if w > 0.0 && h > 0.0 => Ok(Rect::new(x, y, x + w, y + h)),
        _ => Err(format!("invalid-params: target.area is {value}: an area is {{x, y, width, height}} in page coordinates, its sides above 0")),
    }
}

/// The view a call names (module docs): the first page without one.
fn subject(doc: &Document, params: &Value) -> Result<Subject, String> {
    let mut page: Option<NodeId> = None;
    let mut name_page = |found: NodeId, by: &str| -> Result<(), String> {
        match page {
            Some(other) if other != found => Err(format!("invalid-params: {by} is on page {found}, and the view names page {other}: name one page")),
            _ => {
                page = Some(found);
                Ok(())
            }
        }
    };
    if let Some(number) = integer(params, "page")? {
        name_page(page_numbered(doc, number)?, "`page`")?;
    }
    if let Some(slide) = integer(params, "slide")? {
        name_page(page_numbered(doc, slide.saturating_add(1))?, "`slide`")?;
    }
    let Some(target) = params.get("target").filter(|t| !t.is_null()) else {
        return Ok(Subject::Page(page.or_else(|| doc.pages.first().copied()).ok_or("not-found: the design has no page")?));
    };
    let Some(members) = target.as_object() else { return Err(format!("invalid-params: `target` must be an object, not {target}")) };
    if let Some(other) = members.keys().find(|key| !VIEW_MEMBERS.contains(&key.as_str())) {
        return Err(format!(
            "invalid-params: `target.{other}` is no view of a design: a view takes {} (a page by id or number, a layer, a frame, an area)",
            VIEW_MEMBERS.join(", ")
        ));
    }
    if members.is_empty() {
        return Err("invalid-params: `target` names no page, layer, frame or area".to_owned());
    }
    if let Some(value) = members.get("pageId") {
        name_page(page_by_id(doc, value)?, "`pageId`")?;
    }
    if let Some(number) = integer(target, "page")? {
        name_page(page_numbered(doc, number)?, "`page`")?;
    }
    let named: Vec<&str> = ["objectId", "path", "frame"].into_iter().filter(|key| members.contains_key(*key)).collect();
    if named.len() > 1 || (!named.is_empty() && members.contains_key("area")) {
        return Err("invalid-params: a view is one layer (`objectId`, `path` or `frame`) or one `area`, not several".to_owned());
    }
    if let Some(key) = named.first() {
        let (id, on) = layer(doc, key, &members[*key])?;
        name_page(on, &format!("layer {id}"))?;
        return Ok(Subject::Layer(id, on));
    }
    let page = page.or_else(|| doc.pages.first().copied()).ok_or("not-found: the design has no page")?;
    match members.get("area") {
        Some(value) => Ok(Subject::Area(page, area(value)?)),
        None => Ok(Subject::Page(page)),
    }
}

/// The page's content: its visible top-level layers as they paint (shadows and strokes included).
fn content(doc: &Document, page: NodeId) -> Option<Rect> {
    let finite = |r: &Rect| [r.x0, r.y0, r.x1, r.y1].iter().all(|v| v.is_finite() && v.abs() < crate::area::MAX_SIDE);
    doc.node(page)?
        .children
        .iter()
        .filter(|c| doc.node(**c).is_some_and(|n| n.visible))
        .filter_map(|c| doc.world_render_bounds(*c).or_else(|| doc.world_bounds(*c)))
        .filter(finite)
        .reduce(|a, b| a.union(b))
}

/// `file.render`: the view the call names, at its scale within its size budget. Pure: it reads the
/// design and the call alone, and draws on the calling thread (one band) with the fonts built into
/// the kit, so the same call answers the same bytes whatever the editor shows and however many
/// threads the host runs.
fn render(doc: &DesignDocument, params: &Value) -> Result<Value, String> {
    let ds = doc.ds()?;
    let d: &Document = &ds.doc;
    let scale = match params.get("scale") {
        None | Some(Value::Null) => 1.0,
        Some(value) => value
            .as_f64()
            .filter(|s| s.is_finite() && *s > 0.0 && *s <= MAX_SCALE)
            .ok_or_else(|| format!("invalid-params: `scale` must be above 0 and at most {MAX_SCALE}, not {value}"))?,
    };
    let max_size = match integer(params, "maxSize")? {
        None => DEFAULT_SIZE,
        Some(size) => u32::try_from(size)
            .ok()
            .filter(|s| (1..=MAX_SIZE).contains(s))
            .ok_or_else(|| format!("invalid-params: `maxSize` must be 1 to {MAX_SIZE}, not {size}"))?,
    };
    let (page, area, drew, object) = match subject(d, params)? {
        Subject::Page(page) => (page, content(d, page).unwrap_or_else(|| Rect::new(0.0, 0.0, EMPTY.0, EMPTY.1)), "page", None),
        Subject::Layer(id, page) => {
            let bounds =
                d.world_render_bounds(id).or_else(|| d.world_bounds(id)).ok_or_else(|| format!("invalid-params: layer {id} draws nothing"))?;
            (page, bounds, "layer", Some(id))
        }
        Subject::Area(page, area) => (page, area, "area", None),
    };
    // Whole canvas units, so the picture lines up with the pixel grid.
    let area = Rect::new(area.x0.floor(), area.y0.floor(), area.x1.ceil().max(area.x0.floor() + 1.0), area.y1.ceil().max(area.y0.floor() + 1.0));
    let longest = area.width().max(area.height());
    let scale = scale.min(f64::from(max_size) / longest);
    let pixels = |side: f64| ((side * scale).round().clamp(1.0, f64::from(MAX_SIZE))) as u32;
    let (width, height) = (pixels(area.width()), pixels(area.height()));
    let view = View { origin: (area.x0, area.y0), scale, width, height };
    let options = Options::default();
    let mut pixmap = kition_design_render::render_view_single(d, page, &view, &options).ok_or_else(|| format!("cannot render {width} x {height}"))?;
    kition_design_render::to_srgb(&mut pixmap, d.color_profile);
    let png = kition_design_render::encode_png(&pixmap)?;
    let number = d.pages.iter().position(|p| *p == page).map_or(0, |index| index + 1);
    Ok(json!({
        "png": base64::engine::general_purpose::STANDARD.encode(png),
        "mimeType": "image/png",
        "width": width,
        "height": height,
        "scale": scale,
        "page": number,
        "pageId": page,
        "pageName": d.node(page).map(|node| node.name.clone()).unwrap_or_default(),
        "drew": drew,
        "objectId": object,
        "bounds": { "x": area.x0, "y": area.y0, "width": area.width(), "height": area.height() },
    }))
}

/// The page `params` names by `id` or `index`, and its index.
fn named_page(d: &Document, params: &Value) -> Result<(NodeId, usize), String> {
    let id = match params.get("id") {
        None | Some(Value::Null) => None,
        Some(value) => Some(
            value
                .as_u64()
                .or_else(|| value.as_str().and_then(|text| text.trim().parse().ok()))
                .map(NodeId)
                .ok_or_else(|| format!("invalid-params: `id` is {value}, not a page id"))?,
        ),
    };
    let count = d.pages.len();
    match (id, integer(params, "index")?) {
        (Some(_), Some(_)) => Err("invalid-params: give the page by `id` or by `index`, not both".to_owned()),
        (Some(id), None) => d
            .pages
            .iter()
            .position(|page| *page == id)
            .map(|index| (id, index))
            .ok_or_else(|| format!("invalid-params: the design has no page {id}; its {count} pages' ids are in inspect's `pages`")),
        (None, Some(index)) => usize::try_from(index)
            .ok()
            .and_then(|index| d.pages.get(index).map(|page| (*page, index)))
            .ok_or_else(|| format!("invalid-params: there is no page {index}: pages count from 0 and the design has {count} pages")),
        (None, None) => Err("invalid-params: give the page by `id` (inspect's `pages[].id`) or `index` (from 0)".to_owned()),
    }
}

/// `page.go`: makes the page current, as the Pages list does.
fn go(doc: &mut DesignDocument, params: &Value) -> Result<Value, String> {
    let (page, index) = named_page(&doc.ds()?.doc, params)?;
    doc.run_offered("page.select", json!({ "id": page.0 }))?;
    let name = doc.ds()?.doc.node(page).map(|node| node.name.clone()).unwrap_or_default();
    Ok(json!({ "id": page, "index": index, "name": name }))
}

/// One layer and the layers in it, topmost first, at most `left` in all.
fn layer_json(d: &Document, id: NodeId, index: usize, z: usize, left: &mut usize, cut: &mut bool) -> Option<Value> {
    let node = d.node(id)?;
    if *left == 0 {
        *cut = true;
        return None;
    }
    *left -= 1;
    let mut out = json!({ "id": id, "name": node.name, "type": node.kind.type_name(), "index": index, "z": z });
    if !node.visible {
        out["hidden"] = json!(true);
    }
    if !node.children.is_empty() {
        let children = children_json(d, id, left, cut);
        if children.len() < node.children.len() {
            out["childCount"] = json!(node.children.len());
        }
        out["children"] = Value::Array(children);
    }
    Some(out)
}

/// The children of `parent`, topmost first (the Layers panel's order: the last child first), each
/// with its `index` among its siblings and `z`, its place in paint order from the bottom.
fn children_json(d: &Document, parent: NodeId, left: &mut usize, cut: &mut bool) -> Vec<Value> {
    let Some(node) = d.node(parent) else { return Vec::new() };
    let painted: Vec<NodeId> = node.paint_order().collect();
    node.children
        .iter()
        .enumerate()
        .rev()
        .filter_map(|(index, child)| {
            let z = painted.iter().position(|p| p == child).unwrap_or(index);
            layer_json(d, *child, index, z, left, cut)
        })
        .collect()
}

/// `page.inspect`: a page's layers, or one layer's, without going there.
fn list(doc: &mut DesignDocument, params: &Value) -> Result<Value, String> {
    let d: &Document = &doc.ds()?.doc;
    let layer = params
        .get("id")
        .and_then(|value| value.as_u64().or_else(|| value.as_str().and_then(|text| text.trim().parse().ok())))
        .map(NodeId)
        .filter(|id| !d.pages.contains(id) && integer(params, "index").ok().flatten().is_none());
    let (mut left, mut cut) = (MAX_LISTED, false);
    if let Some(id) = layer {
        let node = d.node(id).ok_or_else(|| format!("invalid-params: the design has no page or layer {id}; pages' ids are in inspect's `pages`"))?;
        let page = d
            .page_of(id)
            .filter(|page| d.pages.contains(page))
            .ok_or_else(|| format!("invalid-params: layer {id} is on no page (a library copy)"))?;
        let index = d.pages.iter().position(|p| *p == page).unwrap_or(0);
        let parent = node.parent.and_then(|parent| d.node(parent));
        let place = parent.and_then(|parent| parent.children.iter().position(|c| *c == id)).unwrap_or(0);
        let z = parent.and_then(|parent| parent.paint_order().position(|c| c == id)).unwrap_or(place);
        let listed = layer_json(d, id, place, z, &mut left, &mut cut).into_iter().collect::<Vec<_>>();
        return Ok(json!({ "id": id, "index": index, "name": node.name, "page": page, "layers": listed, "truncated": cut }));
    }
    let (page, index) = named_page(d, params)?;
    let name = d.node(page).map(|node| node.name.clone()).unwrap_or_default();
    let layers = children_json(d, page, &mut left, &mut cut);
    Ok(json!({ "id": page, "index": index, "name": name, "page": page, "layers": layers, "truncated": cut }))
}
