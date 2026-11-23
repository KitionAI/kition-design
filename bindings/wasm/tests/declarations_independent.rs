//! The independent reading of every write command (task card C8-design-read). Of the 372 commands
//! the kit offers that change a design on [`fixture`] (345, and the 27 tool switches that end a
//! vector edit), eight readers each read a family group's handlers and the engine operations
//! behind them (`bindings/wasm/src/lib.rs` dispatch, `context.rs`, `crates/engine/**`), before
//! the declarations (`src/place.rs`, `src/acts.rs`, `src/addressing.rs`, `src/params.rs`,
//! `src/agent_text.rs`, `KitDocument::under`) or their tests were opened: which parameters each
//! resolves and how, what it reads of the design and of the editor, what it writes. The original
//! 799 rows were written from that reading before comparison; none was pinned. Corrections
//! after handler and behavior review say `Reviewed after behavior tests` beside each changed
//! row or name the reviewed family constant. Added text-creation, nested-batch, selection-prefix
//! and read-result batch rows are explicitly pinned after review. The original readers
//! marked 19 rows `UNSURE:`. Rows on board files run on [`board`]. The four `widget.*` commands were read but
//! cannot run on any fixture (no command the kit offers installs a widget plugin): they are not
//! compared ([`NOT_COMPARED`]). One row cannot run and is compared without running (`n`):
//! `file.colorProfile` converting colours (refused in any file holding a library copy). The two
//! `proto.interaction.set` rows with `interaction` could not run either until the binding's schema
//! named it (fixed by this card, `tests/read_defects.rs`).
//!
//! A row: the steps run before it (`before`, what a step made is `$S1`, `$S2`...), the command,
//! its parameters (roles in place of ids), `on` (the step's objects: the selection for the step, or
//! the command's target parameter), the editor state it reads (`selection`, `part`, `mode`), and
//! for each object what the write rests on: its fields (`*` every field, `-` none: only that it is
//! there; registers `name`, `fills`, `frame`, `layout`, `childLayout`, `kind`, `bindings.bound`,
//! `props.<key>`..., derived `~bounds`, `~hierarchy`; the root `design`'s `settings.<key>`, lists and
//! derived values) and whether its place in the layer tree matters. Besides objects:
//! `slot:$F` (a place among `$F`'s children that an index names) and `picks` (what is under a
//! point, told by asking the kit). What the guard holds is computed as it does ([`held`]): the
//! step's `on` and the command's target parameter (every field, and every position unless the
//! command is position independent), the objects its object and name parameters name (every
//! field and position), replaced object by object by `under`'s premises; `frame` is a position
//! field, held only through the positions.
//!
//! Before this card (C8-design-3, C8-expectations review), 360 of the 368 compared commands
//! differed from their reading (98%): 208 held less than their handlers read (a write that can
//! land on what the agent did not observe), 343 held more (a refusal no change needs), 191 both.
//! By class (a command may be in several):
//! - a read that is not held: another object the handler reads (the auto layout parent and the
//!   siblings it re-flows, a group refitting, ancestors' transforms, the main component an
//!   instance copies, the variable or style a bound value follows, a comment pinned to a moved
//!   layer, what undo takes back), 129; a field or the place of a named object, 46;
//! - a default taken from elsewhere: the current page a new layer or a paste lands on, 28;
//! - editor state read but not declared (the tool, the clipboard, preferences, the edit modes,
//!   the selection), 33;
//! - a place not held (an index among a parent's children, a point's picks), 7;
//! - a value naming another object, not held (the steps of `command.batch`, the targets of
//!   `selection.setPaint`), 6;
//! - held more: fields the call does not read, 314; where an object is, for a call that does not
//!   depend on it, 195; objects the call does not read, 153; editor state declared and not
//!   read, 109.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use kition_design_kit::DesignDocument;
use kition_kit::{CallContext, EditorState, KitCommand, KitDocument};
use serde_json::{Value, json};

/// A saved design and its roles (each a layer id or a resource's key).
type Fixture = (Vec<u8>, Vec<(&'static str, Value)>);

/// The library file the design links (lent as the host lends it): component `Primary`, paint
/// style `Lib Blue`, collection `Lib Tokens` (mode `Base`) with the number variable `space`.
const LIBRARY: &str = "libraries/Buttons.kdesign";

/// A 2 by 2 PNG.
const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAFElEQVR4nGP8z8DwnwEJMDGgAQBVBgICyX5rtAAAAABJRU5ErkJggg==";

fn library_bytes() -> &'static [u8] {
    static BYTES: OnceLock<Vec<u8>> = OnceLock::new();
    BYTES.get_or_init(|| {
        let mut lib = DesignDocument::create("Buttons.kdesign", 320, 240).unwrap();
        let mut ex = |c: &str, p: Value| lib.execute(c, p.clone()).unwrap_or_else(|e| panic!("library {c} {p}: {e}"));
        let f = ex("frame.create", json!({ "x": 0, "y": 0, "width": 80, "height": 32, "name": "Primary" }))["id"].clone();
        ex("component.create", json!({ "ids": [f] }));
        ex("style.create", json!({ "type": "paint", "name": "Lib Blue", "value": [{ "type": "solid", "color": "#0000ff" }] }));
        let col = ex("variable.createCollection", json!({ "name": "Lib Tokens", "modes": ["Base"] }))["id"].clone();
        ex("variable.create", json!({ "collection": col, "name": "space", "type": "number", "value": 8 }));
        ex("library.publish", json!({}));
        lib.save(None).unwrap()
    })
}

fn id(answer: &Value) -> Value {
    answer.get("id").or_else(|| answer.get("ids").and_then(|ids| ids.get(0))).cloned().unwrap_or_else(|| panic!("no id in {answer}"))
}

/// A second library file, lent but not linked: component `Primary` (wider), paint style
/// `Lib Blue`.
const OTHER: &str = "libraries/Other.kdesign";

fn other_bytes() -> &'static [u8] {
    static BYTES: OnceLock<Vec<u8>> = OnceLock::new();
    BYTES.get_or_init(|| {
        let mut lib = DesignDocument::create("Other.kdesign", 320, 240).unwrap();
        let mut ex = |c: &str, p: Value| lib.execute(c, p.clone()).unwrap_or_else(|e| panic!("other library {c} {p}: {e}"));
        let f = ex("frame.create", json!({ "x": 0, "y": 0, "width": 120, "height": 32, "name": "Primary" }))["id"].clone();
        ex("component.create", json!({ "ids": [f] }));
        ex("style.create", json!({ "type": "paint", "name": "Lib Blue", "value": [{ "type": "solid", "color": "#0000aa" }] }));
        ex("library.publish", json!({}));
        lib.save(None).unwrap()
    })
}

fn open(bytes: &[u8]) -> DesignDocument {
    let mut doc = DesignDocument::open("Design.kdesign", bytes).unwrap();
    doc.lend_library(LIBRARY, Some(library_bytes())).unwrap();
    doc.lend_library(OTHER, Some(other_bytes())).unwrap();
    doc
}

/// The design every row is read on, saved and opened again: page `$P1` "Page 1" current, nothing
/// selected, the move tool, no edit mode. Page 1's layers, bottom first:
/// - `$D` "Frame 1" (1440 by 1024 at 0, 0: the frame a new design has), below every other
///   top-level layer, which sit beside it on the page; `$LI` is inside it;
/// - `$R` rectangle "Box" (0, 0, 100 by 60): its fill bound to `$Primary`, a prototype
///   interaction (on click, navigate to `$X`), comment thread `$Th` pinned to it;
/// - `$E` ellipse (0, 100, 40 by 40), fill style `$Brand`;
/// - `$F` frame "Card" (200, 0, 200 by 300), vertical auto layout, holding `$A` (rectangle 50 by
///   20, an image fill of `$Img` on top of its fill) and `$T` (text "Hello");
/// - `$G` group "Pair" of `$G1` (rectangle at 0, 200) and `$G2` (ellipse at 40, 200);
/// - `$V` vector (an open path M0 0 L40 0 L40 40 at 0, 300);
/// - `$K` main component "Button" (500, 0, 120 by 40) holding text `$KT` "OK", bound to its
///   text property `Label`; `$I` an instance of it (500, 100), its text `$IT`;
/// - `$X` frame "Screen" (800, 0, 375 by 600), the start of prototype flow "Flow 1";
/// - `$S` section "Area" (0, 700, 300 by 200), empty;
/// - `$Set` component set of variants `$VS` "Size=S" and `$VL` "Size=L" (near 1200, 0);
/// - `$LI` an instance of the library's component "Primary", whose local copy `$LK` is in the
///   library store.
///
/// Page `$P2` "Page 2" is empty. Paint style `$Brand` "Brand"; collection `$Colors` "Colors" with
/// modes `$Light` and `$Dark`, colour variables `$Primary` "primary" and `$Accent` "accent" (an
/// alias of `primary`); library link `$Lib` "Buttons" (its paint style `$LibBlue` and collection
/// `$LibTokens` not imported); image `$Img`.
pub fn fixture() -> &'static Fixture {
    static FIXTURE: OnceLock<Fixture> = OnceLock::new();
    FIXTURE.get_or_init(build)
}

fn build() -> Fixture {
    let mut d = DesignDocument::create("Design.kdesign", 1440, 1024).unwrap();
    d.lend_library(LIBRARY, Some(library_bytes())).unwrap();
    let default_frame = d.inspect().unwrap()["tree"][0]["id"].clone();
    let mut ex = |c: &str, p: Value| d.execute(c, p.clone()).unwrap_or_else(|e| panic!("{c} {p}: {e}"));
    let r = id(&ex("shape.rectangle", json!({ "x": 0, "y": 0, "width": 100, "height": 60, "props": { "name": "Box" } })));
    let e = id(&ex("shape.ellipse", json!({ "x": 0, "y": 100, "width": 40, "height": 40 })));
    let f = id(&ex("frame.create", json!({ "x": 200, "y": 0, "width": 200, "height": 300, "name": "Card" })));
    ex("layout.add", json!({ "ids": [f], "direction": "vertical" }));
    let a = id(&ex("shape.rectangle", json!({ "x": 0, "y": 0, "width": 50, "height": 20, "parent": f })));
    let t = id(&ex("text.create", json!({ "text": "Hello", "x": 0, "y": 0, "parent": f })));
    let g1 = id(&ex("shape.rectangle", json!({ "x": 0, "y": 200, "width": 30, "height": 30 })));
    let g2 = id(&ex("shape.ellipse", json!({ "x": 40, "y": 200, "width": 30, "height": 30 })));
    let g = id(&ex("object.group", json!({ "ids": [g1, g2], "name": "Pair" })));
    let v = id(&ex("vector.create", json!({ "path": "M0 0 L40 0 L40 40", "x": 0, "y": 300 })));
    let kf = id(&ex("frame.create", json!({ "x": 500, "y": 0, "width": 120, "height": 40, "name": "Button" })));
    let kt = id(&ex("text.create", json!({ "text": "OK", "x": 10, "y": 10, "parent": kf })));
    let k = id(&ex("component.create", json!({ "ids": [kf] })));
    ex("component.addProperty", json!({ "id": kt, "name": "Label", "type": "text" }));
    let i = id(&ex("instance.create", json!({ "component": k, "x": 500, "y": 100 })));
    let x = id(&ex("frame.create", json!({ "x": 800, "y": 0, "width": 375, "height": 600, "name": "Screen" })));
    let sf = id(&ex("frame.create", json!({ "x": 0, "y": 700, "width": 300, "height": 200, "name": "Area" })));
    let s = id(&ex("object.convertToSection", json!({ "ids": [sf] })));
    let vs = id(&ex("frame.create", json!({ "x": 1200, "y": 0, "width": 40, "height": 20, "name": "Size=S" })));
    let vl = id(&ex("frame.create", json!({ "x": 1260, "y": 0, "width": 80, "height": 20, "name": "Size=L" })));
    ex("component.create", json!({ "ids": [vs] }));
    ex("component.create", json!({ "ids": [vl] }));
    let set = id(&ex("component.combineVariants", json!({ "ids": [vs, vl] })));
    let brand = id(&ex("style.create", json!({ "type": "paint", "name": "Brand", "value": [{ "type": "solid", "color": "#ff0000" }] })));
    ex("style.apply", json!({ "ids": [e], "style": brand }));
    let colors = id(&ex("variable.createCollection", json!({ "name": "Colors", "modes": ["Light", "Dark"] })));
    let primary = id(&ex("variable.create", json!({ "collection": colors, "name": "primary", "type": "color", "value": "#00ff00" })));
    let accent = id(&ex("variable.create", json!({ "collection": colors, "name": "accent", "type": "color", "value": "#0000ff" })));
    ex("variable.set", json!({ "variable": accent, "value": { "alias": primary } }));
    ex("variable.bind", json!({ "ids": [r], "field": "fill", "variable": primary }));
    ex("proto.interaction.add", json!({ "id": r, "dest": x }));
    ex("proto.flow.add", json!({ "start": x, "name": "Flow 1" }));
    let thread = id(&ex("comment.add", json!({ "x": 10, "y": 10, "text": "Look", "anchor": r })));
    let lib = ex("library.add", json!({ "path": LIBRARY }));
    let lib_id = lib["id"].clone();
    let lib_blue = lib["styles"][0]["id"].clone();
    let lib_tokens = lib["collections"][0]["id"].clone();
    let li = id(&ex("library.place", json!({ "library": lib_id, "component": "Primary", "x": 1000, "y": 800 })));
    let img = ex("image.place", json!({ "data": PNG, "target": a }))["image"].clone();
    let p2 = id(&ex("page.add", json!({ "name": "Page 2" })));
    ex("page.select", json!({ "index": 0 }));
    ex("select.set", json!({ "ids": [] }));
    let bytes = d.save(None).unwrap();
    let doc = open(&bytes);
    let ds = doc.session().ds().unwrap();
    let node = |id: &Value| ds.doc.node(kition_design_doc::NodeId(id.as_u64().unwrap())).unwrap();
    let it = json!(node(&i).children[0].0);
    let lk = match &node(&li).kind {
        kition_design_doc::NodeKind::Instance { component, .. } => json!(component.0),
        other => panic!("{other:?}"),
    };
    let modes: Vec<Value> = ds.doc.variables.collections.iter().find(|c| json!(c.id) == colors).unwrap().modes.iter().map(|m| json!(m.id)).collect();
    let roles = vec![
        ("$P1", json!(1)),
        ("$P2", p2),
        ("$D", default_frame),
        ("$R", r),
        ("$E", e),
        ("$F", f),
        ("$A", a),
        ("$T", t),
        ("$G", g),
        ("$G1", g1),
        ("$G2", g2),
        ("$V", v),
        ("$K", k),
        ("$KT", kt),
        ("$I", i),
        ("$IT", it),
        ("$X", x),
        ("$S", s),
        ("$Set", set),
        ("$VS", vs),
        ("$VL", vl),
        ("$LI", li),
        ("$LK", lk),
        ("$Brand", brand),
        ("$Colors", colors),
        ("$Light", modes[0].clone()),
        ("$Dark", modes[1].clone()),
        ("$Primary", primary),
        ("$Accent", accent),
        ("$Th", thread),
        ("$Lib", lib_id),
        ("$LibBlue", lib_blue),
        ("$LibTokens", lib_tokens),
        ("$Img", img),
    ];
    (bytes, roles)
}

/// `@FIG` in a row: base64 of a `.fig` file of one rectangle (copied design layers).
fn fig_layers() -> &'static str {
    static FIG: OnceLock<String> = OnceLock::new();
    FIG.get_or_init(|| {
        let mut session = kition_design_engine::Session::with_document(kition_design_doc::Document::new("Copied"));
        session.execute("shape.rectangle", &json!({ "x": 0, "y": 0, "width": 20, "height": 20 })).unwrap();
        let (bytes, _) = kition_design_format::fig::export(session.doc().unwrap(), None).unwrap();
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes)
    })
}

/// `@var:<collection role>:<name>` in `text`: the variable of that name in the collection (one a
/// step made), as a role.
fn var_roles(doc: &DesignDocument, text: &str, roles: &mut Vec<(&'static str, Value)>) {
    let Ok(ds) = doc.session().ds() else { return };
    for text in text.split('"').filter(|t| t.starts_with("@var:")) {
        let mut parts = text.splitn(3, ':').skip(1);
        let (collection, name) = (parts.next().unwrap_or_default(), parts.next().unwrap_or_default());
        let collection = roles.iter().find(|(role, _)| *role == collection).map(|(_, id)| id.clone());
        if let Some(variable) = ds.doc.variables.variables.iter().find(|v| Some(json!(v.collection)) == collection && v.name == name) {
            roles.push((Box::leak(text.to_owned().into_boxed_str()), json!(variable.id)));
            if let Some(parent) = &variable.extends {
                roles.push((Box::leak(format!("@parent:{text}").into_boxed_str()), json!(parent)));
            }
        }
    }
}

/// The roles of what `before` steps made, by the step's place.
const STEP_ROLES: [&str; 9] = ["$S1", "$S2", "$S3", "$S4", "$S5", "$S6", "$S7", "$S8", "$S9"];

/// A generated instance layer whose source is named by a step: `@copy:<instance>:<source>`.
fn copy_roles(doc: &DesignDocument, row: &Row, roles: &mut Vec<(&'static str, Value)>) {
    let Ok(ds) = doc.session().ds() else { return };
    for (role, _, _) in row.holds {
        let Some((instance, source)) = role.strip_prefix("@copy:").and_then(|s| s.split_once(':')) else { continue };
        let find = |name| roles.iter().find(|(role, _)| *role == name).and_then(|(_, value)| value.as_u64()).map(kition_design_doc::NodeId);
        let (Some(instance), Some(source)) = (find(instance), find(source)) else { continue };
        // review: source identity alone is not an alias for the first
        // matching copy when several mirrors exist. Require the fixture to disambiguate.
        let copies: Vec<_> = ds.doc.subtree(instance).into_iter().filter(|id| ds.doc.node(*id).is_some_and(|n| n.source == Some(source))).collect();
        match copies.as_slice() {
            [copy] => roles.push((*role, json!(copy.0))),
            _ => panic!("{role} must identify exactly one generated copy, found {}", copies.len()),
        }
    }
}

/// One code path of a command: the steps run before it (`before`, a JSON list of
/// `[command, params]`, roles in place of ids: an edit mode entered, a clipboard filled), its
/// parameters (roles in place of ids), the step's `on` (the selection for the step, or its target
/// parameter), the editor state it reads, and what a write with `basis` rests on: (object, fields,
/// whether where it is matters).
pub struct Row {
    before: &'static str,
    command: &'static str,
    params: &'static str,
    on: &'static [&'static str],
    editor: &'static str,
    holds: &'static [(&'static str, &'static str, bool)],
    /// Not written blind from the reading: pinned from the declaration after review.
    pinned: bool,
    /// Read on [`board`] rather than [`fixture`].
    board: bool,
    /// The command cannot run on the fixture (said in the comment before the row): compared
    /// without running it.
    runs: bool,
}

/// A row written from the reading, before it was compared with the declarations.
const fn r(
    command: &'static str,
    params: &'static str,
    on: &'static [&'static str],
    editor: &'static str,
    holds: &'static [(&'static str, &'static str, bool)],
) -> Row {
    Row { before: "", command, params, on, editor, holds, pinned: false, board: false, runs: true }
}

/// A row written from the reading, after `before` steps.
const fn s(
    before: &'static str,
    command: &'static str,
    params: &'static str,
    on: &'static [&'static str],
    editor: &'static str,
    holds: &'static [(&'static str, &'static str, bool)],
) -> Row {
    Row { before, command, params, on, editor, holds, pinned: false, board: false, runs: true }
}

/// A row pinned from the declaration after review (the reason in the comment before it).
#[allow(dead_code)]
const fn p(
    before: &'static str,
    command: &'static str,
    params: &'static str,
    on: &'static [&'static str],
    editor: &'static str,
    holds: &'static [(&'static str, &'static str, bool)],
) -> Row {
    Row { before, command, params, on, editor, holds, pinned: true, board: false, runs: true }
}

/// A row written from the reading of a path that cannot run on the fixture (the reason in the
/// comment before it): compared without running.
const fn n(
    before: &'static str,
    command: &'static str,
    params: &'static str,
    on: &'static [&'static str],
    editor: &'static str,
    holds: &'static [(&'static str, &'static str, bool)],
) -> Row {
    Row { before, command, params, on, editor, holds, pinned: false, board: false, runs: false }
}

/// A row written from the reading, read on the [`board`] after `before` steps.
const fn b(
    before: &'static str,
    command: &'static str,
    params: &'static str,
    on: &'static [&'static str],
    editor: &'static str,
    holds: &'static [(&'static str, &'static str, bool)],
) -> Row {
    Row { before, command, params, on, editor, holds, pinned: false, board: true, runs: true }
}

fn with_roles(value: &Value, roles: &[(&str, Value)]) -> Value {
    match value {
        Value::String(text) => roles.iter().find(|(role, _)| role == text).map_or_else(|| value.clone(), |(_, id)| id.clone()),
        Value::Array(items) => Value::Array(items.iter().map(|item| with_roles(item, roles)).collect()),
        Value::Object(fields) => Value::Object(fields.iter().map(|(key, item)| (key.clone(), with_roles(item, roles))).collect()),
        _ => value.clone(),
    }
}

/// The values a declared object parameter (`a.b`, `items[].c`) holds in `params`.
fn values_at(params: &Value, path: &str) -> Vec<Value> {
    let mut current = vec![params.clone()];
    for part in path.split('.') {
        let key = part.trim_end_matches("[]");
        let mut next = Vec::new();
        for value in &current {
            match value.get(key) {
                Some(Value::Array(items)) if part.ends_with("[]") => next.extend(items.iter().cloned()),
                Some(item) => next.push(item.clone()),
                None => {}
            }
        }
        current = next;
    }
    current.into_iter().flat_map(|value| if let Value::Array(items) = value { items } else { vec![value] }).collect()
}

/// Whether `id` names a layer (a number), not a style, variable, library or the root.
fn is_layer(id: &Value) -> bool {
    id.as_u64().is_some() || id.as_str().is_some_and(|text| !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()))
}

/// The fields a need holds, as the guard reads them: `frame` is a position field, so it is held
/// only through the positions (never through the fields); every register of a layer is every
/// field.
fn held_fields(fields: Option<&Vec<String>>, positions: Option<&Vec<String>>, layer: bool) -> Option<BTreeSet<String>> {
    let frame = positions.is_none_or(|positions| positions.iter().any(|p| p == "frame" || p.starts_with("frame.")));
    let mut held: BTreeSet<String> = match fields {
        None if frame || !layer => return None,
        None => kition_design_kdm::fields::NODE_FIELDS.iter().map(|r| (*r).to_owned()).collect(),
        Some(fields) => fields.iter().cloned().collect(),
    };
    if layer {
        held.retain(|field| field != "frame" && !field.starts_with("frame."));
        // These declared facts hold exactly the corresponding stored frame components.
        if held.remove("~transform") {
            held.insert("frame.transform".to_owned());
        }
        for dimension in ["width", "height"] {
            if held.remove(&format!("~{dimension}")) {
                held.insert(format!("frame.{dimension}"));
            }
        }
        for (fact, input) in [("boundVariables", "bound"), ("styleLinks", "styles")] {
            if held.remove(&format!("~{fact}")) {
                held.insert(format!("bindings.{input}"));
            }
        }
        if frame {
            held.insert("frame".to_owned());
        }
    }
    if layer && kition_design_kdm::fields::NODE_FIELDS.iter().all(|r| held.contains(*r)) {
        return None;
    }
    Some(held)
}

/// A field list as a set: `*` every field (`None`), `-` none.
fn field_set(fields: &str) -> Option<BTreeSet<String>> {
    match fields.trim() {
        "*" => None,
        "-" | "" => Some(BTreeSet::new()),
        list => Some(list.split(',').map(|field| field.trim().to_owned()).collect()),
    }
}

type Holds = BTreeMap<String, (Option<BTreeSet<String>>, bool)>;

/// The role an id or key names (`design`: the design's root), else the value itself.
fn name_of(value: &Value, roles: &[(&'static str, Value)]) -> String {
    let number = value.as_str().and_then(|text| text.parse::<u64>().ok()).map(|n| json!(n));
    let value = number.as_ref().unwrap_or(value);
    roles
        .iter()
        .find(|(_, id)| id == value)
        .map_or_else(|| value.as_str().map_or_else(|| value.to_string(), str::to_owned), |(role, _)| (*role).to_owned())
}

/// The object a name names, among the types a parameter declares: layers, styles, variables,
/// collections, libraries.
fn by_name(doc: &DesignDocument, name: &str, types: &[&str]) -> Option<Value> {
    let ds = doc.session().ds().ok()?;
    let any = types.is_empty();
    let wants = |t: &str| any || types.contains(&t);
    if wants("Node")
        && let Some(node) = ds.doc.nodes().find(|n| n.name == name)
    {
        return Some(json!(node.id.0));
    }
    if wants("Style")
        && let Some(style) = ds.doc.styles.iter().find(|s| s.name == name)
    {
        return Some(json!(style.id));
    }
    if wants("Variable")
        && let Some(variable) = ds.doc.variables.variables.iter().find(|v| v.name == name)
    {
        return Some(json!(variable.id));
    }
    if wants("Collection")
        && let Some(collection) = ds.doc.variables.collections.iter().find(|c| c.name == name)
    {
        return Some(json!(collection.id));
    }
    None
}

fn editor_name(state: &EditorState) -> String {
    state.as_str().to_owned()
}

/// What a write with `basis` of `command` with `params` and `on` holds, as the guard computes it:
/// the editor state the command declares; the step's `on` and the command's target parameter
/// (every field, and where unless the command is position independent), the objects its other
/// object and name parameters name (every field and where), replaced object by object by
/// `under`'s premises; the objects `under` answers for a place, its windows (`slot:`) and a place
/// told by asking (`picks`).
fn held(row: &Row, command: &KitCommand) -> Result<(BTreeSet<String>, Holds), String> {
    let (bytes, roles) = if row.board { board() } else { fixture() };
    let mut roles = roles.clone();
    roles.push(("@FIG", json!(fig_layers())));
    let mut doc = if row.board { DesignDocument::open("Board.kdesign", bytes).unwrap() } else { open(bytes) };
    if !row.before.is_empty() {
        let steps: Value = serde_json::from_str(row.before).map_err(|e| format!("before {}: {e}", row.before))?;
        for (n, step) in steps.as_array().unwrap().iter().enumerate() {
            var_roles(&doc, &step[1].to_string(), &mut roles);
            let (c, p) = (step[0].as_str().unwrap(), with_roles(&step[1], &roles));
            let answer = doc.execute(c, p.clone()).map_err(|e| format!("before: {c} {p}: {e}"))?;
            // What a step made is `$S1`, `$S2`, ... (by the step's place in `before`).
            if let Some(made) = answer.get("id").filter(|id| !id.is_null()) {
                roles.push((STEP_ROLES[n], made.clone()));
            }
        }
    }
    var_roles(&doc, row.params, &mut roles);
    copy_roles(&doc, row, &mut roles);
    let roles = roles.as_slice();
    let params = with_roles(&serde_json::from_str(row.params).map_err(|e| format!("params: {e}"))?, roles);
    let on: Vec<Value> = row
        .on
        .iter()
        .map(|role| roles.iter().find(|(name, _)| name == role).map(|(_, id)| id.clone()).ok_or(format!("no role {role}")))
        .collect::<Result<_, _>>()?;
    let editor: BTreeSet<String> = command.reads.iter().map(editor_name).collect();
    let mut asked = params.clone();
    let mut holds: Holds = BTreeMap::new();
    let positioned = !command.position_independent;
    // What the guard needs of an object a step names by its declarations: every field, and
    // every position unless the command is position independent (then not the frame either).
    let named = |id: &Value, positioned: bool| (held_fields(None, (!positioned).then_some(&Vec::new()), is_layer(id)), positioned);
    for id in &on {
        holds.insert(name_of(id, roles), named(id, positioned));
    }
    match command.targets {
        Some(param) if !on.is_empty() => {
            let many = command
                .schema
                .as_ref()
                .and_then(|s| s.get("params")?.get(param)?.get("type")?.as_str().map(|t| t == "array"))
                .unwrap_or(param == "ids");
            asked[param] = if many || on.len() > 1 { json!(on) } else { on[0].clone() };
        }
        _ if !on.is_empty() => {
            doc.set_context(&CallContext { on: on.clone(), ..Default::default() })?;
        }
        _ => {}
    }
    let types_of =
        |path: &str| -> Vec<&str> { command.object_types.iter().find(|(at, _)| *at == path).map(|(_, types)| types.to_vec()).unwrap_or_default() };
    let resolve = |value: &Value, path: &str, names: bool| -> Option<Value> {
        match value {
            Value::Number(_) => Some(value.clone()),
            Value::String(text) if !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()) => Some(json!(text.parse::<u64>().ok()?)),
            Value::String(text) if roles.iter().any(|(_, id)| id == value) => Some(json!(text)),
            Value::String(text) if names => by_name(&doc, text, &types_of(path)),
            Value::String(_) => Some(value.clone()),
            _ => None,
        }
    };
    if let Some(param) = command.targets {
        for value in values_at(&asked, param) {
            if let Some(id) = resolve(&value, param, command.name_field.is_some()) {
                holds.insert(name_of(&id, roles), named(&id, positioned));
            }
        }
    }
    for path in command.objects {
        let names = command.name_params.iter().any(|(at, _)| at == path);
        for value in values_at(&asked, path) {
            if let Some(id) = resolve(&value, path, names) {
                holds.insert(name_of(&id, roles), (None, true));
            }
        }
    }
    for (path, _) in command.name_params {
        if command.objects.contains(path) {
            continue;
        }
        for value in values_at(&asked, path) {
            if let Some(id) = resolve(&value, path, true) {
                holds.insert(name_of(&id, roles), (None, true));
            }
        }
    }
    if let Some(under) = doc.under(&command.id, &asked)? {
        for object in &under.objects {
            holds.entry(name_of(object, roles)).or_insert((None, true));
        }
        for premise in &under.premises {
            static SCHEMA: OnceLock<Value> = OnceLock::new();
            let schema = SCHEMA.get_or_init(|| serde_json::from_str(kition_design_kdm::SCHEMA_JSON).unwrap());
            let ty = if is_layer(&premise.id) { "Node" } else { "Design" };
            for field in premise.fields.iter().flatten().filter_map(|field| field.strip_prefix('~')) {
                let name = field.split('.').next().unwrap();
                if schema["types"][ty]["derived"].get(name).is_none() {
                    return Err(format!("undeclared {ty} derived fact ~{name}"));
                }
            }
            let positions = premise.positions.as_ref();
            let fields = held_fields(premise.fields.as_ref(), positions, is_layer(&premise.id));
            let at = positions.is_none_or(|positions| positions.iter().any(|p| p == "nodes" || p == "libraryStore"));
            holds.insert(name_of(&premise.id, roles), (fields, at));
        }
        for within in &under.within {
            holds.insert(format!("slot:{}", name_of(&within.parent, roles)), (Some(BTreeSet::new()), false));
        }
        if let Some(covers) = &under.covers {
            holds.insert("picks".to_owned(), (Some(covers.fields.iter().cloned().collect()), false));
        }
    }
    // The row is a path the command takes: it runs.
    if row.runs {
        doc.execute(&command.id, asked).map_err(|e| format!("the command fails: {e}"))?;
    }
    Ok((editor, holds))
}

/// A row's holds (two entries for one object: what either reads).
fn expected(row: &Row) -> Holds {
    let mut holds = Holds::new();
    for (object, fields, at) in row.holds {
        let fields = field_set(fields);
        // A place among a parent's children is held by its window: no fields, no position.
        let (fields, at) = if object.starts_with("slot:") { (Some(BTreeSet::new()), &false) } else { (fields, at) };
        match holds.get_mut(*object) {
            Some((known, known_at)) => {
                *known = match (known.take(), fields) {
                    (Some(mut known), Some(more)) => {
                        known.extend(more);
                        Some(known)
                    }
                    _ => None,
                };
                *known_at |= *at;
            }
            None => {
                holds.insert((*object).to_owned(), (fields, *at));
            }
        }
    }
    holds
}

/// Whether `held` (a field, or every field) covers `wanted`: a field covers the paths inside it.
fn covers(held: &Option<BTreeSet<String>>, wanted: &str) -> bool {
    held.as_ref().is_none_or(|held| held.iter().any(|field| field == wanted || wanted.starts_with(&format!("{field}."))))
}

fn show(fields: &Option<BTreeSet<String>>) -> String {
    match fields {
        None => "*".to_owned(),
        Some(fields) if fields.is_empty() => "-".to_owned(),
        Some(fields) => fields.iter().cloned().collect::<Vec<_>>().join(","),
    }
}

/// What a declaration holds less than the reading (a write can land on what was not observed)
/// and more (a refusal no change needs).
#[derive(Default)]
struct Difference {
    less: Vec<String>,
    more: Vec<String>,
}

fn compare(holds: &Holds, wanted: &Holds) -> Difference {
    let mut difference = Difference::default();
    for (object, (fields, at)) in wanted {
        let Some((held_fields, held_at)) = holds.get(object) else {
            difference.less.push(format!("{object} not held"));
            continue;
        };
        match fields {
            None if held_fields.is_some() => difference.less.push(format!("{object}: every field read, holds {}", show(held_fields))),
            None => {}
            Some(fields) => {
                for field in fields.iter().filter(|field| !covers(held_fields, field)) {
                    difference.less.push(format!("{object}.{field} not held"));
                }
            }
        }
        if *at && !held_at {
            difference.less.push(format!("{object}: where it is not held"));
        }
    }
    for (object, (held_fields, held_at)) in holds {
        let Some((fields, at)) = wanted.get(object) else {
            // That an object the step names is there is the least the guard holds of it.
            if held_fields.as_ref().is_some_and(BTreeSet::is_empty) && !held_at {
                continue;
            }
            difference.more.push(format!("{object} held ({}{}), not read", show(held_fields), if *held_at { ", where" } else { "" }));
            continue;
        };
        match held_fields {
            None if fields.is_some() => difference.more.push(format!("{object}: holds every field, reads {}", show(fields))),
            None => {}
            Some(held_fields) => {
                for field in held_fields.iter().filter(|field| !covers(fields, field)) {
                    difference.more.push(format!("{object}.{field} held, not read"));
                }
            }
        }
        if *held_at && !at {
            difference.more.push(format!("{object}: where it is held, not read"));
        }
    }
    difference
}

fn editor_of(row: &Row) -> BTreeSet<String> {
    row.editor.split(',').map(str::trim).filter(|state| !state.is_empty()).map(str::to_owned).collect()
}

/// The write commands: what changes the design on the fixture, and the tool switches that do
/// while a vector is being edited (they end the edit and remove an emptied layer).
fn writes() -> BTreeSet<String> {
    let (bytes, roles) = fixture();
    let mut doc = open(bytes);
    let mut writes: BTreeSet<String> = doc.commands().into_iter().filter(|c| c.mutates).map(|c| c.id).collect();
    let v = roles.iter().find(|(role, _)| *role == "$V").unwrap().1.clone();
    doc.execute("vector.edit.begin", json!({ "id": v })).unwrap();
    writes.extend(doc.commands().into_iter().filter(|c| c.mutates).map(|c| c.id));
    writes
}

/// `DESIGN_READ_ONLY=text.,vector.` compares only the commands with these prefixes (and skips the
/// floors): for working on one family.
fn only(command: &str) -> bool {
    std::env::var("DESIGN_READ_ONLY").map_or(true, |prefixes| prefixes.split(',').any(|prefix| command.starts_with(prefix.trim())))
}

/// Every write command's rows hold what its reading says, and every write command has a row.
#[test]
fn every_write_rests_on_what_its_handler_reads() {
    let (bytes, _) = fixture();
    let commands = open(bytes).commands();
    let mut report = Vec::new();
    let mut machine = Vec::new();
    let mut broken = Vec::new();
    let (mut differing, mut less, mut more) = (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
    let mut editors: BTreeMap<&str, (BTreeSet<String>, BTreeSet<String>)> = BTreeMap::new();
    let mut rows = BTreeSet::new();
    for row in ROWS {
        rows.insert(row.command);
        if NOT_COMPARED.contains(&row.command) || !only(row.command) {
            continue;
        }
        let command = commands.iter().find(|c| c.id == row.command).unwrap_or_else(|| panic!("no command {}", row.command));
        let (editor, holds) = match held(row, command) {
            Ok(held) => held,
            Err(refused) => {
                broken.push(format!("{} {} on {:?}: {refused}", row.command, row.params, row.on));
                continue;
            }
        };
        editors.entry(row.command).or_insert_with(|| (editor, BTreeSet::new())).1.extend(editor_of(row));
        let difference = compare(&holds, &expected(row));
        if difference.less.is_empty() && difference.more.is_empty() {
            continue;
        }
        differing.insert(row.command);
        if !difference.less.is_empty() {
            less.insert(row.command);
        }
        if !difference.more.is_empty() {
            more.insert(row.command);
        }
        machine.push(
            json!({ "command": row.command, "params": row.params, "on": row.on, "editor": row.editor,
            "targets": command.targets, "less": difference.less, "more": difference.more })
            .to_string(),
        );
        report.push(format!(
            "{} {} {} on {:?}{}:\n  less: {}\n  more: {}",
            row.command,
            row.before,
            row.params,
            row.on,
            if row.pinned { " (pinned)" } else { "" },
            difference.less.join("; "),
            difference.more.join("; ")
        ));
    }
    for (command, (declared, read)) in &editors {
        if declared == read {
            continue;
        }
        differing.insert(command);
        let fewer: Vec<_> = read.difference(declared).collect();
        let wider: Vec<_> = declared.difference(read).collect();
        if !fewer.is_empty() {
            less.insert(command);
        }
        if !wider.is_empty() {
            more.insert(command);
        }
        machine.push(json!({ "command": command, "editorLess": fewer, "editorMore": wider }).to_string());
        report.push(format!("{command}: editor state read but not declared {fewer:?}; declared but not read {wider:?}"));
    }
    let writes = writes();
    let missing: Vec<_> = writes.iter().filter(|c| !rows.contains(c.as_str())).collect();
    let stray: Vec<_> = rows.iter().filter(|c| !writes.contains(**c)).collect();
    let suffix = std::env::var("DESIGN_READ_ONLY").map(|only| format!("-{}", only.replace([',', '.'], "_"))).unwrap_or_default();
    std::fs::write(std::env::temp_dir().join(format!("design-declarations-independent{suffix}.txt")), report.join("\n")).ok();
    std::fs::write(std::env::temp_dir().join(format!("design-declarations-independent{suffix}.jsonl")), machine.join("\n")).ok();
    println!("{}", report.join("\n"));
    println!("rows {}, pinned after review {}", ROWS.len(), ROWS.iter().filter(|row| row.pinned).count());
    println!("broken {}: {}", broken.len(), broken.join("\n"));
    println!("missing {missing:?}\nstray {stray:?}");
    println!(
        "commands differing {} of {}: holding less {}, holding more {}, both {}",
        differing.len(),
        writes.len() - NOT_COMPARED.len(),
        less.len(),
        more.len(),
        less.intersection(&more).count()
    );
    println!("LESS {less:?}\nMORE {more:?}");
    if std::env::var("DESIGN_READ_ONLY").is_ok() {
        assert!(broken.is_empty(), "rows that do not run");
        return;
    }
    assert!(broken.is_empty() && missing.is_empty() && stray.is_empty(), "rows that do not run, write commands without rows, rows of no write");
    assert_eq!(less.len(), HELD_LESS_FLOOR, "commands hold less than their handlers read");
    assert!(more.len() <= HELD_MORE_FLOOR, "{} commands hold more than their handlers read (floor {HELD_MORE_FLOOR})", more.len());
    assert!(differing.len() <= DIFFERING_FLOOR, "{} commands differ from their reading (floor {DIFFERING_FLOOR})", differing.len());
}

/// Write commands read but not compared: a widget needs a plugin installed, and the kit offers no
/// command that installs one (`plugin.*` is left out), so no widget can be on a fixture.
const NOT_COMPARED: &[&str] = &["widget.insert", "widget.click", "widget.setText", "widget.menu"];

/// The commands holding less than their reading (a write that can land on what the agent did not
/// observe), more (a refusal no change needs), and differing either way: these numbers only go
/// down except for independently reviewed oracle corrections noted below.
const HELD_LESS_FLOOR: usize = 0;
// review: five spatial commands hold unread painted bounds; three text commands
// unconditionally hold reflow inputs even on the unchanged fixture paths checked below.
// Resolving Dev Mode status also holds the unread current subtree (metadata/snapshot is read).
const HELD_MORE_FLOOR: usize = 12;
const DIFFERING_FLOOR: usize = 12;

/// Reviewed after behavior tests: layout_frame (layout/src/lib.rs:345-350) reads the auto
/// layout parent's own sizing and min/max. The text-size and vector/image-insertion limit
/// regressions exercise limits that leave the old box unchanged but clamp the next reflow.
const TEXT_FLOW_PARENT: &str = "layout,frame,childLayout";

// review: geometric world_bounds is not painted ~bounds (doc/src/lib.rs:721).
// Reviewed after comparison against canvas/drop.rs: drop_parent walks geometry/hierarchy and
// instance-slot metadata; auto_layout_slot reads each child's absolute flag. No paints or props.
const DROP_PICKS: &str = "visible,locked,kind,layout,childLayout,frame,instance,mirrors,~hierarchy";
// review: effects never participate in hit testing; painted ~bounds is excess.
// Reviewed against doc/geometry.rs: hit_test reads shape/stroke coverage, clipping and masks,
// then swap_target follows the hit's ancestors. Other props and child sizing are not inputs.
const SWAP_PICKS: &str = "visible,locked,kind,frame,corners,fills,strokes,stroke,props.clip,props.is_mask,~hierarchy";
// review: frame_at uses local_rect and world_bounds, neither includes effects.
// Reviewed against cmd/create.rs: frame_at uses board identity and slot metadata, then geometry;
// the tool does not hit-test paint. The selected parent's own layout is held separately.
const TOOL_PICKS: &str = "visible,locked,kind,frame,props.board,instance,mirrors,~hierarchy";
// Reviewed against cmd/comment.rs: anchoring walks visible containers' local rectangles.
const COMMENT_PICKS: &str = "visible,kind,frame,~hierarchy";

const ROWS: &[Row] = &[
    // ---- Reading 1 ----
    // edit.undo: takes back the agent's last step (a rename of $E): rests on the history and on the field that step wrote, which it puts back
    s(
        r##"[["object.rename", {"ids": ["$E"], "name": "Oval"}]]"##,
        "edit.undo",
        r##"{}"##,
        &[],
        "",
        &[("design", "~history", false), ("$E", "name", false)],
    ),
    // edit.redo: re-applies the undone rename: rests on the redo history and $E's name, which it replaces
    s(
        r##"[["object.rename", {"ids": ["$E"], "name": "Oval"}], ["edit.undo", {}]]"##,
        "edit.redo",
        r##"{}"##,
        &[],
        "",
        &[("design", "~history", false), ("$E", "name", false)],
    ),
    // Reviewed after behavior tests: read_defects::an_agents_cut_by_id_can_be_pasted proves the
    // clipboard survives bracketed cuts (fixed by this card), so its whole source is read.
    r("edit.cut", r##"{"ids": ["$E"]}"##, &[], "", &[("$E", "*", true)]),
    // edit.cut on the selection (not bracketed): copies the whole subtree in page space (world transform) into the clipboard, then deletes it
    r("edit.cut", r##"{}"##, &["$E"], "selection", &[("$E", "*", true)]),
    // edit.paste with no selection: pastes the clipboard ($G1 copied) on top of the current page, in place because the copy is in view
    s(
        r##"[["select.set", {"ids": ["$G1"]}], ["edit.copy", {}], ["select.none", {}]]"##,
        "edit.paste",
        r##"{}"##,
        &[],
        "selection,part,mode",
        &[("$P1", "-", false)],
    ),
    // edit.paste into a named parent: the copy is not inside $F, so it is centred on $F's world box, then $F's vertical auto layout re-flows it with $A and $T
    s(
        r##"[["select.set", {"ids": ["$G1"]}], ["edit.copy", {}], ["select.none", {}]]"##,
        "edit.paste",
        r##"{"parent": "$F"}"##,
        &[],
        "mode",
        &[("$F", "kind,frame,layout,childLayout", true), ("$A", "frame,visible,childLayout", true), ("$T", "frame,visible,childLayout", true)],
    ),
    // edit.paste with one plain frame selected: pastes into $X, centred on its world box
    s(
        r##"[["select.set", {"ids": ["$G1"]}], ["edit.copy", {}], ["select.none", {}]]"##,
        "edit.paste",
        r##"{}"##,
        &["$X"],
        "selection,mode",
        &[("$X", "kind,frame", true)],
    ),
    // edit.paste of a main component copied in this file: makes a new instance of $K from $K's current subtree
    s(
        r##"[["select.set", {"ids": ["$K"]}], ["edit.copy", {}], ["select.none", {}]]"##,
        "edit.paste",
        r##"{}"##,
        &[],
        "selection,part,mode",
        &[("$P1", "-", false), ("$K", "*", false), ("$KT", "*", false)],
    ),
    // edit.pasteOver with a selection: pastes into the selected layer's parent at its world top-left
    s(
        r##"[["select.set", {"ids": ["$G1"]}], ["edit.copy", {}], ["select.none", {}]]"##,
        "edit.pasteOver",
        r##"{}"##,
        &["$E"],
        "selection,mode",
        &[("$E", "frame", true)],
    ),
    // edit.pasteOver with no selection: on the current page, at the copied place
    s(
        r##"[["select.set", {"ids": ["$G1"]}], ["edit.copy", {}], ["select.none", {}]]"##,
        "edit.pasteOver",
        r##"{}"##,
        &[],
        "selection,part,mode",
        &[("$P1", "-", false)],
    ),
    // edit.pasteReplace: pastes in $E's parent at $E's index and world top-left, then removes $E
    s(
        r##"[["select.set", {"ids": ["$G1"]}], ["edit.copy", {}], ["select.none", {}]]"##,
        "edit.pasteReplace",
        r##"{"ids": ["$E"]}"##,
        &[],
        "mode",
        &[("$E", "frame", true)],
    ),
    // edit.duplicate of a top-level frame: copies the whole subtree to index+1, moved right by its width + 100; the name is bumped per prefs; the repeat offset comes from last_duplicate
    r("edit.duplicate", r##"{"ids": ["$X"]}"##, &[], "mode", &[("$X", "*", true)]),
    // edit.duplicate of a main component: the copy is an instance of $K built from $K's subtree
    r("edit.duplicate", r##"{"ids": ["$K"]}"##, &[], "mode", &[("$K", "*", true), ("$KT", "*", false)]),
    // edit.duplicate of an auto layout child: copy at index+1 in $F, which re-flows with its other children
    r(
        "edit.duplicate",
        r##"{"ids": ["$A"]}"##,
        &[],
        "mode",
        &[("$A", "*", true), ("$F", "frame,layout,childLayout", false), ("$T", "frame,visible,childLayout", true)],
    ),
    // edit.delete of a top-level layer: removes it (it only has to exist)
    r("edit.delete", r##"{"ids": ["$E"]}"##, &[], "", &[("$E", "-", false)]),
    // edit.delete in auto layout: $F re-flows with what is left ($T moves up)
    r(
        "edit.delete",
        r##"{"ids": ["$A"]}"##,
        &[],
        "",
        &[("$A", "-", true), ("$F", "frame,layout,childLayout", false), ("$T", "frame,visible,childLayout", true)],
    ),
    // edit.delete of a layer with a comment pin: the thread $Th is re-pinned to page coordinates from $R's world transform
    r("edit.delete", r##"{"ids": ["$R"]}"##, &[], "", &[("$R", "frame.transform", true), ("$Th", "anchor", false)]),
    // UNSURE: edit.findReplace in texts: scans every layer of the current page (kind: is it text, its characters), writes $T's characters (auto-size, then $F re-flows); text layers inside $LI (library's "Primary") have no role here and are not listed
    r(
        "edit.findReplace",
        r##"{"find": "Hello", "replace": "Hi"}"##,
        &[],
        "part",
        &[
            ("$P1", "~hierarchy", false),
            ("$T", "kind,text", true),
            ("$KT", "kind,text", true),
            ("$IT", "kind,text", true),
            ("$F", "frame,layout,childLayout", false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // UNSURE: edit.findReplace in layer names: reads every page layer's name (and text), renames $R "Box" -> "Crate"; $LI's inner layers are unlisted
    r(
        "edit.findReplace",
        r##"{"find": "Box", "replace": "Crate", "layerNames": true, "select": false}"##,
        &[],
        "part",
        &[
            ("$P1", "~hierarchy", false),
            ("$D", "name,kind", true),
            ("$R", "name,kind", true),
            ("$E", "name,kind", true),
            ("$F", "name,kind", true),
            ("$A", "name,kind", true),
            ("$T", "name,kind,text", true),
            ("$G", "name,kind", true),
            ("$G1", "name,kind", true),
            ("$G2", "name,kind", true),
            ("$V", "name,kind", true),
            ("$K", "name,kind", true),
            ("$KT", "name,kind,text", true),
            ("$I", "name,kind", true),
            ("$IT", "name,kind,text", true),
            ("$X", "name,kind", true),
            ("$S", "name,kind", true),
            ("$Set", "name,kind", true),
            ("$VS", "name,kind", true),
            ("$VL", "name,kind", true),
            ("$LI", "name,kind", true),
        ],
    ),
    // edit.pasteProperties: applies copied properties ($G1's) to $E. It reads $E's type (text keys, corners) and its paint bindings; normalize compares the new fills with $Brand to drop the style link. It replaces fills, strokes, stroke, effects, opacity and blend
    s(
        r##"[["select.set", {"ids": ["$G1"]}], ["edit.copyProperties", {}], ["select.none", {}]]"##,
        "edit.pasteProperties",
        r##"{"ids": ["$E"]}"##,
        &[],
        "mode",
        &[("$E", "kind,fills,strokes,stroke,effects,opacity,props.blend,bindings.styles,bindings.bound", false), ("$Brand", "value", false)],
    ),
    // object.frameSelection: new clipping frame around the world boxes of $E and $V, inserted above the topmost one in its parent, layers reparented with their world place kept
    r("object.frameSelection", r##"{"ids": ["$E", "$V"]}"##, &[], "", &[("$E", "frame", true), ("$V", "frame", true)]),
    // object.frameSelection on the selection
    r("object.frameSelection", r##"{}"##, &["$E", "$V"], "selection", &[("$E", "frame", true), ("$V", "frame", true)]),
    // object.group: same as frameSelection; the group then fits its children
    // review: wrap rejects sections, so source kind is an input.
    r("object.group", r##"{"ids": ["$E", "$V"]}"##, &[], "", &[("$E", "kind,frame", true), ("$V", "kind,frame", true)]),
    // object.group on the selection
    // review: wrap rejects sections, so source kind is an input.
    r("object.group", r##"{}"##, &["$E", "$V"], "selection", &[("$E", "kind,frame", true), ("$V", "kind,frame", true)]),
    // object.ungroup: the type decides whether it acts; children go to $G's parent at $G's index with world places kept ($G's transform folded in); $G's opacity multiplies onto them when below 1; $G removed
    r(
        "object.ungroup",
        r##"{"ids": ["$G"]}"##,
        &[],
        "",
        &[
            ("$G", "kind,opacity,frame.transform,~hierarchy", true),
            ("$G1", "frame.transform,props.bool_op", true),
            ("$G2", "frame.transform,props.bool_op", true),
        ],
    ),
    // object.removeFrame: the same handler on a frame: $A and $T go to the page at $F's index with world places kept (normalize fixes their auto-layout sizing), $F removed
    r(
        "object.removeFrame",
        r##"{"ids": ["$F"]}"##,
        &[],
        "",
        &[
            ("$F", "kind,opacity,frame.transform,~hierarchy", true),
            ("$A", "frame.transform,childLayout,props.bool_op", true),
            ("$T", "frame.transform,childLayout,props.bool_op", true),
        ],
    ),
    // object.resizeToFit on an auto layout frame: sets hug both ways, then the re-flow sizes $F from its children
    r(
        "object.resizeToFit",
        r##"{"ids": ["$F"]}"##,
        &[],
        "",
        &[
            ("$F", "kind,layout,childLayout,frame,~hierarchy", false),
            ("$A", "frame,visible,childLayout", true),
            ("$T", "frame,visible,childLayout", true),
        ],
    ),
    // object.resizeToFit on a plain frame: $D's box becomes its visible children's box ($LI), and the children move back by the offset
    r("object.resizeToFit", r##"{"ids": ["$D"]}"##, &[], "", &[("$D", "kind,layout,frame,~hierarchy", false), ("$LI", "frame,visible", true)]),
    // object.mask on one layer: toggles its mask flag (read and replaced)
    r("object.mask", r##"{"ids": ["$E"]}"##, &[], "", &[("$E", "props.is_mask", false)]),
    // object.mask on several: groups them as object.group does ("Mask group"), and the bottom child ($E) becomes the mask
    r("object.mask", r##"{"ids": ["$E", "$V"]}"##, &[], "", &[("$E", "frame,props.is_mask", true), ("$V", "frame", true)]),
    // object.releaseMask: sets the mask flag off (replaced, not read)
    r("object.releaseMask", r##"{"ids": ["$E"]}"##, &[], "", &[("$E", "props.is_mask", false)]),
    // UNSURE: object.setThumbnail: scans every layer's plugin_data to remove the old thumbnail marker, then marks $X; the other layers' plugin_data is read but cannot be listed by role
    r("object.setThumbnail", r##"{"id": "$X"}"##, &[], "", &[("$X", "props.plugin_data", false), ("design", "nodes", false)]),
    // object.flipH: mirrors about the centre of the targets' world box: reads world boxes and transforms
    r("object.flipH", r##"{"ids": ["$E"]}"##, &[], "", &[("$E", "frame", true)]),
    // object.flipH on several: about the centre of their combined world box
    r("object.flipH", r##"{"ids": ["$E", "$V"]}"##, &[], "", &[("$E", "frame", true), ("$V", "frame", true)]),
    // object.flipH in vector edit mode without ids: flips the selected points of $V within their box (its network)
    s(
        r##"[["vector.edit.begin", {"id": "$V"}], ["vector.edit.selectAll", {}]]"##,
        "object.flipH",
        r##"{}"##,
        &[],
        "mode",
        &[("$V", "kind,frame", true)],
    ),
    // object.flipV: mirrors vertically about the centre of the world box
    r("object.flipV", r##"{"ids": ["$E"]}"##, &[], "", &[("$E", "frame", true)]),
    // object.flipV in vector edit mode: the selected points
    s(
        r##"[["vector.edit.begin", {"id": "$V"}], ["vector.edit.selectAll", {}]]"##,
        "object.flipV",
        r##"{}"##,
        &[],
        "mode",
        &[("$V", "kind,frame", true)],
    ),
    // object.rotate180 with ids (not the selection): turns about the centre of the world box
    r("object.rotate180", r##"{"ids": ["$E"]}"##, &[], "", &[("$E", "frame", true)]),
    // object.rotate180 on the selection: the pivot is the moved rotation origin if there is one, else the box centre
    r("object.rotate180", r##"{}"##, &["$E"], "selection,mode", &[("$E", "frame", true)]),
    // object.rotate180 in vector edit mode: the selected points about their box centre
    s(
        r##"[["vector.edit.begin", {"id": "$V"}], ["vector.edit.selectAll", {}]]"##,
        "object.rotate180",
        r##"{}"##,
        &[],
        "mode",
        &[("$V", "kind,frame", true)],
    ),
    // object.rotateLeft: 90 degrees about the world box centre
    r("object.rotateLeft", r##"{"ids": ["$E"]}"##, &[], "", &[("$E", "frame", true)]),
    // object.rotateLeft on the selection (rotation origin consulted)
    r("object.rotateLeft", r##"{}"##, &["$E"], "selection,mode", &[("$E", "frame", true)]),
    // object.rotateRight: -90 degrees about the world box centre
    r("object.rotateRight", r##"{"ids": ["$E"]}"##, &[], "", &[("$E", "frame", true)]),
    // object.rotateRight in vector edit mode: the selected points
    s(
        r##"[["vector.edit.begin", {"id": "$V"}], ["vector.edit.selectAll", {}]]"##,
        "object.rotateRight",
        r##"{}"##,
        &[],
        "mode",
        &[("$V", "kind,frame", true)],
    ),
    // object.rasterize: renders $E (everything that paints it, its export bounds), puts an image rectangle at its index+1 in its parent (world box through the parent), takes its name, removes $E
    r("object.rasterize", r##"{"ids": ["$E"]}"##, &[], "", &[("$E", "*", true)]),
    // object.toggleVisible without a value: reads whether all targets are visible and replaces visible
    r("object.toggleVisible", r##"{"ids": ["$E"]}"##, &[], "", &[("$E", "visible", false)]),
    // object.toggleVisible with a value on an auto layout child: replaces visible; $F re-flows ($T moves up)
    r(
        "object.toggleVisible",
        r##"{"ids": ["$A"], "visible": false}"##,
        &[],
        "",
        &[("$A", "visible", true), ("$F", "frame,layout,childLayout", false), ("$T", "frame,visible,childLayout", true)],
    ),
    // object.toggleLock: reads and replaces locked (locking clears the selection)
    r("object.toggleLock", r##"{"ids": ["$E"]}"##, &[], "", &[("$E", "locked", false)]),
    // object.hideOthers: hides every top-level layer of the current page except the selection's top-level ancestors
    r(
        "object.hideOthers",
        r##"{}"##,
        &["$E"],
        "selection,part",
        &[
            ("$P1", "~hierarchy", false),
            ("$E", "-", true),
            ("$D", "visible", true),
            ("$R", "visible", true),
            ("$F", "visible", true),
            ("$G", "visible", true),
            ("$V", "visible", true),
            ("$K", "visible", true),
            ("$I", "visible", true),
            ("$X", "visible", true),
            ("$S", "visible", true),
            ("$Set", "visible", true),
        ],
    ),
    // object.removeFill on a plain layer: clears fills (replaced)
    r("object.removeFill", r##"{"ids": ["$G1"]}"##, &[], "", &[("$G1", "fills", false)]),
    // object.removeFill on a styled layer: normalize compares the empty fills with $Brand and drops the fill style link
    r("object.removeFill", r##"{"ids": ["$E"]}"##, &[], "", &[("$E", "fills,bindings.styles", false), ("$Brand", "value", false)]),
    // object.removeStroke: clears strokes (replaced)
    r("object.removeStroke", r##"{"ids": ["$G1"]}"##, &[], "", &[("$G1", "strokes", false)]),
    // object.swapFillStroke: reads both lists and swaps them
    r("object.swapFillStroke", r##"{"ids": ["$G1"]}"##, &[], "", &[("$G1", "fills,strokes", false)]),
    // object.swapFillStroke on a styled layer: the fill style link is dropped against $Brand's value
    r("object.swapFillStroke", r##"{"ids": ["$E"]}"##, &[], "", &[("$E", "fills,strokes,bindings.styles", false), ("$Brand", "value", false)]),
    // object.rename with a name: replaces the name
    r("object.rename", r##"{"ids": ["$E"], "name": "Oval"}"##, &[], "", &[("$E", "name", false)]),
    // object.rename on the selection
    r("object.rename", r##"{"name": "Oval"}"##, &["$E"], "selection", &[("$E", "name", false)]),
    // object.rename without a name: only asks the UI for an inline rename, writes nothing
    r("object.rename", r##"{"ids": ["$E"]}"##, &[], "", &[("$E", "-", false)]),
    // object.batchRename with a pattern: numbers the layers in stacking order and replaces their names
    r("object.batchRename", r##"{"ids": ["$E", "$V"], "name": "Item $n"}"##, &[], "", &[("$E", "name", true), ("$V", "name", true)]),
    // object.batchRename find/replace: reads the current name to match and rebuild it
    r("object.batchRename", r##"{"ids": ["$E"], "find": "Ell", "replace": "Oval"}"##, &[], "", &[("$E", "name", false)]),
    // object.moveToPage: reparents $E to the top of $P2, keeping its world transform
    r("object.moveToPage", r##"{"ids": ["$E"], "page": "$P2"}"##, &[], "", &[("$E", "frame.transform", true), ("$P2", "-", false)]),
    // object.moveToPage of a layer with a comment pin: the thread follows $R to the other page
    r(
        "object.moveToPage",
        r##"{"ids": ["$R"], "page": "$P2"}"##,
        &[],
        "",
        &[("$R", "frame.transform", true), ("$P2", "-", false), ("$Th", "anchor", false)],
    ),
    // UNSURE: cannot run. edit.pasteDesignLayers takes only a binary .fig as base64 (`data`) or inside clipboard HTML (`html`). It needs a "fig-" header (kiwi schema + compressed data, format/src/fig.rs:82) or a zip holding canvas.fig. That can't be written by hand, and a before step can't hand it over ($S<n> are ids). The reading: it replaces the clipboard's layers, then pastes as edit.paste does (page, view, selection)
    r("edit.pasteDesignLayers", r##"{"data": "@FIG"}"##, &[], "selection,part,mode", &[("$P1", "-", false)]),
    // object.opacityKeys: replaces opacity and checks its own bound fields to drop an opacity binding the edit overrode
    r("object.opacityKeys", r##"{"digits": "5", "ids": ["$E"]}"##, &[], "", &[("$E", "opacity,bindings.bound", false)]),
    // object.opacityKeys on the selection
    r("object.opacityKeys", r##"{"digits": "45"}"##, &["$E"], "selection", &[("$E", "opacity,bindings.bound", false)]),
    // command.batch: runs each step through the engine (not bracketed) as one undo step; rests on what its steps rest on
    r(
        "command.batch",
        r##"{"commands": [{"command": "object.rename", "params": {"ids": ["$E"], "name": "Oval"}}, {"command": "object.toggleVisible", "params": {"ids": ["$V"], "visible": false}}]}"##,
        &[],
        "",
        &[("$E", "name", false), ("$V", "visible", false)],
    ),
    // Pinned from the declaration after review: the nested batch regression independently
    // verifies that the leaf's target and name premise apply through a second batch level.
    p(
        "",
        "command.batch",
        r##"{"commands": [{"command": "command.batch", "params": {"commands": [{"command": "object.rename", "params": {"ids": ["$E"], "name": "Oval"}}]}}]}"##,
        &[],
        "",
        &[("$E", "name", false)],
    ),
    // Pinned from the declaration after review: the failed-first outside/read regression
    // establishes that a selection produced inside the batch names the following rename.
    p(
        "",
        "command.batch",
        r##"{"commands": [{"command": "select.set", "params": {"ids": ["$E"]}}, {"command": "object.rename", "params": {"name": "Agent"}}]}"##,
        &[],
        "",
        &[("$E", "name", false)],
    ),
    // Pinned from the declaration after review: the failed-first outside/read regression
    // consumes node.get's width. props::apply reads the target's old size/proportions and
    // fixes its child sizing; no other source-query fields feed the write.
    p(
        "",
        "command.batch",
        r##"{"commands": [{"command": "node.get", "params": {"id": "$E"}}, {"command": "node.set", "params": {"id": "$R", "props": {"width": {"$step": 0, "field": "width"}}}}]}"##,
        &[],
        "",
        &[("$E", "frame.width", false), ("$R", "frame,childLayout,props.constrain_proportions,~hierarchy", false)],
    ),
    // edit.repeat: re-runs the last history command with its stored params (object.flipH {} on the selection $E)
    s(r##"[["select.set", {"ids": ["$E"]}], ["object.flipH", {}]]"##, "edit.repeat", r##"{}"##, &[], "selection,mode", &[("$E", "frame", true)]),
    // object.convertToGroup: $D becomes a group: it reads the type and its children (non-empty, fit). It replaces kind, clears layout, clip, overflow, fills, strokes, grids, guides and corners, drops style links and bindings, sets the children not absolute, and fits the box to $LI
    r(
        "object.convertToGroup",
        r##"{"ids": ["$D"]}"##,
        &[],
        "",
        &[
            (
                "$D",
                "kind,layout,fills,strokes,corners,props.clip,props.grids,props.guides,prototype.overflow,bindings.styles,bindings.bound,frame,~hierarchy",
                false,
            ),
            ("$LI", "frame,childLayout", true),
        ],
    ),
    // object.convertToGroup on the selection
    r(
        "object.convertToGroup",
        r##"{}"##,
        &["$D"],
        "selection",
        &[
            (
                "$D",
                "kind,layout,fills,strokes,corners,props.clip,props.grids,props.guides,prototype.overflow,bindings.styles,bindings.bound,frame,~hierarchy",
                false,
            ),
            ("$LI", "frame,childLayout", true),
        ],
    ),
    // object.convertToFrame of a group: reads the type, clears fills and strokes, kind=Frame, no clip
    r("object.convertToFrame", r##"{"ids": ["$G"]}"##, &[], "", &[("$G", "kind,fills,strokes,props.clip", false)]),
    // object.convertToFrame of a section: reads the type (and that it holds no sections), kind=Frame, no clip, fill kept
    r("object.convertToFrame", r##"{"ids": ["$S"]}"##, &[], "", &[("$S", "kind,props.clip,~hierarchy", false)]),
    // object.convertToSection: frame on the page becomes a section: replaces kind, clears layout, clip, overflow, grids, guides, the grid style and layout bindings
    r(
        "object.convertToSection",
        r##"{"ids": ["$X"]}"##,
        &[],
        "",
        &[("$X", "kind,layout,props.clip,prototype.overflow,props.grids,props.guides,bindings.styles,bindings.bound", false)],
    ),
    // object.wrapInSection: a section 40 larger than $E's world box, at $E's index+1 on the page, $E reparented inside
    r("object.wrapInSection", r##"{"ids": ["$E"]}"##, &[], "", &[("$E", "frame", true)]),
    // object.wrapInSection on the selection
    r("object.wrapInSection", r##"{}"##, &["$E", "$V"], "selection", &[("$E", "frame", true), ("$V", "frame", true)]),
    // data.fill: duplicates $F per row (whole subtree, placed (width + gap) to the right at $F's index+1), names the copy from the row
    r("data.fill", r##"{"id": "$F", "rows": [{"name": "Card A"}]}"##, &[], "", &[("$F", "*", true), ("$A", "*", true), ("$T", "*", true)]),
    // UNSURE: data.fill with a key: sets text on the copy's layers named by the key (its name copied from $T; if $T is not named "Hello" the key is reported as missing)
    r("data.fill", r##"{"id": "$F", "rows": [{"Hello": "Hi"}]}"##, &[], "", &[("$F", "*", true), ("$A", "*", true), ("$T", "*", true)]),
    // ---- Reading 2 ----
    // layout.add: existing empty frame $X gets an inferred layout (no children -> defaults); writes $X.layout; re-flow of $X
    // Reviewed after comparison: structure.rs:545 reads kind to choose an existing frame or a wrapper.
    r("layout.add", r##"{"ids": ["$X"]}"##, &[], "", &[("$X", "kind,layout,frame,childLayout,~hierarchy", false)]),
    // layout.add: non-frames $R,$E are wrapped in a new frame above the topmost ($E), reparented (world kept), layout inferred from their boxes, hug; re-flow moves them
    // Reviewed after comparison: structure.rs:545 chooses by kind/layout; derived.rs:111 also
    // re-flows a changed layer with its own layout after it is placed in the wrapper.
    r(
        "layout.add",
        r##"{"ids": ["$R", "$E"], "direction": "vertical"}"##,
        &[],
        "",
        &[("$R", "kind,layout,frame,visible,childLayout", true), ("$E", "kind,layout,frame,visible,childLayout", true)],
    ),
    // layout.remove: $F loses its layout; coherent sizing of $F (hug->fixed) and its children (fill->fixed, text auto-resize)
    r(
        "layout.remove",
        r##"{"ids": ["$F"]}"##,
        &[],
        "",
        &[("$F", "layout,childLayout,~hierarchy", false), ("$A", "childLayout", true), ("$T", "childLayout,kind,text", true)],
    ),
    // layout.suggest: frame $D without layout: layout inferred from $LI's box and $D's size, $D hugs; re-flow moves $LI and resizes $D
    // Reviewed after comparison: the same layout_add branch reads kind (structure.rs:545).
    r(
        "layout.suggest",
        r##"{"ids": ["$D"]}"##,
        &[],
        "",
        &[("$D", "kind,frame,layout,childLayout,~hierarchy", false), ("$LI", "frame,visible,childLayout", true)],
    ),
    // arrange.front: $R moves to the top of $P1's child list (reads its parent and the parent's child count)
    r("arrange.front", r##"{"ids": ["$R"]}"##, &[], "", &[("$R", "-", true), ("$P1", "~hierarchy", false)]),
    // arrange.forward: $R swaps with the sibling just above it ($E), checking that sibling is not among ids
    r("arrange.forward", r##"{"ids": ["$R"]}"##, &[], "", &[("$R", "-", true), ("$E", "-", true)]),
    // arrange.backward: $E swaps with the sibling just below it ($R)
    r("arrange.backward", r##"{"ids": ["$E"]}"##, &[], "", &[("$E", "-", true), ("$R", "-", true)]),
    // arrange.back: $E moves to index 0 of its parent
    r("arrange.back", r##"{"ids": ["$E"]}"##, &[], "", &[("$E", "-", true)]),
    // align.left: several roots align to the union of their world bounds; moves $R/$X (frame.transform)
    r("align.left", r##"{"ids": ["$R", "$X"]}"##, &[], "", &[("$R", "frame", true), ("$X", "frame", true)]),
    // align.left: one layer aligns to its parent frame's world bounds ($LI in $D)
    r("align.left", r##"{"ids": ["$LI"]}"##, &[], "", &[("$LI", "frame", true), ("$D", "frame", true)]),
    // align.hcenter: union bounds of $R,$X, centres aligned
    r("align.hcenter", r##"{"ids": ["$R", "$X"]}"##, &[], "", &[("$R", "frame", true), ("$X", "frame", true)]),
    // align.right: union bounds, right edges aligned
    r("align.right", r##"{"ids": ["$R", "$X"]}"##, &[], "", &[("$R", "frame", true), ("$X", "frame", true)]),
    // align.top: union bounds, top edges aligned
    r("align.top", r##"{"ids": ["$R", "$E"]}"##, &[], "", &[("$R", "frame", true), ("$E", "frame", true)]),
    // align.vcenter: union bounds, vertical centres aligned
    r("align.vcenter", r##"{"ids": ["$R", "$E"]}"##, &[], "", &[("$R", "frame", true), ("$E", "frame", true)]),
    // align.bottom: union bounds, bottom edges aligned
    r("align.bottom", r##"{"ids": ["$R", "$E"]}"##, &[], "", &[("$R", "frame", true), ("$E", "frame", true)]),
    // distribute.horizontal: 3 roots sorted by x, equal gaps from their span and widths
    r("distribute.horizontal", r##"{"ids": ["$R", "$F", "$X"]}"##, &[], "", &[("$R", "frame", true), ("$F", "frame", true), ("$X", "frame", true)]),
    // distribute.horizontal: explicit spacing (2 roots allowed), from the first's left edge
    r("distribute.horizontal", r##"{"ids": ["$R", "$X"], "spacing": 10}"##, &[], "", &[("$R", "frame", true), ("$X", "frame", true)]),
    // distribute.vertical: 3 roots sorted by y, equal gaps
    r("distribute.vertical", r##"{"ids": ["$R", "$E", "$S"]}"##, &[], "", &[("$R", "frame", true), ("$E", "frame", true), ("$S", "frame", true)]),
    // arrange.tidy: roots grouped into rows by centre overlap and laid on a grid of max w/h + gap
    r("arrange.tidy", r##"{"ids": ["$R", "$E"]}"##, &[], "", &[("$R", "frame", true), ("$E", "frame", true)]),
    // arrange.packH: distribute with spacing 0 along x
    r("arrange.packH", r##"{"ids": ["$R", "$X"]}"##, &[], "", &[("$R", "frame", true), ("$X", "frame", true)]),
    // arrange.packV: distribute with spacing 0 along y
    r("arrange.packV", r##"{"ids": ["$R", "$E"]}"##, &[], "", &[("$R", "frame", true), ("$E", "frame", true)]),
    // arrange.roundToPixels: local translation and size rounded (no ancestors read)
    r("arrange.roundToPixels", r##"{"ids": ["$R"]}"##, &[], "", &[("$R", "frame", false)]),
    // node.create: no parent -> on the current page, at the top; new layer only
    r("node.create", r##"{"type": "rectangle", "x": 10, "y": 10, "width": 20, "height": 20}"##, &[], "part", &[("$P1", "-", false)]),
    // Reviewed after behavior tests: pinned from the declaration after review; create.rs:313
    // also reads ui_dark for type=text's default fill. This path was absent from the blind rows.
    p("", "node.create", r##"{"type": "text", "text": "Hi", "x": 10, "y": 900}"##, &[], "part,mode", &[("$P1", "-", false)]),
    // node.create: parent $F (auto layout): added at the top of $F, re-flow of $F places it and moves siblings
    r(
        "node.create",
        r##"{"type": "rectangle", "parent": "$F", "width": 10, "height": 10}"##,
        &[],
        "",
        &[("$F", "layout,frame,childLayout", false), ("$A", "frame,visible,childLayout", true), ("$T", "frame,visible,childLayout,kind,text", true)],
    ),
    // node.create: parent $G (group): x/y in the group's frame-relative space; the group refits to its children
    r(
        "node.create",
        r##"{"type": "ellipse", "parent": "$G", "x": 0, "y": 0, "width": 10, "height": 10}"##,
        &[],
        "",
        &[("$G", "frame", true), ("$G1", "frame", true), ("$G2", "frame", true)],
    ),
    // node.create: parent $D at index 0 (below $LI)
    r(
        "node.create",
        r##"{"type": "ellipse", "parent": "$D", "index": 0}"##,
        &[],
        "",
        &[("$D", "-", false), ("slot:$D", "-", true), ("$LI", "-", true)],
    ),
    // frame.create: explicit position on the current page
    r("frame.create", r##"{"x": 0, "y": 2000, "width": 100, "height": 100}"##, &[], "part", &[("$P1", "-", false)]),
    // frame.create: preset without position: placed 100 right of the right-most top-level layer (all top-level frames read)
    r(
        "frame.create",
        r##"{"preset": "iPhone 16"}"##,
        &[],
        "part",
        &[
            ("$P1", "~hierarchy", false),
            ("$D", "frame", false),
            ("$R", "frame", false),
            ("$E", "frame", false),
            ("$F", "frame", false),
            ("$G", "frame", false),
            ("$V", "frame", false),
            ("$K", "frame", false),
            ("$I", "frame", false),
            ("$X", "frame", false),
            ("$S", "frame", false),
            ("$Set", "frame", false),
        ],
    ),
    // shape.rectangle: node.create type rectangle on the current page
    r("shape.rectangle", r##"{"x": 0, "y": 2000, "width": 50, "height": 50}"##, &[], "part", &[("$P1", "-", false)]),
    // shape.rectangle: in frame $X (no layout, not a group: no geometry of $X read)
    r("shape.rectangle", r##"{"x": 10, "y": 10, "width": 50, "height": 50, "parent": "$X"}"##, &[], "", &[("$X", "-", false)]),
    // shape.ellipse: node.create type ellipse on the current page
    r("shape.ellipse", r##"{"x": 0, "y": 2000, "width": 50, "height": 50}"##, &[], "part", &[("$P1", "-", false)]),
    // shape.polygon: node.create type polygon on the current page
    r("shape.polygon", r##"{"x": 0, "y": 2000, "width": 50, "height": 50}"##, &[], "part", &[("$P1", "-", false)]),
    // shape.star: node.create type star on the current page
    r("shape.star", r##"{"x": 0, "y": 2000, "width": 50, "height": 50}"##, &[], "part", &[("$P1", "-", false)]),
    // shape.line: node.create type line (height 0) on the current page
    r("shape.line", r##"{"x": 0, "y": 2000, "width": 80}"##, &[], "part", &[("$P1", "-", false)]),
    // shape.arrow: line with an arrow end cap on the current page
    r("shape.arrow", r##"{"x": 0, "y": 2000, "width": 80}"##, &[], "part", &[("$P1", "-", false)]),
    // frame.swapOrientation: $X (empty) width<->height via resize_cmd; sizing becomes fixed
    r("frame.swapOrientation", r##"{"ids": ["$X"]}"##, &[], "", &[("$X", "frame,childLayout", false)]),
    // frame.swapOrientation: $D resized; its child $LI follows its constraints
    r("frame.swapOrientation", r##"{"ids": ["$D"]}"##, &[], "", &[("$D", "frame,childLayout", false), ("$LI", "constraints,frame", true)]),
    // transform.nudge: selection ($R via on), non-flowing: local translation by dx * nudge preference; parent checked for auto layout flow
    r("transform.nudge", r##"{"dx": 1, "dy": 0}"##, &["$R"], "selection,mode", &[("$R", "frame.transform,visible,childLayout", true)]),
    // transform.nudge: selection ($A via on) flows in $F (step = nudge preference): moves one place along the flow (swaps with $T in $F's child list); re-flow
    r(
        "transform.nudge",
        r##"{"dx": 0, "dy": 1}"##,
        &["$A"],
        "selection,mode",
        &[
            ("$A", "frame,visible,childLayout", true),
            ("$F", "layout,frame,childLayout,~hierarchy", false),
            ("$T", "frame,visible,childLayout,kind,text", true),
        ],
    ),
    // transform.nudgeSize: selection ($R via on): width += nudge preference, via resize_cmd (top-left kept, sizing fixed)
    r("transform.nudgeSize", r##"{"dw": 1, "dh": 0}"##, &["$R"], "selection,mode", &[("$R", "frame,childLayout", false)]),
    // node.set: opacity replaced; reviewed against styles::detach_edited, which checks bindings.
    r("node.set", r##"{"ids": ["$R"], "props": {"opacity": 0.5}}"##, &[], "", &[("$R", "opacity,bindings.bound", false)]),
    // node.set: name replaced
    r("node.set", r##"{"ids": ["$E"], "props": {"name": "Dot"}}"##, &[], "", &[("$E", "name", false)]),
    // node.set: fill replaced; the fill's variable binding is dropped when the colour changed (reads old fills and bound)
    // Reviewed after comparison: normalize also compares the layer's style link.
    r("node.set", r##"{"ids": ["$R"], "props": {"fill": "#00ff00"}}"##, &[], "", &[("$R", "fills,bindings.bound,bindings.styles", false)]),
    // node.set: x of $G1 inside group $G: in the group's frame-relative space; group refits to $G1,$G2
    // Reviewed after comparison: doc::fit_to_children (doc/src/lib.rs:790-796) reads G1's
    // local rectangle as well as its transform when recomputing the group's bounds.
    r("node.set", r##"{"ids": ["$G1"], "props": {"x": 5}}"##, &[], "", &[("$G1", "frame", true), ("$G", "frame", true), ("$G2", "frame", true)]),
    // node.set: width of auto layout frame $F: resize (constrainProportions read), sizing fixed, re-flow of its children
    // Reviewed after comparison: resize_node (props.rs:1241) and layout_frame enumerate the
    // children; membership changes can introduce another constrained or flowing child.
    r(
        "node.set",
        r##"{"ids": ["$F"], "props": {"width": 250}}"##,
        &[],
        "",
        &[
            ("$F", "frame,layout,childLayout,props.constrain_proportions,~hierarchy", false),
            ("$A", "frame,visible,childLayout,constraints", true),
            ("$T", "frame,visible,childLayout,constraints,kind,text", true),
        ],
    ),
    // node.reparent: $E into $X on top (world position kept: reads $E's and $X's world transforms)
    r("node.reparent", r##"{"ids": ["$E"], "parent": "$X"}"##, &[], "", &[("$E", "frame.transform", true), ("$X", "frame.transform", true)]),
    // node.reparent: $R into $D at index 0 (lands under $LI, the sibling at that index)
    r(
        "node.reparent",
        r##"{"ids": ["$R"], "parent": "$D", "index": 0}"##,
        &[],
        "",
        &[("$R", "frame.transform", true), ("$D", "frame.transform", true), ("slot:$D", "-", true), ("$LI", "-", true)],
    ),
    // node.reorder: $R moved to index 3 of $P1's children (after removal: just below $G)
    r("node.reorder", r##"{"id": "$R", "index": 3}"##, &[], "", &[("$R", "-", true), ("slot:$P1", "-", true), ("$G", "-", true)]),
    // arrange.spacing: $R,$E form a column (one vertical track each); the gap becomes 20, $E moves
    r("arrange.spacing", r##"{"ids": ["$R", "$E"], "vertical": 20}"##, &[], "", &[("$R", "frame", true), ("$E", "frame", true)]),
    // arrange.reorder: column $R,$E: $R inserted at $E's slot, both re-placed keeping sizes and gap
    r("arrange.reorder", r##"{"ids": ["$R", "$E"], "source": "$R", "target": "$E"}"##, &[], "", &[("$R", "frame", true), ("$E", "frame", true)]),
    // transform.move: $R moved in page units (through its ancestors)
    r("transform.move", r##"{"ids": ["$R"], "dx": 5, "dy": 5}"##, &[], "", &[("$R", "frame.transform", true)]),
    // transform.move: $G1 in group $G: projected through $G; $G refits to $G1,$G2
    r("transform.move", r##"{"ids": ["$G1"], "dx": 5, "dy": 0}"##, &[], "", &[("$G1", "frame", true), ("$G", "frame", true), ("$G2", "frame", true)]),
    // transform.moveTo: union bounds top-left of $R,$E moved to x,y
    r("transform.moveTo", r##"{"ids": ["$R", "$E"], "x": 10, "y": 10}"##, &[], "", &[("$R", "frame", true), ("$E", "frame", true)]),
    // transform.rotate: no cx/cy: pivot = moved rotation origin when the selection equals ids, else the centre of $R's bounds
    r("transform.rotate", r##"{"ids": ["$R"], "degrees": 45}"##, &[], "selection,mode", &[("$R", "frame", true)]),
    // transform.rotate: pivot given: world transform rotated about it
    r("transform.rotate", r##"{"ids": ["$R"], "degrees": 45, "cx": 0, "cy": 0}"##, &[], "", &[("$R", "frame", true)]),
    // transform.scale: $R and its paint/stroke/effects/corners/layout values scaled by 2 about its top-left
    r(
        "transform.scale",
        r##"{"ids": ["$R"], "factor": 2}"##,
        &[],
        "",
        &[("$R", "frame,fills,strokes,stroke,corners,effects,childLayout,layout,kind,props.grids,props.guides", true)],
    ),
    // transform.scale: group $G: whole subtree scaled (children's translations too)
    r(
        "transform.scale",
        r##"{"ids": ["$G"], "factor": 2}"##,
        &[],
        "",
        &[
            ("$G", "frame,fills,strokes,stroke,corners,effects,childLayout,layout,kind,props.grids,props.guides", true),
            ("$G1", "frame,fills,strokes,stroke,corners,effects,childLayout,layout,kind,props.grids,props.guides", true),
            ("$G2", "frame,fills,strokes,stroke,corners,effects,childLayout,layout,kind,props.grids,props.guides", true),
        ],
    ),
    // transform.resize: $R width set, top-left anchor kept, sizing fixed
    r("transform.resize", r##"{"ids": ["$R"], "width": 200}"##, &[], "", &[("$R", "frame,childLayout", false)]),
    // transform.resize: $D resized about its centre; child $LI follows its constraints
    r(
        "transform.resize",
        r##"{"ids": ["$D"], "width": 1000, "height": 800, "anchor": "c"}"##,
        &[],
        "",
        &[("$D", "frame,childLayout", false), ("$LI", "constraints,frame", true)],
    ),
    // section.toggleContents: no `hidden`: hide unless all hidden (reads $S's kind), writes contents_hidden
    r("section.toggleContents", r##"{"ids": ["$S"]}"##, &[], "", &[("$S", "kind", false)]),
    // section.toggleContents: explicit hidden: contents_hidden replaced (kind register)
    r("section.toggleContents", r##"{"ids": ["$S"], "hidden": true}"##, &[], "", &[("$S", "kind", false)]),
    // layout.setTrack: grid frame $X (made by a step): column 0 becomes fill; rewrites $X.layout's track list
    s(
        r##"[["node.set", {"ids": ["$X"], "props": {"layout": "grid", "gridColumns": 2, "gridRows": 1}}]]"##,
        "layout.setTrack",
        r##"{"ids": ["$X"], "axis": "columns", "index": 0, "sizing": "fill"}"##,
        &[],
        "",
        &[("$X", "layout,frame", false)],
    ),
    // UNSURE: layout.setTrack fixed without value: current track size from grid_geometry ($X's frame and children); $X is empty here
    s(
        r##"[["node.set", {"ids": ["$X"], "props": {"layout": "grid", "gridColumns": 2, "gridRows": 1}}]]"##,
        "layout.setTrack",
        r##"{"ids": ["$X"], "axis": "columns", "index": 1, "sizing": "fixed"}"##,
        &[],
        "",
        &[("$X", "layout,frame,~hierarchy", false)],
    ),
    // guide.add: no frame/page: appended to the current page's guides (page kind)
    r("guide.add", r##"{"vertical": true, "offset": 100}"##, &[], "part", &[("$P1", "kind", false)]),
    // guide.add: on top-level frame $D (props.guides)
    r("guide.add", r##"{"vertical": false, "offset": 50, "frame": "$D"}"##, &[], "", &[("$D", "props.guides", false)]),
    // guide.move: guide 0 of $D gets a new offset
    s(
        r##"[["guide.add", {"vertical": true, "offset": 10, "frame": "$D"}]]"##,
        "guide.move",
        r##"{"index": 0, "offset": 60, "frame": "$D"}"##,
        &[],
        "",
        &[("$D", "props.guides", false)],
    ),
    // guide.remove: guide 0 of $D removed
    s(
        r##"[["guide.add", {"vertical": true, "offset": 10, "frame": "$D"}]]"##,
        "guide.remove",
        r##"{"index": 0, "frame": "$D"}"##,
        &[],
        "",
        &[("$D", "props.guides", false)],
    ),
    // guide.remove: no index: the selected guide (editor state) of the current page is removed
    s(
        r##"[["guide.add", {"vertical": true, "offset": 10}], ["guide.select", {"index": 0}]]"##,
        "guide.remove",
        r##"{}"##,
        &[],
        "mode",
        &[("$P1", "kind", false)],
    ),
    // guide.clear: all guides of $D removed (list replaced)
    s(
        r##"[["guide.add", {"vertical": true, "offset": 10, "frame": "$D"}]]"##,
        "guide.clear",
        r##"{"frame": "$D"}"##,
        &[],
        "",
        &[("$D", "props.guides", false)],
    ),
    // guide.clear: no owner: the current page's guides cleared
    r("guide.clear", r##"{}"##, &[], "part", &[("$P1", "kind", false)]),
    // ---- Reading 3 ----
    // text.bold: ids names $T; toggle reads every span's weight, restyles all runs, detaches changed bindings; auto-size + $F reflow
    r(
        "text.bold",
        r##"{"ids": ["$T"]}"##,
        &[],
        "",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.bold: no ids -> the selection; same reads on the selected text
    r(
        "text.bold",
        r##"{}"##,
        &["$T"],
        "selection",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.bold: ids names container $F: toggle checks $F itself (not text), restyles the texts in its subtree ($T)
    r(
        "text.bold",
        r##"{"ids": ["$F"]}"##,
        &[],
        "",
        &[
            ("$F", "~hierarchy", false),
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.bold: $KT inside main $K: restyle propagates to instance $I's $IT unless overridden
    r(
        "text.bold",
        r##"{"ids": ["$KT"]}"##,
        &[],
        "",
        &[
            (
                "$KT",
                "kind,text,frame.width,frame.height,childLayout.sizing_h,childLayout.sizing_v,bindings.bound,bindings.styles,instance.prop_refs",
                false,
            ),
            ("$K", "layout,props", false),
            ("$I", "kind,instance.overrides", false),
            ("$IT", "kind,text,instance.overrides,mirrors,frame", false),
        ],
    ),
    // text.bold: in a text edit session the edited layer's chars 0..2 (ids ignored); toggle reads their style
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 0}], ["text.edit.caret", {"index": 2, "extend": true}]]"##,
        "text.bold",
        r##"{}"##,
        &[],
        "mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.italic: ids names $T; toggle on italic of every span; writes all runs; auto-size + reflow
    r(
        "text.italic",
        r##"{"ids": ["$T"]}"##,
        &[],
        "",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.italic: edit session: chars 0..2 of the edited layer, toggle on italic of every span over that range
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 0}], ["text.edit.caret", {"index": 2, "extend": true}]]"##,
        "text.italic",
        r##"{}"##,
        &[],
        "mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.underline: ids names $T; toggle on decoration==underline; writes all runs; auto-size + reflow
    r(
        "text.underline",
        r##"{"ids": ["$T"]}"##,
        &[],
        "",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.underline: edit session: chars 0..2 of the edited layer, toggle on decoration==underline over that range
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 0}], ["text.edit.caret", {"index": 2, "extend": true}]]"##,
        "text.underline",
        r##"{}"##,
        &[],
        "mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.strikethrough: ids names $T; toggle on decoration==strikethrough; writes all runs; auto-size + reflow
    r(
        "text.strikethrough",
        r##"{"ids": ["$T"]}"##,
        &[],
        "",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.strikethrough: edit session: chars 0..2 of the edited layer, toggle on decoration==strikethrough over that range
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 0}], ["text.edit.caret", {"index": 2, "extend": true}]]"##,
        "text.strikethrough",
        r##"{}"##,
        &[],
        "mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.link: ids names $T; writes link on all runs (replaces existing links)
    r(
        "text.link",
        r##"{"url": "https://example.com", "ids": ["$T"]}"##,
        &[],
        "",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.link: edit session: link on chars 0..2 of the edited layer
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 0}], ["text.edit.caret", {"index": 2, "extend": true}]]"##,
        "text.link",
        r##"{"url": "https://example.com"}"##,
        &[],
        "mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.bulletList: ids names $T; toggle reads the paragraphs' common list; sets every paragraph's list
    r(
        "text.bulletList",
        r##"{"ids": ["$T"]}"##,
        &[],
        "",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.bulletList: edit session: the paragraphs of the selected range of the edited layer
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 0}], ["text.edit.caret", {"index": 2, "extend": true}]]"##,
        "text.bulletList",
        r##"{}"##,
        &[],
        "mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.numberList: ids names $T; toggle reads the paragraphs' common list; sets every paragraph's list
    r(
        "text.numberList",
        r##"{"ids": ["$T"]}"##,
        &[],
        "",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.numberList: edit session: the paragraphs of the selected range of the edited layer
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 0}], ["text.edit.caret", {"index": 2, "extend": true}]]"##,
        "text.numberList",
        r##"{}"##,
        &[],
        "mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.alignLeft: ids names $T (and texts in its subtree); writes align_h; normalize reflow
    // review: unchanged fixture text does not schedule reflow (derived.rs:148).
    r(
        "text.alignLeft",
        r##"{"ids": ["$T"]}"##,
        &[],
        "",
        &[("$T", "kind,text,frame.width,frame.height,childLayout.sizing_h,childLayout.sizing_v,bindings.styles", false)],
    ),
    // text.alignLeft: edit session: the whole edited layer (ids ignored)
    // review: unchanged fixture text does not schedule reflow (derived.rs:148).
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 0}], ["text.edit.caret", {"index": 2, "extend": true}]]"##,
        "text.alignLeft",
        r##"{}"##,
        &[],
        "mode",
        &[("$T", "kind,text,frame.width,frame.height,childLayout.sizing_h,childLayout.sizing_v,bindings.styles", false)],
    ),
    // text.alignCenter: ids names $T (and texts in its subtree); writes align_h; normalize reflow
    r(
        "text.alignCenter",
        r##"{"ids": ["$T"]}"##,
        &[],
        "",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.alignCenter: edit session: the whole edited layer (ids ignored)
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 0}], ["text.edit.caret", {"index": 2, "extend": true}]]"##,
        "text.alignCenter",
        r##"{}"##,
        &[],
        "mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.alignRight: ids names $T (and texts in its subtree); writes align_h; normalize reflow
    r(
        "text.alignRight",
        r##"{"ids": ["$T"]}"##,
        &[],
        "",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.alignRight: edit session: the whole edited layer (ids ignored)
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 0}], ["text.edit.caret", {"index": 2, "extend": true}]]"##,
        "text.alignRight",
        r##"{}"##,
        &[],
        "mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.alignJustify: ids names $T (and texts in its subtree); writes align_h; normalize reflow
    r(
        "text.alignJustify",
        r##"{"ids": ["$T"]}"##,
        &[],
        "",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.alignJustify: edit session: the whole edited layer (ids ignored)
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 0}], ["text.edit.caret", {"index": 2, "extend": true}]]"##,
        "text.alignJustify",
        r##"{}"##,
        &[],
        "mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.sizeUp: ids names $T; reads each run's font size, +1; detaches changed bindings; auto-size + $F reflow
    r(
        "text.sizeUp",
        r##"{"ids": ["$T"]}"##,
        &[],
        "",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.sizeUp: edit session: chars 0..2 of the edited layer (reads each run's font size, +1)
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 0}], ["text.edit.caret", {"index": 2, "extend": true}]]"##,
        "text.sizeUp",
        r##"{}"##,
        &[],
        "mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.sizeDown: ids names $T; reads each run's font size, -1; detaches changed bindings; auto-size + $F reflow
    r(
        "text.sizeDown",
        r##"{"ids": ["$T"]}"##,
        &[],
        "",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.sizeDown: edit session: chars 0..2 of the edited layer (reads each run's font size, -1)
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 0}], ["text.edit.caret", {"index": 2, "extend": true}]]"##,
        "text.sizeDown",
        r##"{}"##,
        &[],
        "mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.weightUp: ids names $T; reads each run's weight and italic, next weight; detaches changed bindings; auto-size + $F reflow
    r(
        "text.weightUp",
        r##"{"ids": ["$T"]}"##,
        &[],
        "",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.weightUp: edit session: chars 0..2 of the edited layer (reads each run's weight and italic, next weight)
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 0}], ["text.edit.caret", {"index": 2, "extend": true}]]"##,
        "text.weightUp",
        r##"{}"##,
        &[],
        "mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.weightDown: ids names $T; reads each run's weight and italic, previous weight; detaches changed bindings; auto-size + $F reflow
    r(
        "text.weightDown",
        r##"{"ids": ["$T"]}"##,
        &[],
        "",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.weightDown: edit session: chars 0..2 of the edited layer (reads each run's weight and italic, previous weight)
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 0}], ["text.edit.caret", {"index": 2, "extend": true}]]"##,
        "text.weightDown",
        r##"{}"##,
        &[],
        "mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.lineHeightUp: ids names $T; reads each run's line height and font size, px +1; detaches changed bindings; auto-size + $F reflow
    r(
        "text.lineHeightUp",
        r##"{"ids": ["$T"]}"##,
        &[],
        "",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.lineHeightUp: edit session: chars 0..2 of the edited layer (reads each run's line height and font size, px +1)
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 0}], ["text.edit.caret", {"index": 2, "extend": true}]]"##,
        "text.lineHeightUp",
        r##"{}"##,
        &[],
        "mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.lineHeightDown: ids names $T; reads each run's line height and font size, px -1; detaches changed bindings; auto-size + $F reflow
    r(
        "text.lineHeightDown",
        r##"{"ids": ["$T"]}"##,
        &[],
        "",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.lineHeightDown: edit session: chars 0..2 of the edited layer (reads each run's line height and font size, px -1)
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 0}], ["text.edit.caret", {"index": 2, "extend": true}]]"##,
        "text.lineHeightDown",
        r##"{}"##,
        &[],
        "mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.trackingUp: ids names $T; reads each run's letter spacing and font size, +1%; detaches changed bindings; auto-size + $F reflow
    r(
        "text.trackingUp",
        r##"{"ids": ["$T"]}"##,
        &[],
        "",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.trackingUp: edit session: chars 0..2 of the edited layer (reads each run's letter spacing and font size, +1%)
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 0}], ["text.edit.caret", {"index": 2, "extend": true}]]"##,
        "text.trackingUp",
        r##"{}"##,
        &[],
        "mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.trackingDown: ids names $T; reads each run's letter spacing and font size, -1%; detaches changed bindings; auto-size + $F reflow
    r(
        "text.trackingDown",
        r##"{"ids": ["$T"]}"##,
        &[],
        "",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.trackingDown: edit session: chars 0..2 of the edited layer (reads each run's letter spacing and font size, -1%)
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 0}], ["text.edit.caret", {"index": 2, "extend": true}]]"##,
        "text.trackingDown",
        r##"{}"##,
        &[],
        "mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.caseOriginal: ids names $T; sets case; detaches changed bindings; auto-size + $F reflow
    // review: unchanged fixture text does not schedule reflow (derived.rs:148).
    r(
        "text.caseOriginal",
        r##"{"ids": ["$T"]}"##,
        &[],
        "",
        &[("$T", "kind,text,frame.width,frame.height,childLayout.sizing_h,childLayout.sizing_v,bindings.bound,bindings.styles", false)],
    ),
    // text.caseOriginal: edit session: chars 0..2 of the edited layer (sets case)
    // review: unchanged fixture text does not schedule reflow (derived.rs:148).
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 0}], ["text.edit.caret", {"index": 2, "extend": true}]]"##,
        "text.caseOriginal",
        r##"{}"##,
        &[],
        "mode",
        &[("$T", "kind,text,frame.width,frame.height,childLayout.sizing_h,childLayout.sizing_v,bindings.bound,bindings.styles", false)],
    ),
    // text.caseUpper: ids names $T; sets case; detaches changed bindings; auto-size + $F reflow
    r(
        "text.caseUpper",
        r##"{"ids": ["$T"]}"##,
        &[],
        "",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.caseUpper: edit session: chars 0..2 of the edited layer (sets case)
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 0}], ["text.edit.caret", {"index": 2, "extend": true}]]"##,
        "text.caseUpper",
        r##"{}"##,
        &[],
        "mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.caseLower: ids names $T; sets case; detaches changed bindings; auto-size + $F reflow
    r(
        "text.caseLower",
        r##"{"ids": ["$T"]}"##,
        &[],
        "",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.caseLower: edit session: chars 0..2 of the edited layer (sets case)
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 0}], ["text.edit.caret", {"index": 2, "extend": true}]]"##,
        "text.caseLower",
        r##"{}"##,
        &[],
        "mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.caseTitle: ids names $T; sets case; detaches changed bindings; auto-size + $F reflow
    r(
        "text.caseTitle",
        r##"{"ids": ["$T"]}"##,
        &[],
        "",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.caseTitle: edit session: chars 0..2 of the edited layer (sets case)
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 0}], ["text.edit.caret", {"index": 2, "extend": true}]]"##,
        "text.caseTitle",
        r##"{}"##,
        &[],
        "mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // UNSURE: text.create: no parent -> the current page; new text node on top (when the session knows the UI theme, ui_dark, the page background / parent fills are read to pick white text; not modelled here)
    // Reviewed after behavior tests: create.rs:313 reads ui_dark even though this headless
    // fixture has no theme; the command-wide editor declaration must include that UI path.
    r("text.create", r##"{"text": "Hi", "x": 10, "y": 900}"##, &[], "part,mode", &[("$P1", "-", false)]),
    // text.create: parent $F (auto layout): appended, $F re-flows with its children
    // Reviewed after comparison: layout_frame calls fit_hug_text (layout/src/lib.rs:254,352),
    // which remeasures the existing text sibling from its kind/content and resize mode.
    r(
        "text.create",
        r##"{"text": "Hi", "x": 0, "y": 0, "parent": "$F"}"##,
        &[],
        "",
        &[("$F", TEXT_FLOW_PARENT, false), ("$A", "frame,visible,childLayout", true), ("$T", "kind,text,frame,visible,childLayout", true)],
    ),
    // text.create: parent group $G: x/y in $G's frame space (ancestors' transforms), $G refits to its children
    // Reviewed after behavior tests: doc::box_children reads visibility only on masks, and
    // these children are not masks. Group/child placement supplies frame space and mask order.
    r(
        "text.create",
        r##"{"text": "Hi", "x": 0, "y": 250, "parent": "$G"}"##,
        &[],
        "",
        &[("$G", "frame", true), ("$G1", "frame", true), ("$G2", "frame", true)],
    ),
    // text.onPath: text $T and path $V by id: $T takes $V's slot and geometry (and its fills/effects/styles/bindings when shown), $V removed; $F re-flows without $T
    r(
        "text.onPath",
        r##"{"text": "$T", "path": "$V", "start": 0.2}"##,
        &[],
        "",
        &[
            ("$V", "kind,frame,fills,effects,bindings.styles,bindings.bound", true),
            ("$T", "kind,text,frame,fills,effects,bindings.styles,bindings.bound", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.onPath: no params -> the selection (set by select.set): first selected text and first other layer
    s(
        r##"[["select.set", {"ids": ["$T", "$V"]}]]"##,
        "text.onPath",
        r##"{}"##,
        &[],
        "selection",
        &[
            ("$V", "kind,frame,fills,effects,bindings.styles,bindings.bound", true),
            ("$T", "kind,text,frame,fills,effects,bindings.styles,bindings.bound", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.onPath: path is ellipse $E: geometry from its kind (arc params) and size; its fill + style $Brand move to $T
    r(
        "text.onPath",
        r##"{"text": "$T", "path": "$E"}"##,
        &[],
        "",
        &[
            ("$E", "kind,frame,corners,fills,effects,bindings.styles,bindings.bound", true),
            ("$T", "kind,text,frame,fills,effects,bindings.styles,bindings.bound", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.pathOptions: text on a path: reads its path start/flip (kept when absent), writes them
    // Reviewed after comparison: para normalizes (text.rs:47-65); sync_text_styles reads the
    // layer's text-style link and can materialize it in the runs (styles.rs:705-739).
    s(
        r##"[["text.onPath", {"text": "$T", "path": "$V"}]]"##,
        "text.pathOptions",
        r##"{"start": 0.5, "flip": true, "ids": ["$T"]}"##,
        &[],
        "",
        &[("$T", "kind,text,frame.width,frame.height,childLayout.sizing_h,childLayout.sizing_v,bindings.styles", false)],
    ),
    // text.removeFromPath: path taken off, auto_resize -> width and height; auto-size writes $T's size
    s(
        r##"[["text.onPath", {"text": "$T", "path": "$V"}]]"##,
        "text.removeFromPath",
        r##"{"ids": ["$T"]}"##,
        &[],
        "",
        &[("$T", "kind,text,frame.width,frame.height,childLayout.sizing_h,childLayout.sizing_v,bindings.styles", false)],
    ),
    // text.caseSmallCaps: ids names $T; sets case small caps
    r(
        "text.caseSmallCaps",
        r##"{"ids": ["$T"]}"##,
        &[],
        "",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.caseSmallCaps: edit session: chars 0..2
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 0}], ["text.edit.caret", {"index": 2, "extend": true}]]"##,
        "text.caseSmallCaps",
        r##"{}"##,
        &[],
        "mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.case: ids names $T; sets case on all runs
    r(
        "text.case",
        r##"{"case": "upper", "ids": ["$T"]}"##,
        &[],
        "",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.case: edit session: chars 0..2
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 0}], ["text.edit.caret", {"index": 2, "extend": true}]]"##,
        "text.case",
        r##"{"case": "title"}"##,
        &[],
        "mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.decoration: ids names $T; sets decoration (no toggle read)
    r(
        "text.decoration",
        r##"{"decoration": "underline", "ids": ["$T"]}"##,
        &[],
        "",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.decoration: edit session: chars 0..2
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 0}], ["text.edit.caret", {"index": 2, "extend": true}]]"##,
        "text.decoration",
        r##"{"decoration": "strikethrough"}"##,
        &[],
        "mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.listStyle: ids names $T; every paragraph's list set (no toggle)
    r(
        "text.listStyle",
        r##"{"style": "bulleted", "ids": ["$T"]}"##,
        &[],
        "",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.listStyle: edit session: the paragraphs of the selected range
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 0}], ["text.edit.caret", {"index": 2, "extend": true}]]"##,
        "text.listStyle",
        r##"{"style": "numbered"}"##,
        &[],
        "mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.paragraphIndent: ids names $T; writes paragraph indent
    r(
        "text.paragraphIndent",
        r##"{"indent": 10, "ids": ["$T"]}"##,
        &[],
        "",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.paragraphIndent: edit session: the whole edited layer (ids ignored)
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 0}], ["text.edit.caret", {"index": 2, "extend": true}]]"##,
        "text.paragraphIndent",
        r##"{"indent": 10}"##,
        &[],
        "mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.paragraphSpacing: ids names $T; writes paragraph spacing
    r(
        "text.paragraphSpacing",
        r##"{"spacing": 8, "ids": ["$T"]}"##,
        &[],
        "",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.maxLines: ids names $T; reads auto_resize, writes max lines and truncate mode
    r(
        "text.maxLines",
        r##"{"lines": 2, "ids": ["$T"]}"##,
        &[],
        "",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.setFeatures: no target param in the agent schema -> the selection ($T); merges into its features map
    s(
        r##"[["select.set", {"ids": ["$T"]}]]"##,
        "text.setFeatures",
        r##"{"features": {"liga": false}}"##,
        &[],
        "selection",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.setVariations: no target param in the agent schema -> the selection ($T); merges into each run's variations; detaches bindings
    s(
        r##"[["select.set", {"ids": ["$T"]}]]"##,
        "text.setVariations",
        r##"{"variations": {"wght": 500}}"##,
        &[],
        "selection",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.setVariations: edit session: chars 0..2 of the edited layer
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 0}], ["text.edit.caret", {"index": 2, "extend": true}]]"##,
        "text.setVariations",
        r##"{"variations": {"wght": 500}}"##,
        &[],
        "mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.set: id names $T; replaces all characters (keeps first style/paragraph), marks characters override; auto-size + reflow
    r(
        "text.set",
        r##"{"id": "$T", "characters": "Hi there"}"##,
        &[],
        "",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.set: no id -> the selection
    r(
        "text.set",
        r##"{"characters": "Hi there"}"##,
        &["$T"],
        "selection",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.set: $KT in main $K: characters propagate to instance text $IT unless overridden
    r(
        "text.set",
        r##"{"id": "$KT", "characters": "Go"}"##,
        &[],
        "",
        &[
            (
                "$KT",
                "kind,text,frame.width,frame.height,childLayout.sizing_h,childLayout.sizing_v,bindings.bound,bindings.styles,instance.prop_refs",
                false,
            ),
            ("$K", "layout,props", false),
            ("$I", "kind,instance.overrides", false),
            ("$IT", "kind,text,instance.overrides,mirrors,frame", false),
        ],
    ),
    // text.style: id names $T; restyles chars start..end (clamped to its length); auto-size + reflow
    r(
        "text.style",
        r##"{"id": "$T", "start": 0, "end": 2, "fontSize": 24}"##,
        &[],
        "",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.style: fontFamily: keeps the nearest style the installed family has (reads each run's style)
    r(
        "text.style",
        r##"{"id": "$T", "fontFamily": "Inter"}"##,
        &[],
        "",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.edit.insert: edit session at caret 5: inserts at the caret/selection of the edited layer
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 5}]]"##,
        "text.edit.insert",
        r##"{"text": " world"}"##,
        &[],
        "mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.edit.insert: "- " typed at a paragraph start: reads the line's chars and paragraph list, turns it into a bulleted list
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 0}], ["text.edit.insert", {"text": "-"}]]"##,
        "text.edit.insert",
        r##"{"text": " "}"##,
        &[],
        "mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.edit.key: Backspace at caret 5 deletes the grapheme before it
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 5}]]"##,
        "text.edit.key",
        r##"{"key": "Backspace"}"##,
        &[],
        "mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.edit.key: Tab in a list item: the selected paragraphs one level deeper
    s(
        r##"[["text.edit.begin", {"id": "$T"}], ["text.bulletList", {}]]"##,
        "text.edit.key",
        r##"{"key": "Tab"}"##,
        &[],
        "mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.edit.key: Escape after emptying $T: ends the session and removes the empty layer; $F re-flows
    s(
        r##"[["text.edit.begin", {"id": "$T"}], ["text.edit.key", {"key": "Backspace"}]]"##,
        "text.edit.key",
        r##"{"key": "Escape"}"##,
        &[],
        "mode",
        &[("$T", "kind,text", true), ("$F", TEXT_FLOW_PARENT, false), ("$A", "frame,visible,childLayout", true)],
    ),
    // text.edit.key: Left: caret moves through the laid-out text; no design change
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 2}]]"##,
        "text.edit.key",
        r##"{"key": "Left"}"##,
        &[],
        "mode",
        &[("$T", "kind,text,frame.width", false)],
    ),
    // text.listIndent: edit session: the selected list paragraphs one level deeper
    s(
        r##"[["text.edit.begin", {"id": "$T"}], ["text.bulletList", {}]]"##,
        "text.listIndent",
        r##"{}"##,
        &[],
        "mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.listOutdent: edit session: selected list paragraphs at level 0 leave the list
    s(
        r##"[["text.edit.begin", {"id": "$T"}], ["text.bulletList", {}]]"##,
        "text.listOutdent",
        r##"{}"##,
        &[],
        "mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.refit: ids names $T: re-measures auto-sizing text in the subtree, writes size when it differs
    // Reviewed after behavior tests: fonts::refit normalizes only changed sizes; this text
    // already fits, so no parent or sibling is re-flowed.
    r("text.refit", r##"{"ids": ["$T"]}"##, &[], "", &[("$T", "kind,text,frame.width,frame.height", false)]),
    // UNSURE: text.refit: no ids: every text layer of the document (incl. instance and library-store copies); "design nodes" stands for "which text layers exist"
    // Reviewed after behavior tests: fonts::refit enumerates text identities, not the contents
    // of unrelated non-text layers; ~textNodes names exactly that set.
    r(
        "text.refit",
        r##"{}"##,
        &[],
        "",
        &[
            ("design", "~textNodes", false),
            ("$T", "kind,text,frame.width,frame.height", false),
            ("$KT", "kind,text,frame.width,frame.height", false),
            ("$IT", "kind,text,frame.width,frame.height", false),
        ],
    ),
    // text.fit: fixed-width text $T (an explicit width makes it auto-height and fixed-width in $F's layout): largest font size fitting its width (height when fixed); scales all runs; auto-height + reflow
    // review: unchanged fixture text does not schedule reflow (derived.rs:148).
    s(
        r##"[["node.set", {"id": "$T", "props": {"width": 200}}]]"##,
        "text.fit",
        r##"{"ids": ["$T"], "maxLines": 1, "minSize": 4}"##,
        &[],
        "",
        &[("$T", "kind,text,frame.width,frame.height,childLayout.sizing_h,childLayout.sizing_v,bindings.bound,bindings.styles", false)],
    ),
    // ---- Reading 4 ----
    // vector.flatten: one rectangle named by id; reads its outline (kind, size, corners) and look (name, fills, strokes, stroke, effects, opacity, blend), inserts a vector at its place and removes it; the comment thread pinned to it is re-anchored
    r(
        "vector.flatten",
        r##"{"ids": ["$R"]}"##,
        &[],
        "",
        &[("$R", "kind,frame,corners,name,fills,strokes,stroke,effects,opacity,props.blend,~hierarchy", true), ("$Th", "anchor", false)],
    ),
    // vector.flatten: a group; its visible children's outlines (kind, frame, visible) in the group's space, the group's look (name, fills empty -> default) and place; group and children removed
    // Reviewed after comparison: structure.rs:398-405 chooses subtree outlines by kind,
    // and :436 distinguishes a group's empty fill from other node kinds.
    r(
        "vector.flatten",
        r##"{"ids": ["$G"]}"##,
        &[],
        "",
        &[
            ("$G", "kind,frame,name,fills,strokes,stroke,effects,opacity,props.blend,~hierarchy", true),
            ("$G1", "kind,frame,visible,corners", false),
            ("$G2", "kind,frame,visible", false),
        ],
    ),
    // vector.outlineStroke: open vector with a stroke (added first): its stroke area (kind, frame, strokes, stroke) becomes a vector filled with the strokes and standing in for it (name, opacity, blend, effects, styles, constraints, childLayout, visible, locked) at its place; the layer is removed
    s(
        r##"[["stroke.add", {"ids": ["$V"]}]]"##,
        "vector.outlineStroke",
        r##"{"ids": ["$V"]}"##,
        &[],
        "",
        &[(
            "$V",
            "kind,frame,strokes,stroke,name,opacity,props.blend,effects,bindings.styles,constraints,childLayout,visible,locked,~hierarchy",
            true,
        )],
    ),
    // vector.outlineStroke: closed filled rectangle with a stroke: its strokes are cleared, the outline goes above it and both are wrapped in a new group at its place (reparent rewrites its frame)
    s(
        r##"[["stroke.add", {"ids": ["$R"]}]]"##,
        "vector.outlineStroke",
        r##"{"ids": ["$R"]}"##,
        &[],
        "",
        &[("$R", "kind,frame,corners,strokes,stroke,fills,bindings.styles,name,~hierarchy", true)],
    ),
    // bool.union: two layers outside boolean groups: wrapped in a new boolean group at the top one's place, looking like the top one ($E: fills, strokes, stroke, effects); box from their bounds; operands' own bool_op cleared
    r(
        "bool.union",
        r##"{"ids": ["$R", "$E"]}"##,
        &[],
        "",
        &[("$R", "frame,props.bool_op,~hierarchy", true), ("$E", "frame,fills,strokes,stroke,effects,props.bool_op,~hierarchy", true)],
    ),
    // bool.subtract: as union, but the group looks like the bottom layer ($R)
    r(
        "bool.subtract",
        r##"{"ids": ["$R", "$E"]}"##,
        &[],
        "",
        &[("$R", "frame,fills,strokes,stroke,effects,props.bool_op,~hierarchy", true), ("$E", "frame,props.bool_op,~hierarchy", true)],
    ),
    // bool.subtract on an operand of the boolean group $S1 (made by the before step): reads the group's operation (its kind) and the operand's place in it, sets the operand's own bool_op
    s(
        r##"[["bool.union", {"ids": ["$R", "$E"]}]]"##,
        "bool.subtract",
        r##"{"ids": ["$R"]}"##,
        &[],
        "",
        &[("$S1", "kind", false), ("$R", "props.bool_op,~hierarchy", true)],
    ),
    // bool.intersect: wrap path, looks like the top layer ($E)
    r(
        "bool.intersect",
        r##"{"ids": ["$R", "$E"]}"##,
        &[],
        "",
        &[("$R", "frame,props.bool_op,~hierarchy", true), ("$E", "frame,fills,strokes,stroke,effects,props.bool_op,~hierarchy", true)],
    ),
    // bool.exclude: wrap path, looks like the top layer ($E)
    r(
        "bool.exclude",
        r##"{"ids": ["$R", "$E"]}"##,
        &[],
        "",
        &[("$R", "frame,props.bool_op,~hierarchy", true), ("$E", "frame,fills,strokes,stroke,effects,props.bool_op,~hierarchy", true)],
    ),
    // bool.exclude on one boolean group $S1 (named by ids): sets its operation (kind) and its default name (name read: renamed only if an operation label), clears its operands' bool_op
    s(
        r##"[["bool.union", {"ids": ["$R", "$E"]}]]"##,
        "bool.exclude",
        r##"{"ids": ["$S1"]}"##,
        &[],
        "",
        &[("$S1", "kind,name", false), ("$R", "props.bool_op", false), ("$E", "props.bool_op", false)],
    ),
    // bool.flatten: same code as vector.flatten: one vector from both outlines at the top one's place, styled like the bottom one ($R); both removed, $R's comment re-anchored
    r(
        "bool.flatten",
        r##"{"ids": ["$R", "$E"]}"##,
        &[],
        "",
        &[
            ("$R", "kind,frame,corners,name,fills,strokes,stroke,effects,opacity,props.blend,~hierarchy", true),
            ("$E", "kind,frame,~hierarchy", true),
            ("$Th", "anchor", false),
        ],
    ),
    // vector.join (not editing, one open vector): closes it: reads and writes its network
    r("vector.join", r##"{"ids": ["$V"]}"##, &[], "", &[("$V", "kind", false)]),
    // vector.join (not editing, two layers): flattens them into one vector (as bool.flatten)
    r(
        "vector.join",
        r##"{"ids": ["$R", "$E"]}"##,
        &[],
        "",
        &[
            ("$R", "kind,frame,corners,name,fills,strokes,stroke,effects,opacity,props.blend,~hierarchy", true),
            ("$E", "kind,frame,~hierarchy", true),
            ("$Th", "anchor", false),
        ],
    ),
    // vector.join (editing $V, points 0 and 2 selected): joins the edited layer's selected points (params ignored); refit
    s(
        r##"[["vector.edit.begin", {"id": "$V"}], ["vector.edit.select", {"vertices": [0, 2]}]]"##,
        "vector.join",
        r##"{}"##,
        &[],
        "mode",
        &[("$V", "kind,frame", false)],
    ),
    // vector.smoothJoin (editing $V, points 0 and 2 selected): joins them with a curved segment; refit (the curve grows the box)
    s(
        r##"[["vector.edit.begin", {"id": "$V"}], ["vector.edit.select", {"vertices": [0, 2]}]]"##,
        "vector.smoothJoin",
        r##"{}"##,
        &[],
        "mode",
        &[("$V", "kind,frame", false)],
    ),
    // vector.smoothJoin (not editing, one open vector): closes it like vector.join
    r("vector.smoothJoin", r##"{"ids": ["$V"]}"##, &[], "", &[("$V", "kind", false)]),
    // vector.deleteHeal (editing $V): deletes point 1 of the active layer and heals the path through it
    s(r##"[["vector.edit.begin", {"id": "$V"}]]"##, "vector.deleteHeal", r##"{"vertices": [1]}"##, &[], "mode", &[("$V", "kind,frame", false)]),
    // vector.simplify: replaces the network by a simplified one (no refit)
    r("vector.simplify", r##"{"ids": ["$V"], "tolerance": 1}"##, &[], "", &[("$V", "kind", false)]),
    // vector.reverse: swaps each segment's ends and tangents in the network
    r("vector.reverse", r##"{"ids": ["$V"]}"##, &[], "", &[("$V", "kind", false)]),
    // vector.offset: the rectangle's outline (kind, size, corners) grown by the offset becomes its network; box moved and resized
    r("vector.offset", r##"{"ids": ["$R"], "offset": 5}"##, &[], "", &[("$R", "kind,frame,corners", false)]),
    // vector.create (no parent): a new vector on top of the current page
    r("vector.create", r##"{"path": "M0 0 L10 10"}"##, &[], "part", &[("$P1", "-", false)]),
    // vector.create (parent: auto layout frame $F): the new vector joins $F's flow, which re-flows from $F's layout and size and its children's sizes, visibility and child layout
    r(
        "vector.create",
        r##"{"path": "M0 0 L10 10", "parent": "$F"}"##,
        &[],
        "",
        &[("$F", TEXT_FLOW_PARENT, false), ("$A", "frame,visible,childLayout", true), ("$T", "frame,visible,childLayout", true)],
    ),
    // vector.fillRule ($V closed first): sets the winding of every region of its network
    s(r##"[["vector.join", {"ids": ["$V"]}]]"##, "vector.fillRule", r##"{"ids": ["$V"], "rule": "evenOdd"}"##, &[], "", &[("$V", "kind", false)]),
    // vector.fillRule with `region`: that region's winding only
    s(
        r##"[["vector.join", {"ids": ["$V"]}]]"##,
        "vector.fillRule",
        r##"{"id": "$V", "region": 0, "rule": "evenOdd"}"##,
        &[],
        "",
        &[("$V", "kind", false)],
    ),
    // vector.reverseLoop ($V closed first): reverses loop 0 of region 0 of its network
    s(
        r##"[["vector.join", {"ids": ["$V"]}]]"##,
        "vector.reverseLoop",
        r##"{"ids": ["$V"], "region": 0, "loop": 0}"##,
        &[],
        "",
        &[("$V", "kind", false)],
    ),
    // vector.edit.begin on a vector: no document change, enters edit mode (decided by its kind)
    r("vector.edit.begin", r##"{"id": "$V"}"##, &[], "", &[("$V", "kind", false)]),
    // vector.edit.begin on a rectangle: converts it to a vector from its outline (kind, size, corners), corners reset, box refitted
    r("vector.edit.begin", r##"{"id": "$R"}"##, &[], "", &[("$R", "kind,frame,corners", false)]),
    // vector.edit.begin with no id: the selection; the ellipse is converted
    r("vector.edit.begin", r##"{}"##, &["$E"], "selection", &[("$E", "kind,frame", false)]),
    // vector.edit.end: leaves edit mode; an edited layer left without segments would be removed (reads the network); nothing written here
    s(r##"[["vector.edit.begin", {"id": "$V"}]]"##, "vector.edit.end", r##"{}"##, &[], "mode", &[("$V", "kind", false)]),
    // vector.edit.end after every point was deleted: removes the emptied layer
    s(
        r##"[["vector.edit.begin", {"id": "$V"}], ["vector.edit.delete", {"vertices": [0, 1, 2]}]]"##,
        "vector.edit.end",
        r##"{}"##,
        &[],
        "mode",
        &[("$V", "kind", false)],
    ),
    // vector.edit.moveVertices: point 0 of the active layer moved in local units; refit shifts the box (frame)
    s(
        r##"[["vector.edit.begin", {"id": "$V"}]]"##,
        "vector.edit.moveVertices",
        r##"{"vertices": [0], "dx": 5, "dy": 0}"##,
        &[],
        "mode",
        &[("$V", "kind,frame", false)],
    ),
    // vector.edit.moveVertices without vertices: the selected points of every edited layer
    s(
        r##"[["vector.edit.begin", {"id": "$V"}], ["vector.edit.select", {"vertices": [0]}]]"##,
        "vector.edit.moveVertices",
        r##"{"dx": 5, "dy": 0}"##,
        &[],
        "mode",
        &[("$V", "kind,frame", false)],
    ),
    // vector.edit.nudge: selected points moved by a page offset (through the layer's world transform); refit
    s(
        r##"[["vector.edit.begin", {"id": "$V"}], ["vector.edit.select", {"vertices": [0]}]]"##,
        "vector.edit.nudge",
        r##"{"dx": 5, "dy": 0}"##,
        &[],
        "mode",
        &[("$V", "kind,frame", false)],
    ),
    // vector.edit.position: the selected points' top-left (through the layer's transform) moved to x; refit
    s(
        r##"[["vector.edit.begin", {"id": "$V"}], ["vector.edit.select", {"vertices": [0]}]]"##,
        "vector.edit.position",
        r##"{"x": 10}"##,
        &[],
        "mode",
        &[("$V", "kind,frame", false)],
    ),
    // vector.edit.setHandle: the handle of segment 0 at its start set (local); the curve leaves the box, refit
    s(
        r##"[["vector.edit.begin", {"id": "$V"}]]"##,
        "vector.edit.setHandle",
        r##"{"segment": 0, "start": true, "x": 10, "y": -10}"##,
        &[],
        "mode",
        &[("$V", "kind,frame", false)],
    ),
    // vector.edit.moveHandles: named handle moved by a page offset (world transform); refit
    s(
        r##"[["vector.edit.begin", {"id": "$V"}]]"##,
        "vector.edit.moveHandles",
        r##"{"dx": 0, "dy": -10, "handles": [[0, true]]}"##,
        &[],
        "mode",
        &[("$V", "kind,frame", false)],
    ),
    // vector.edit.mirroring: point 1's mirroring (and its handles) in the network
    s(
        r##"[["vector.edit.begin", {"id": "$V"}]]"##,
        "vector.edit.mirroring",
        r##"{"mode": "angle", "vertices": [1]}"##,
        &[],
        "mode",
        &[("$V", "kind", false)],
    ),
    // vector.edit.smooth: point 1 gets handles from its neighbours; the curves grow the box (refit)
    s(r##"[["vector.edit.begin", {"id": "$V"}]]"##, "vector.edit.smooth", r##"{"vertices": [1]}"##, &[], "mode", &[("$V", "kind,frame", false)]),
    // vector.edit.bend: segment 0 bent through a page point (world transform); refit
    s(
        r##"[["vector.edit.begin", {"id": "$V"}]]"##,
        "vector.edit.bend",
        r##"{"segment": 0, "t": 0.5, "x": 20, "y": 290}"##,
        &[],
        "mode",
        &[("$V", "kind,frame", false)],
    ),
    // vector.edit.addVertex: segment 0 split at t in the network
    s(r##"[["vector.edit.begin", {"id": "$V"}]]"##, "vector.edit.addVertex", r##"{"segment": 0, "t": 0.5}"##, &[], "mode", &[("$V", "kind", false)]),
    // vector.edit.delete: point 0 (and its segment) removed from the active layer; refit moves the box
    s(r##"[["vector.edit.begin", {"id": "$V"}]]"##, "vector.edit.delete", r##"{"vertices": [0]}"##, &[], "mode", &[("$V", "kind,frame", false)]),
    // vector.edit.delete without params: the selection of every edited layer
    s(
        r##"[["vector.edit.begin", {"id": "$V"}], ["vector.edit.select", {"vertices": [0]}]]"##,
        "vector.edit.delete",
        r##"{}"##,
        &[],
        "mode",
        &[("$V", "kind,frame", false)],
    ),
    // vector.edit.cornerRadius: point 1's corner radius in the network
    s(
        r##"[["vector.edit.begin", {"id": "$V"}]]"##,
        "vector.edit.cornerRadius",
        r##"{"radius": 5, "vertices": [1]}"##,
        &[],
        "mode",
        &[("$V", "kind", false)],
    ),
    // vector.edit.strokeCap: point 0's own cap in the network
    s(
        r##"[["vector.edit.begin", {"id": "$V"}]]"##,
        "vector.edit.strokeCap",
        r##"{"cap": "round", "vertices": [0]}"##,
        &[],
        "mode",
        &[("$V", "kind", false)],
    ),
    // vector.edit.strokeJoin: point 1's own join in the network
    s(
        r##"[["vector.edit.begin", {"id": "$V"}]]"##,
        "vector.edit.strokeJoin",
        r##"{"join": "round", "vertices": [1]}"##,
        &[],
        "mode",
        &[("$V", "kind", false)],
    ),
    // vector.edit.paint ($V closed first): the region under the page point (world transform) is toggled (here unfilled; fills are read only when filling a layer with no visible fill)
    s(
        r##"[["vector.join", {"ids": ["$V"]}], ["vector.edit.begin", {"id": "$V"}]]"##,
        "vector.edit.paint",
        r##"{"x": 35, "y": 305}"##,
        &[],
        "mode",
        &[("$V", "kind,frame", false)],
    ),
    // vector.edit.cut at a segment: split into two loose ends in the network
    s(r##"[["vector.edit.begin", {"id": "$V"}]]"##, "vector.edit.cut", r##"{"segment": 0, "t": 0.5}"##, &[], "mode", &[("$V", "kind", false)]),
    // vector.edit.cut at a point: the point is split into one per segment
    s(r##"[["vector.edit.begin", {"id": "$V"}]]"##, "vector.edit.cut", r##"{"vertex": 1}"##, &[], "mode", &[("$V", "kind", false)]),
    // vector.edit.cut across (page line): cuts segment 0; the part that comes apart goes to a full copy of the layer above it (duplicate: every field, its place)
    s(
        r##"[["vector.edit.begin", {"id": "$V"}]]"##,
        "vector.edit.cut",
        r##"{"from": [20, 290], "to": [20, 310]}"##,
        &[],
        "mode",
        &[("$V", "*", true)],
    ),
    // vector.edit.erase: the brush (page points, radius through the world transform) removes the part of segment 0 it covers
    s(
        r##"[["vector.edit.begin", {"id": "$V"}]]"##,
        "vector.edit.erase",
        r##"{"points": [[20, 300]], "radius": 4}"##,
        &[],
        "mode",
        &[("$V", "kind,frame", false)],
    ),
    // vector.edit.connect: a straight segment between the open ends 0 and 2 (closes a region)
    s(r##"[["vector.edit.begin", {"id": "$V"}]]"##, "vector.edit.connect", r##"{"a": 0, "b": 2}"##, &[], "mode", &[("$V", "kind", false)]),
    // vector.edit.pen: a page point (world transform; hit-tested on the edited network) becomes a new point, the pen's open end
    s(r##"[["vector.edit.begin", {"id": "$V"}]]"##, "vector.edit.pen", r##"{"x": 100, "y": 400}"##, &[], "mode", &[("$V", "kind,frame", false)]),
    // vector.edit.resize: the selected points' box (in the vector's axes, through its world transform) scaled to width 20
    s(
        r##"[["vector.edit.begin", {"id": "$V"}], ["vector.edit.select", {"vertices": [0, 1]}]]"##,
        "vector.edit.resize",
        r##"{"width": 20}"##,
        &[],
        "mode",
        &[("$V", "kind,frame", false)],
    ),
    // vector.edit.rotate: the selected points rotated about their box centre (page); refit
    s(
        r##"[["vector.edit.begin", {"id": "$V"}], ["vector.edit.select", {"vertices": [0, 1]}]]"##,
        "vector.edit.rotate",
        r##"{"angle": 90}"##,
        &[],
        "mode",
        &[("$V", "kind,frame", false)],
    ),
    // vector.edit.transform: named points mapped by a page-space matrix (through the world transform); refit
    s(
        r##"[["vector.edit.begin", {"id": "$V"}]]"##,
        "vector.edit.transform",
        r##"{"matrix": [1, 0, 0, 1, 5, 0], "vertices": [0]}"##,
        &[],
        "mode",
        &[("$V", "kind,frame", false)],
    ),
    // vector.edit.transform without vertices: the selected points
    s(
        r##"[["vector.edit.begin", {"id": "$V"}], ["vector.edit.select", {"vertices": [0]}]]"##,
        "vector.edit.transform",
        r##"{"matrix": [1, 0, 0, 1, 5, 0]}"##,
        &[],
        "mode",
        &[("$V", "kind,frame", false)],
    ),
    // vector.edit.widthPoint: adds a width point to the active layer's stroke profile (read and replaced in `stroke`)
    s(
        r##"[["vector.edit.begin", {"id": "$V"}]]"##,
        "vector.edit.widthPoint",
        r##"{"action": "add", "t": 0.5, "scale": 2}"##,
        &[],
        "mode",
        &[("$V", "stroke", false)],
    ),
    // shapeBuilder.merge: arrangement of $R and $E (visible, unlocked shapes outside instances/booleans: kind, world frame, corners, strokes); the regions under the points merge into a new vector styled like $R (name, fills, strokes, stroke, effects, opacity, blend) above it; both lose those areas and are removed; $R's comment re-anchored
    // Reviewed after behavior tests: shapebuilder::arrangement uses only the named shapes;
    // points index that arrangement's regions, so unrelated canvas layers need no pick replay.
    r(
        "shapeBuilder.merge",
        r##"{"ids": ["$R", "$E"], "points": [[50, 30], [20, 120]]}"##,
        &[],
        "",
        &[
            ("$R", "kind,frame,corners,visible,locked,name,fills,strokes,stroke,effects,opacity,props.blend,~hierarchy", true),
            ("$E", "kind,frame,visible,locked,strokes,stroke,~hierarchy", true),
            ("$Th", "anchor", false),
        ],
    ),
    // shapeBuilder.remove: the region under the point ($R's whole area) is removed: $R is deleted; $E's geometry still decides the regions; $R's comment re-anchored
    // Reviewed after behavior tests: the points pick regions of the named-shape arrangement,
    // whose geometry is held below; they do not hit-test the rest of the page.
    r(
        "shapeBuilder.remove",
        r##"{"ids": ["$R", "$E"], "points": [[50, 30]]}"##,
        &[],
        "",
        &[
            ("$R", "kind,frame,corners,visible,locked,strokes,stroke,~hierarchy", true),
            ("$E", "kind,frame,visible,locked,strokes,stroke,~hierarchy", true),
            ("$Th", "anchor", false),
        ],
    ),
    // ---- Reading 5 ----
    // component.create: one frame by id: its kind is checked (Frame/Group) and replaced by Component; ancestors only refuse (inside instance)
    r("component.create", r##"{"ids": ["$F"]}"##, &[], "", &[("$F", "kind", false)]),
    // component.create: a group becomes a component: kind replaced, fills cleared, clip off, blend PassThrough (each replaced whole)
    r("component.create", r##"{"ids": ["$G"]}"##, &[], "", &[("$G", "kind,fills,props.clip,props.blend", false)]),
    // component.create: a rectangle is wrapped in a new frame at its world bounds, at its parent/index (topmost), taking its name ("Box"), then the frame becomes the component
    r("component.create", r##"{"ids": ["$R"]}"##, &[], "", &[("$R", "name,kind,frame", true)]),
    // component.create: no ids: the selection names the frame
    r("component.create", r##"{}"##, &["$F"], "selection", &[("$F", "kind", false)]),
    // component.createMultiple: each id runs component.create alone (frame: kind; group: kind, fills, clip, blend)
    r(
        "component.createMultiple",
        r##"{"ids": ["$F", "$G"]}"##,
        &[],
        "",
        &[("$F", "kind", false), ("$G", "kind,fills,props.clip,props.blend", false)],
    ),
    // component.combineVariants: main components (kind, parent kind, remote, roots checked); z order and the top-most's parent/index place the new set; world bounds size it; first's name names the set; names without '=' are replaced ("Property 1=...")
    s(
        r##"[["component.create", {"ids": ["$F"]}]]"##,
        "component.combineVariants",
        r##"{"ids": ["$K", "$F"]}"##,
        &[],
        "",
        &[("$K", "name,kind,frame", true), ("$F", "name,kind,frame", true)],
    ),
    // component.resetInstance: the instance's and its layers' overrides cleared (top root keeps size), props reset to the main's defaults (main kind), prop bindings dropped; normalize rebuilds the instance from the main's whole subtree
    r(
        "component.resetInstance",
        r##"{"ids": ["$I"]}"##,
        &[],
        "",
        &[("$I", "kind,instance,bindings", false), ("$IT", "instance,mirrors", true), ("$K", "*", false), ("$KT", "*", true)],
    ),
    // component.resetInstance: a layer inside the instance resolves to its owning instance (through its ancestors)
    r(
        "component.resetInstance",
        r##"{"ids": ["$IT"]}"##,
        &[],
        "",
        &[("$IT", "instance,mirrors", true), ("$I", "kind,instance,bindings", false), ("$K", "*", false), ("$KT", "*", true)],
    ),
    // component.detach: instance becomes a frame: kind, source, overrides, prop_refs, prop bindings replaced; its layers lose mirrors/overrides/prop_refs; the main's var_modes become its own
    r(
        "component.detach",
        r##"{"ids": ["$I"]}"##,
        &[],
        "",
        &[("$I", "kind,mirrors,instance,bindings", false), ("$IT", "mirrors,instance", true), ("$K", "bindings", false)],
    ),
    // component.pushChanges: outermost instance of the id; each overridden layer's groups (here opacity) are copied onto its source in the main (replaced whole) and the override dropped; normalize resyncs the main's instances
    s(
        r##"[["node.set", {"ids": ["$IT"], "props": {"opacity": 0.5}}]]"##,
        "component.pushChanges",
        r##"{"id": "$I"}"##,
        &[],
        "",
        &[
            ("$I", "kind,instance", false),
            ("$IT", "instance,mirrors,opacity,props.blend", true),
            ("$KT", "opacity,props.blend", true),
            ("$K", "-", false),
        ],
    ),
    // component.restoreMain: the instance (main deleted) is removed and pasted back as the main with the old ids, at its parent and index, from its whole subtree; props values become defs
    s(r##"[["edit.delete", {"ids": ["$K"]}]]"##, "component.restoreMain", r##"{"id": "$I"}"##, &[], "", &[("$I", "*", true), ("$IT", "*", true)]),
    // component.hideWhenPublishing: the asset root (outermost component/set among self and ancestors) gets a leading '.' (toggle reads the names); name replaced whole
    r("component.hideWhenPublishing", r##"{"ids": ["$K"]}"##, &[], "", &[("$K", "name,kind", true)]),
    // component.hideWhenPublishing: a layer inside a component resolves to the component through its ancestors; explicit on
    r("component.hideWhenPublishing", r##"{"ids": ["$KT"], "on": true}"##, &[], "", &[("$KT", "-", true), ("$K", "name,kind", true)]),
    // component.addVariant: on a set: the last child is duplicated (whole), named from the set's variant options/existing combos, placed below the lowest variant; the set grows to fit (width/height replaced by max)
    r("component.addVariant", r##"{"id": "$Set"}"##, &[], "", &[("$Set", "kind,frame", false), ("$VS", "name,kind,frame", true), ("$VL", "*", true)]),
    // component.addVariant: on a lone main: wrapped in a new set at its parent/index/bounds, renamed "Property 1=Default", then duplicated whole
    r("component.addVariant", r##"{"id": "$K"}"##, &[], "", &[("$K", "*", true), ("$KT", "*", true)]),
    // instance.create: the main copied whole with its subtree onto the current page (no parent), beside the main's world bounds
    r("instance.create", r##"{"component": "$K"}"##, &[], "part", &[("$K", "*", false), ("$KT", "*", true)]),
    // instance.create: a set gives its default variant (top-left by the variants' transforms); the set's name names the instance; explicit parent and place
    r(
        "instance.create",
        r##"{"component": "$Set", "parent": "$X", "x": 10, "y": 10}"##,
        &[],
        "",
        &[("$Set", "name,kind", false), ("$VS", "*", true), ("$VL", "kind,frame", true), ("$X", "-", false)],
    ),
    // instance.place: drop slot under the page point on the current page (empty area: the page); the main copied whole
    r(
        "instance.place",
        r##"{"component": "$K", "x": 2000, "y": 1500}"##,
        &[],
        "part",
        &[("picks", DROP_PICKS, false), ("$K", "*", false), ("$KT", "*", true)],
    ),
    // instance.place: the point is in auto layout frame $F: index from its placed children's world bounds/visible/absolute; local place through $F's world transform; normalize re-flows $F (layout, children frame/visible/childLayout)
    r(
        "instance.place",
        r##"{"component": "$K", "x": 300, "y": 150}"##,
        &[],
        "part",
        &[
            ("picks", DROP_PICKS, false),
            ("$F", "frame,visible,locked,kind,layout,childLayout,~hierarchy", true),
            ("$A", "frame,visible,childLayout", true),
            ("$T", "frame,visible,childLayout,kind,text", true),
            ("$K", "*", false),
            ("$KT", "*", true),
        ],
    ),
    // component.exposeInstance: the instance must be nested in a main (checked through its ancestors: the wrapper component $S1 the step made, its kind) and not inside an instance; props.exposed replaced
    s(
        r##"[["component.create", {"ids": ["$I"]}]]"##,
        "component.exposeInstance",
        r##"{"id": "$I"}"##,
        &[],
        "",
        &[("$I", "kind,props.exposed", true), ("$S1", "kind", true)],
    ),
    // component.makeSlot: a frame inside a main (kind, ancestors: owning component/instance, not already a slot); its name names the property; the def goes into the owning component's kind (unnamed wrapper made by the step), the frame's prop_refs.slot set
    // Reviewed after comparison: the wrapper is $S1; make_slot resolves its owner and
    // add_prop_for reads/writes its definitions (component/instance.rs:158-170, props.rs:214-295).
    s(
        r##"[["component.create", {"ids": ["$F", "$E"]}]]"##,
        "component.makeSlot",
        r##"{"id": "$F"}"##,
        &[],
        "",
        &[("$F", "name,kind,instance", true), ("$S1", "kind", true)],
    ),
    // component.removeSlot: slot name from the frame's prop_refs and its component's defs; delete_prop removes the def and every prop_ref naming it in the component's subtree (instances: none)
    // Reviewed after comparison: remove_slot/delete_prop resolves the same wrapper's kind
    // and place before rewriting its definitions (component/instance.rs:172-182, props.rs:522-555).
    s(
        r##"[["component.create", {"ids": ["$F", "$E"]}], ["component.makeSlot", {"id": "$F"}]]"##,
        "component.removeSlot",
        r##"{"id": "$F"}"##,
        &[],
        "",
        &[("$F", "instance", true), ("$S1", "kind", true), ("$E", "instance", true), ("$A", "instance", true), ("$T", "instance", true)],
    ),
    // instance.slotInsert: $KT wrapped in frame $S1 made slot "Body" of $K; the slot is found by name among $I's layers through their mirrors to $S1 (prop_refs.slot) and $K's defs; $VS copied whole into $I's copy of $S1 on top.
    // Reviewed after comparison: @copy:$I:$S1 names the generated slot whose source, overrides,
    // children and place instance_slot/slot_insert reads (component/instance.rs:185-216).
    s(
        r##"[["object.frameSelection", {"ids": ["$KT"]}], ["component.makeSlot", {"id": "$S1", "name": "Body"}]]"##,
        "instance.slotInsert",
        r##"{"id": "$I", "slot": "Body", "component": "$VS"}"##,
        &[],
        "",
        &[
            ("$I", "kind", false),
            ("$S1", "kind,instance", false),
            ("$K", "kind", false),
            ("$VS", "*", false),
            ("@copy:$I:$S1", "instance,mirrors,~hierarchy", true),
        ],
    ),
    // instance.resetSlot: slot "Body" found as in slotInsert; its SLOT override removed; normalize rebuilds $I: the slot takes the main's default content (the copy of $S1's children: $KT whole); the inserted $S3 goes.
    // Reviewed after comparison: the generated slot is a real read of instance_slot/reset_slot
    // (component/instance.rs:185-227); its source/parent also decide rebuilding (sync.rs:52-81).
    s(
        r##"[["object.frameSelection", {"ids": ["$KT"]}], ["component.makeSlot", {"id": "$S1", "name": "Body"}], ["instance.slotInsert", {"id": "$I", "slot": "Body", "component": "$VS"}]]"##,
        "instance.resetSlot",
        r##"{"id": "$I", "slot": "Body"}"##,
        &[],
        "",
        &[
            ("$I", "kind", false),
            ("$S1", "kind,instance", false),
            ("$K", "kind", false),
            ("$KT", "*", true),
            ("@copy:$I:$S1", "instance,mirrors,~hierarchy", true),
        ],
    ),
    // component.resetOverride: the top instance: overrides cleared (replaced), props reset to the main's defaults; resync from the main
    r("component.resetOverride", r##"{"id": "$I"}"##, &[], "", &[("$I", "kind,instance,mirrors", false), ("$K", "*", false), ("$KT", "*", true)]),
    // component.resetOverride: a layer inside the instance, one group: its overrides minus the group; normalize copies the main layer's value back
    r(
        "component.resetOverride",
        r##"{"id": "$IT", "group": "characters"}"##,
        &[],
        "",
        &[("$IT", "instance,mirrors", true), ("$KT", "kind,text", false)],
    ),
    // instance.swap: kind.component replaced, props rebuilt from the new main's defs keeping same-named values; normalize rebuilds from the new main (old layers matched by parent/source/name path, overrides carried)
    // review: sync.rs:199 preserves modes used to resolve the new main.
    r(
        "instance.swap",
        r##"{"ids": ["$I"], "component": "$VS"}"##,
        &[],
        "",
        &[("$I", "kind,instance,bindings.var_modes", false), ("$IT", "name,mirrors,instance", true), ("$VS", "*", false)],
    ),
    // instance.swap: a set gives its default variant (variants' transforms)
    // review: sync.rs:199 preserves modes used to resolve the new main.
    r(
        "instance.swap",
        r##"{"ids": ["$I"], "component": "$Set"}"##,
        &[],
        "",
        &[
            ("$I", "kind,instance,bindings.var_modes", false),
            ("$IT", "name,mirrors,instance", true),
            ("$Set", "kind", false),
            ("$VS", "*", true),
            ("$VL", "kind,frame", true),
        ],
    ),
    // instance.swapAt: hit test at the point on the current page; the innermost instance there (prefers one of the dropped variant's set, by the set's name); overrides but text layers' dropped; then instance.swap
    // Reviewed after behavior tests: the picked instance's frame and spatial replay hold the
    // point identity; swapping keeps that instance's existing tree slot rather than addressing it.
    r(
        "instance.swapAt",
        r##"{"component": "$VS", "x": 560, "y": 120}"##,
        &[],
        "part",
        &[
            ("picks", SWAP_PICKS, false),
            // sync_instance_depth preserves the instance's explicit variable modes before
            // applying the new main; the point/mode matrix demonstrates this dependency.
            ("$I", "kind,instance,frame,visible,locked,bindings.var_modes", false),
            ("$IT", "kind,text,name,mirrors,instance", true),
            ("$Set", "name", false),
            ("$VS", "*", true),
        ],
    ),
    // instance.setProperties: text property value: defs from the main's kind, the main's parent decides variant or not; props entry written; normalize rebuilds the bound text from $KT's prop_refs
    // Reviewed after behavior tests: sync_instance_depth clones the main root's appearance
    // as well as its kind (sync.rs:172); its unused page location is independent in the property matrix.
    r(
        "instance.setProperties",
        r##"{"ids": ["$I"], "properties": {"Label": "Go"}}"##,
        &[],
        "",
        &[("$I", "kind", false), ("$K", "*", false), ("$KT", "*", true), ("$IT", "instance,mirrors", true)],
    ),
    // instance.setProperties: variant switch on $S1 (instance of $Set's default $VS): current variant's name and parent (the set), the set's children names (options, find_variant); kind.component replaced; normalize rebuilds $S1 from $VL whole
    s(
        r##"[["instance.create", {"component": "$Set"}]]"##,
        "instance.setProperties",
        r##"{"ids": ["$S1"], "properties": {"Size": "L"}}"##,
        &[],
        "",
        &[("$S1", "kind,instance", false), ("$Set", "kind", false), ("$VS", "name,kind", true), ("$VL", "*", true)],
    ),
    // component.addProperty: boolean bound to a layer: owner $K (its parent kind: not a set), its defs (kind) get the def; default from the layer's visibility; instances of $K get the default value; layer prop_refs set and visible := default
    r(
        "component.addProperty",
        r##"{"id": "$K", "name": "Show", "type": "boolean", "layer": "$KT"}"##,
        &[],
        "",
        &[("$K", "kind", true), ("$KT", "visible,kind,text,instance", true), ("$I", "kind", false), ("$IT", "instance,mirrors", true)],
    ),
    // component.addProperty: text without a layer: default is the name; instances get it
    // Reviewed after comparison: adding I's default property requests a rebuild (patch.rs:37-45),
    // which matches and preserves its copied layer's source, overrides and parent (sync.rs:27-43).
    r(
        "component.addProperty",
        r##"{"id": "$K", "name": "Title", "type": "text"}"##,
        &[],
        "",
        &[("$K", "kind", true), ("$I", "kind", false), ("$IT", "instance,mirrors", true)],
    ),
    // component.addProperty: variant on a set: every variant's name gets ", State=Default" (replaced whole)
    r(
        "component.addProperty",
        r##"{"id": "$Set", "name": "State", "type": "variant"}"##,
        &[],
        "",
        &[("$Set", "kind", false), ("$VS", "name,kind", true), ("$VL", "name,kind", true)],
    ),
    // component.editProperty: rename a text property: def renamed in $K's kind, prop_refs in $K's subtree and instances' props/bound keys follow
    r(
        "component.editProperty",
        r##"{"id": "$K", "name": "Label", "newName": "Text"}"##,
        &[],
        "",
        &[("$K", "kind", true), ("$KT", "instance", true), ("$I", "kind,bindings", false)],
    ),
    // component.editProperty: new default of a text property (def in kind replaced)
    r("component.editProperty", r##"{"id": "$K", "name": "Label", "default": "Hi"}"##, &[], "", &[("$K", "kind", true)]),
    // component.editProperty: rename a variant value: every variant's name rewritten from its pairs and the property order
    r(
        "component.editProperty",
        r##"{"id": "$Set", "name": "Size", "value": "S", "newValue": "Small"}"##,
        &[],
        "",
        &[("$Set", "kind", false), ("$VS", "name,kind", true), ("$VL", "name,kind", true)],
    ),
    // component.deleteProperty: def removed from $K's kind, prop_refs naming it removed, instances' values/bindings dropped; normalize: $IT follows $KT's own characters
    r(
        "component.deleteProperty",
        r##"{"id": "$K", "name": "Label"}"##,
        &[],
        "",
        &[("$K", "kind", true), ("$KT", "kind,text,instance", true), ("$I", "kind,bindings", false), ("$IT", "instance,mirrors", true)],
    ),
    // component.deleteProperty: a variant property: every variant's name loses it ("Default" when empty)
    r(
        "component.deleteProperty",
        r##"{"id": "$Set", "name": "Size"}"##,
        &[],
        "",
        &[("$Set", "kind", false), ("$VS", "name,kind", true), ("$VL", "name,kind", true)],
    ),
    // component.bindProperty: unbind: the layer's prop_refs cleared (ancestors: not in an instance, owning component); instances' copies then follow the main's own characters
    r(
        "component.bindProperty",
        r##"{"layer": "$KT", "property": ""}"##,
        &[],
        "",
        &[("$KT", "kind,text,instance", true), ("$IT", "instance,mirrors", true)],
    ),
    // component.bindProperty: bind to a boolean property (def in the owning component's kind): prop_refs.visible set, visible := the def's default
    // Reviewed after comparison: changing the main's property references rebuilds I
    // (patch.rs:106-178); sync.rs:27-43 reads IT's source/overrides and parent to preserve it.
    s(
        r##"[["component.addProperty", {"id": "$K", "name": "Show", "type": "boolean"}]]"##,
        "component.bindProperty",
        r##"{"layer": "$KT", "property": "Show"}"##,
        &[],
        "",
        &[("$KT", "kind,text,instance,visible", true), ("$K", "kind", false), ("$IT", "instance,mirrors", true)],
    ),
    // library.publish: every local component/set (whole subtrees), local style, local collection with its variables (+ alias closure) is snapshotted; names decide the default offer; the previous published library gives versions; writes only doc.published (~published, not stored)
    // Reviewed after behavior tests: library::publishable selects local asset identities, not
    // ordinary canvas layers. The asset-membership fact also includes variables in publishable
    // collections: a new variable joins the snapshot without changing its collection record.
    r(
        "library.publish",
        r##"{}"##,
        &[],
        "",
        &[
            // review: library.rs:429 copies the document name into the manifest.
            ("design", "settings.name,~assets,~published", false),
            ("$K", "*", true),
            ("$KT", "*", true),
            ("$Set", "*", true),
            ("$VS", "*", true),
            ("$VL", "*", true),
            ("$Brand", "*", false),
            ("$Colors", "*", false),
            ("$Primary", "*", false),
            ("$Accent", "*", false),
        ],
    ),
    // library.publish: include one component by id: its subtree and what it uses (none here)
    // Reviewed after behavior tests: publish_args validates this id, then snapshots its closure;
    // adding an unrelated component or ordinary layer does not change the explicit include.
    // review: explicit inclusion still copies the document name.
    r(
        "library.publish",
        r##"{"include": ["$K"]}"##,
        &[],
        "",
        &[("design", "settings.name,~published", false), ("$K", "*", true), ("$KT", "*", true)],
    ),
    // library.add: the library FILE at the path (lent bytes: manifest id/name/version); this file's published id refuses; the existing link with that id compared (path, name) and kept (here nothing changes)
    r(
        "library.add",
        r##"{"path": "libraries/Buttons.kdesign"}"##,
        &[],
        "",
        &[("$Lib", "name,path", false), ("design", "libraries,~published", false)],
    ),
    // library.swap: Other.kdesign linked by the step; prune(from) reads every node's mains/style links/bindings; A's copies in use ($LK via $LI) matched by name to TO's published assets (TO's file: "Primary" imported), $LI swapped (kind replaced, layers matched/overrides carried)
    // Reviewed after behavior tests: assets::library::prune follows reference membership, not
    // unrelated layer/resource contents; the same ~libraryUsage query guards acceptUpdates.
    s(
        r##"[["library.add", {"path": "libraries/Other.kdesign"}]]"##,
        "library.swap",
        r##"{"from": "Buttons", "to": "Other"}"##,
        &[],
        "",
        &[
            ("$Lib", "name", false),
            ("design", "libraries,libraryCopies,~libraryUsage", false),
            ("$LI", "kind,instance,name,mirrors", false),
            ("$LK", "*", false),
        ],
    ),
    // library.remove: link found by name and removed; prune keeps copies something uses ($LK by $LI): every node's instance main, style links, bound variables, var_modes, interactions; variables' aliases/extends; copy records
    // Reviewed after behavior tests: the removal/prune path consumes the same reference query;
    // ordinary canvas membership and independent resource metadata do not keep a copy alive.
    r(
        "library.remove",
        r##"{"id": "Buttons"}"##,
        &[],
        "",
        &[("$Lib", "name", false), ("$LI", "kind,bindings,prototype", false), ("$LK", "-", false), ("design", "libraryCopies,~libraryUsage", false)],
    ),
    // library.acceptUpdates: all enabled links; each FILE's manifest versions vs the copy records (stale copies re-imported in place); link version/name replaced when they differ; prune
    // Reviewed after behavior tests: prune scans instance, style and variable references;
    // ~libraryUsage expresses their membership, excluding unrelated ordinary-layer content.
    r(
        "library.acceptUpdates",
        r##"{}"##,
        &[],
        "",
        &[
            ("$Lib", "name,path,version", false),
            ("design", "libraries,libraryCopies,~libraryUsage", false),
            ("$LK", "*", true),
            ("$LI", "kind,instance", false),
        ],
    ),
    // library.acceptUpdates: named link
    // Reviewed after behavior tests: selecting a link still prunes from the same reference
    // query; unrelated canvas-node membership does not alter the references keeping copies live.
    r(
        "library.acceptUpdates",
        r##"{"ids": ["Buttons"]}"##,
        &[],
        "",
        &[
            ("$Lib", "name,path,version", false),
            ("design", "libraryCopies,~libraryUsage", false),
            ("$LK", "*", true),
            ("$LI", "kind,instance", false),
        ],
    ),
    // library.place: link by name, FILE manifest names "Primary"; its copy record exists so $LK is used as is; an instance of $LK at the drop slot under the point (empty page area)
    r(
        "library.place",
        r##"{"library": "Buttons", "component": "Primary", "x": 2000, "y": 1500}"##,
        &[],
        "part",
        &[("$Lib", "name,path", false), ("design", "libraryCopies", false), ("$LK", "*", true), ("picks", DROP_PICKS, false)],
    ),
    // library.place: no point: the view's centre
    r(
        "library.place",
        r##"{"library": "Buttons", "component": "Primary"}"##,
        &[],
        "part,mode",
        &[
            ("$Lib", "name,path", false),
            ("design", "libraryCopies", false),
            ("$LK", "*", true),
            ("picks", DROP_PICKS, false),
            ("$D", "frame,visible,locked,kind,layout", false),
        ],
    ),
    // library.swapAt: copy resolved as library.place, then instance.swapAt at the point with $LK (family: $LK's parent is the store, not a set)
    // Reviewed after behavior tests: swap_at delegates to the same point resolver and preserves
    // the instance's tree slot; spatial identity is held by its frame and the pick replay.
    r(
        "library.swapAt",
        r##"{"library": "Buttons", "component": "Primary", "x": 560, "y": 120}"##,
        &[],
        "part",
        &[
            ("$Lib", "name,path", false),
            ("design", "libraryCopies", false),
            ("$LK", "*", true),
            ("picks", SWAP_PICKS, false),
            ("$I", "kind,instance,frame,visible,locked,bindings.var_modes", false),
            ("$IT", "kind,text,name,mirrors,instance", true),
        ],
    ),
    // library.import: link + FILE manifest resolve "Primary"; existing record: no write, answers $LK
    r(
        "library.import",
        r##"{"library": "Buttons", "component": "Primary"}"##,
        &[],
        "",
        &[("$Lib", "name,path", false), ("design", "libraryCopies", false), ("$LK", "-", false)],
    ),
    // library.applyStyle: FILE's published style "Lib Blue" copied (record), then style.apply fill: $R's fills replaced, styles.fill set, the fill colour binding to $Primary dropped
    r(
        "library.applyStyle",
        r##"{"library": "Buttons", "style": "Lib Blue", "ids": ["$R"]}"##,
        &[],
        "",
        &[("$Lib", "name,path", false), ("design", "styles,libraryCopies", false), ("$R", "fills,bindings", false)],
    ),
    // library.applyStyle: stroke target, layers from the selection
    r(
        "library.applyStyle",
        r##"{"library": "Buttons", "style": "Lib Blue", "target": "stroke"}"##,
        &["$E"],
        "selection",
        &[("$Lib", "name,path", false), ("design", "styles,libraryCopies", false), ("$E", "strokes,bindings", false)],
    ),
    // library.importVariable: FILE's "space" (alias closure) and its collection "Lib Tokens" copied with records
    r(
        "library.importVariable",
        r##"{"library": "Buttons", "variable": "space"}"##,
        &[],
        "",
        &[("$Lib", "name,path", false), ("design", "libraryCopies,collections,variables", false)],
    ),
    // library.bindVariable: as importVariable, then variable.bind gap on $F (field_takes: kind/layout scope; bound[gap] written)
    r(
        "library.bindVariable",
        r##"{"library": "Buttons", "variable": "space", "field": "gap", "ids": ["$F"]}"##,
        &[],
        "",
        &[("$Lib", "name,path", false), ("design", "libraryCopies,collections,variables", false), ("$F", "kind,layout,bindings", false)],
    ),
    // library.aliasVariable: local number variable $S1 (not remote; its type checked against "space"); the copy of FILE's "space" made; $S1's value in its collection's default mode replaced by an alias of it
    s(
        r##"[["variable.create", {"collection": "$Colors", "name": "gap", "type": "number", "value": 4}]]"##,
        "library.aliasVariable",
        r##"{"library": "Buttons", "alias": "space", "variable": "$S1"}"##,
        &[],
        "",
        &[
            ("$Lib", "name,path", false),
            ("design", "libraryCopies,collections,variables", false),
            ("$S1", "type,collection,values", false),
            ("$Colors", "modes", false),
        ],
    ),
    // library.setVariableMode: FILE collection "Lib Tokens" and mode "Base" copied; var_modes[copy] set on $X; its bound layers re-resolve
    r(
        "library.setVariableMode",
        r##"{"library": "Buttons", "collection": "Lib Tokens", "mode": "Base", "ids": ["$X"]}"##,
        &[],
        "",
        &[("$Lib", "name,path", false), ("design", "libraryCopies,collections", false), ("$X", "bindings", false)],
    ),
    // library.setVariableMode: no ids, nothing selected: the current page takes the mode
    r(
        "library.setVariableMode",
        r##"{"library": "Buttons", "collection": "Lib Tokens", "mode": "Base"}"##,
        &[],
        "selection,part",
        &[("$Lib", "name,path", false), ("design", "libraryCopies,collections", false), ("$P1", "bindings", false)],
    ),
    // library.extendCollection: FILE collection + its published variables copied; a new local collection extending the copy (name made unique only)
    r(
        "library.extendCollection",
        r##"{"library": "Buttons", "collection": "Lib Tokens"}"##,
        &[],
        "",
        &[("$Lib", "name,path", false), ("design", "libraryCopies,collections,variables", false)],
    ),
    // ---- Reading 6 ----
    // image.place: no target/parent: a new image rectangle on the current page (part), on top; reads nothing of other layers
    r(
        "image.place",
        r##"{"data": "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAFElEQVR4nGP8z8DwnwEJMDGgAQBVBgICyX5rtAAAAABJRU5ErkJggg==", "x": 0, "y": 1300}"##,
        &[],
        "part",
        &[("$P1", "-", false)],
    ),
    // UNSURE: image.place into auto layout parent $F: inserted on top of $F's children, then normalize re-flows $F (its layout, its children's frames/visibility/child layout; $F may hug) - exact layout fields are my estimate
    r(
        "image.place",
        r##"{"data": "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAFElEQVR4nGP8z8DwnwEJMDGgAQBVBgICyX5rtAAAAABJRU5ErkJggg==", "parent": "$F"}"##,
        &[],
        "",
        &[("$F", TEXT_FLOW_PARENT, false), ("$A", "frame,visible,childLayout", true), ("$T", "frame,visible,childLayout", true)],
    ),
    // image.place with target $A: pushes an image fill on top of $A's fills and clears its fill style link (styles/bindings re-checked by normalize)
    r(
        "image.place",
        r##"{"data": "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAFElEQVR4nGP8z8DwnwEJMDGgAQBVBgICyX5rtAAAAABJRU5ErkJggg==", "target": "$A"}"##,
        &[],
        "",
        &[("$A", "fills,bindings.styles,bindings.bound", false)],
    ),
    // image.place with target $R + index 0: fill 0 becomes the image keeping opacity/blend/visibility; drops fills/0/* bindings (the $Primary binding) and the fill style link
    r(
        "image.place",
        r##"{"data": "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAFElEQVR4nGP8z8DwnwEJMDGgAQBVBgICyX5rtAAAAABJRU5ErkJggg==", "target": "$R", "index": 0}"##,
        &[],
        "",
        &[("$R", "fills,bindings.styles,bindings.bound", false)],
    ),
    // fill.add with a paint on $X: pushes the paint onto $X's fills (read-modify-write); normalize re-checks style link and bindings
    r("fill.add", r##"{"ids": ["$X"], "paint": "#00ff00"}"##, &[], "", &[("$X", "fills,bindings.styles,bindings.bound", false)]),
    // fill.add without paint: default paint depends on the layer type (white for frame-like, #D9D9D9 otherwise) -> reads kind
    r("fill.add", r##"{"ids": ["$X"]}"##, &[], "", &[("$X", "kind,fills,bindings.styles,bindings.bound", false)]),
    // fill.add on $E (linked to $Brand): normalize sync_list_styles compares the new fills with $Brand's value and unlinks $E
    r(
        "fill.add",
        r##"{"ids": ["$E"], "paint": "#00ff00"}"##,
        &[],
        "",
        &[("$E", "fills,bindings.styles,bindings.bound", false), ("$Brand", "value,props.bound", false)],
    ),
    // fill.add on $IT inside instance $I: normalize records the `fills` override, comparing with the main layer $KT it mirrors
    r(
        "fill.add",
        r##"{"ids": ["$IT"], "paint": "#00ff00"}"##,
        &[],
        "",
        &[("$IT", "fills,instance.overrides,mirrors,bindings.styles,bindings.bound", false), ("$KT", "fills", false)],
    ),
    // stroke.add: pushes black (or the paint) onto $X's strokes
    r("stroke.add", r##"{"ids": ["$X"]}"##, &[], "", &[("$X", "strokes,bindings.styles,bindings.bound", false)]),
    // fill.remove index 0 on $R: removes fill 0 and its fills/0/color binding, later items' bindings shift down
    r("fill.remove", r##"{"ids": ["$R"], "index": 0}"##, &[], "", &[("$R", "fills,bindings.styles,bindings.bound", false)]),
    // stroke.remove index 0 after a stroke was added on $X
    s(
        r##"[["stroke.add", {"ids": ["$X"]}]]"##,
        "stroke.remove",
        r##"{"ids": ["$X"], "index": 0}"##,
        &[],
        "",
        &[("$X", "strokes,bindings.styles,bindings.bound", false)],
    ),
    // fill.set color on $X: replaces the top fill (pushes a default when empty), rebinds/drops that paint's bindings, clears fill style link
    r("fill.set", r##"{"ids": ["$X"], "color": "#ff0000"}"##, &[], "", &[("$X", "fills,bindings.styles,bindings.bound", false)]),
    // fill.set on $R (bound to $Primary): the colour changed, so the fills/0/color binding is dropped
    r(
        "fill.set",
        r##"{"ids": ["$R"], "index": 0, "color": "#ff0000", "opacity": 0.5}"##,
        &[],
        "",
        &[("$R", "fills,bindings.styles,bindings.bound", false)],
    ),
    // fill.set on $KT inside main component $K: normalize patches the copy $IT in instance $I unless $IT overrides fills
    r(
        "fill.set",
        r##"{"ids": ["$KT"], "color": "#ff0000"}"##,
        &[],
        "",
        &[("$KT", "fills,bindings.styles,bindings.bound", false), ("$IT", "fills,instance.overrides,mirrors", false)],
    ),
    // stroke.set color on $X: top stroke (a black one pushed when none) set to the colour
    r("stroke.set", r##"{"ids": ["$X"], "color": "#ff0000"}"##, &[], "", &[("$X", "strokes,bindings.styles,bindings.bound", false)]),
    // fill.paste: the copied fill (clipboard) pushed onto $X's fills
    s(
        r##"[["select.set", {"ids": ["$R"]}], ["fill.copy", {}]]"##,
        "fill.paste",
        r##"{"ids": ["$X"]}"##,
        &[],
        "mode",
        &[("$X", "fills,bindings.styles,bindings.bound", false)],
    ),
    // fill.paste with index: replaces fill 0 of $X with the clipboard paint (via fill.set)
    s(
        r##"[["select.set", {"ids": ["$R"]}], ["fill.copy", {}]]"##,
        "fill.paste",
        r##"{"ids": ["$X"], "index": 0}"##,
        &[],
        "mode",
        &[("$X", "fills,bindings.styles,bindings.bound", false)],
    ),
    // stroke.paste: copied stroke paint pushed onto $S's strokes and the copied stroke settings replace $S's whole `stroke`
    s(
        r##"[["stroke.add", {"ids": ["$X"]}], ["select.set", {"ids": ["$X"]}], ["stroke.copy", {}]]"##,
        "stroke.paste",
        r##"{"ids": ["$S"]}"##,
        &[],
        "mode",
        &[("$S", "strokes,stroke,bindings.styles,bindings.bound", false)],
    ),
    // fill.pattern: top fill of $X becomes a pattern paint pointing at source $V (source only has to exist; the ancestor check only refuses)
    r("fill.pattern", r##"{"source": "$V", "ids": ["$X"]}"##, &[], "", &[("$X", "fills,bindings.styles,bindings.bound", false), ("$V", "-", false)]),
    // stroke.widthProfile: sets stroke.width_profile of $V (rest of `stroke` kept)
    r("stroke.widthProfile", r##"{"ids": ["$V"], "preset": "taper"}"##, &[], "", &[("$V", "stroke", false)]),
    // stroke.brush with a preset: pushes a black stroke when $V has none (reads strokes), sets stroke.brush
    r("stroke.brush", r##"{"ids": ["$V"], "preset": "ink"}"##, &[], "", &[("$V", "strokes,stroke,bindings.styles,bindings.bound", false)]),
    // stroke.brush null: only clears stroke.brush
    r("stroke.brush", r##"{"ids": ["$V"], "preset": null}"##, &[], "", &[("$V", "stroke", false)]),
    // effect.add: pushes the effect (type default or given) onto $X's effects
    r("effect.add", r##"{"ids": ["$X"], "type": "layerBlur"}"##, &[], "", &[("$X", "effects,bindings.styles,bindings.bound", false)]),
    // effect.remove: removes effect 0 and its effects/0/* bindings (later ones shift)
    s(
        r##"[["effect.add", {"ids": ["$X"]}]]"##,
        "effect.remove",
        r##"{"ids": ["$X"], "index": 0}"##,
        &[],
        "",
        &[("$X", "effects,bindings.styles,bindings.bound", false)],
    ),
    // effect.set: replaces effect 0 (no-op when out of range); bindings on it are NOT dropped (see DEFECTS), normalize re-applies them
    s(
        r##"[["effect.add", {"ids": ["$X"]}]]"##,
        "effect.set",
        r##"{"ids": ["$X"], "index": 0, "effect": {"type": "layerBlur", "radius": 8}}"##,
        &[],
        "",
        &[("$X", "effects,bindings.styles,bindings.bound", false)],
    ),
    // effect.paste: clipboard effect pushed onto $S's effects
    s(
        r##"[["effect.add", {"ids": ["$X"]}], ["select.set", {"ids": ["$X"]}], ["effect.copy", {}]]"##,
        "effect.paste",
        r##"{"ids": ["$S"]}"##,
        &[],
        "mode",
        &[("$S", "effects,bindings.styles,bindings.bound", false)],
    ),
    // grid.add: pushes a (default) layout grid onto $X's grids
    r("grid.add", r##"{"ids": ["$X"]}"##, &[], "", &[("$X", "props.grids,bindings.styles,bindings.bound", false)]),
    // grid.remove: removes grid 0 and its layoutGrids/0/* bindings
    s(
        r##"[["grid.add", {"ids": ["$X"]}]]"##,
        "grid.remove",
        r##"{"ids": ["$X"], "index": 0}"##,
        &[],
        "",
        &[("$X", "props.grids,bindings.styles,bindings.bound", false)],
    ),
    // export.add without setting: the new setting is the step after the last existing one (reads exports), pushed
    r("export.add", r##"{"ids": ["$X"]}"##, &[], "", &[("$X", "props.exports", false)]),
    // export.add with a setting: pushed onto the list
    r("export.add", r##"{"ids": ["$X"], "setting": {"size": "2x", "format": "png"}}"##, &[], "", &[("$X", "props.exports", false)]),
    // export.update: setting 0 updated from its current value with the given keys
    s(
        r##"[["export.add", {"ids": ["$X"]}]]"##,
        "export.update",
        r##"{"ids": ["$X"], "index": 0, "format": "svg"}"##,
        &[],
        "",
        &[("$X", "props.exports", false)],
    ),
    // export.remove: removes setting 0
    s(r##"[["export.add", {"ids": ["$X"]}]]"##, "export.remove", r##"{"ids": ["$X"], "index": 0}"##, &[], "", &[("$X", "props.exports", false)]),
    // fill.move: reorders $X's fills, moves bindings with items, unlinks the fill style
    s(
        r##"[["fill.add", {"ids": ["$X"], "paint": "#ff0000"}], ["fill.add", {"ids": ["$X"], "paint": "#0000ff"}]]"##,
        "fill.move",
        r##"{"ids": ["$X"], "from": 0, "to": 1}"##,
        &[],
        "",
        &[("$X", "fills,bindings.styles,bindings.bound", false)],
    ),
    // stroke.move: reorders $X's strokes
    s(
        r##"[["stroke.add", {"ids": ["$X"]}], ["stroke.add", {"ids": ["$X"], "paint": "#ff0000"}]]"##,
        "stroke.move",
        r##"{"ids": ["$X"], "from": 0, "to": 1}"##,
        &[],
        "",
        &[("$X", "strokes,bindings.styles,bindings.bound", false)],
    ),
    // effect.move: reorders $X's effects
    s(
        r##"[["effect.add", {"ids": ["$X"]}], ["effect.add", {"ids": ["$X"], "type": "layerBlur"}]]"##,
        "effect.move",
        r##"{"ids": ["$X"], "from": 0, "to": 1}"##,
        &[],
        "",
        &[("$X", "effects,bindings.styles,bindings.bound", false)],
    ),
    // grid.move: reorders $X's layout grids
    s(
        r##"[["grid.add", {"ids": ["$X"]}], ["grid.add", {"ids": ["$X"]}]]"##,
        "grid.move",
        r##"{"ids": ["$X"], "from": 0, "to": 1}"##,
        &[],
        "",
        &[("$X", "props.grids,bindings.styles,bindings.bound", false)],
    ),
    // selection.setPaint list target: $R's fill 0 replaced, fill style cleared, fills/0/color binding dropped (colour changed)
    r(
        "selection.setPaint",
        r##"{"targets": [{"id": "$R", "fills": true, "index": 0, "range": null}], "paint": "#0000ff"}"##,
        &[],
        "",
        &[("$R", "fills,bindings.styles,bindings.bound", false)],
    ),
    // selection.setPaint text range target: chars 0..2 of $T get own fills = the layer's fills with slot 0 replaced (reads fills, writes kind)
    r(
        "selection.setPaint",
        r##"{"targets": [{"id": "$T", "fills": true, "index": 0, "range": [0, 2]}], "paint": "#0000ff"}"##,
        &[],
        "",
        &[("$T", "fills,kind,text", false)],
    ),
    // selection.applyColorStyle by id: $R's fills replaced by $Brand's paints, link set, fill bindings dropped
    r(
        "selection.applyColorStyle",
        r##"{"targets": [{"id": "$R", "fills": true, "index": 0, "range": null}], "style": "$Brand"}"##,
        &[],
        "",
        &[("$R", "fills,bindings.styles,bindings.bound", false), ("$Brand", "value,props.bound", false)],
    ),
    // selection.applyColorStyle by name
    r(
        "selection.applyColorStyle",
        r##"{"targets": [{"id": "$R", "fills": true, "index": 0, "range": null}], "style": "Brand"}"##,
        &[],
        "",
        &[("$R", "fills,bindings.styles,bindings.bound", false), ("$Brand", "name,value,props.bound", false)],
    ),
    // selection.bindColorVariable on $E's fill 0: drops $Brand link, binds fills/0/color to $Primary; normalize writes $Primary's value (in $E's modes) into $E's fill
    r(
        "selection.bindColorVariable",
        r##"{"targets": [{"id": "$E", "fills": true, "index": 0, "range": null}], "variable": "$Primary"}"##,
        &[],
        "",
        &[
            ("$E", "fills,bindings.styles,bindings.bound,bindings.var_modes", true),
            ("$Primary", "collection,values", false),
            ("$Colors", "modes", false),
            ("$P1", "bindings.var_modes", false),
        ],
    ),
    // selection.bindColorVariable by variable name
    r(
        "selection.bindColorVariable",
        r##"{"targets": [{"id": "$E", "fills": true, "index": 0, "range": null}], "variable": "primary"}"##,
        &[],
        "",
        &[
            ("$E", "fills,bindings.styles,bindings.bound,bindings.var_modes", true),
            ("$Primary", "name,collection,values", false),
            ("$Colors", "modes", false),
            ("$P1", "bindings.var_modes", false),
        ],
    ),
    // selection.detachColor on $R's fill 0: unbinds fills/0/color, moves the colour's alpha into the paint opacity, clears fill style link
    r(
        "selection.detachColor",
        r##"{"targets": [{"id": "$R", "fills": true, "index": 0, "range": null}]}"##,
        &[],
        "",
        &[("$R", "fills,bindings.styles,bindings.bound", false)],
    ),
    // selection.detachColor on $E's fill 0: clears the $Brand link (no binding to drop)
    r(
        "selection.detachColor",
        r##"{"targets": [{"id": "$E", "fills": true, "index": 0, "range": null}]}"##,
        &[],
        "",
        &[("$E", "bindings.styles,bindings.bound", false)],
    ),
    // style.create with value: a new style from the given paints; reads no layer
    r("style.create", r##"{"type": "paint", "name": "Green", "value": "#00ff00"}"##, &[], "", &[]),
    // style.create from $X: takes $X's fills (and their bindings), links $X to the new style (fills rewritten, fill bindings dropped)
    r(
        "style.create",
        r##"{"type": "paint", "name": "Screen fill", "from": "$X"}"##,
        &[],
        "",
        &[("$X", "fills,bindings.styles,bindings.bound", false)],
    ),
    // style.create without from/value: takes the first selected layer (here via `on`) for the effect style and links it
    r(
        "style.create",
        r##"{"type": "effect", "name": "Screen fx"}"##,
        &["$X"],
        "selection",
        &[("$X", "effects,bindings.styles,bindings.bound", false)],
    ),
    // style.create text from $T: takes $T's text style, links all its chars (kind) and styles.text
    r(
        "style.create",
        r##"{"type": "text", "name": "Body", "from": "$T"}"##,
        &[],
        "mode",
        &[("$T", "kind,text,bindings.styles,bindings.bound", false)],
    ),
    // style.apply paint style by id on $X: fills replaced by $Brand's paints, link set, fill bindings dropped
    r(
        "style.apply",
        r##"{"ids": ["$X"], "style": "$Brand"}"##,
        &[],
        "",
        &[("$X", "fills,bindings.styles,bindings.bound", false), ("$Brand", "value,props.bound", false)],
    ),
    // style.apply by name as stroke style
    r(
        "style.apply",
        r##"{"ids": ["$X"], "style": "Brand", "target": "stroke"}"##,
        &[],
        "",
        &[("$X", "strokes,bindings.styles,bindings.bound", false), ("$Brand", "name,value,props.bound", false)],
    ),
    // style.detach fill on $E: clears styles.fill
    r("style.detach", r##"{"ids": ["$E"], "which": "fill"}"##, &[], "", &[("$E", "bindings.styles", false)]),
    // style.detach text with no range: checks the text edit selection (mode), then unlinks every char of $T and styles.text
    r("style.detach", r##"{"ids": ["$T"], "which": "text"}"##, &[], "mode", &[("$T", "kind,text,bindings.styles", false)]),
    // style.detach text with start/end: unlinks chars 0..2 only
    r("style.detach", r##"{"ids": ["$T"], "which": "text", "start": 0, "end": 2}"##, &[], "", &[("$T", "kind,text", false)]),
    // style.update name: replaces $Brand's name
    r("style.update", r##"{"style": "$Brand", "name": "Brand/Main"}"##, &[], "", &[("$Brand", "name", false)]),
    // style.update value: replaces $Brand's value (prunes its bindings) and propagates to linked $E (fills rewritten)
    r(
        "style.update",
        r##"{"style": "$Brand", "value": "#00ff00"}"##,
        &[],
        "",
        &[("$Brand", "value,props.bound", false), ("$E", "fills,bindings.styles,bindings.bound", false)],
    ),
    // style.update from $X: $Brand's value taken from $X's fills, then propagated to $E
    r(
        "style.update",
        r##"{"style": "$Brand", "from": "$X"}"##,
        &[],
        "",
        &[("$X", "fills", false), ("$Brand", "value,props.bound", false), ("$E", "fills,bindings.styles,bindings.bound", false)],
    ),
    // style.bind: binds $Brand's fill colour to $Primary; style value settled in default modes, $E's fills re-resolved in $E's modes
    r(
        "style.bind",
        r##"{"style": "$Brand", "field": "fill", "variable": "$Primary"}"##,
        &[],
        "",
        &[
            ("$Brand", "value,props.bound", false),
            ("$Primary", "collection,values", false),
            ("$Colors", "modes", false),
            ("$E", "fills,bindings.styles,bindings.bound,bindings.var_modes", true),
            ("$P1", "bindings.var_modes", false),
        ],
    ),
    // style.duplicate: a copy of $Brand (all of it) inserted right after it in the style list
    r("style.duplicate", r##"{"style": "$Brand"}"##, &[], "", &[("$Brand", "name,value,props", true)]),
    // style.move into a folder: $Brand renamed "<folder>/<leaf>" and moved to the end of the list
    r("style.move", r##"{"styles": ["$Brand"], "folder": "Kit"}"##, &[], "", &[("$Brand", "name", false)]),
    // style.move before $Brand: the moved style lands where $Brand is in the list
    s(
        r##"[["style.create", {"type": "paint", "name": "Other", "value": "#0000ff"}]]"##,
        "style.move",
        r##"{"styles": ["$S1"], "before": "$Brand"}"##,
        &[],
        "",
        &[("$Brand", "-", true)],
    ),
    // style.renameFolder: every style whose name starts with "Old/" renamed (reads names)
    // Reviewed after behavior tests: rename_folder scans names, including styles that another
    // writer moves into or out of the folder; replay must hold that name-based membership.
    s(
        r##"[["style.update", {"style": "$Brand", "name": "Old/Brand"}]]"##,
        "style.renameFolder",
        r##"{"folder": "Old", "name": "New"}"##,
        &[],
        "",
        &[("$Brand", "name", false), ("picks", "name", false)],
    ),
    // style.delete by id: removes $Brand, unlinks the layers linked to it ($E)
    r("style.delete", r##"{"style": "$Brand"}"##, &[], "", &[("$Brand", "-", false), ("$E", "bindings.styles", false)]),
    // style.delete by name
    r("style.delete", r##"{"style": "Brand"}"##, &[], "", &[("$Brand", "name", false), ("$E", "bindings.styles", false)]),
    // variable.createCollection: new collection; name uniqueness only refuses
    r("variable.createCollection", r##"{"name": "Sizes", "modes": ["S", "L"]}"##, &[], "", &[]),
    // variable.addMode: appended to $Colors' modes; every variable of $Colors gets the default mode's value for it
    r(
        "variable.addMode",
        r##"{"collection": "$Colors", "name": "Contrast"}"##,
        &[],
        "",
        &[("$Colors", "modes", false), ("$Primary", "collection,values", false), ("$Accent", "collection,values", false)],
    ),
    // variable.addMode from a mode named "Dark", collection by name
    r(
        "variable.addMode",
        r##"{"collection": "Colors", "name": "Dim", "from": "Dark"}"##,
        &[],
        "",
        &[("$Colors", "name,modes", false), ("$Primary", "collection,values", false), ("$Accent", "collection,values", false)],
    ),
    // variable.create with one value: given value in every mode of $Colors
    r("variable.create", r##"{"collection": "$Colors", "name": "space", "type": "number", "value": 4}"##, &[], "", &[("$Colors", "modes", false)]),
    // variable.create with an alias value by name: alias target $Primary resolved by name (and type checked)
    // Reviewed after behavior tests: var_create resolves aliases inside each values entry;
    // replay of name/type/collection detects a renamed or reassigned alias target too.
    r(
        "variable.create",
        r##"{"collection": "$Colors", "name": "link", "type": "color", "values": {"Light": {"alias": "primary"}, "Dark": "#000000"}}"##,
        &[],
        "",
        &[("$Colors", "modes", false), ("$Primary", "name,type", false), ("picks", "name,type,collection", false)],
    ),
    // variable.set default mode: $Primary's value in $Colors' first mode replaced; normalize writes it into bound $R's fill (resolved in $R's modes)
    r(
        "variable.set",
        r##"{"variable": "$Primary", "value": "#00ff00"}"##,
        &[],
        "",
        &[
            ("$Primary", "type,collection,values", false),
            ("$Colors", "modes", false),
            ("$R", "fills,bindings.bound,bindings.var_modes", true),
            ("$P1", "bindings.var_modes", false),
        ],
    ),
    // variable.set by names in mode "Dark": no layer shows Dark, nothing else written
    r(
        "variable.set",
        r##"{"variable": "primary", "mode": "Dark", "value": "#000000"}"##,
        &[],
        "",
        &[("$Primary", "name,type,collection,values", false), ("$Colors", "modes", false)],
    ),
    // variable.set alias: $Accent's default value becomes an alias of $Primary (type and loop check walk $Primary's values)
    r(
        "variable.set",
        r##"{"variable": "$Accent", "value": {"alias": "$Primary"}}"##,
        &[],
        "",
        &[("$Accent", "type,collection,values", false), ("$Colors", "modes", false), ("$Primary", "type,values", false)],
    ),
    // variable.delete: removes $Primary; $Accent (aliasing it) takes its values; $R loses its binding and keeps its value (alpha moved into opacity)
    r(
        "variable.delete",
        r##"{"variable": "$Primary"}"##,
        &[],
        "",
        &[("$Primary", "collection,values,props.extends", false), ("$Accent", "values", false), ("$R", "fills,bindings.bound", false)],
    ),
    // variable.bind fill of $X to $Primary: binding added (paint alpha moved), normalize writes $Primary's value into $X's fill 0
    r(
        "variable.bind",
        r##"{"ids": ["$X"], "field": "fill", "variable": "$Primary"}"##,
        &[],
        "",
        &[
            ("$X", "fills,bindings.styles,bindings.bound,bindings.var_modes", true),
            ("$Primary", "collection,values", false),
            ("$Colors", "modes", false),
            ("$P1", "bindings.var_modes", false),
        ],
    ),
    // variable.bind to the alias $Accent by name: the value resolves through $Accent to $Primary
    r(
        "variable.bind",
        r##"{"ids": ["$X"], "field": "fills/0/color", "variable": "accent"}"##,
        &[],
        "",
        &[
            ("$X", "fills,bindings.styles,bindings.bound,bindings.var_modes", true),
            ("$Accent", "name,collection,values", false),
            ("$Primary", "collection,values", false),
            ("$Colors", "modes", false),
            ("$P1", "bindings.var_modes", false),
        ],
    ),
    // variable.bind null: unbinds the field (the colour's alpha moves back into the opacity)
    s(
        r##"[["variable.bind", {"ids": ["$X"], "field": "fill", "variable": "$Primary"}]]"##,
        "variable.bind",
        r##"{"ids": ["$X"], "field": "fill", "variable": null}"##,
        &[],
        "",
        &[("$X", "fills,bindings.bound", false)],
    ),
    // variable.setMode on $X: sets $X's mode for $Colors (empty frame: nothing bound below to re-resolve)
    r(
        "variable.setMode",
        r##"{"ids": ["$X"], "collection": "$Colors", "mode": "$Dark"}"##,
        &[],
        "",
        &[("$X", "bindings.var_modes", false), ("$Colors", "-", false)],
    ),
    // variable.setMode without ids: no selection -> the current page; every bound layer on it re-resolves ($R's fill takes $Primary's Dark value)
    r(
        "variable.setMode",
        r##"{"collection": "Colors", "mode": "Dark"}"##,
        &[],
        "selection,part",
        &[
            ("$P1", "bindings.var_modes", false),
            ("$Colors", "name,modes", false),
            ("$R", "fills,bindings.bound,bindings.var_modes", true),
            ("$Primary", "collection,values", false),
        ],
    ),
    // variable.setMode null: removes $X's explicit mode
    s(
        r##"[["variable.setMode", {"ids": ["$X"], "collection": "$Colors", "mode": "$Dark"}]]"##,
        "variable.setMode",
        r##"{"ids": ["$X"], "collection": "$Colors", "mode": null}"##,
        &[],
        "",
        &[("$X", "bindings.var_modes", false), ("$Colors", "-", false)],
    ),
    // image.adjust preset: preset looked up among the file's saved presets first, then built-ins; every image paint of $A's fills adjusted; fill style cleared
    // Reviewed after behavior tests: image::find_preset reads only the record named vivid;
    // another saved preset does not alter that lookup or the resulting adjustments.
    r(
        "image.adjust",
        r##"{"ids": ["$A"], "preset": "vivid"}"##,
        &[],
        "",
        &[("$A", "fills,bindings.styles,bindings.bound", false), ("design", "settings.image_presets[name=\"vivid\"]", false)],
    ),
    // image.adjust filters only: applied over each image paint's current adjustments
    r(
        "image.adjust",
        r##"{"ids": ["$A"], "index": 1, "filters": {"exposure": 0.3}}"##,
        &[],
        "",
        &[("$A", "fills,bindings.styles,bindings.bound", false)],
    ),
    // image.savePreset with filters: replaces a same-named preset or appends
    // Reviewed after behavior tests: save_preset replaces only Mine; independent named presets merge.
    // review: image.rs:166 also checks list capacity before appending Mine.
    r(
        "image.savePreset",
        r##"{"name": "Mine", "filters": {"exposure": 0.2}}"##,
        &[],
        "",
        &[("design", "settings.image_presets[name=\"Mine\"],~imagePresetCount", false)],
    ),
    // image.savePreset from $A's first image paint
    // Reviewed after behavior tests: the source fills and the destination record Mine are inputs.
    // review: image.rs:166 also checks list capacity before appending Mine.
    r(
        "image.savePreset",
        r##"{"name": "Mine", "ids": ["$A"]}"##,
        &[],
        "",
        &[("$A", "fills", false), ("design", "settings.image_presets[name=\"Mine\"],~imagePresetCount", false)],
    ),
    // image.deletePreset: removes presets of that name
    // Reviewed after behavior tests: deletePreset's retain predicate removes only Mine;
    // the contents of a differently named preset do not affect that deletion.
    s(
        r##"[["image.savePreset", {"name": "Mine", "filters": {"exposure": 0.2}}]]"##,
        "image.deletePreset",
        r##"{"name": "Mine"}"##,
        &[],
        "",
        &[("design", "settings.image_presets[name=\"Mine\"]", false)],
    ),
    // variable.update description: replaces props.description (code syntax written back unchanged)
    r("variable.update", r##"{"variable": "$Primary", "description": "Main colour"}"##, &[], "", &[("$Primary", "props.description", false)]),
    // variable.update name by variable name: renames (uniqueness in its collection only refuses)
    r("variable.update", r##"{"variable": "primary", "name": "brand/primary"}"##, &[], "", &[("$Primary", "name", false)]),
    // variable.update codeSyntax: merged into the existing map
    r(
        "variable.update",
        r##"{"variable": "$Primary", "codeSyntax": {"WEB": "var(--primary)"}}"##,
        &[],
        "",
        &[("$Primary", "props.codeSyntax", false)],
    ),
    // variable.duplicate: a copy of $Primary (all of it, "<name> copy") inserted right after it
    r("variable.duplicate", r##"{"variable": "$Primary"}"##, &[], "", &[("$Primary", "*", true)]),
    // variable.renameMode: renames mode $Dark in $Colors' modes
    r("variable.renameMode", r##"{"collection": "$Colors", "mode": "$Dark", "name": "Night"}"##, &[], "", &[("$Colors", "modes", false)]),
    // variable.removeMode $Dark: mode removed, its values dropped from $Colors' variables (layers set to it go back to the default: none here)
    r(
        "variable.removeMode",
        r##"{"collection": "$Colors", "mode": "$Dark"}"##,
        &[],
        "",
        &[("$Colors", "modes", false), ("$Primary", "collection,values", false), ("$Accent", "collection,values", false)],
    ),
    // variable.removeMode of the default $Light: $Dark becomes the default, so bound $R re-resolves in it
    r(
        "variable.removeMode",
        r##"{"collection": "$Colors", "mode": "$Light"}"##,
        &[],
        "",
        &[
            ("$Colors", "modes", false),
            ("$Primary", "collection,values", false),
            ("$Accent", "collection,values", false),
            ("$R", "fills,bindings.bound,bindings.var_modes", true),
            ("$P1", "bindings.var_modes", false),
        ],
    ),
    // variable.moveMode $Dark to 0: becomes the default mode; bound $R (on Auto) re-resolves in it
    r(
        "variable.moveMode",
        r##"{"collection": "$Colors", "mode": "$Dark", "to": 0}"##,
        &[],
        "",
        &[
            ("$Colors", "modes", false),
            ("$Primary", "collection,values", false),
            ("$R", "fills,bindings.bound,bindings.var_modes", true),
            ("$P1", "bindings.var_modes", false),
        ],
    ),
    // variable.renameCollection: replaces $Colors' name (uniqueness only refuses)
    r("variable.renameCollection", r##"{"collection": "$Colors", "name": "Palette"}"##, &[], "", &[("$Colors", "name", false)]),
    // variable.deleteCollection: $Colors and its variables ($Primary, $Accent: membership by collection) removed; bound $R keeps its value, loses the binding
    r(
        "variable.deleteCollection",
        r##"{"collection": "$Colors"}"##,
        &[],
        "",
        &[("$Colors", "-", false), ("$Primary", "collection", false), ("$Accent", "collection", false), ("$R", "fills,bindings.bound", false)],
    ),
    // variable.resetOverride (no mode): every mode in the inherited variable's overrides drops
    // out and takes its parent's (the library copy of "space") value again.
    // Reviewed after behavior tests: variables::reset_override copies parent.values; var_roles
    // now names that parent explicitly rather than leaving this known read out of the row.
    s(
        r##"[["library.extendCollection", {"library": "$Lib", "collection": "Lib Tokens", "name": "Ext"}], ["variable.set", {"variable": "@var:$S1:space", "value": 8}]]"##,
        "variable.resetOverride",
        r##"{"variable": "@var:$S1:space"}"##,
        &[],
        "",
        &[("@var:$S1:space", "values,props.extends,props.overrides", false), ("@parent:@var:$S1:space", "values", false)],
    ),
    // variable.import joining "Colors": matched by name (local, not extended), modes by name, "primary" by name+type; its Light value replaced; normalize re-resolves bound $R
    r(
        "variable.import",
        r##"{"variables": {"meta": {"variableCollections": {"C1": {"id": "C1", "name": "Colors", "modes": [{"modeId": "M1", "name": "Light"}], "defaultModeId": "M1", "variableIds": ["V1"]}}, "variables": {"V1": {"id": "V1", "name": "primary", "variableCollectionId": "C1", "resolvedType": "COLOR", "valuesByMode": {"M1": {"r": 0, "g": 0, "b": 1, "a": 1}}}}}}}"##,
        &[],
        "",
        &[
            ("$Colors", "name,modes,props.extends", false),
            ("$Primary", "name,collection,type,values", false),
            ("$R", "fills,bindings.bound,bindings.var_modes", true),
            ("$P1", "bindings.var_modes", false),
        ],
    ),
    // variable.import of a new collection: added (free name only made unique)
    r(
        "variable.import",
        r##"{"variables": {"meta": {"variableCollections": {"C2": {"id": "C2", "name": "Spacing", "modes": [{"modeId": "M1", "name": "Base"}], "defaultModeId": "M1", "variableIds": ["V2"]}}, "variables": {"V2": {"id": "V2", "name": "gap", "variableCollectionId": "C2", "resolvedType": "FLOAT", "valuesByMode": {"M1": 8}}}}}}"##,
        &[],
        "",
        &[],
    ),
    // ---- Reading 7 ----
    // file.import: PNG data at x,y: a new image rectangle on top of the current page; the image is stored by hash (reuses $Img when the bytes are the same 2×2 PNG)
    r(
        "file.import",
        r##"{"data": "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAFElEQVR4nGP8z8DwnwEJMDGgAQBVBgICyX5rtAAAAABJRU5ErkJggg==", "name": "dot.png", "x": 1600, "y": 0}"##,
        &[],
        "part",
        &[("$P1", "-", false), ("$Img", "-", false)],
    ),
    // file.import: PNG data without x,y: placed at the view's centre on the current page
    r(
        "file.import",
        r##"{"data": "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAFElEQVR4nGP8z8DwnwEJMDGgAQBVBgICyX5rtAAAAABJRU5ErkJggg=="}"##,
        &[],
        "part,mode",
        &[("$P1", "-", false), ("$Img", "-", false)],
    ),
    // file.import: SVG data at x,y: a new frame built from the SVG on top of the current page; reads nothing else of the design
    r(
        "file.import",
        r##"{"data": "PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIxMCIgaGVpZ2h0PSIxMCI+PHJlY3Qgd2lkdGg9IjEwIiBoZWlnaHQ9IjEwIiBmaWxsPSIjZmYwMDAwIi8+PC9zdmc+", "name": "box.svg", "x": 1600, "y": 100}"##,
        &[],
        "part",
        &[("$P1", "-", false)],
    ),
    // file.placeImage: like image.place without a target: a new image rectangle on top of the current page at x,y (default 0,0, not the view centre)
    r(
        "file.placeImage",
        r##"{"data": "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAFElEQVR4nGP8z8DwnwEJMDGgAQBVBgICyX5rtAAAAABJRU5ErkJggg==", "name": "Dot", "x": 1600, "y": 200}"##,
        &[],
        "part",
        &[("$P1", "-", false), ("$Img", "-", false)],
    ),
    // file.saveVersion: snapshots the whole design (and the current page it opens on) into the version history
    r("file.saveVersion", r##"{"name": "v1"}"##, &[], "part", &[("design", "*", false), ("design", "~snapshots", false)]),
    // UNSURE: branch.create: the branch's base and head are a copy of the whole design; the branch list grows (name default from its count); it opens the branch tab
    r("branch.create", r##"{"name": "Try"}"##, &[], "", &[("design", "*", false), ("design", "~branches", false)]),
    // file.rename: replaces the document name
    r("file.rename", r##"{"name": "Renamed"}"##, &[], "", &[("design", "settings.name", false)]),
    // UNSURE: cannot run on the fixture: the converting path rewrites every layer (node_mut), library copy $LK included, so check_read_only (assets/library.rs:1440-1461, run from session/exec.rs:143) refuses it whenever a library copy exists
    // file.colorProfile: convert (default): every colour value of every layer, page, style, variable and the document colours is converted, then the profile set
    n(
        "",
        "file.colorProfile",
        r##"{"profile": "displayP3"}"##,
        &[],
        "",
        &[
            ("design", "settings.color_profile,settings.document_colors", false),
            ("$P1", "kind", false),
            ("$P2", "kind", false),
            ("$D", "fills,strokes,effects,props.grids,prototype.interactions", false),
            ("$LI", "fills,strokes,effects,props.grids,prototype.interactions", false),
            ("$R", "fills,strokes,effects,props.grids,prototype.interactions", false),
            ("$E", "fills,strokes,effects,props.grids,prototype.interactions", false),
            ("$F", "fills,strokes,effects,props.grids,prototype.interactions", false),
            ("$A", "fills,strokes,effects,props.grids,prototype.interactions", false),
            ("$T", "fills,strokes,effects,props.grids,prototype.interactions,kind,text", false),
            ("$G", "fills,strokes,effects,props.grids,prototype.interactions", false),
            ("$G1", "fills,strokes,effects,props.grids,prototype.interactions", false),
            ("$G2", "fills,strokes,effects,props.grids,prototype.interactions", false),
            ("$V", "fills,strokes,effects,props.grids,prototype.interactions", false),
            ("$K", "fills,strokes,effects,props.grids,prototype.interactions", false),
            ("$KT", "fills,strokes,effects,props.grids,prototype.interactions,kind,text", false),
            ("$I", "fills,strokes,effects,props.grids,prototype.interactions", false),
            ("$IT", "fills,strokes,effects,props.grids,prototype.interactions,kind,text", false),
            ("$X", "fills,strokes,effects,props.grids,prototype.interactions", false),
            ("$S", "fills,strokes,effects,props.grids,prototype.interactions", false),
            ("$Set", "fills,strokes,effects,props.grids,prototype.interactions", false),
            ("$VS", "fills,strokes,effects,props.grids,prototype.interactions", false),
            ("$VL", "fills,strokes,effects,props.grids,prototype.interactions", false),
            ("$LK", "fills,strokes,effects,props.grids,prototype.interactions", false),
            ("$Brand", "value", false),
            ("$Primary", "values", false),
            ("$Accent", "values", false),
        ],
    ),
    // file.colorProfile: convert false: only the profile changes (values kept and reinterpreted); the current profile decides whether anything changes
    r("file.colorProfile", r##"{"profile": "displayP3", "convert": false}"##, &[], "", &[("design", "settings.color_profile", false)]),
    // dev.resolveChange: a ready-for-dev frame changed since marked goes back to Ready for dev: reads and writes its dev status/changed flag/note
    // Reviewed after behavior tests: resolving blesses the current subtree and drops its
    // comparison snapshot (derived::mark_dev_changes); a newly changed descendant must refuse it.
    // review: section.rs:84 reads status/change flags; derived.rs:229 drops
    // the cached comparison. It does not inspect descendant appearance to resolve it.
    s(
        r##"[["node.set", {"id": "$F", "props": {"devStatus": "readyForDev"}}], ["node.set", {"id": "$A", "props": {"opacity": 0.5}}]]"##,
        "dev.resolveChange",
        r##"{"ids": ["$F"], "reason": "ok"}"##,
        &[],
        "",
        &[("$F", "dev", false)],
    ),
    // dev.resolveChange: without ids, the selection
    // Reviewed after behavior tests: the selected frame's same subtree is acknowledged and
    // its saved comparison removed; the selection only supplies the root identity.
    // review: section.rs:84 reads status/change flags; derived.rs:229 drops
    // the cached comparison. It does not inspect descendant appearance to resolve it.
    s(
        r##"[["node.set", {"id": "$F", "props": {"devStatus": "readyForDev"}}], ["node.set", {"id": "$A", "props": {"opacity": 0.5}}]]"##,
        "dev.resolveChange",
        r##"{}"##,
        &["$F"],
        "selection",
        &[("$F", "dev", false)],
    ),
    // version.restore: replaces the whole design with the saved version (name, published, comments kept from the current design and re-pinned); may first keep the current design as an autosave
    // Reviewed after behavior tests: restore_version keeps conversation and publication while
    // replacing these design fields; outside/review.rs verifies a new comment remains independent.
    s(
        r##"[["file.saveVersion", {"name": "v1"}]]"##,
        "version.restore",
        r##"{"index": 0}"##,
        &[],
        "",
        &[("design", "settings,nodes,libraryStore,styles,collections,variables,libraries,libraryCopies,images,~history,~snapshots", false)],
    ),
    // version.rename: the version at the index gets the name (and becomes a named version)
    // Reviewed after behavior tests: file.rs's setter replaces version metadata, never reads
    // its saved design; the outside history-label test holds ~history rather than snapshot bytes.
    s(
        r##"[["file.saveVersion", {"name": "v1"}]]"##,
        "version.rename",
        r##"{"index": 0, "name": "First"}"##,
        &[],
        "",
        &[("design", "~history", false)],
    ),
    // version.delete: removes the version at the index. Reviewed against file.rs:259-266:
    // version_index and preview comparison read the ordered identities, not snapshot contents.
    s(r##"[["file.saveVersion", {"name": "v1"}]]"##, "version.delete", r##"{"index": 0}"##, &[], "", &[("design", "~versionIds", false)]),
    // version.autosave: after an edit, the whole design is snapshotted as an autosave (dirty state / undo history and pref.autoSave decide whether it saves)
    s(
        r##"[["node.set", {"id": "$R", "props": {"opacity": 0.5}}]]"##,
        "version.autosave",
        r##"{}"##,
        &[],
        "part,mode",
        &[("design", "*", false), ("design", "~snapshots,~history", false)],
    ),
    // page.add: a new page appended last, with the current page's background; it becomes the current page
    r("page.add", r##"{"name": "Page 3"}"##, &[], "part", &[("$P1", "kind", false)]),
    // page.rename: by id: replaces the page's name
    r("page.rename", r##"{"id": "$P2", "name": "Second"}"##, &[], "", &[("$P2", "name", false)]),
    // page.rename: without id: the current page
    r("page.rename", r##"{"name": "First"}"##, &[], "part", &[("$P1", "name", false)]),
    // page.delete: by id: the page and everything on it go
    r("page.delete", r##"{"id": "$P2"}"##, &[], "", &[("$P2", "-", false)]),
    // page.duplicate: by id: copies the page's layers (none here), its background, flows, prototype settings and guides, named "Copy of <name>", inserted right after it
    r("page.duplicate", r##"{"id": "$P2"}"##, &[], "", &[("$P2", "name,kind", true)]),
    // page.duplicate: without id: the current page with every layer on it (whole subtrees, in order; flows follow the copied frames)
    r(
        "page.duplicate",
        r##"{}"##,
        &[],
        "part",
        &[
            ("$P1", "name,kind", true),
            ("$D", "*", true),
            ("$LI", "*", true),
            ("$R", "*", true),
            ("$E", "*", true),
            ("$F", "*", true),
            ("$A", "*", true),
            ("$T", "*", true),
            ("$G", "*", true),
            ("$G1", "*", true),
            ("$G2", "*", true),
            ("$V", "*", true),
            ("$K", "*", true),
            ("$KT", "*", true),
            ("$I", "*", true),
            ("$IT", "*", true),
            ("$X", "*", true),
            ("$S", "*", true),
            ("$Set", "*", true),
            ("$VS", "*", true),
            ("$VL", "*", true),
        ],
    ),
    // page.move: the page goes to the index in the page list without it (what is at that place matters); its own place is replaced
    r("page.move", r##"{"id": "$P2", "index": 0}"##, &[], "", &[("$P2", "-", true), ("slot:design", "-", true)]),
    // page.setBackground: by id: the page's background inside its kind
    r("page.setBackground", r##"{"id": "$P2", "color": "#202020"}"##, &[], "", &[("$P2", "kind", false)]),
    // page.setBackground: without id: the current page
    r("page.setBackground", r##"{"color": "#202020"}"##, &[], "part", &[("$P1", "kind", false)]),
    // proto.interaction.add: {id, dest}: appends a click-navigate to the layer's interactions; the destination must exist
    r("proto.interaction.add", r##"{"id": "$E", "dest": "$X"}"##, &[], "", &[("$E", "prototype.interactions", false), ("$X", "-", false)]),
    // proto.interaction.add: setVariableMode naming the collection and mode by name: stored by id after a name lookup
    // Reviewed after comparison: canonical_actions (proto.rs:325-331) stores the matched mode's
    // id, not its ordinal or the other modes; collection/name and this mode/name are its inputs.
    r(
        "proto.interaction.add",
        r##"{"id": "$E", "trigger": {"type": "onClick"}, "action": {"type": "setVariableMode", "collection": "Colors", "mode": "Dark"}}"##,
        &[],
        "",
        &[("$E", "prototype.interactions", false), ("$Colors", "name", false), ("$Dark", "name", false)],
    ),
    // proto.interaction.add: without id: on the selection
    r("proto.interaction.add", r##"{"dest": "$X"}"##, &["$E"], "selection", &[("$E", "prototype.interactions", false), ("$X", "-", false)]),
    // proto.interaction.clear: the named layers' interactions are emptied
    r("proto.interaction.clear", r##"{"ids": ["$R"]}"##, &[], "", &[("$R", "prototype.interactions", false)]),
    // proto.interaction.clear: without ids: the selection
    r("proto.interaction.clear", r##"{}"##, &["$R"], "selection", &[("$R", "prototype.interactions", false)]),
    // proto.interaction.remove: the interaction at the index goes
    r("proto.interaction.remove", r##"{"id": "$R", "index": 0}"##, &[], "", &[("$R", "prototype.interactions", false)]),
    // proto.interaction.set: `interaction` replaces interaction 0 whole (the current one is read only for being there); its navigate to $X must exist
    r(
        "proto.interaction.set",
        r##"{"id": "$R", "index": 0, "interaction": {"trigger": {"type": "onClick"}, "actions": [{"type": "navigate", "dest": "$X"}], "transition": {"type": "dissolve"}, "durationMs": 200}}"##,
        &[],
        "",
        &[("$R", "prototype.interactions", false), ("$X", "-", false)],
    ),
    // proto.flow.add: a flow on the CURRENT page starting at the frame (Flow n from the count); an existing flow for it is answered without a write
    r("proto.flow.add", r##"{"start": "$F"}"##, &[], "part", &[("$P1", "kind", false), ("$F", "-", false)]),
    // proto.flow.add: without start: the first selected layer
    r("proto.flow.add", r##"{"name": "Card flow"}"##, &["$F"], "part,selection", &[("$P1", "kind", false), ("$F", "-", false)]),
    // proto.flow.rename: the current page's flow starting at the frame
    r("proto.flow.rename", r##"{"start": "$X", "name": "Main", "description": "d"}"##, &[], "part", &[("$P1", "kind", false)]),
    // proto.flow.remove: the current page's flows without that start
    r("proto.flow.remove", r##"{"start": "$X"}"##, &[], "part", &[("$P1", "kind", false)]),
    // proto.settings: the current page's prototype settings, merged with the given fields
    r("proto.settings", r##"{"device": "Custom size", "width": 400, "height": 800}"##, &[], "part", &[("$P1", "kind", false)]),
    // proto.action.add: appends an action to interaction 0's actions
    r("proto.action.add", r##"{"id": "$R", "index": 0, "action": {"type": "back"}}"##, &[], "", &[("$R", "prototype.interactions", false)]),
    // proto.action.set: replaces action [0] with a navigate to $F (which must exist)
    r(
        "proto.action.set",
        r##"{"id": "$R", "index": 0, "path": [0], "action": {"type": "navigate", "dest": "$F"}}"##,
        &[],
        "",
        &[("$R", "prototype.interactions", false), ("$F", "-", false)],
    ),
    // proto.action.remove: removes action [1] of interaction 0
    s(
        r##"[["proto.action.add", {"id": "$R", "index": 0, "action": {"type": "back"}}]]"##,
        "proto.action.remove",
        r##"{"id": "$R", "index": 0, "path": [1]}"##,
        &[],
        "",
        &[("$R", "prototype.interactions", false)],
    ),
    // proto.action.move: reorders action [1] to 0
    s(
        r##"[["proto.action.add", {"id": "$R", "index": 0, "action": {"type": "back"}}]]"##,
        "proto.action.move",
        r##"{"id": "$R", "index": 0, "path": [1], "to": 0}"##,
        &[],
        "",
        &[("$R", "prototype.interactions", false)],
    ),
    // proto.conditional.addBlock: an else-if before any else, its condition by default the first boolean variable (else the first variable) equal to its value in the default modes
    // Reviewed after comparison: default_condition (proto_actions.rs:155-160) selects across
    // all variables; adding a boolean can change the answer without editing the listed variables.
    s(
        r##"[["proto.action.add", {"id": "$R", "index": 0, "action": {"type": "conditional", "blocks": [{"expression": "true", "actions": []}]}}]]"##,
        "proto.conditional.addBlock",
        r##"{"id": "$R", "index": 0, "path": [1]}"##,
        &[],
        "",
        &[
            ("$R", "prototype.interactions", false),
            ("$Primary", "type,values", false),
            ("$Accent", "type", false),
            ("$Colors", "modes", false),
            ("design", "~defaultCondition", false),
        ],
    ),
    // proto.conditional.addBlock: an else block: reads only the blocks
    s(
        r##"[["proto.action.add", {"id": "$R", "index": 0, "action": {"type": "conditional", "blocks": [{"expression": "true", "actions": []}]}}]]"##,
        "proto.conditional.addBlock",
        r##"{"id": "$R", "index": 0, "path": [1], "else": true}"##,
        &[],
        "",
        &[("$R", "prototype.interactions", false)],
    ),
    // proto.conditional.setCondition: replaces block 0's condition
    s(
        r##"[["proto.action.add", {"id": "$R", "index": 0, "action": {"type": "conditional", "blocks": [{"expression": "true", "actions": []}]}}]]"##,
        "proto.conditional.setCondition",
        r##"{"id": "$R", "index": 0, "path": [1], "block": 0, "expression": "false"}"##,
        &[],
        "",
        &[("$R", "prototype.interactions", false)],
    ),
    // proto.conditional.removeBlock: removes block 1 (an if stays, so the conditional stays)
    s(
        r##"[["proto.action.add", {"id": "$R", "index": 0, "action": {"type": "conditional", "blocks": [{"expression": "true", "actions": []}, {"expression": "false", "actions": []}]}}]]"##,
        "proto.conditional.removeBlock",
        r##"{"id": "$R", "index": 0, "path": [1], "block": 1}"##,
        &[],
        "",
        &[("$R", "prototype.interactions", false)],
    ),
    // comment.add: a thread on the current page at the point, signed with the preferences' name (no author given)
    r("comment.add", r##"{"x": 1600, "y": 50, "text": "Hello"}"##, &[], "part,mode", &[("$P1", "-", false)]),
    // comment.add: page given, author given: no editor state
    r("comment.add", r##"{"x": 10, "y": 10, "text": "Hello", "page": "$P2", "author": "Sam"}"##, &[], "", &[("$P2", "-", false)]),
    // comment.add: anchor true: pinned to the innermost visible frame under the point ($F, the topmost container there), in its local coordinates
    r(
        "comment.add",
        r##"{"x": 250, "y": 50, "text": "Here", "author": "Sam", "anchor": true}"##,
        &[],
        "part",
        &[("$P1", "-", false), ("picks", COMMENT_PICKS, false), ("$F", "frame,visible,kind", true)],
    ),
    // comment.add: anchor a layer id on the page: pinned to it in its local coordinates (its world transform)
    r(
        "comment.add",
        r##"{"x": 20, "y": 20, "text": "Box", "author": "Sam", "anchor": "$R"}"##,
        &[],
        "part",
        &[("$P1", "-", false), ("$R", "frame", true)],
    ),
    // comment.reply: appends a message, reopens the thread (resolved := false) and sets who has read it (in the anchor)
    r("comment.reply", r##"{"id": "$Th", "text": "Done", "author": "Sam"}"##, &[], "", &[("$Th", "anchor,resolved", false)]),
    // comment.edit: replaces the first message's text (author checked only to refuse; same text is no write)
    r("comment.edit", r##"{"id": "$Th", "text": "Edited"}"##, &[], "", &[("$Th", "comments", false)]),
    // comment.react: toggles the author's reaction on the first message (its reactions read to decide)
    r("comment.react", r##"{"id": "$Th", "emoji": "👍", "author": "Sam"}"##, &[], "", &[("$Th", "comments", false)]),
    // comment.resolve: sets resolved
    r("comment.resolve", r##"{"id": "$Th"}"##, &[], "", &[("$Th", "resolved", false)]),
    // comment.move: to a point on the thread's page, pinned (default) to the innermost frame there ($F) in its local coordinates
    r(
        "comment.move",
        r##"{"id": "$Th", "x": 250, "y": 50}"##,
        &[],
        "",
        &[("$Th", "anchor", false), ("picks", COMMENT_PICKS, false), ("$F", "frame,visible,kind", true)],
    ),
    // comment.move: anchor false: to the page at the point
    r("comment.move", r##"{"id": "$Th", "x": 1600, "y": 50, "anchor": false}"##, &[], "", &[("$Th", "anchor", false)]),
    // comment.delete: the whole thread (the first message's author is checked only to refuse)
    r("comment.delete", r##"{"id": "$Th"}"##, &[], "", &[("$Th", "-", false)]),
    // comment.delete: one reply by index
    s(
        r##"[["comment.reply", {"id": "$Th", "text": "r", "author": "Sam"}]]"##,
        "comment.delete",
        r##"{"id": "$Th", "reply": 0, "author": "Sam"}"##,
        &[],
        "",
        &[("$Th", "comments", false)],
    ),
    // codeConnect.set: on the main component: its existing link for the language is kept where not given; default name from its name, default template from its property definitions (kind)
    r("codeConnect.set", r##"{"id": "$K", "importPath": "@/ui"}"##, &[], "", &[("$K", "name,kind,dev.code_links", false)]),
    // codeConnect.set: on an instance: the link goes on its main component
    r(
        "codeConnect.set",
        r##"{"id": "$I", "name": "Button", "template": "<Button />"}"##,
        &[],
        "",
        &[("$I", "kind", false), ("$K", "name,kind,dev.code_links", false)],
    ),
    // codeConnect.set: on a variant: the link goes on its set; type name from the set's name, properties from every variant's name
    r("codeConnect.set", r##"{"id": "$VS"}"##, &[], "", &[("$VS", "name,kind", true), ("$VL", "name", true), ("$Set", "name,dev.code_links", false)]),
    // codeConnect.set: without id: the selection
    r("codeConnect.set", r##"{"name": "Button", "template": "<Button />"}"##, &["$K"], "selection", &[("$K", "name,kind,dev.code_links", false)]),
    // codeConnect.remove: removes the main component's links for the language
    s(
        r##"[["codeConnect.set", {"id": "$K", "name": "Button", "template": "<Button />"}]]"##,
        "codeConnect.remove",
        r##"{"id": "$K", "language": "react"}"##,
        &[],
        "",
        &[("$K", "dev.code_links", false)],
    ),
    // codeConnect.import: links every component or set NAMED "Button" (including library copies), replacing its link for the language; checked against its properties
    // Reviewed after comparison: code_connect.rs:663 enumerates owner identities/names, so a
    // new matching owner matters; the name bucket expresses matching owner identities without
    // unrelated components. The suffix is the SHA-256 key of the JSON name "Button".
    r(
        "codeConnect.import",
        r##"{"json": {"version": 1, "links": [{"component": "Button", "language": "react", "name": "Button", "template": "<Button />"}]}}"##,
        &[],
        "",
        &[
            ("$K", "name,kind,dev.code_links", false),
            ("$Set", "name", false),
            ("$LK", "name", false),
            ("design", "~codeOwners.707eab0c23ecbf6e5382ba61a346bc76aefee81303280fc0e08e59df18afbf8a", false),
        ],
    ),
    // annotation.add: appends a note to the layer's annotations
    r("annotation.add", r##"{"id": "$R", "label": "Note"}"##, &[], "", &[("$R", "dev.annotations", false)]),
    // annotation.add: with a category by name (the file's categories, or the defaults)
    r(
        "annotation.add",
        r##"{"id": "$R", "label": "Note", "category": "Development"}"##,
        &[],
        "",
        &[("$R", "dev.annotations", false), ("design", "settings.annotation_categories", false)],
    ),
    // annotation.add: without id: the first selected layer
    r("annotation.add", r##"{"properties": ["width"]}"##, &["$R"], "selection", &[("$R", "dev.annotations", false)]),
    // annotation.edit: the annotation at the index gets the new label
    s(
        r##"[["annotation.add", {"id": "$R", "label": "Note"}]]"##,
        "annotation.edit",
        r##"{"id": "$R", "index": 0, "label": "Changed"}"##,
        &[],
        "",
        &[("$R", "dev.annotations", false)],
    ),
    // annotation.remove: the annotation at the index
    s(
        r##"[["annotation.add", {"id": "$R", "label": "Note"}]]"##,
        "annotation.remove",
        r##"{"id": "$R", "index": 0}"##,
        &[],
        "",
        &[("$R", "dev.annotations", false)],
    ),
    // annotation.addCategory: the whole category list (defaults when unset) plus one, colour the first unused
    r("annotation.addCategory", r##"{"name": "QA"}"##, &[], "", &[("design", "settings.annotation_categories", false)]),
    // annotation.editCategory: by name: rename in the whole list
    r("annotation.editCategory", r##"{"id": "Development", "name": "Dev"}"##, &[], "", &[("design", "settings.annotation_categories", false)]),
    // annotation.removeCategory: the list without it; every layer's annotations in it lose their category (every layer's annotations are read to find them)
    s(
        r##"[["annotation.add", {"id": "$R", "label": "Note", "category": "Development"}]]"##,
        "annotation.removeCategory",
        r##"{"id": "Development"}"##,
        &[],
        "",
        &[("design", "settings.annotation_categories", false), ("$R", "dev.annotations", false)],
    ),
    // devResource.add: appends a link to the layer's own dev resources
    r("devResource.add", r##"{"id": "$R", "url": "https://example.com/docs"}"##, &[], "", &[("$R", "dev.dev_resources", false)]),
    // devResource.add: without id: the first selected layer
    r("devResource.add", r##"{"url": "https://example.com/docs", "name": "Docs"}"##, &["$R"], "selection", &[("$R", "dev.dev_resources", false)]),
    // devResource.edit: the resource at the index
    s(
        r##"[["devResource.add", {"id": "$R", "url": "https://example.com/docs"}]]"##,
        "devResource.edit",
        r##"{"id": "$R", "index": 0, "name": "Docs"}"##,
        &[],
        "",
        &[("$R", "dev.dev_resources", false)],
    ),
    // devResource.remove: every own resource of the layer
    s(
        r##"[["devResource.add", {"id": "$R", "url": "https://example.com/docs"}]]"##,
        "devResource.remove",
        r##"{"id": "$R"}"##,
        &[],
        "",
        &[("$R", "dev.dev_resources", false)],
    ),
    // devMode.addMeasurement: from $R to $E: appended to $R's measurements ($E must be on the same page: refusal only)
    r("devMode.addMeasurement", r##"{"from": "$R", "to": "$E"}"##, &[], "", &[("$R", "dev.measurements", false), ("$E", "-", false)]),
    // devMode.addMeasurement: neither to nor side: to the layer's parent
    r("devMode.addMeasurement", r##"{"from": "$A"}"##, &[], "", &[("$A", "dev.measurements", true), ("$F", "-", false)]),
    // devMode.addMeasurement: without from: the first selected layer, across a side
    r("devMode.addMeasurement", r##"{"side": "top"}"##, &["$R"], "selection", &[("$R", "dev.measurements", false)]),
    // devMode.removeMeasurement: every measurement starting from the layer
    s(
        r##"[["devMode.addMeasurement", {"from": "$R", "to": "$E"}]]"##,
        "devMode.removeMeasurement",
        r##"{"from": "$R"}"##,
        &[],
        "",
        &[("$R", "dev.measurements", false)],
    ),
    // UNSURE: branch.updateFromMain: three-way merge of base, branch and main; the branch tab's design and the branch's base/head are replaced (rests on the whole main design and the branch); cross-tab, the active tab after branch.create is the branch
    // Reviewed after behavior tests: merge::merges excludes file-name/current-page changes,
    // and diff drops publication; branch_combination_holds_main_resources verifies that boundary.
    // Comment entries do merge (collab/docprops.rs:61), so their list remains a real input.
    s(
        r##"[["branch.create", {"name": "Try"}]]"##,
        "branch.updateFromMain",
        r##"{}"##,
        &[],
        "",
        &[("design", "settings,nodes,libraryStore,styles,collections,variables,libraries,libraryCopies,images,comments,~branches", false)],
    ),
    // UNSURE: branch.merge: merges the branch into the main design (whole main design rests), adds versions, marks the branch merged and closes its tab
    // Reviewed after behavior tests: the same three-way plan merges design and comment entries;
    // publication is outside the plan, and version snapshot contents are not read to append history.
    s(
        r##"[["branch.create", {"name": "Try"}]]"##,
        "branch.merge",
        r##"{}"##,
        &[],
        "",
        &[("design", "settings,nodes,libraryStore,styles,collections,variables,libraries,libraryCopies,images,comments,~branches", false)],
    ),
    // UNSURE: branch.archive: the branch's state (must be active) becomes archived; its tab closes
    s(r##"[["branch.create", {"name": "Try"}]]"##, "branch.archive", r##"{}"##, &[], "", &[("design", "~branches", false)]),
    // UNSURE: widget.insert: needs an installed widget plugin (none in the fixture; this row cannot succeed as is): a new widget frame on top of the current page (view centre without x,y), the plugin given the selection
    r("widget.insert", r##"{"plugin": "example.widget", "x": 1600, "y": 0}"##, &[], "part,mode,selection", &[("$P1", "-", false)]),
    // UNSURE: widget.click: needs a widget (none in the fixture; $W stands for one): the plugin is called with its state and the selection, and the widget is rebuilt (children replaced, root props, name, state, menu)
    r("widget.click", r##"{"id": "$W", "target": "add"}"##, &[], "mode,selection", &[("$W", "*", false)]),
    // UNSURE: widget.setText: needs a widget with a field: state and the field's text set, then rebuilt
    r("widget.setText", r##"{"id": "$W", "field": "title", "value": "Hi"}"##, &[], "mode,selection", &[("$W", "*", false)]),
    // UNSURE: widget.menu: needs a widget with a menu: the item's kind and (toggle) current state read, then rebuilt
    r("widget.menu", r##"{"id": "$W", "item": "dark", "value": true}"##, &[], "mode,selection", &[("$W", "*", false)]),
    // ---- Reading 8 ----
    // tool.place rectangle at (720,512): parent = frame under the click box's top-left (670,462) = $D (picks: visible/locked/kind, $D world frame); new layer's transform is $D's world transform inverse; $D's layout decides reflow; pixel snap + keep_tool prefs
    r(
        "tool.place",
        r##"{"tool": "rectangle", "x": 720, "y": 512}"##,
        &[],
        "part,mode",
        &[("picks", TOOL_PICKS, false), ("$D", "frame,layout,visible,locked,kind", false)],
    ),
    // tool.place frame at (300,600) in $D: as above, then take_enclosed checks $D's children (visible, unlocked, non-section, world bounds fully inside) to reparent into the new frame
    r(
        "tool.place",
        r##"{"tool": "frame", "x": 300, "y": 600}"##,
        &[],
        "part,mode",
        &[("picks", TOOL_PICKS, false), ("$D", "frame,layout,visible,locked,kind", false), ("$LI", "visible,locked,kind,frame", false)],
    ),
    // tool.place with the current tool (ellipse, chosen before) at (2000,2000): nothing under the corner, parent = current page
    s(r##"[["tool.set", {"tool": "ellipse"}]]"##, "tool.place", r##"{"x": 2000, "y": 2000}"##, &[], "part,mode", &[("picks", TOOL_PICKS, false)]),
    // tool.place section on empty canvas: parent = page (frame_at keeps only sections), take_enclosed over the page's top-level layers (visible, locked, kind, bounds)
    r("tool.place", r##"{"tool": "section", "x": 3000, "y": 3000}"##, &[], "part,mode", &[("picks", TOOL_PICKS, false)]),
    // tool.place star at the view centre: reopening fits the view on Page 1's content (DocState::new -> zoom_to_fit: centre of the union of top-level bounds, ~(720,512)), so the click box's corner (~670,462) is in $D (view cx/cy/zoom read)
    r("tool.place", r##"{"tool": "star"}"##, &[], "part,mode", &[("picks", TOOL_PICKS, false), ("$D", "frame,layout,visible,locked,kind", false)]),
    // board.sticky on the current page: colour from the sticky tool, author from prefs/live name; nothing else read
    b(r##""##, "board.sticky", r##"{"x": 0, "y": 900, "text": "Idea"}"##, &[], "part,mode", &[]),
    // board.sticky with colour and author given: only the current page is read
    b(r##""##, "board.sticky", r##"{"x": 0, "y": 900, "color": "blue", "author": false}"##, &[], "part", &[]),
    // board.sticky into a section made by a before step: the parent named by id (it exists; its kind only validates), no layout there, so nothing else read
    b(
        r##"[["node.create", {"type": "section", "name": "Area", "x": 0, "y": 1200, "width": 600, "height": 400}]]"##,
        "board.sticky",
        r##"{"x": 20, "y": 20, "parent": "$S1", "color": "blue", "author": false}"##,
        &[],
        "",
        &[("$S1", "-", false)],
    ),
    // board.nextSticky by id: reads the sticky's props.board (must be a sticky), fills (colour), parent (where), transform and size (offset); author from prefs; new sticky in the same parent
    b(r##""##, "board.nextSticky", r##"{"id": "$Sticky", "side": "below"}"##, &[], "mode", &[("$Sticky", "props.board,fills,frame", true)]),
    // board.nextSticky falling back to the first selected layer
    b(r##""##, "board.nextSticky", r##"{}"##, &["$Sticky2"], "selection,mode", &[("$Sticky2", "props.board,fills,frame", true)]),
    // board.shape with shape and colour given: only the current page is read
    b(r##""##, "board.shape", r##"{"shape": "diamond", "x": 0, "y": 900, "color": "blue", "text": "Q"}"##, &[], "part", &[]),
    // board.shape with defaults: shape and colour from the board tool state
    b(r##""##, "board.shape", r##"{"x": 0, "y": 900}"##, &[], "part,mode", &[]),
    // board.connector between $Sticky and $Shape: route from both layers' world bounds (auto sides; elbow avoids both boxes); ends stored by id; routing default from the tool; added to the current page
    b(
        r##""##,
        "board.connector",
        r##"{"from": "$Sticky", "to": "$Shape"}"##,
        &[],
        "part,mode",
        &[("$Sticky", "frame", false), ("$Shape", "frame", false)],
    ),
    // board.connector between two free points with style given: only the current page
    b(r##""##, "board.connector", r##"{"from": {"x": 0, "y": 900}, "to": {"x": 300, "y": 1000}, "style": "straight"}"##, &[], "part", &[]),
    // board.reattach $Conn's `to` end onto $Shape: reads the connector (props.board: from end $Sticky, routing), the kept end's and the target's world bounds; replaces props.board, the network (kind) and frame; the label follows in normalize
    b(
        r##""##,
        "board.reattach",
        r##"{"id": "$Conn", "end": "to", "target": "$Shape"}"##,
        &[],
        "",
        &[("$Conn", "props.board,kind,frame", false), ("$Sticky", "frame", false), ("$Shape", "frame", false)],
    ),
    // board.reattach on the selection, end left free at a point: the kept end $Sticky's bounds
    b(
        r##""##,
        "board.reattach",
        r##"{"end": "to", "target": {"x": 500, "y": 500}}"##,
        &["$Conn"],
        "selection",
        &[("$Conn", "props.board,kind,frame", false), ("$Sticky", "frame", false)],
    ),
    // board.label replaces the existing label's text: label_of looks among $Conn's parent's children for the frame whose props.board is Label{$Conn} ($Label), text_of takes its first text child ($LabelText) and replaces its characters whole; the label hugs the new text (its layout)
    // review: board::require consumes the persisted file kind.
    b(
        r##""##,
        "board.label",
        r##"{"id": "$Conn", "text": "go"}"##,
        &[],
        "",
        &[
            ("design", "settings.kind", false),
            ("$Conn", "props.board,~hierarchy", true),
            ("$Label", "props.board,layout,~hierarchy", true),
            ("$LabelText", "kind,text,~hierarchy", true),
        ],
    ),
    // board.label with empty text removes the label found among $Conn's parent's children
    // review: board::require consumes the persisted file kind.
    b(
        r##""##,
        "board.label",
        r##"{"id": "$Conn", "text": ""}"##,
        &[],
        "",
        &[("design", "settings.kind", false), ("$Conn", "props.board,~hierarchy", true), ("$Label", "props.board,~hierarchy", true)],
    ),
    // board.stamp at a page point with its stamp given: author from prefs; only the current page
    b(r##""##, "board.stamp", r##"{"x": 50, "y": 900, "stamp": "heart"}"##, &[], "part,mode", &[]),
    // board.ink from points with a colour: a new vector on the current page
    b(r##""##, "board.ink", r##"{"points": [[0, 900], [40, 940]], "color": "blue"}"##, &[], "part", &[]),
    // board.setColor on a sticky: replaces its fills whole
    b(r##""##, "board.setColor", r##"{"ids": ["$Sticky"], "color": "blue"}"##, &[], "", &[("$Sticky", "props.board,fills", false)]),
    // board.setColor on a shape: replaces the fills and strokes of its FIRST child (taken as the outline)
    b(
        r##""##,
        "board.setColor",
        r##"{"ids": ["$Shape"], "color": "green"}"##,
        &[],
        "",
        &[("$Shape", "props.board", false), ("slot:$Shape", "fills,strokes", true)],
    ),
    // board.setColor on the selection (connector and ink): replaces their strokes
    b(
        r##""##,
        "board.setColor",
        r##"{"color": "red"}"##,
        &["$Conn", "$Ink"],
        "selection",
        &[("$Conn", "props.board,strokes", false), ("$Ink", "props.board,strokes", false)],
    ),
    // board.setStyle on a connector: replaces props.board (routing) and stroke (caps); normalize re-routes it from its ends' bounds (kind, frame written)
    b(
        r##""##,
        "board.setStyle",
        r##"{"ids": ["$Conn"], "style": "curved", "endCap": "none"}"##,
        &[],
        "",
        &[("$Conn", "props.board,stroke,kind,frame", false), ("$Sticky", "frame", false), ("$Sticky2", "frame", false)],
    ),
    // board.setStyle on a shape: replaces name, props.board, layout padding (from its width/height), the first child's network/frame (outline), and the first text child's font size
    // Reviewed after comparison: resize_text modifies the existing text's styles and runs
    // (boards/board/edit.rs:172-177), separate from the outline selected by the first-child slot.
    b(
        r##""##,
        "board.setStyle",
        r##"{"ids": ["$Shape"], "shape": "ellipse", "fontSize": 24}"##,
        &[],
        "",
        &[("$Shape", "name,props.board,layout,frame,~hierarchy", false), ("slot:$Shape", "kind,frame", true), ("$ShapeText", "kind,text", false)],
    ),
    // board.timer start: overwrites the document timer (settings.timer) from the clock
    b(r##""##, "board.timer", r##"{"action": "start", "minutes": 5}"##, &[], "", &[("design", "settings.timer", false)]),
    // board.timer pause: reads the running timer
    b(r##"[["board.timer", {"action": "start"}]]"##, "board.timer", r##"{"action": "pause"}"##, &[], "", &[("design", "settings.timer", false)]),
    // board.vote start: refused while one runs; replaces the vote and clears the ballots (each
    // voter's own settings key; none yet); author from prefs
    b(r##""##, "board.vote", r##"{"action": "start", "votes": 2}"##, &[], "mode", &[("design", "settings.vote", false)]),
    // board.vote start on the selection: ids from the selection (layers with a parent)
    b(
        r##""##,
        "board.vote",
        r##"{"action": "start", "on": "selection"}"##,
        &["$Sticky", "$Shape"],
        "selection,mode",
        &[("design", "settings.vote", false), ("$Sticky", "-", true), ("$Shape", "-", true)],
    ),
    // board.vote cast on $Sticky: reads the running vote's scope and per-person count and the voter's ballot (its own settings key; existing nodes only); voter from prefs/live
    b(
        r##"[["board.vote", {"action": "start"}]]"##,
        "board.vote",
        r##"{"action": "cast", "id": "$Sticky"}"##,
        &[],
        "mode",
        &[("design", "settings.vote,settings.votes/You", false), ("$Sticky", "-", true)],
    ),
    // board.vote cast with a stickies-only vote: the scope reads the target's props.board
    b(
        r##"[["board.vote", {"action": "start", "on": "stickies"}]]"##,
        "board.vote",
        r##"{"action": "cast", "id": "$Sticky2"}"##,
        &[],
        "mode",
        &[("design", "settings.vote,settings.votes/You", false), ("$Sticky2", "props.board", true)],
    ),
    // board.vote end: sets the running vote ended
    b(r##"[["board.vote", {"action": "start"}]]"##, "board.vote", r##"{"action": "end"}"##, &[], "", &[("design", "settings.vote", false)]),
    // board.table at a given place: a new table on the current page
    b(r##""##, "board.table", r##"{"rows": 2, "columns": 2, "x": 0, "y": 900}"##, &[], "part", &[]),
    // board.table without x/y: centred in the view
    b(r##""##, "board.table", r##"{"cells": [["a", "b"]]}"##, &[], "part,mode", &[]),
    // board.tableEdit addColumn: rows = the table's non-absolute children ($Row0, $Row1), column count from $Row0's cells, the table's width/height; replaces the table's width, appends a cell to each row at index 2, rewrites each row's fills/strokes, deletes the absolute $ColumnLine and remakes the lines
    b(
        r##""##,
        "board.tableEdit",
        r##"{"id": "$Table", "action": "addColumn"}"##,
        &[],
        "",
        &[
            ("$Table", "props.board,frame", false),
            ("$Row0", "childLayout,fills,strokes,~hierarchy", true),
            ("$Row1", "childLayout,fills,strokes,~hierarchy", true),
            ("$ColumnLine", "childLayout", true),
        ],
    ),
    // board.tableEdit from a cell's text: walks up $Cell11's ancestors to the Table (props.board), removes the last row ($Row1), restyles $Row0, remakes the column lines
    // Reviewed against board_table::table_of: every node up to the table, including the
    // intervening cell frame, is tested for the table marker and contributes its parent link.
    b(
        r##""##,
        "board.tableEdit",
        r##"{"id": "$Cell11", "action": "removeRow"}"##,
        &[],
        "",
        &[
            ("$Cell11", "props.board,~hierarchy", true),
            ("$CellFrame11", "props.board,~hierarchy", true),
            ("$Row1", "props.board,childLayout,~hierarchy", true),
            ("$Table", "props.board,frame", false),
            ("$Row0", "childLayout,fills,strokes,~hierarchy", true),
            ("$ColumnLine", "childLayout", true),
        ],
    ),
    // board.tableEdit addRow at index 0 on the selection: the new row goes at $Row0's place, with $Row0's cell count; restyles both rows, remakes the column lines
    b(
        r##""##,
        "board.tableEdit",
        r##"{"action": "addRow", "index": 0}"##,
        &["$Table"],
        "selection",
        &[
            ("$Table", "props.board,frame", false),
            ("$Row0", "childLayout,fills,strokes,~hierarchy", true),
            ("$Row1", "childLayout,fills,strokes,~hierarchy", true),
            ("$ColumnLine", "childLayout", true),
        ],
    ),
    // board.template at a given place: only creates layers on the current page
    b(r##""##, "board.template", r##"{"name": "kanban", "x": 0, "y": 900}"##, &[], "part", &[]),
    // board.template without x/y: centred in the view
    b(r##""##, "board.template", r##"{"name": "flowChart"}"##, &[], "part,mode", &[]),
    // tool.move during a vector edit: set_tool does not end a vector edit for Move/Scale/Hand (no pen active), so nothing in the document is read or written
    s(r##"[["vector.edit.begin", {"id": "$V"}]]"##, "tool.move", r##"{}"##, &[], "mode", &[]),
    // tool.scale during a vector edit: set_tool does not end a vector edit for Move/Scale/Hand (no pen active), so nothing in the document is read or written
    s(r##"[["vector.edit.begin", {"id": "$V"}]]"##, "tool.scale", r##"{}"##, &[], "mode", &[]),
    // tool.hand during a vector edit: set_tool does not end a vector edit for Move/Scale/Hand (no pen active), so nothing in the document is read or written
    s(r##"[["vector.edit.begin", {"id": "$V"}]]"##, "tool.hand", r##"{}"##, &[], "mode", &[]),
    // tool.frame during a vector edit: the switch ends the vector edit, reading $V's network to remove it if emptied
    s(r##"[["vector.edit.begin", {"id": "$V"}]]"##, "tool.frame", r##"{}"##, &[], "mode", &[("$V", "kind", false)]),
    // tool.section during a vector edit: the switch ends the vector edit, reading $V's network to remove it if emptied
    s(r##"[["vector.edit.begin", {"id": "$V"}]]"##, "tool.section", r##"{}"##, &[], "mode", &[("$V", "kind", false)]),
    // tool.slice during a vector edit: the switch ends the vector edit, reading $V's network to remove it if emptied
    s(r##"[["vector.edit.begin", {"id": "$V"}]]"##, "tool.slice", r##"{}"##, &[], "mode", &[("$V", "kind", false)]),
    // tool.rectangle during a vector edit: the switch ends the vector edit, reading $V's network to remove it if emptied
    s(r##"[["vector.edit.begin", {"id": "$V"}]]"##, "tool.rectangle", r##"{}"##, &[], "mode", &[("$V", "kind", false)]),
    // tool.line during a vector edit: the switch ends the vector edit, reading $V's network to remove it if emptied
    s(r##"[["vector.edit.begin", {"id": "$V"}]]"##, "tool.line", r##"{}"##, &[], "mode", &[("$V", "kind", false)]),
    // tool.arrow during a vector edit: the switch ends the vector edit, reading $V's network to remove it if emptied
    s(r##"[["vector.edit.begin", {"id": "$V"}]]"##, "tool.arrow", r##"{}"##, &[], "mode", &[("$V", "kind", false)]),
    // tool.ellipse during a vector edit: the switch ends the vector edit, reading $V's network to remove it if emptied
    s(r##"[["vector.edit.begin", {"id": "$V"}]]"##, "tool.ellipse", r##"{}"##, &[], "mode", &[("$V", "kind", false)]),
    // tool.polygon during a vector edit: the switch ends the vector edit, reading $V's network to remove it if emptied
    s(r##"[["vector.edit.begin", {"id": "$V"}]]"##, "tool.polygon", r##"{}"##, &[], "mode", &[("$V", "kind", false)]),
    // tool.star during a vector edit: the switch ends the vector edit, reading $V's network to remove it if emptied
    s(r##"[["vector.edit.begin", {"id": "$V"}]]"##, "tool.star", r##"{}"##, &[], "mode", &[("$V", "kind", false)]),
    // tool.pen during a vector edit: the switch ends the vector edit, reading $V's network to remove it if emptied
    s(r##"[["vector.edit.begin", {"id": "$V"}]]"##, "tool.pen", r##"{}"##, &[], "mode", &[("$V", "kind", false)]),
    // tool.pencil during a vector edit: the switch ends the vector edit, reading $V's network to remove it if emptied
    s(r##"[["vector.edit.begin", {"id": "$V"}]]"##, "tool.pencil", r##"{}"##, &[], "mode", &[("$V", "kind", false)]),
    // tool.text during a vector edit: the switch ends the vector edit, reading $V's network to remove it if emptied
    s(r##"[["vector.edit.begin", {"id": "$V"}]]"##, "tool.text", r##"{}"##, &[], "mode", &[("$V", "kind", false)]),
    // tool.comment during a vector edit: the switch ends the vector edit, reading $V's network to remove it if emptied
    s(r##"[["vector.edit.begin", {"id": "$V"}]]"##, "tool.comment", r##"{}"##, &[], "mode", &[("$V", "kind", false)]),
    // tool.shapeBuilder during a vector edit: the switch ends the vector edit, reading $V's network to remove it if emptied (then shape builder overlay of the selection, no write)
    s(r##"[["vector.edit.begin", {"id": "$V"}]]"##, "tool.shapeBuilder", r##"{}"##, &[], "mode", &[("$V", "kind", false)]),
    // tool.set during a vector edit: the switch ends the vector edit, reading $V's network to remove it if emptied
    s(r##"[["vector.edit.begin", {"id": "$V"}]]"##, "tool.set", r##"{"tool": "rectangle"}"##, &[], "mode", &[("$V", "kind", false)]),
    // tool.set move during a vector edit: no end of the edit, no write
    s(r##"[["vector.edit.begin", {"id": "$V"}]]"##, "tool.set", r##"{"tool": "move"}"##, &[], "mode", &[]),
    // tool.textOnPath during a vector edit: the switch ends the vector edit, reading $V's network to remove it if emptied
    s(r##"[["vector.edit.begin", {"id": "$V"}]]"##, "tool.textOnPath", r##"{}"##, &[], "mode", &[("$V", "kind", false)]),
    // tool.sticky is refused on a design (require board), so it runs on the board during a vector edit of $Ink: the switch ends the edit, reading $Ink's network (removed if emptied)
    b(r##"[["vector.edit.begin", {"id": "$Ink"}]]"##, "tool.sticky", r##"{}"##, &[], "mode", &[("$Ink", "kind", false)]),
    // tool.shape is refused on a design (require board), so it runs on the board during a vector edit of $Ink: the switch ends the edit, reading $Ink's network (removed if emptied)
    b(r##"[["vector.edit.begin", {"id": "$Ink"}]]"##, "tool.shape", r##"{}"##, &[], "mode", &[("$Ink", "kind", false)]),
    // tool.connector is refused on a design (require board), so it runs on the board during a vector edit of $Ink: the switch ends the edit, reading $Ink's network (removed if emptied)
    b(r##"[["vector.edit.begin", {"id": "$Ink"}]]"##, "tool.connector", r##"{}"##, &[], "mode", &[("$Ink", "kind", false)]),
    // tool.marker is refused on a design (require board), so it runs on the board during a vector edit of $Ink: the switch ends the edit, reading $Ink's network (removed if emptied)
    b(r##"[["vector.edit.begin", {"id": "$Ink"}]]"##, "tool.marker", r##"{}"##, &[], "mode", &[("$Ink", "kind", false)]),
    // tool.highlighter is refused on a design (require board), so it runs on the board during a vector edit of $Ink: the switch ends the edit, reading $Ink's network (removed if emptied)
    b(r##"[["vector.edit.begin", {"id": "$Ink"}]]"##, "tool.highlighter", r##"{}"##, &[], "mode", &[("$Ink", "kind", false)]),
    // tool.eraser is refused on a design (require board), so it runs on the board during a vector edit of $Ink: the switch ends the edit, reading $Ink's network (removed if emptied)
    b(r##"[["vector.edit.begin", {"id": "$Ink"}]]"##, "tool.eraser", r##"{}"##, &[], "mode", &[("$Ink", "kind", false)]),
    // tool.stamp is refused on a design (require board), so it runs on the board during a vector edit of $Ink: the switch ends the edit, reading $Ink's network (removed if emptied)
    // review: board::require consumes the persisted file kind.
    b(
        r##"[["vector.edit.begin", {"id": "$Ink"}]]"##,
        "tool.stamp",
        r##"{}"##,
        &[],
        "mode",
        &[("design", "settings.kind", false), ("$Ink", "kind", false)],
    ),
    // edit.pickColor during a vector edit: switching to the eyedropper ends the vector edit, reading $V's network
    s(r##"[["vector.edit.begin", {"id": "$V"}]]"##, "edit.pickColor", r##"{}"##, &[], "mode", &[("$V", "kind", false)]),
    // ---- Reading 1: the fallback paths ----
    // edit.pasteReplace on the selection: replaces the selected $E with the clipboard at $E's index and world top-left
    s(
        r##"[["select.set", {"ids": ["$G1"]}], ["edit.copy", {}], ["select.none", {}]]"##,
        "edit.pasteReplace",
        r##"{}"##,
        &["$E"],
        "selection,mode",
        &[("$E", "frame", true)],
    ),
    // edit.duplicate on the selection: copies $E's whole subtree in place at index+1 (prefs name bump; the repeat offset if the selection holds the last copies)
    r("edit.duplicate", r##"{}"##, &["$E"], "selection,mode", &[("$E", "*", true)]),
    // edit.delete on the selection: removes $E
    r("edit.delete", r##"{}"##, &["$E"], "selection", &[("$E", "-", false)]),
    // edit.pasteProperties on the selection: copied properties onto $E (type, paint bindings, style link against $Brand)
    s(
        r##"[["select.set", {"ids": ["$G1"]}], ["edit.copyProperties", {}], ["select.none", {}]]"##,
        "edit.pasteProperties",
        r##"{}"##,
        &["$E"],
        "selection,mode",
        &[("$E", "kind,fills,strokes,stroke,effects,opacity,props.blend,bindings.styles,bindings.bound", false), ("$Brand", "value", false)],
    ),
    // object.ungroup on the selection: $G's children go to its parent at its index with world places kept, $G removed
    r(
        "object.ungroup",
        r##"{}"##,
        &["$G"],
        "selection",
        &[
            ("$G", "kind,opacity,frame.transform,~hierarchy", true),
            ("$G1", "frame.transform,props.bool_op", true),
            ("$G2", "frame.transform,props.bool_op", true),
        ],
    ),
    // object.removeFrame on the selection: $F's children go to the page at its index, $F removed
    r(
        "object.removeFrame",
        r##"{}"##,
        &["$F"],
        "selection",
        &[
            ("$F", "kind,opacity,frame.transform,~hierarchy", true),
            ("$A", "frame.transform,childLayout,props.bool_op", true),
            ("$T", "frame.transform,childLayout,props.bool_op", true),
        ],
    ),
    // object.resizeToFit on the selection: $D's box becomes the box of its visible children
    r("object.resizeToFit", r##"{}"##, &["$D"], "selection", &[("$D", "kind,layout,frame,~hierarchy", false), ("$LI", "frame,visible", true)]),
    // object.mask on the selection (one layer): toggles its mask flag
    r("object.mask", r##"{}"##, &["$E"], "selection", &[("$E", "props.is_mask", false)]),
    // object.releaseMask on the selection: mask flag off
    r("object.releaseMask", r##"{}"##, &["$E"], "selection", &[("$E", "props.is_mask", false)]),
    // UNSURE: object.setThumbnail on the selection (first selected): scans every layer's plugin_data, marks $X
    r("object.setThumbnail", r##"{}"##, &["$X"], "selection", &[("$X", "props.plugin_data", false), ("design", "nodes", false)]),
    // object.flipH on the selection (no edit mode): mirrors about the centre of its world box
    r("object.flipH", r##"{}"##, &["$E"], "selection,mode", &[("$E", "frame", true)]),
    // object.flipV on the selection (no edit mode)
    r("object.flipV", r##"{}"##, &["$E"], "selection,mode", &[("$E", "frame", true)]),
    // object.rotateLeft in vector edit mode: the selected points of $V about their box centre
    s(
        r##"[["vector.edit.begin", {"id": "$V"}], ["vector.edit.selectAll", {}]]"##,
        "object.rotateLeft",
        r##"{}"##,
        &[],
        "mode",
        &[("$V", "kind,frame", true)],
    ),
    // object.rotateRight on the selection: the pivot is the moved rotation origin, else the box centre
    r("object.rotateRight", r##"{}"##, &["$E"], "selection,mode", &[("$E", "frame", true)]),
    // object.rasterize on the selection: renders $E into an image rectangle at its place, $E removed
    r("object.rasterize", r##"{}"##, &["$E"], "selection", &[("$E", "*", true)]),
    // object.toggleVisible on the selection: reads and replaces visible
    r("object.toggleVisible", r##"{}"##, &["$E"], "selection", &[("$E", "visible", false)]),
    // object.toggleLock on the selection: reads and replaces locked, then clears the selection
    r("object.toggleLock", r##"{}"##, &["$E"], "selection", &[("$E", "locked", false)]),
    // object.hideOthers with nothing selected: hides every top-level layer of the current page
    r(
        "object.hideOthers",
        r##"{}"##,
        &[],
        "selection,part",
        &[
            ("$P1", "~hierarchy", false),
            ("$D", "visible", true),
            ("$R", "visible", true),
            ("$E", "visible", true),
            ("$F", "visible", true),
            ("$G", "visible", true),
            ("$V", "visible", true),
            ("$K", "visible", true),
            ("$I", "visible", true),
            ("$X", "visible", true),
            ("$S", "visible", true),
            ("$Set", "visible", true),
        ],
    ),
    // object.removeFill on the selection: clears fills
    r("object.removeFill", r##"{}"##, &["$G1"], "selection", &[("$G1", "fills", false)]),
    // object.removeStroke on the selection: clears strokes
    r("object.removeStroke", r##"{}"##, &["$G1"], "selection", &[("$G1", "strokes", false)]),
    // object.swapFillStroke on the selection: swaps fills and strokes
    r("object.swapFillStroke", r##"{}"##, &["$G1"], "selection", &[("$G1", "fills,strokes", false)]),
    // object.batchRename on the selection: numbers the selected layers in stacking order
    r("object.batchRename", r##"{"name": "Item $n"}"##, &["$E", "$V"], "selection", &[("$E", "name", true), ("$V", "name", true)]),
    // object.moveToPage on the selection: $E to the top of $P2 with its world transform kept
    r("object.moveToPage", r##"{"page": "$P2"}"##, &["$E"], "selection", &[("$E", "frame.transform", true), ("$P2", "-", false)]),
    // command.batch whose step names no target: the step acts on the selection
    r(
        "command.batch",
        r##"{"commands": [{"command": "object.rename", "params": {"name": "Oval"}}]}"##,
        &["$E"],
        "selection",
        &[("$E", "name", false)],
    ),
    // object.convertToFrame on the selection: group $G becomes a frame (fills/strokes cleared, no clip)
    r("object.convertToFrame", r##"{}"##, &["$G"], "selection", &[("$G", "kind,fills,strokes,props.clip", false)]),
    // object.convertToSection on the selection: frame $X becomes a section
    r(
        "object.convertToSection",
        r##"{}"##,
        &["$X"],
        "selection",
        &[("$X", "kind,layout,props.clip,prototype.overflow,props.grids,props.guides,bindings.styles,bindings.bound", false)],
    ),
    // ---- Reading 2: the fallback paths ----
    // layout.add: no ids -> the selection ($X, an empty frame without layout); layout inferred, re-flow
    // Reviewed after comparison: the selected path has the same kind-dependent branch (structure.rs:545).
    s(
        r##"[["select.set", {"ids": ["$X"]}]]"##,
        "layout.add",
        r##"{}"##,
        &[],
        "selection",
        &[("$X", "kind,layout,frame,childLayout,~hierarchy", false)],
    ),
    // layout.remove: no ids -> the selection ($F); layout dropped, coherent sizing of $F and its children
    s(
        r##"[["select.set", {"ids": ["$F"]}]]"##,
        "layout.remove",
        r##"{}"##,
        &[],
        "selection",
        &[("$F", "layout,childLayout,~hierarchy", false), ("$A", "childLayout", true), ("$T", "childLayout,kind,text", true)],
    ),
    // layout.suggest: no ids -> the selection ($D); layout inferred from $LI, $D hugs, re-flow
    // Reviewed after comparison: the selected path uses the same layout_add branch (structure.rs:545).
    s(
        r##"[["select.set", {"ids": ["$D"]}]]"##,
        "layout.suggest",
        r##"{}"##,
        &[],
        "selection",
        &[("$D", "kind,frame,layout,childLayout,~hierarchy", false), ("$LI", "frame,visible,childLayout", true)],
    ),
    // arrange.front: no ids -> the selection ($R) to the top of its parent
    s(r##"[["select.set", {"ids": ["$R"]}]]"##, "arrange.front", r##"{}"##, &[], "selection", &[("$R", "-", true), ("$P1", "~hierarchy", false)]),
    // arrange.forward: no ids -> the selection ($R) swaps with $E above it
    s(r##"[["select.set", {"ids": ["$R"]}]]"##, "arrange.forward", r##"{}"##, &[], "selection", &[("$R", "-", true), ("$E", "-", true)]),
    // arrange.backward: no ids -> the selection ($E) swaps with $R below it
    s(r##"[["select.set", {"ids": ["$E"]}]]"##, "arrange.backward", r##"{}"##, &[], "selection", &[("$E", "-", true), ("$R", "-", true)]),
    // arrange.back: no ids -> the selection ($E) to index 0
    s(r##"[["select.set", {"ids": ["$E"]}]]"##, "arrange.back", r##"{}"##, &[], "selection", &[("$E", "-", true)]),
    // align.left: no ids -> the selection ($R,$X), union bounds
    s(r##"[["select.set", {"ids": ["$R", "$X"]}]]"##, "align.left", r##"{}"##, &[], "selection", &[("$R", "frame", true), ("$X", "frame", true)]),
    // align.hcenter: no ids -> the selection
    s(r##"[["select.set", {"ids": ["$R", "$X"]}]]"##, "align.hcenter", r##"{}"##, &[], "selection", &[("$R", "frame", true), ("$X", "frame", true)]),
    // align.right: no ids -> the selection
    s(r##"[["select.set", {"ids": ["$R", "$X"]}]]"##, "align.right", r##"{}"##, &[], "selection", &[("$R", "frame", true), ("$X", "frame", true)]),
    // align.top: no ids -> the selection
    s(r##"[["select.set", {"ids": ["$R", "$E"]}]]"##, "align.top", r##"{}"##, &[], "selection", &[("$R", "frame", true), ("$E", "frame", true)]),
    // align.vcenter: no ids -> the selection
    s(r##"[["select.set", {"ids": ["$R", "$E"]}]]"##, "align.vcenter", r##"{}"##, &[], "selection", &[("$R", "frame", true), ("$E", "frame", true)]),
    // align.bottom: no ids -> the selection; one layer ($LI) aligns to its parent $D
    s(r##"[["select.set", {"ids": ["$LI"]}]]"##, "align.bottom", r##"{}"##, &[], "selection", &[("$LI", "frame", true), ("$D", "frame", true)]),
    // distribute.horizontal: no ids -> the selection ($R,$F,$X)
    s(
        r##"[["select.set", {"ids": ["$R", "$F", "$X"]}]]"##,
        "distribute.horizontal",
        r##"{}"##,
        &[],
        "selection",
        &[("$R", "frame", true), ("$F", "frame", true), ("$X", "frame", true)],
    ),
    // distribute.vertical: no ids -> the selection ($R,$E,$S)
    s(
        r##"[["select.set", {"ids": ["$R", "$E", "$S"]}]]"##,
        "distribute.vertical",
        r##"{}"##,
        &[],
        "selection",
        &[("$R", "frame", true), ("$E", "frame", true), ("$S", "frame", true)],
    ),
    // arrange.tidy: no ids -> the selection ($R,$E)
    s(r##"[["select.set", {"ids": ["$R", "$E"]}]]"##, "arrange.tidy", r##"{}"##, &[], "selection", &[("$R", "frame", true), ("$E", "frame", true)]),
    // arrange.packH: no ids -> the selection ($R,$X)
    s(r##"[["select.set", {"ids": ["$R", "$X"]}]]"##, "arrange.packH", r##"{}"##, &[], "selection", &[("$R", "frame", true), ("$X", "frame", true)]),
    // arrange.packV: no ids -> the selection ($R,$E)
    s(r##"[["select.set", {"ids": ["$R", "$E"]}]]"##, "arrange.packV", r##"{}"##, &[], "selection", &[("$R", "frame", true), ("$E", "frame", true)]),
    // arrange.roundToPixels: no ids -> the selection ($R)
    s(r##"[["select.set", {"ids": ["$R"]}]]"##, "arrange.roundToPixels", r##"{}"##, &[], "selection", &[("$R", "frame", false)]),
    // frame.create: preset, no position, current page empty ($P2): placed at the view's centre (view state)
    s(r##"[["page.select", {"id": "$P2"}]]"##, "frame.create", r##"{"preset": "iPhone 16"}"##, &[], "part,mode", &[("$P2", "~hierarchy", false)]),
    // frame.swapOrientation: no ids -> the selection ($X)
    s(r##"[["select.set", {"ids": ["$X"]}]]"##, "frame.swapOrientation", r##"{}"##, &[], "selection", &[("$X", "frame,childLayout", false)]),
    // node.set: no ids -> the selection ($R); reviewed opacity binding detachment as above.
    s(
        r##"[["select.set", {"ids": ["$R"]}]]"##,
        "node.set",
        r##"{"props": {"opacity": 0.5}}"##,
        &[],
        "selection",
        &[("$R", "opacity,bindings.bound", false)],
    ),
    // node.reparent: no ids -> the selection ($E) moved into $X (world position kept)
    s(
        r##"[["select.set", {"ids": ["$E"]}]]"##,
        "node.reparent",
        r##"{"parent": "$X"}"##,
        &[],
        "selection",
        &[("$E", "frame.transform", true), ("$X", "frame.transform", true)],
    ),
    // arrange.spacing: no ids -> the selection ($R,$E column), vertical gap 20
    s(
        r##"[["select.set", {"ids": ["$R", "$E"]}]]"##,
        "arrange.spacing",
        r##"{"vertical": 20}"##,
        &[],
        "selection",
        &[("$R", "frame", true), ("$E", "frame", true)],
    ),
    // arrange.reorder: no ids -> the selection ($R,$E column), $R to $E's slot
    s(
        r##"[["select.set", {"ids": ["$R", "$E"]}]]"##,
        "arrange.reorder",
        r##"{"source": "$R", "target": "$E"}"##,
        &[],
        "selection",
        &[("$R", "frame", true), ("$E", "frame", true)],
    ),
    // transform.move: no ids -> the selection ($R)
    s(r##"[["select.set", {"ids": ["$R"]}]]"##, "transform.move", r##"{"dx": 5, "dy": 5}"##, &[], "selection", &[("$R", "frame.transform", true)]),
    // transform.moveTo: no ids -> the selection ($R,$E)
    s(
        r##"[["select.set", {"ids": ["$R", "$E"]}]]"##,
        "transform.moveTo",
        r##"{"x": 10, "y": 10}"##,
        &[],
        "selection",
        &[("$R", "frame", true), ("$E", "frame", true)],
    ),
    // transform.rotate: no ids -> the selection ($R) whose rotation origin was moved: turns about that origin (editor state)
    s(
        r##"[["select.set", {"ids": ["$R"]}], ["transform.setRotationOrigin", {"x": 0, "y": 0}]]"##,
        "transform.rotate",
        r##"{"degrees": 45}"##,
        &[],
        "selection,mode",
        &[("$R", "frame", true)],
    ),
    // transform.scale: no ids -> the selection ($R)
    s(
        r##"[["select.set", {"ids": ["$R"]}]]"##,
        "transform.scale",
        r##"{"factor": 2}"##,
        &[],
        "selection",
        &[("$R", "frame,fills,strokes,stroke,corners,effects,childLayout,layout,kind,props.grids,props.guides", true)],
    ),
    // transform.resize: no ids -> the selection ($R)
    s(r##"[["select.set", {"ids": ["$R"]}]]"##, "transform.resize", r##"{"width": 200}"##, &[], "selection", &[("$R", "frame,childLayout", false)]),
    // section.toggleContents: no ids -> the selection ($S); reads its kind for the default
    s(r##"[["select.set", {"ids": ["$S"]}]]"##, "section.toggleContents", r##"{}"##, &[], "selection", &[("$S", "kind", false)]),
    // layout.setTrack: no ids -> the selection ($X made a grid by the first step)
    s(
        r##"[["node.set", {"ids": ["$X"], "props": {"layout": "grid", "gridColumns": 2, "gridRows": 1}}], ["select.set", {"ids": ["$X"]}]]"##,
        "layout.setTrack",
        r##"{"axis": "columns", "index": 0, "sizing": "fill"}"##,
        &[],
        "selection",
        &[("$X", "layout,frame", false)],
    ),
    // guide.add: owner named by `page` ($P2, not the current page): its guides (page kind)
    r("guide.add", r##"{"vertical": true, "offset": 10, "page": "$P2"}"##, &[], "", &[("$P2", "kind", false)]),
    // guide.move: no frame/page -> the current page's guide 0
    s(
        r##"[["guide.add", {"vertical": true, "offset": 10}]]"##,
        "guide.move",
        r##"{"index": 0, "offset": 60}"##,
        &[],
        "part",
        &[("$P1", "kind", false)],
    ),
    // guide.remove: index, no frame/page -> the current page's guide 0
    s(r##"[["guide.add", {"vertical": true, "offset": 10}]]"##, "guide.remove", r##"{"index": 0}"##, &[], "part", &[("$P1", "kind", false)]),
    // ---- Reading 3: the fallback paths ----
    // text.italic: no ids -> the selection ($T, and the texts in its subtree); same reads as the ids path; detaches changed bindings; auto-size + $F reflow
    s(
        r##"[["select.set", {"ids": ["$T"]}]]"##,
        "text.italic",
        r##"{}"##,
        &[],
        "selection",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.underline: no ids -> the selection ($T, and the texts in its subtree); same reads as the ids path; detaches changed bindings; auto-size + $F reflow
    s(
        r##"[["select.set", {"ids": ["$T"]}]]"##,
        "text.underline",
        r##"{}"##,
        &[],
        "selection",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.strikethrough: no ids -> the selection ($T, and the texts in its subtree); same reads as the ids path; detaches changed bindings; auto-size + $F reflow
    s(
        r##"[["select.set", {"ids": ["$T"]}]]"##,
        "text.strikethrough",
        r##"{}"##,
        &[],
        "selection",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.link: no ids -> the selection ($T, and the texts in its subtree); same reads as the ids path; detaches changed bindings; auto-size + $F reflow
    s(
        r##"[["select.set", {"ids": ["$T"]}]]"##,
        "text.link",
        r##"{"url": "https://example.com"}"##,
        &[],
        "selection",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.sizeUp: no ids -> the selection ($T, and the texts in its subtree); same reads as the ids path; detaches changed bindings; auto-size + $F reflow
    s(
        r##"[["select.set", {"ids": ["$T"]}]]"##,
        "text.sizeUp",
        r##"{}"##,
        &[],
        "selection",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.sizeDown: no ids -> the selection ($T, and the texts in its subtree); same reads as the ids path; detaches changed bindings; auto-size + $F reflow
    s(
        r##"[["select.set", {"ids": ["$T"]}]]"##,
        "text.sizeDown",
        r##"{}"##,
        &[],
        "selection",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.weightUp: no ids -> the selection ($T, and the texts in its subtree); same reads as the ids path; detaches changed bindings; auto-size + $F reflow
    s(
        r##"[["select.set", {"ids": ["$T"]}]]"##,
        "text.weightUp",
        r##"{}"##,
        &[],
        "selection",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.weightDown: no ids -> the selection ($T, and the texts in its subtree); same reads as the ids path; detaches changed bindings; auto-size + $F reflow
    s(
        r##"[["select.set", {"ids": ["$T"]}]]"##,
        "text.weightDown",
        r##"{}"##,
        &[],
        "selection",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.lineHeightUp: no ids -> the selection ($T, and the texts in its subtree); same reads as the ids path; detaches changed bindings; auto-size + $F reflow
    s(
        r##"[["select.set", {"ids": ["$T"]}]]"##,
        "text.lineHeightUp",
        r##"{}"##,
        &[],
        "selection",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.lineHeightDown: no ids -> the selection ($T, and the texts in its subtree); same reads as the ids path; detaches changed bindings; auto-size + $F reflow
    s(
        r##"[["select.set", {"ids": ["$T"]}]]"##,
        "text.lineHeightDown",
        r##"{}"##,
        &[],
        "selection",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.trackingUp: no ids -> the selection ($T, and the texts in its subtree); same reads as the ids path; detaches changed bindings; auto-size + $F reflow
    s(
        r##"[["select.set", {"ids": ["$T"]}]]"##,
        "text.trackingUp",
        r##"{}"##,
        &[],
        "selection",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.trackingDown: no ids -> the selection ($T, and the texts in its subtree); same reads as the ids path; detaches changed bindings; auto-size + $F reflow
    s(
        r##"[["select.set", {"ids": ["$T"]}]]"##,
        "text.trackingDown",
        r##"{}"##,
        &[],
        "selection",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.caseOriginal: no ids -> the selection ($T, and the texts in its subtree); same reads as the ids path; detaches changed bindings; auto-size + $F reflow
    // review: unchanged fixture text does not schedule reflow (derived.rs:148).
    s(
        r##"[["select.set", {"ids": ["$T"]}]]"##,
        "text.caseOriginal",
        r##"{}"##,
        &[],
        "selection",
        &[("$T", "kind,text,frame.width,frame.height,childLayout.sizing_h,childLayout.sizing_v,bindings.bound,bindings.styles", false)],
    ),
    // text.caseUpper: no ids -> the selection ($T, and the texts in its subtree); same reads as the ids path; detaches changed bindings; auto-size + $F reflow
    s(
        r##"[["select.set", {"ids": ["$T"]}]]"##,
        "text.caseUpper",
        r##"{}"##,
        &[],
        "selection",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.caseLower: no ids -> the selection ($T, and the texts in its subtree); same reads as the ids path; detaches changed bindings; auto-size + $F reflow
    s(
        r##"[["select.set", {"ids": ["$T"]}]]"##,
        "text.caseLower",
        r##"{}"##,
        &[],
        "selection",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.caseTitle: no ids -> the selection ($T, and the texts in its subtree); same reads as the ids path; detaches changed bindings; auto-size + $F reflow
    s(
        r##"[["select.set", {"ids": ["$T"]}]]"##,
        "text.caseTitle",
        r##"{}"##,
        &[],
        "selection",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.caseSmallCaps: no ids -> the selection ($T, and the texts in its subtree); same reads as the ids path; detaches changed bindings; auto-size + $F reflow
    s(
        r##"[["select.set", {"ids": ["$T"]}]]"##,
        "text.caseSmallCaps",
        r##"{}"##,
        &[],
        "selection",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.case: no ids -> the selection ($T, and the texts in its subtree); same reads as the ids path; detaches changed bindings; auto-size + $F reflow
    s(
        r##"[["select.set", {"ids": ["$T"]}]]"##,
        "text.case",
        r##"{"case": "upper"}"##,
        &[],
        "selection",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.decoration: no ids -> the selection ($T, and the texts in its subtree); same reads as the ids path; detaches changed bindings; auto-size + $F reflow
    s(
        r##"[["select.set", {"ids": ["$T"]}]]"##,
        "text.decoration",
        r##"{"decoration": "underline"}"##,
        &[],
        "selection",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.bulletList: no ids -> the selection ($T and its subtree); toggle reads the paragraphs' common list
    s(
        r##"[["select.set", {"ids": ["$T"]}]]"##,
        "text.bulletList",
        r##"{}"##,
        &[],
        "selection",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.numberList: no ids -> the selection ($T and its subtree); toggle reads the paragraphs' common list
    s(
        r##"[["select.set", {"ids": ["$T"]}]]"##,
        "text.numberList",
        r##"{}"##,
        &[],
        "selection",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.alignLeft: no ids -> the selection ($T and its subtree); writes align
    // review: unchanged fixture text does not schedule reflow (derived.rs:148).
    s(
        r##"[["select.set", {"ids": ["$T"]}]]"##,
        "text.alignLeft",
        r##"{}"##,
        &[],
        "selection",
        &[("$T", "kind,text,frame.width,frame.height,childLayout.sizing_h,childLayout.sizing_v,bindings.styles", false)],
    ),
    // text.alignCenter: no ids -> the selection ($T and its subtree); writes align
    s(
        r##"[["select.set", {"ids": ["$T"]}]]"##,
        "text.alignCenter",
        r##"{}"##,
        &[],
        "selection",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.alignRight: no ids -> the selection ($T and its subtree); writes align
    s(
        r##"[["select.set", {"ids": ["$T"]}]]"##,
        "text.alignRight",
        r##"{}"##,
        &[],
        "selection",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.alignJustify: no ids -> the selection ($T and its subtree); writes align
    s(
        r##"[["select.set", {"ids": ["$T"]}]]"##,
        "text.alignJustify",
        r##"{}"##,
        &[],
        "selection",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.listStyle: no ids -> the selection ($T and its subtree); sets every paragraph's list
    s(
        r##"[["select.set", {"ids": ["$T"]}]]"##,
        "text.listStyle",
        r##"{"style": "bulleted"}"##,
        &[],
        "selection",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.paragraphIndent: no ids -> the selection ($T and its subtree); writes indent
    s(
        r##"[["select.set", {"ids": ["$T"]}]]"##,
        "text.paragraphIndent",
        r##"{"indent": 10}"##,
        &[],
        "selection",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.paragraphSpacing: no ids -> the selection ($T and its subtree); writes spacing
    s(
        r##"[["select.set", {"ids": ["$T"]}]]"##,
        "text.paragraphSpacing",
        r##"{"spacing": 8}"##,
        &[],
        "selection",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.maxLines: no ids -> the selection ($T and its subtree); reads auto_resize, writes max lines/truncate
    s(
        r##"[["select.set", {"ids": ["$T"]}]]"##,
        "text.maxLines",
        r##"{"lines": 2}"##,
        &[],
        "selection",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.paragraphSpacing: edit session: the whole edited layer (no ids)
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 0}], ["text.edit.caret", {"index": 2, "extend": true}]]"##,
        "text.paragraphSpacing",
        r##"{"spacing": 8}"##,
        &[],
        "mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.maxLines: edit session: the whole edited layer
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 0}], ["text.edit.caret", {"index": 2, "extend": true}]]"##,
        "text.maxLines",
        r##"{"lines": 2}"##,
        &[],
        "mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.setFeatures: edit session: the whole edited layer (features are per layer)
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 0}], ["text.edit.caret", {"index": 2, "extend": true}]]"##,
        "text.setFeatures",
        r##"{"features": {"liga": false}}"##,
        &[],
        "mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.pathOptions: no ids -> the selected text on a path; reads its start/flip, writes them
    // Reviewed after comparison: para's normalization reads the text-style link (styles.rs:705-739).
    s(
        r##"[["text.onPath", {"text": "$T", "path": "$V"}], ["select.set", {"ids": ["$T"]}]]"##,
        "text.pathOptions",
        r##"{"start": 0.5, "flip": true}"##,
        &[],
        "selection",
        &[("$T", "kind,text,frame.width,frame.height,childLayout.sizing_h,childLayout.sizing_v,bindings.styles", false)],
    ),
    // text.pathOptions: edit session on the text on a path: the edited layer
    // Reviewed after comparison: the edited-range path also normalizes its text-style link.
    s(
        r##"[["text.onPath", {"text": "$T", "path": "$V"}], ["text.edit.begin", {"id": "$T"}]]"##,
        "text.pathOptions",
        r##"{"start": 0.5}"##,
        &[],
        "mode",
        &[("$T", "kind,text,frame.width,frame.height,childLayout.sizing_h,childLayout.sizing_v,bindings.styles", false)],
    ),
    // text.removeFromPath: no ids -> the selected text on a path; auto-width, auto-size writes its size
    s(
        r##"[["text.onPath", {"text": "$T", "path": "$V"}], ["select.set", {"ids": ["$T"]}]]"##,
        "text.removeFromPath",
        r##"{}"##,
        &[],
        "selection",
        &[("$T", "kind,text,frame.width,frame.height,childLayout.sizing_h,childLayout.sizing_v,bindings.styles", false)],
    ),
    // text.removeFromPath: edit session on the text on a path: the edited layer
    s(
        r##"[["text.onPath", {"text": "$T", "path": "$V"}], ["text.edit.begin", {"id": "$T"}]]"##,
        "text.removeFromPath",
        r##"{}"##,
        &[],
        "mode",
        &[("$T", "kind,text,frame.width,frame.height,childLayout.sizing_h,childLayout.sizing_v,bindings.styles", false)],
    ),
    // text.style: no id -> the selection ($T); restyles chars start..end
    s(
        r##"[["select.set", {"ids": ["$T"]}]]"##,
        "text.style",
        r##"{"start": 0, "end": 2, "fontSize": 24}"##,
        &[],
        "selection",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.style: inside a text edit session no id -> the selection, which begin made the edited layer (the edit range is NOT used)
    s(
        r##"[["text.edit.begin", {"id": "$T", "caret": 0}], ["text.edit.caret", {"index": 2, "extend": true}]]"##,
        "text.style",
        r##"{"fontSize": 24}"##,
        &[],
        "selection,mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // text.fit: no ids -> the selected fixed-width text
    // review: unchanged fixture text does not schedule reflow (derived.rs:148).
    s(
        r##"[["node.set", {"id": "$T", "props": {"width": 200}}], ["select.set", {"ids": ["$T"]}]]"##,
        "text.fit",
        r##"{"maxLines": 1, "minSize": 4}"##,
        &[],
        "selection",
        &[("$T", "kind,text,frame.width,frame.height,childLayout.sizing_h,childLayout.sizing_v,bindings.bound,bindings.styles", false)],
    ),
    // text.refit: ids names container $F: re-measures the auto-sizing texts in $F's subtree
    // Reviewed after behavior tests: fonts::refit uses text membership only; already-fitting
    // text causes neither normalization nor a parent/sibling reflow.
    r("text.refit", r##"{"ids": ["$F"]}"##, &[], "", &[("$F", "~textNodes", false), ("$T", "kind,text,frame.width,frame.height", false)]),
    // text.set: inside a text edit session no id -> the selection (the edited layer); replaces all characters
    s(
        r##"[["text.edit.begin", {"id": "$T"}]]"##,
        "text.set",
        r##"{"characters": "Hi there"}"##,
        &[],
        "selection,mode",
        &[
            ("$T", "kind,text,frame.width,frame.height,childLayout,bindings.bound,bindings.styles", true),
            ("$F", TEXT_FLOW_PARENT, false),
            ("$A", "frame,visible,childLayout", true),
        ],
    ),
    // ---- Reading 4: the fallback paths ----
    // vector.flatten without ids: the selection ($R) is flattened (as the ids row)
    s(
        r##"[["select.set", {"ids": ["$R"]}]]"##,
        "vector.flatten",
        r##"{}"##,
        &[],
        "selection",
        &[("$R", "kind,frame,corners,name,fills,strokes,stroke,effects,opacity,props.blend,~hierarchy", true), ("$Th", "anchor", false)],
    ),
    // vector.outlineStroke without ids: the selected open vector with a stroke becomes its outline
    s(
        r##"[["stroke.add", {"ids": ["$V"]}], ["select.set", {"ids": ["$V"]}]]"##,
        "vector.outlineStroke",
        r##"{}"##,
        &[],
        "selection",
        &[(
            "$V",
            "kind,frame,strokes,stroke,name,opacity,props.blend,effects,bindings.styles,constraints,childLayout,visible,locked,~hierarchy",
            true,
        )],
    ),
    // bool.union without ids: the selection is wrapped in a new boolean group looking like the top one ($E)
    s(
        r##"[["select.set", {"ids": ["$R", "$E"]}]]"##,
        "bool.union",
        r##"{}"##,
        &[],
        "selection",
        &[("$R", "frame,props.bool_op,~hierarchy", true), ("$E", "frame,fills,strokes,stroke,effects,props.bool_op,~hierarchy", true)],
    ),
    // bool.subtract without ids: the selection wrapped, looking like the bottom one ($R)
    s(
        r##"[["select.set", {"ids": ["$R", "$E"]}]]"##,
        "bool.subtract",
        r##"{}"##,
        &[],
        "selection",
        &[("$R", "frame,fills,strokes,stroke,effects,props.bool_op,~hierarchy", true), ("$E", "frame,props.bool_op,~hierarchy", true)],
    ),
    // bool.intersect without ids: the selection wrapped, looking like the top one ($E)
    s(
        r##"[["select.set", {"ids": ["$R", "$E"]}]]"##,
        "bool.intersect",
        r##"{}"##,
        &[],
        "selection",
        &[("$R", "frame,props.bool_op,~hierarchy", true), ("$E", "frame,fills,strokes,stroke,effects,props.bool_op,~hierarchy", true)],
    ),
    // bool.exclude without ids: the selection wrapped, looking like the top one ($E)
    s(
        r##"[["select.set", {"ids": ["$R", "$E"]}]]"##,
        "bool.exclude",
        r##"{}"##,
        &[],
        "selection",
        &[("$R", "frame,props.bool_op,~hierarchy", true), ("$E", "frame,fills,strokes,stroke,effects,props.bool_op,~hierarchy", true)],
    ),
    // bool.flatten without ids: the selection flattened into one vector styled like the bottom one ($R)
    s(
        r##"[["select.set", {"ids": ["$R", "$E"]}]]"##,
        "bool.flatten",
        r##"{}"##,
        &[],
        "selection",
        &[
            ("$R", "kind,frame,corners,name,fills,strokes,stroke,effects,opacity,props.blend,~hierarchy", true),
            ("$E", "kind,frame,~hierarchy", true),
            ("$Th", "anchor", false),
        ],
    ),
    // vector.join without ids, not editing: the selected open vector is closed
    s(r##"[["select.set", {"ids": ["$V"]}]]"##, "vector.join", r##"{}"##, &[], "selection", &[("$V", "kind", false)]),
    // vector.smoothJoin without ids, not editing: the selected open vector is closed
    s(r##"[["select.set", {"ids": ["$V"]}]]"##, "vector.smoothJoin", r##"{}"##, &[], "selection", &[("$V", "kind", false)]),
    // vector.simplify without ids: the selected vector's network simplified
    s(r##"[["select.set", {"ids": ["$V"]}]]"##, "vector.simplify", r##"{}"##, &[], "selection", &[("$V", "kind", false)]),
    // vector.reverse without ids: the selected vector's segments reversed
    s(r##"[["select.set", {"ids": ["$V"]}]]"##, "vector.reverse", r##"{}"##, &[], "selection", &[("$V", "kind", false)]),
    // vector.offset without ids: the selected rectangle's outline grown
    s(r##"[["select.set", {"ids": ["$R"]}]]"##, "vector.offset", r##"{"offset": 5}"##, &[], "selection", &[("$R", "kind,frame,corners", false)]),
    // vector.fillRule without ids: the selected (closed) vector's regions
    s(
        r##"[["vector.join", {"ids": ["$V"]}], ["select.set", {"ids": ["$V"]}]]"##,
        "vector.fillRule",
        r##"{"rule": "evenOdd"}"##,
        &[],
        "selection",
        &[("$V", "kind", false)],
    ),
    // vector.reverseLoop without ids: loop 0 of region 0 of the selected (closed) vector
    s(
        r##"[["vector.join", {"ids": ["$V"]}], ["select.set", {"ids": ["$V"]}]]"##,
        "vector.reverseLoop",
        r##"{"region": 0, "loop": 0}"##,
        &[],
        "selection",
        &[("$V", "kind", false)],
    ),
    // shapeBuilder.merge without ids: the arrangement of the selection ($R, $E)
    // Reviewed after behavior tests: points select regions of these selected shapes, not the
    // page; their held geometry supplies the full arrangement.
    s(
        r##"[["select.set", {"ids": ["$R", "$E"]}]]"##,
        "shapeBuilder.merge",
        r##"{"points": [[50, 30], [20, 120]]}"##,
        &[],
        "selection",
        &[
            ("$R", "kind,frame,corners,visible,locked,name,fills,strokes,stroke,effects,opacity,props.blend,~hierarchy", true),
            ("$E", "kind,frame,visible,locked,strokes,stroke,~hierarchy", true),
            ("$Th", "anchor", false),
        ],
    ),
    // shapeBuilder.remove without ids: the arrangement of the selection ($R, $E)
    // Reviewed after behavior tests: arrangement inputs are the selected shapes alone;
    // other page layers do not participate in the region lookup.
    s(
        r##"[["select.set", {"ids": ["$R", "$E"]}]]"##,
        "shapeBuilder.remove",
        r##"{"points": [[50, 30]]}"##,
        &[],
        "selection",
        &[
            ("$R", "kind,frame,corners,visible,locked,strokes,stroke,~hierarchy", true),
            ("$E", "kind,frame,visible,locked,strokes,stroke,~hierarchy", true),
            ("$Th", "anchor", false),
        ],
    ),
    // vector.deleteHeal without vertices: the selected points of every edited layer (point 1), healed
    s(
        r##"[["vector.edit.begin", {"id": "$V"}], ["vector.edit.select", {"vertices": [1]}]]"##,
        "vector.deleteHeal",
        r##"{}"##,
        &[],
        "mode",
        &[("$V", "kind,frame", false)],
    ),
    // vector.edit.mirroring without vertices: the selected points (and selected segments' ends)
    s(
        r##"[["vector.edit.begin", {"id": "$V"}], ["vector.edit.select", {"vertices": [1]}]]"##,
        "vector.edit.mirroring",
        r##"{"mode": "angle"}"##,
        &[],
        "mode",
        &[("$V", "kind", false)],
    ),
    // vector.edit.smooth without vertices: the selected points smoothed; the curves grow the box
    s(
        r##"[["vector.edit.begin", {"id": "$V"}], ["vector.edit.select", {"vertices": [1]}]]"##,
        "vector.edit.smooth",
        r##"{}"##,
        &[],
        "mode",
        &[("$V", "kind,frame", false)],
    ),
    // vector.edit.cornerRadius without vertices: the selected points' radius
    s(
        r##"[["vector.edit.begin", {"id": "$V"}], ["vector.edit.select", {"vertices": [1]}]]"##,
        "vector.edit.cornerRadius",
        r##"{"radius": 5}"##,
        &[],
        "mode",
        &[("$V", "kind", false)],
    ),
    // vector.edit.strokeCap without vertices: the selected points' cap
    s(
        r##"[["vector.edit.begin", {"id": "$V"}], ["vector.edit.select", {"vertices": [0]}]]"##,
        "vector.edit.strokeCap",
        r##"{"cap": "round"}"##,
        &[],
        "mode",
        &[("$V", "kind", false)],
    ),
    // vector.edit.strokeJoin without vertices: the selected points' join
    s(
        r##"[["vector.edit.begin", {"id": "$V"}], ["vector.edit.select", {"vertices": [1]}]]"##,
        "vector.edit.strokeJoin",
        r##"{"join": "round"}"##,
        &[],
        "mode",
        &[("$V", "kind", false)],
    ),
    // vector.edit.moveHandles without handles: the selected handles moved by a page offset; refit
    s(
        r##"[["vector.edit.begin", {"id": "$V"}], ["vector.edit.select", {"handles": [[0, true]]}]]"##,
        "vector.edit.moveHandles",
        r##"{"dx": 0, "dy": -10}"##,
        &[],
        "mode",
        &[("$V", "kind,frame", false)],
    ),
    // ---- Reading 5: the fallback paths ----
    // component.createMultiple: no ids: the selection names the layers; each runs component.create (frame: kind; group: kind, fills, clip, blend)
    r(
        "component.createMultiple",
        r##"{}"##,
        &["$F", "$G"],
        "selection",
        &[("$F", "kind", false), ("$G", "kind,fills,props.clip,props.blend", false)],
    ),
    // component.combineVariants: no ids: the selected main components are combined (kinds, z order, top-most's parent/index, world bounds, names)
    s(
        r##"[["component.create", {"ids": ["$F"]}]]"##,
        "component.combineVariants",
        r##"{}"##,
        &["$K", "$F"],
        "selection",
        &[("$K", "name,kind,frame", true), ("$F", "name,kind,frame", true)],
    ),
    // component.resetInstance: no ids: the selected instance; overrides/props/prop bindings reset; rebuilt from the main
    r(
        "component.resetInstance",
        r##"{}"##,
        &["$I"],
        "selection",
        &[("$I", "kind,instance,bindings", false), ("$IT", "instance,mirrors", true), ("$K", "*", false), ("$KT", "*", true)],
    ),
    // component.detach: no ids: the selected instance becomes a frame
    r(
        "component.detach",
        r##"{}"##,
        &["$I"],
        "selection",
        &[("$I", "kind,mirrors,instance,bindings", false), ("$IT", "mirrors,instance", true), ("$K", "bindings", false)],
    ),
    // component.pushChanges: no id: the selection's first layer resolves to its outermost instance; overridden opacity pushed to the main layer
    s(
        r##"[["node.set", {"ids": ["$IT"], "props": {"opacity": 0.5}}]]"##,
        "component.pushChanges",
        r##"{}"##,
        &["$I"],
        "selection",
        &[
            ("$I", "kind,instance", false),
            ("$IT", "instance,mirrors,opacity,props.blend", true),
            ("$KT", "opacity,props.blend", true),
            ("$K", "-", false),
        ],
    ),
    // component.restoreMain: no id: the selected instance (main deleted) is pasted back as the main at its parent/index
    s(r##"[["edit.delete", {"ids": ["$K"]}]]"##, "component.restoreMain", r##"{}"##, &["$I"], "selection", &[("$I", "*", true), ("$IT", "*", true)]),
    // component.hideWhenPublishing: no ids: the selection's asset roots get the leading '.' (toggle reads the names)
    r("component.hideWhenPublishing", r##"{}"##, &["$K"], "selection", &[("$K", "name,kind", true)]),
    // component.addVariant: no id: the selected set; its last child duplicated, named from the variants' names, placed below them; set grows
    r(
        "component.addVariant",
        r##"{}"##,
        &["$Set"],
        "selection",
        &[("$Set", "kind,frame", false), ("$VS", "name,kind,frame", true), ("$VL", "*", true)],
    ),
    // component.exposeInstance: no id: the selected instance nested in the wrapper component $S1; props.exposed replaced
    s(
        r##"[["component.create", {"ids": ["$I"]}]]"##,
        "component.exposeInstance",
        r##"{}"##,
        &["$I"],
        "selection",
        &[("$I", "kind,props.exposed", true), ("$S1", "kind", true)],
    ),
    // component.makeSlot: no id: the selected frame inside the main $S1; its name names the property; def into $S1's kind, frame prop_refs.slot
    s(
        r##"[["component.create", {"ids": ["$F", "$E"]}]]"##,
        "component.makeSlot",
        r##"{}"##,
        &["$F"],
        "selection",
        &[("$F", "name,kind,instance", true), ("$S1", "kind", true)],
    ),
    // component.removeSlot: no id: the selected slot frame; def removed from $S1's kind, prop_refs naming it in $S1's subtree removed
    // Reviewed after comparison: Owner::resolve also uses the wrapper's place (component/props.rs:528).
    s(
        r##"[["component.create", {"ids": ["$F", "$E"]}], ["component.makeSlot", {"id": "$F"}]]"##,
        "component.removeSlot",
        r##"{}"##,
        &["$F"],
        "selection",
        &[("$F", "instance", true), ("$S1", "kind", true), ("$E", "instance", true), ("$A", "instance", true), ("$T", "instance", true)],
    ),
    // instance.slotInsert: no id: the selected instance with slot name "Body"; $VS copied whole into $I's copy of slot $S1
    // Reviewed after comparison: the generated slot has the same source/override/place reads
    // as the explicit-target path (component/instance.rs:185-216).
    s(
        r##"[["object.frameSelection", {"ids": ["$KT"]}], ["component.makeSlot", {"id": "$S1", "name": "Body"}]]"##,
        "instance.slotInsert",
        r##"{"slot": "Body", "component": "$VS"}"##,
        &["$I"],
        "selection",
        &[
            ("$I", "kind", false),
            ("$S1", "kind,instance", false),
            ("$K", "kind", false),
            ("$VS", "*", false),
            ("@copy:$I:$S1", "instance,mirrors,~hierarchy", true),
        ],
    ),
    // instance.resetSlot: no id: the selected instance's slot "Body" loses its SLOT override; rebuilt with the main's default content ($KT)
    // Reviewed after comparison: reset_slot reads the generated slot's override and source,
    // whose parent also guides the rebuild (component/instance.rs:219-227, sync.rs:52-81).
    s(
        r##"[["object.frameSelection", {"ids": ["$KT"]}], ["component.makeSlot", {"id": "$S1", "name": "Body"}], ["instance.slotInsert", {"id": "$I", "slot": "Body", "component": "$VS"}]]"##,
        "instance.resetSlot",
        r##"{"slot": "Body"}"##,
        &["$I"],
        "selection",
        &[
            ("$I", "kind", false),
            ("$S1", "kind,instance", false),
            ("$K", "kind", false),
            ("$KT", "*", true),
            ("@copy:$I:$S1", "instance,mirrors,~hierarchy", true),
        ],
    ),
    // component.resetOverride: no id: the selected layer inside the instance; the group's override removed, main value copied back
    r(
        "component.resetOverride",
        r##"{"group": "characters"}"##,
        &["$IT"],
        "selection",
        &[("$IT", "instance,mirrors", true), ("$KT", "kind,text", false)],
    ),
    // instance.swap: no ids: the selected instance shows $VS (props by name kept; rebuilt from $VS)
    // review: sync.rs:199 preserves modes used to resolve the new main.
    r(
        "instance.swap",
        r##"{"component": "$VS"}"##,
        &["$I"],
        "selection",
        &[("$I", "kind,instance,bindings.var_modes", false), ("$IT", "name,mirrors,instance", true), ("$VS", "*", false)],
    ),
    // instance.setProperties: no ids: the selected instance's text property set; $KT's binding rebuilds $IT
    // Reviewed after behavior tests: the selected instance rebuild copies the whole main root
    // in sync.rs, while the main's unused page placement does not affect its copied appearance.
    r(
        "instance.setProperties",
        r##"{"properties": {"Label": "Go"}}"##,
        &["$I"],
        "selection",
        &[("$I", "kind", false), ("$K", "*", false), ("$KT", "*", true), ("$IT", "instance,mirrors", true)],
    ),
    // component.addProperty: no id: the selected layer inside $K: owner $K, the layer gets bound (default from its visibility)
    r(
        "component.addProperty",
        r##"{"name": "Show", "type": "boolean"}"##,
        &["$KT"],
        "selection",
        &[("$K", "kind", true), ("$KT", "visible,kind,text,instance", true), ("$I", "kind", false), ("$IT", "instance,mirrors", true)],
    ),
    // component.editProperty: no id: the selected component; text default replaced in its kind
    r("component.editProperty", r##"{"name": "Label", "default": "Hi"}"##, &["$K"], "selection", &[("$K", "kind", true)]),
    // component.deleteProperty: no id: the selected component; def, prop_refs and instances' values removed
    r(
        "component.deleteProperty",
        r##"{"name": "Label"}"##,
        &["$K"],
        "selection",
        &[("$K", "kind", true), ("$KT", "kind,text,instance", true), ("$I", "kind,bindings", false), ("$IT", "instance,mirrors", true)],
    ),
    // component.bindProperty: no layer: the selected layer is unbound (prop_refs cleared); copies follow the main's own characters
    r(
        "component.bindProperty",
        r##"{"property": ""}"##,
        &["$KT"],
        "selection",
        &[("$KT", "kind,text,instance", true), ("$IT", "instance,mirrors", true)],
    ),
    // library.bindVariable: no ids: the selected frame gets the copy of "space" bound to gap
    r(
        "library.bindVariable",
        r##"{"library": "Buttons", "variable": "space", "field": "gap"}"##,
        &["$F"],
        "selection",
        &[("$Lib", "name,path", false), ("design", "libraryCopies,collections,variables", false), ("$F", "kind,layout,bindings", false)],
    ),
    // library.setVariableMode: no ids with a selection: the selected frame (not the page) takes the mode
    r(
        "library.setVariableMode",
        r##"{"library": "Buttons", "collection": "Lib Tokens", "mode": "Base"}"##,
        &["$X"],
        "selection",
        &[("$Lib", "name,path", false), ("design", "libraryCopies,collections", false), ("$X", "bindings", false)],
    ),
    // ---- Reading 6: the fallback paths ----
    // fill.add without ids: targets are the selection
    r("fill.add", r##"{"paint": "#00ff00"}"##, &["$X"], "selection", &[("$X", "fills,bindings.styles,bindings.bound", false)]),
    // stroke.add without ids: the selection
    r("stroke.add", r##"{}"##, &["$X"], "selection", &[("$X", "strokes,bindings.styles,bindings.bound", false)]),
    // fill.remove without ids: the selection's fill 0 (and its binding) removed
    r("fill.remove", r##"{"index": 0}"##, &["$R"], "selection", &[("$R", "fills,bindings.styles,bindings.bound", false)]),
    // stroke.remove without ids: the selection
    s(
        r##"[["stroke.add", {"ids": ["$X"]}]]"##,
        "stroke.remove",
        r##"{"index": 0}"##,
        &["$X"],
        "selection",
        &[("$X", "strokes,bindings.styles,bindings.bound", false)],
    ),
    // fill.set without ids: the selection's top fill
    r("fill.set", r##"{"color": "#ff0000"}"##, &["$X"], "selection", &[("$X", "fills,bindings.styles,bindings.bound", false)]),
    // stroke.set without ids: the selection's top stroke
    r("stroke.set", r##"{"color": "#ff0000"}"##, &["$X"], "selection", &[("$X", "strokes,bindings.styles,bindings.bound", false)]),
    // fill.paste without ids: clipboard fill onto the selection
    s(
        r##"[["select.set", {"ids": ["$R"]}], ["fill.copy", {}]]"##,
        "fill.paste",
        r##"{}"##,
        &["$X"],
        "selection,mode",
        &[("$X", "fills,bindings.styles,bindings.bound", false)],
    ),
    // stroke.paste without ids: clipboard stroke paint + settings onto the selection
    s(
        r##"[["stroke.add", {"ids": ["$X"]}], ["select.set", {"ids": ["$X"]}], ["stroke.copy", {}]]"##,
        "stroke.paste",
        r##"{}"##,
        &["$S"],
        "selection,mode",
        &[("$S", "strokes,stroke,bindings.styles,bindings.bound", false)],
    ),
    // fill.pattern without ids: the selection's top fill becomes the pattern of $V
    r("fill.pattern", r##"{"source": "$V"}"##, &["$X"], "selection", &[("$X", "fills,bindings.styles,bindings.bound", false), ("$V", "-", false)]),
    // stroke.widthProfile without ids: the selection
    r("stroke.widthProfile", r##"{"preset": "taper"}"##, &["$V"], "selection", &[("$V", "stroke", false)]),
    // stroke.brush without ids: the selection
    r("stroke.brush", r##"{"preset": "ink"}"##, &["$V"], "selection", &[("$V", "strokes,stroke,bindings.styles,bindings.bound", false)]),
    // effect.add without ids: the selection
    r("effect.add", r##"{"type": "layerBlur"}"##, &["$X"], "selection", &[("$X", "effects,bindings.styles,bindings.bound", false)]),
    // effect.remove without ids: the selection
    s(
        r##"[["effect.add", {"ids": ["$X"]}]]"##,
        "effect.remove",
        r##"{"index": 0}"##,
        &["$X"],
        "selection",
        &[("$X", "effects,bindings.styles,bindings.bound", false)],
    ),
    // effect.set without ids: the selection
    s(
        r##"[["effect.add", {"ids": ["$X"]}]]"##,
        "effect.set",
        r##"{"index": 0, "effect": {"type": "layerBlur", "radius": 8}}"##,
        &["$X"],
        "selection",
        &[("$X", "effects,bindings.styles,bindings.bound", false)],
    ),
    // effect.paste without ids: clipboard effect onto the selection
    s(
        r##"[["effect.add", {"ids": ["$X"]}], ["select.set", {"ids": ["$X"]}], ["effect.copy", {}]]"##,
        "effect.paste",
        r##"{}"##,
        &["$S"],
        "selection,mode",
        &[("$S", "effects,bindings.styles,bindings.bound", false)],
    ),
    // grid.add without ids: the selection
    r("grid.add", r##"{}"##, &["$X"], "selection", &[("$X", "props.grids,bindings.styles,bindings.bound", false)]),
    // grid.remove without ids: the selection
    s(
        r##"[["grid.add", {"ids": ["$X"]}]]"##,
        "grid.remove",
        r##"{"index": 0}"##,
        &["$X"],
        "selection",
        &[("$X", "props.grids,bindings.styles,bindings.bound", false)],
    ),
    // export.add without ids: the selection (next step after its last setting)
    r("export.add", r##"{}"##, &["$X"], "selection", &[("$X", "props.exports", false)]),
    // export.update without ids: the selection
    s(
        r##"[["export.add", {"ids": ["$X"]}]]"##,
        "export.update",
        r##"{"index": 0, "format": "svg"}"##,
        &["$X"],
        "selection",
        &[("$X", "props.exports", false)],
    ),
    // export.remove without ids: the selection
    s(r##"[["export.add", {"ids": ["$X"]}]]"##, "export.remove", r##"{"index": 0}"##, &["$X"], "selection", &[("$X", "props.exports", false)]),
    // fill.move without ids: the selection
    s(
        r##"[["fill.add", {"ids": ["$X"], "paint": "#ff0000"}], ["fill.add", {"ids": ["$X"], "paint": "#0000ff"}]]"##,
        "fill.move",
        r##"{"from": 0, "to": 1}"##,
        &["$X"],
        "selection",
        &[("$X", "fills,bindings.styles,bindings.bound", false)],
    ),
    // stroke.move without ids: the selection
    s(
        r##"[["stroke.add", {"ids": ["$X"]}], ["stroke.add", {"ids": ["$X"], "paint": "#ff0000"}]]"##,
        "stroke.move",
        r##"{"from": 0, "to": 1}"##,
        &["$X"],
        "selection",
        &[("$X", "strokes,bindings.styles,bindings.bound", false)],
    ),
    // effect.move without ids: the selection
    s(
        r##"[["effect.add", {"ids": ["$X"]}], ["effect.add", {"ids": ["$X"], "type": "layerBlur"}]]"##,
        "effect.move",
        r##"{"from": 0, "to": 1}"##,
        &["$X"],
        "selection",
        &[("$X", "effects,bindings.styles,bindings.bound", false)],
    ),
    // grid.move without ids: the selection
    s(
        r##"[["grid.add", {"ids": ["$X"]}], ["grid.add", {"ids": ["$X"]}]]"##,
        "grid.move",
        r##"{"from": 0, "to": 1}"##,
        &["$X"],
        "selection",
        &[("$X", "props.grids,bindings.styles,bindings.bound", false)],
    ),
    // style.apply without ids: paint style onto the selection
    r(
        "style.apply",
        r##"{"style": "$Brand"}"##,
        &["$X"],
        "selection",
        &[("$X", "fills,bindings.styles,bindings.bound", false), ("$Brand", "value,props.bound", false)],
    ),
    // style.detach fill without ids: the selection's fill style link cleared
    r("style.detach", r##"{"which": "fill"}"##, &["$E"], "selection", &[("$E", "bindings.styles", false)]),
    // variable.bind without ids: the selection's fill bound; value resolved in its modes
    r(
        "variable.bind",
        r##"{"field": "fill", "variable": "$Primary"}"##,
        &["$X"],
        "selection",
        &[
            ("$X", "fills,bindings.styles,bindings.bound,bindings.var_modes", true),
            ("$Primary", "collection,values", false),
            ("$Colors", "modes", false),
            ("$P1", "bindings.var_modes", false),
        ],
    ),
    // variable.setMode without ids, with a selection: the selected frame's mode (not the page)
    r(
        "variable.setMode",
        r##"{"collection": "$Colors", "mode": "$Dark"}"##,
        &["$X"],
        "selection",
        &[("$X", "bindings.var_modes", false), ("$Colors", "-", false)],
    ),
    // image.adjust without ids: the selection's image paints
    r("image.adjust", r##"{"filters": {"exposure": 0.3}}"##, &["$A"], "selection", &[("$A", "fills,bindings.styles,bindings.bound", false)]),
    // image.savePreset without filters/ids: adjustments of the selection's first image paint
    // Reviewed after behavior tests: only the named destination Mine is replaced; other saved presets merge.
    // review: image.rs:166 also checks list capacity before appending Mine.
    r(
        "image.savePreset",
        r##"{"name": "Mine"}"##,
        &["$A"],
        "selection",
        &[("$A", "fills", false), ("design", "settings.image_presets[name=\"Mine\"],~imagePresetCount", false)],
    ),
    // ---- Reading 7: the fallback paths ----
    // page.delete: without id: the current page and everything on it go; the canvas moves to the first page left
    r("page.delete", r##"{}"##, &[], "part", &[("$P1", "-", false)]),
    // page.move: without id: the current page goes to the index in the page list (what is at that place matters)
    r("page.move", r##"{"index": 1}"##, &[], "part", &[("$P1", "-", true), ("slot:design", "-", true)]),
    // proto.interaction.remove: without id: the selection's interaction at the index
    r("proto.interaction.remove", r##"{"index": 0}"##, &["$R"], "selection", &[("$R", "prototype.interactions", false)]),
    // proto.interaction.set: without id: the selection's interaction at the index replaced whole (its navigate to $X must exist)
    r(
        "proto.interaction.set",
        r##"{"index": 0, "interaction": {"trigger": {"type": "onClick"}, "actions": [{"type": "navigate", "dest": "$X"}], "transition": {"type": "dissolve"}, "durationMs": 200}}"##,
        &["$R"],
        "selection",
        &[("$R", "prototype.interactions", false), ("$X", "-", false)],
    ),
    // proto.action.add: without id: appends to the selection's interaction 0
    r("proto.action.add", r##"{"index": 0, "action": {"type": "back"}}"##, &["$R"], "selection", &[("$R", "prototype.interactions", false)]),
    // proto.action.set: without id: the selection's action [0] becomes a navigate to $F
    r(
        "proto.action.set",
        r##"{"index": 0, "path": [0], "action": {"type": "navigate", "dest": "$F"}}"##,
        &["$R"],
        "selection",
        &[("$R", "prototype.interactions", false), ("$F", "-", false)],
    ),
    // proto.action.remove: without id: the selection's action [1]
    s(
        r##"[["proto.action.add", {"id": "$R", "index": 0, "action": {"type": "back"}}]]"##,
        "proto.action.remove",
        r##"{"index": 0, "path": [1]}"##,
        &["$R"],
        "selection",
        &[("$R", "prototype.interactions", false)],
    ),
    // proto.action.move: without id: the selection's action [1] to 0
    s(
        r##"[["proto.action.add", {"id": "$R", "index": 0, "action": {"type": "back"}}]]"##,
        "proto.action.move",
        r##"{"index": 0, "path": [1], "to": 0}"##,
        &["$R"],
        "selection",
        &[("$R", "prototype.interactions", false)],
    ),
    // proto.conditional.addBlock: without id: an else block on the selection's conditional
    s(
        r##"[["proto.action.add", {"id": "$R", "index": 0, "action": {"type": "conditional", "blocks": [{"expression": "true", "actions": []}]}}]]"##,
        "proto.conditional.addBlock",
        r##"{"index": 0, "path": [1], "else": true}"##,
        &["$R"],
        "selection",
        &[("$R", "prototype.interactions", false)],
    ),
    // proto.conditional.setCondition: without id: the selection's block 0 condition
    s(
        r##"[["proto.action.add", {"id": "$R", "index": 0, "action": {"type": "conditional", "blocks": [{"expression": "true", "actions": []}]}}]]"##,
        "proto.conditional.setCondition",
        r##"{"index": 0, "path": [1], "block": 0, "expression": "false"}"##,
        &["$R"],
        "selection",
        &[("$R", "prototype.interactions", false)],
    ),
    // proto.conditional.removeBlock: without id: the selection's block 1
    s(
        r##"[["proto.action.add", {"id": "$R", "index": 0, "action": {"type": "conditional", "blocks": [{"expression": "true", "actions": []}, {"expression": "false", "actions": []}]}}]]"##,
        "proto.conditional.removeBlock",
        r##"{"index": 0, "path": [1], "block": 1}"##,
        &["$R"],
        "selection",
        &[("$R", "prototype.interactions", false)],
    ),
    // comment.reply: without author: signed with the preferences' name (written as the author and as who has read it)
    r("comment.reply", r##"{"id": "$Th", "text": "Done"}"##, &[], "mode", &[("$Th", "anchor,resolved", false)]),
    // comment.react: without author: the preferences' name reacts (its reaction read to toggle)
    r("comment.react", r##"{"id": "$Th", "emoji": "👍"}"##, &[], "mode", &[("$Th", "comments", false)]),
    // codeConnect.remove: without id: the selection's component
    s(
        r##"[["codeConnect.set", {"id": "$K", "name": "Button", "template": "<Button />"}]]"##,
        "codeConnect.remove",
        r##"{}"##,
        &["$K"],
        "selection",
        &[("$K", "dev.code_links", false)],
    ),
    // annotation.edit: without id: the first selected layer's annotation at the index
    s(
        r##"[["annotation.add", {"id": "$R", "label": "Note"}]]"##,
        "annotation.edit",
        r##"{"index": 0, "label": "Changed"}"##,
        &["$R"],
        "selection",
        &[("$R", "dev.annotations", false)],
    ),
    // annotation.remove: without id and index: every annotation of the first selected layer
    s(
        r##"[["annotation.add", {"id": "$R", "label": "Note"}]]"##,
        "annotation.remove",
        r##"{}"##,
        &["$R"],
        "selection",
        &[("$R", "dev.annotations", false)],
    ),
    // devResource.edit: without id: the first selected layer's resource at the index
    s(
        r##"[["devResource.add", {"id": "$R", "url": "https://example.com/docs"}]]"##,
        "devResource.edit",
        r##"{"index": 0, "url": "https://example.com/other"}"##,
        &["$R"],
        "selection",
        &[("$R", "dev.dev_resources", false)],
    ),
    // devResource.remove: without id: the first selected layer's resource at the index
    s(
        r##"[["devResource.add", {"id": "$R", "url": "https://example.com/docs"}]]"##,
        "devResource.remove",
        r##"{"index": 0}"##,
        &["$R"],
        "selection",
        &[("$R", "dev.dev_resources", false)],
    ),
    // devMode.removeMeasurement: without from: the first selected layer's measurement at the index
    s(
        r##"[["devMode.addMeasurement", {"from": "$R", "to": "$E"}]]"##,
        "devMode.removeMeasurement",
        r##"{"index": 0}"##,
        &["$R"],
        "selection",
        &[("$R", "dev.measurements", false)],
    ),
    // ---- Reading 8: the fallback paths ----
    // board.label falling back to the selection (no id): the selected connector's label (found among its parent's children) gets its text replaced whole
    // review: board::require consumes the persisted file kind.
    b(
        r##""##,
        "board.label",
        r##"{"text": "go"}"##,
        &["$Conn"],
        "selection",
        &[
            ("design", "settings.kind", false),
            ("$Conn", "props.board,~hierarchy", true),
            ("$Label", "props.board,layout,~hierarchy", true),
            ("$LabelText", "kind,text,~hierarchy", true),
        ],
    ),
    // board.ink without a colour: the marker tool's colour (tool state) is used; a new vector on the current page
    b(r##""##, "board.ink", r##"{"path": "M0 900 L100 950"}"##, &[], "part,mode", &[]),
    // board.setStyle falling back to the selection (no ids): the selected connector's routing (props.board) and caps (stroke) replaced; normalize re-routes it from its ends' bounds
    b(
        r##""##,
        "board.setStyle",
        r##"{"style": "elbow", "startCap": "circleFilled"}"##,
        &["$Conn"],
        "selection",
        &[("$Conn", "props.board,stroke,kind,frame", false), ("$Sticky", "frame", false), ("$Sticky2", "frame", false)],
    ),
];

/// The board the board rows (`b`) are read on, saved and opened again: its page `$BP` current,
/// nothing selected. Sticky `$Sticky` "Idea" (yellow, at 0, 0), sticky `$Sticky2` (at 300, 0),
/// shape with text `$Shape` (a rectangle "Box" at 0, 300, 160 by 160: its outline and its text
/// are children), connector `$Conn` from `$Sticky` to `$Sticky2` with label "next", table
/// `$Table` (2 by 2 at 600, 0, its cells' texts `$Cell00`, `$Cell01`, `$Cell10`, `$Cell11`), ink
/// `$Ink` (a line at 0, 600), stamp `$Stamp` (at 400, 400). The connector's label is the frame
/// `$Label` holding text `$LabelText`; the table's rows are `$Row0` and `$Row1` (frames of cells),
/// its column line `$ColumnLine`.
pub fn board() -> &'static Fixture {
    static BOARD: OnceLock<Fixture> = OnceLock::new();
    BOARD.get_or_init(|| {
        let mut session = kition_design_engine::Session::empty();
        session.execute("file.newBoard", &json!({ "name": "Board" })).unwrap();
        let bytes = kition_design_format::save_bytes(session.doc().unwrap(), None).unwrap();
        let mut d = DesignDocument::open("Board.kdesign", &bytes).unwrap();
        let page = d.inspect().unwrap()["page"].clone();
        let mut ex = |c: &str, p: Value| d.execute(c, p.clone()).unwrap_or_else(|e| panic!("{c} {p}: {e}"));
        let sticky = id(&ex("board.sticky", json!({ "x": 0, "y": 0, "text": "Idea", "color": "yellow" })));
        let sticky2 = id(&ex("board.sticky", json!({ "x": 300, "y": 0, "text": "Next" })));
        let shape = id(&ex("board.shape", json!({ "shape": "rectangle", "x": 0, "y": 300, "text": "Box" })));
        let conn = id(&ex("board.connector", json!({ "from": sticky, "to": sticky2, "label": "next" })));
        let table = ex("board.table", json!({ "rows": 2, "columns": 2, "x": 600, "y": 0 }));
        let cells = table["cells"].clone();
        let ink = id(&ex("board.ink", json!({ "path": "M0 600 L100 650" })));
        let stamp = id(&ex("board.stamp", json!({ "x": 400, "y": 400, "stamp": "heart" })));
        ex("select.set", json!({ "ids": [] }));
        let bytes = d.save(None).unwrap();
        let doc = d.session().doc().unwrap();
        let shape_text = doc
            .get(kition_design_doc::NodeId(shape.as_u64().unwrap()))
            .unwrap()
            .children
            .iter()
            .copied()
            .find(|child| doc.node(*child).is_some_and(|n| n.kind.text().is_some()))
            .unwrap();
        let tree = d.inspect().unwrap()["tree"].clone();
        let named = |list: &Value, name: &str| -> Vec<Value> {
            list.as_array().unwrap().iter().filter(|n| n["name"] == name).map(|n| n["id"].clone()).collect()
        };
        let label = named(&tree, "Label")[0].clone();
        let label_text = tree.as_array().unwrap().iter().find(|n| n["id"] == label).unwrap()["children"][0]["id"].clone();
        let table_node = tree.as_array().unwrap().iter().find(|n| n["id"] == table["id"]).unwrap().clone();
        let row_of = |cell: &Value| -> Value {
            table_node["children"].as_array().unwrap().iter().find(|row| row.to_string().contains(&cell.to_string())).unwrap()["id"].clone()
        };
        let (row0, row1) = (row_of(&cells[0][0]), row_of(&cells[1][0]));
        let cell_frame11 = doc.get(kition_design_doc::NodeId(cells[1][1].as_u64().unwrap())).unwrap().parent.unwrap();
        let column_line = named(&table_node["children"], "Column line")[0].clone();
        let roles = vec![
            ("$BP", page),
            ("$Label", label),
            ("$LabelText", label_text),
            ("$Row0", row0),
            ("$Row1", row1),
            ("$ColumnLine", column_line),
            ("$Sticky", sticky),
            ("$Sticky2", sticky2),
            ("$Shape", shape),
            ("$ShapeText", json!(shape_text.0)),
            ("$Conn", conn),
            ("$Table", table["id"].clone()),
            ("$Cell00", cells[0][0].clone()),
            ("$Cell01", cells[0][1].clone()),
            ("$Cell10", cells[1][0].clone()),
            ("$Cell11", cells[1][1].clone()),
            ("$CellFrame11", json!(cell_frame11.0)),
            ("$Ink", ink),
            ("$Stamp", stamp),
        ];
        (bytes, roles)
    })
}

#[test]
fn board_roles() {
    let (bytes, roles) = board();
    for (role, value) in roles {
        println!("{role} {value}");
    }
    let doc = DesignDocument::open("Board.kdesign", bytes).unwrap();
    println!("{}", doc.inspect().unwrap()["tree"]);
}

#[test]
fn fixture_roles() {
    let (_, roles) = fixture();
    for (role, value) in roles {
        println!("{role} {value}");
    }
}

/// review: these fixture paths change no text value, so affects_layout
/// never schedules layout_frame. An unconsumed parent limit makes that observable.
#[test]
fn review_noop_text_paths_do_not_reflow_the_parent() {
    for command in ["text.alignLeft", "text.caseOriginal", "text.fit"] {
        let (bytes, roles) = fixture();
        let node = |role: &str| roles.iter().find(|(name, _)| *name == role).unwrap().1.as_u64().unwrap();
        let (text, parent) = (kition_design_doc::NodeId(node("$T")), kition_design_doc::NodeId(node("$F")));
        let mut doc = open(bytes);
        if command == "text.fit" {
            doc.execute("node.set", json!({"id":text.0,"props":{"width":200}})).unwrap();
        }
        let mut file = kition_design_format::load_bytes_with_branches(&doc.save(None).unwrap()).unwrap();
        let width = file.doc.node(parent).unwrap().width;
        file.doc.get_mut(parent).unwrap().child_layout.min_w = Some(width + 100.0);
        let bytes = kition_design_format::save_edited(&file, None).unwrap();
        let mut doc = open(&bytes);
        let before = doc.session().doc().unwrap().node(text).unwrap().kind.clone();
        let params = if command == "text.fit" { json!({"ids":[text.0],"maxLines":1,"minSize":4}) } else { json!({"ids":[text.0]}) };
        doc.execute(command, params).unwrap();
        let current = doc.session().doc().unwrap();
        assert_eq!(current.node(text).unwrap().kind, before, "{command} is a no-op on this fixture");
        assert_eq!(current.node(parent).unwrap().width, width, "{command} must not consume the parent's pending minimum");
    }
}
