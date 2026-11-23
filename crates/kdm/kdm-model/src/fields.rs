//! The registers of each object type, from and back to the engine's serde form (the same form
//! `document.json` holds, so the replica keeps exactly what a saved file keeps).
//!
//! A node's registers group its fields by coupling (`docs/features/kdm.md` section 2.1): what is edited as
//! one value is one register, so a merge never pairs one writer's position with another's size.
//! A key of the node's serde form that no register names goes into `props`, so a field a newer
//! engine adds is kept. A node's containers are not registers: its paints and effects are objects
//! of its `fills`, `strokes` and `effects` lists ([`crate::items`]), a text layer's characters,
//! styles and paragraphs its `text` field ([`crate::text`]).

use std::collections::BTreeMap;

use kition_design_doc::{Collection, Comment, LibraryLink, Message, Node, NodeId, Style, Variable};
use serde_json::{Map, Value, json};

use crate::{KdmResult, kdm_err};

/// The register fields of one object, as written: `(field, value)`, `Value::Null` for unset.
pub type Fields = Vec<(&'static str, Value)>;

/// Node registers holding one serde key each: `(register, key)`.
const SINGLE: &[(&str, &str)] = &[
    ("name", "name"),
    ("visible", "visible"),
    ("locked", "locked"),
    ("opacity", "opacity"),
    ("stroke", "stroke"),
    ("corners", "corners"),
    ("layout", "layout"),
    ("childLayout", "child_layout"),
    ("constraints", "constraints"),
    // Inside an instance: the main component's layer this one mirrors. Its own register (F18),
    // so a repair that points it at another copy (V12) never rewrites a register a person edits.
    ("mirrors", "source"),
];

/// Node registers holding several serde keys, as one object: `(register, keys)`.
const GROUPS: &[(&str, &[&str])] = &[
    ("frame", &["transform", "width", "height"]),
    ("prototype", &["interactions", "overflow", "scroll_position"]),
    ("bindings", &["styles", "bound", "var_modes"]),
    ("instance", &["overrides", "prop_refs"]),
    ("dev", &["dev_status", "dev_changed", "dev_note", "annotations", "code_links", "dev_resources", "measurements"]),
];

/// The `dev` register's key for the ready-for-dev snapshot (`Document::dev_snapshots`).
pub(crate) const SNAPSHOT: &str = "snapshot";

/// Every register a node object has, in schema order (`key` and `kind` are required).
pub const NODE_REGISTERS: &[&str] = &[
    "name",
    "visible",
    "locked",
    "opacity",
    "kind",
    "frame",
    "stroke",
    "corners",
    "layout",
    "childLayout",
    "constraints",
    "prototype",
    "bindings",
    "instance",
    "mirrors",
    "dev",
    "props",
];

/// Every field of a node object: its registers, then its containers (`text`, and the item lists
/// [`crate::items::LISTS`]).
pub const NODE_FIELDS: &[&str] = &[
    "name",
    "visible",
    "locked",
    "opacity",
    "kind",
    "frame",
    "text",
    "fills",
    "strokes",
    "stroke",
    "effects",
    "corners",
    "layout",
    "childLayout",
    "constraints",
    "prototype",
    "bindings",
    "instance",
    "mirrors",
    "dev",
    "props",
];

/// The item lists, which [`node_from`] also takes in `registers` (each as the array of its items'
/// values) though they are no registers.
const ITEM_FIELDS: [&str; 3] = ["fills", "strokes", "effects"];

/// Keys that are the tree's, never registers.
const STRUCTURE: &[&str] = &["id", "parent", "children"];

fn to_map(v: &impl serde::Serialize) -> KdmResult<Map<String, Value>> {
    match serde_json::to_value(v).map_err(kdm_err)? {
        Value::Object(map) => Ok(map),
        other => Err(kdm_err(format!("expected an object, found {other}"))),
    }
}

fn object_or_null(map: Map<String, Value>) -> Value {
    if map.is_empty() { Value::Null } else { Value::Object(map) }
}

/// A node's registers (without `key`): the serde form of `n` split by coupling. `snapshot` is its
/// ready-for-dev snapshot, if the document keeps one.
pub fn node_fields(n: &Node, snapshot: Option<&Value>) -> KdmResult<Fields> {
    let mut kind = serde_json::to_value(&n.kind).map_err(kdm_err)?;
    let mut map = to_map(n)?;
    if let Value::Object(k) = &mut kind {
        for key in k.keys() {
            map.remove(key);
        }
        // A text's characters, styles and paragraphs are its `text` field.
        if matches!(n.kind, kition_design_doc::NodeKind::Text(_)) {
            for key in crate::text::TEXT_KEYS {
                k.remove(*key);
            }
        }
    }
    for key in STRUCTURE.iter().chain(&ITEM_FIELDS) {
        map.remove(*key);
    }
    // Bindings of an item's fields are the item's (`Paint.bound`, `Effect.bound`).
    if let Some(Value::Object(bound)) = map.get_mut("bound") {
        bound.retain(|key, _| crate::items::item_key(n, key).is_none());
        if bound.is_empty() {
            map.remove("bound");
        }
    }
    let mut out: Fields = Vec::with_capacity(NODE_REGISTERS.len());
    for (register, key) in SINGLE {
        out.push((register, map.remove(*key).unwrap_or(Value::Null)));
    }
    out.push(("kind", kind));
    for (register, keys) in GROUPS {
        let mut group = Map::new();
        for key in *keys {
            if let Some(v) = map.remove(*key) {
                group.insert((*key).to_owned(), v);
            }
        }
        if *register == "dev"
            && let Some(s) = snapshot
        {
            group.insert(SNAPSHOT.to_owned(), s.clone());
        }
        out.push((register, object_or_null(group)));
    }
    out.push(("props", object_or_null(map)));
    Ok(out)
}

/// The fields of a node object (its registers, its `text` and its item lists) that differ
/// between `a` and `b`.
pub fn changed_fields(a: &Node, b: &Node) -> KdmResult<Vec<&'static str>> {
    let (fa, fb) = (node_fields(a, None)?, node_fields(b, None)?);
    let mut out: Vec<&'static str> = fa.iter().zip(fb.iter()).filter(|(x, y)| x != y).map(|(x, _)| x.0).collect();
    let differ = match (&a.kind, &b.kind) {
        (kition_design_doc::NodeKind::Text(x), kition_design_doc::NodeKind::Text(y)) => !crate::text::same_field(x, y),
        (kition_design_doc::NodeKind::Text(_), _) | (_, kition_design_doc::NodeKind::Text(_)) => true,
        _ => false,
    };
    if differ {
        out.push("text");
    }
    for (field, _, _) in crate::items::LISTS {
        if crate::items::items(a, field)? != crate::items::items(b, field)? {
            out.push(field);
        }
    }
    Ok(out)
}

/// A node from its key, its registers (by name; a missing one reads as unset) and its place in
/// the tree. Answers the node and its ready-for-dev snapshot.
pub fn node_from(key: NodeId, registers: &BTreeMap<&str, Value>, parent: Option<NodeId>, children: Vec<NodeId>) -> KdmResult<(Node, Option<Value>)> {
    let mut map = Map::new();
    for (register, key) in SINGLE {
        if let Some(v) = registers.get(register).filter(|v| !v.is_null()) {
            map.insert((*key).to_owned(), v.clone());
        }
    }
    for field in ITEM_FIELDS {
        if let Some(v) = registers.get(field).filter(|v| !v.is_null()) {
            map.insert(field.to_owned(), v.clone());
        }
    }
    let mut snapshot = None;
    for (register, _) in GROUPS {
        if let Some(Value::Object(group)) = registers.get(register) {
            for (k, v) in group {
                if *register == "dev" && k == SNAPSHOT {
                    snapshot = Some(v.clone());
                } else {
                    map.insert(k.clone(), v.clone());
                }
            }
        }
    }
    if let Some(Value::Object(props)) = registers.get("props") {
        for (k, v) in props {
            map.entry(k.clone()).or_insert_with(|| v.clone());
        }
    }
    match registers.get("kind") {
        Some(Value::Object(kind)) => {
            for (k, v) in kind {
                map.insert(k.clone(), v.clone());
            }
        }
        _ => return Err(kdm_err(format!("node {key} has no kind"))),
    }
    map.entry("name").or_insert_with(|| Value::String(String::new()));
    map.entry("width").or_insert(json!(0.0));
    map.entry("height").or_insert(json!(0.0));
    map.insert("id".into(), json!(key.0));
    if let Some(p) = parent {
        map.insert("parent".into(), json!(p.0));
    }
    map.insert("children".into(), Value::Array(children.iter().map(|c| json!(c.0)).collect()));
    let node: Node = serde_json::from_value(Value::Object(map)).map_err(|e| kdm_err(format!("node {key}: {e}")))?;
    Ok((node, snapshot))
}

/// [`node_from`] for registers any peer may have written (they are `json`, so the KDM takes any
/// value): when they do not read as a node, an unreadable `kind` reads as a frame (which holds
/// any children) and every other register that does not read with the ones before it (in
/// [`NODE_REGISTERS`] order) is left unset. Answers the registers it left out as well, so a
/// writer can leave them as they are. The result depends on the registers alone: every replica
/// reads the same node.
pub fn node_from_lenient(
    key: NodeId,
    registers: &BTreeMap<&str, Value>,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
) -> KdmResult<(Node, Option<Value>, Vec<&'static str>)> {
    if let Ok((node, snapshot)) = node_from(key, registers, parent, children.clone()) {
        return Ok((node, snapshot, Vec::new()));
    }
    let mut kept: BTreeMap<&str, Value> = BTreeMap::new();
    let mut dropped: Vec<&'static str> = Vec::new();
    let kind = registers.get("kind").cloned().unwrap_or(Value::Null);
    kept.insert("kind", kind);
    if node_from(key, &kept, parent, Vec::new()).is_err() {
        kept.insert("kind", serde_json::to_value(kition_design_doc::NodeKind::Frame).map_err(kdm_err)?);
        dropped.push("kind");
    }
    for register in NODE_REGISTERS.iter().chain(&ITEM_FIELDS).copied().filter(|r| *r != "kind") {
        let Some(value) = registers.get(register).filter(|v| !v.is_null()) else { continue };
        kept.insert(register, value.clone());
        if node_from(key, &kept, parent, Vec::new()).is_err() {
            kept.remove(register);
            dropped.push(register);
        }
    }
    let (node, snapshot) = node_from(key, &kept, parent, children)?;
    Ok((node, snapshot, dropped))
}

/// A library link's registers (its `key` is the library id).
pub fn library_fields(l: &LibraryLink) -> Fields {
    vec![("name", json!(l.name)), ("path", json!(l.path)), ("version", json!(l.version))]
}

/// A library link from its registers.
pub fn library_from(key: &str, registers: &BTreeMap<&str, Value>) -> LibraryLink {
    let text = |name: &str| registers.get(name).and_then(Value::as_str).unwrap_or_default().to_owned();
    LibraryLink { id: key.to_owned(), name: text("name"), path: text("path"), version: registers.get("version").and_then(Value::as_u64).unwrap_or(0) }
}

/// A style's registers (without `key`): `name`, `value` (its type and data), `props` (the rest).
pub fn style_fields(s: &Style) -> KdmResult<Fields> {
    let value = serde_json::to_value(&s.kind).map_err(kdm_err)?;
    let mut map = to_map(s)?;
    if let Value::Object(k) = &value {
        for key in k.keys() {
            map.remove(key);
        }
    }
    map.remove("id");
    let name = map.remove("name").unwrap_or(Value::Null);
    Ok(vec![("name", name), ("value", value), ("props", object_or_null(map))])
}

pub fn style_from(key: &str, registers: &BTreeMap<&str, Value>) -> KdmResult<Style> {
    let mut map = Map::new();
    if let Some(Value::Object(props)) = registers.get("props") {
        map.extend(props.clone());
    }
    if let Some(Value::Object(value)) = registers.get("value") {
        map.extend(value.clone());
    }
    map.insert("id".into(), json!(key));
    map.insert("name".into(), registers.get("name").filter(|v| v.is_string()).cloned().unwrap_or_else(|| json!("")));
    serde_json::from_value(Value::Object(map)).map_err(|e| kdm_err(format!("style {key}: {e}")))
}

/// A collection's registers (without `key` and its modes): `name`, `props` (the rest).
pub fn collection_fields(c: &Collection) -> KdmResult<Fields> {
    let mut map = to_map(c)?;
    for key in ["id", "modes"] {
        map.remove(key);
    }
    let name = map.remove("name").unwrap_or(Value::Null);
    Ok(vec![("name", name), ("props", object_or_null(map))])
}

/// A collection from its key, registers and modes (`(id, name)` in order).
pub fn collection_from(key: &str, registers: &BTreeMap<&str, Value>, modes: Vec<(String, String)>) -> KdmResult<Collection> {
    let mut map = Map::new();
    if let Some(Value::Object(props)) = registers.get("props") {
        map.extend(props.clone());
    }
    map.insert("id".into(), json!(key));
    map.insert("name".into(), registers.get("name").filter(|v| v.is_string()).cloned().unwrap_or_else(|| json!("")));
    map.insert("modes".into(), Value::Array(modes.into_iter().map(|(id, name)| json!({ "id": id, "name": name })).collect()));
    serde_json::from_value(Value::Object(map)).map_err(|e| kdm_err(format!("collection {key}: {e}")))
}

/// A variable's registers (without `key` and its values): `name`, `collection`, `type`, `props`.
/// Its values (by mode id) are the `values` map, [`variable_values`].
pub fn variable_fields(v: &Variable) -> KdmResult<Fields> {
    let mut map = to_map(v)?;
    for key in ["id", "values"] {
        map.remove(key);
    }
    let name = map.remove("name").unwrap_or(Value::Null);
    let collection = map.remove("collection").unwrap_or(Value::Null);
    let ty = map.remove("resolvedType").unwrap_or(Value::Null);
    Ok(vec![("name", name), ("collection", collection), ("type", ty), ("props", object_or_null(map))])
}

/// A variable's values by mode id, as the `values` map holds them.
pub fn variable_values(v: &Variable) -> KdmResult<BTreeMap<String, Value>> {
    v.values.iter().map(|(mode, value)| Ok((mode.clone(), serde_json::to_value(value).map_err(kdm_err)?))).collect()
}

pub fn variable_from(key: &str, registers: &BTreeMap<&str, Value>, values: Vec<(String, Value)>) -> KdmResult<Variable> {
    let mut map = Map::new();
    if let Some(Value::Object(props)) = registers.get("props") {
        map.extend(props.clone());
    }
    map.insert("id".into(), json!(key));
    let text = |field: &str| registers.get(field).filter(|v| v.is_string()).cloned().unwrap_or_else(|| json!(""));
    map.insert("name".into(), text("name"));
    map.insert("collection".into(), text("collection"));
    map.insert("resolvedType".into(), registers.get("type").filter(|v| v.is_string()).cloned().unwrap_or_else(|| json!("string")));
    map.insert("values".into(), Value::Object(values.into_iter().collect()));
    serde_json::from_value(Value::Object(map)).map_err(|e| kdm_err(format!("variable {key}: {e}")))
}

/// Unix seconds as ISO 8601 UTC (`created`, KitModel M23); outside the calendar's range, the
/// decimal seconds.
pub fn iso_time(seconds: u64) -> String {
    i64::try_from(seconds)
        .ok()
        .and_then(|s| chrono::DateTime::<chrono::Utc>::from_timestamp(s, 0))
        .map_or_else(|| seconds.to_string(), |t| t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
}

/// [`iso_time`] back to Unix seconds (0 when it is neither form).
pub fn unix_time(text: &str) -> u64 {
    if let Ok(seconds) = text.parse::<u64>() {
        return seconds;
    }
    chrono::DateTime::parse_from_rfc3339(text).ok().and_then(|t| u64::try_from(t.timestamp()).ok()).unwrap_or(0)
}

/// A comment thread's anchor (KitModel M23): the object it is about (the anchor layer's, else
/// the page's) and design's own keys: the thread's `id`, its `page`, the pin's `x` and `y` and the
/// anchor layer (`node`). Who has read a thread is each person's own (`Comment.read_by`): never
/// replicated ([`crate::read`] keeps the view's).
pub fn thread_anchor(c: &Comment, object: &str) -> Value {
    let mut anchor = json!({ "object": object, "id": c.id, "page": c.page.0, "x": c.x, "y": c.y });
    if let Some(a) = c.anchor {
        anchor["node"] = json!(a.0);
    }
    anchor
}

/// One message's registers (without `body`, a text field).
pub fn message_fields(m: &Message) -> KdmResult<Fields> {
    let reactions = if m.reactions.is_empty() { Value::Null } else { serde_json::to_value(&m.reactions).map_err(kdm_err)? };
    let edited = if m.edited == 0 { Value::Null } else { json!(m.edited) };
    let key = if m.id == 0 { Value::Null } else { json!(m.id) };
    Ok(vec![("author", json!(m.author)), ("created", json!(iso_time(m.created))), ("edited", edited), ("reactions", reactions), ("key", key)])
}

pub fn message_from(registers: &BTreeMap<&str, Value>, body: String) -> Message {
    let text = |field: &str| registers.get(field).and_then(Value::as_str).unwrap_or_default().to_owned();
    Message {
        id: registers.get("key").and_then(Value::as_u64).unwrap_or(0),
        author: text("author"),
        text: body,
        created: unix_time(&text("created")),
        edited: registers.get("edited").and_then(Value::as_u64).unwrap_or(0),
        reactions: registers.get("reactions").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default(),
    }
}

/// A thread from its anchor, resolved flag and messages (the first one, then the replies). Its
/// read state is unknown here (`None`): the reader takes it from the view.
pub fn comment_from(anchor: &Value, resolved: bool, mut messages: Vec<Message>) -> Option<Comment> {
    if messages.is_empty() {
        return None;
    }
    let first = messages.remove(0);
    let num = |key: &str| anchor.get(key).and_then(Value::as_f64).unwrap_or(0.0);
    Some(Comment {
        id: anchor.get("id").and_then(Value::as_u64)?,
        page: NodeId(anchor.get("page").and_then(Value::as_u64)?),
        x: num("x"),
        y: num("y"),
        anchor: anchor.get("node").and_then(Value::as_u64).map(NodeId),
        first,
        resolved,
        replies: messages,
        read_by: None,
    })
}
