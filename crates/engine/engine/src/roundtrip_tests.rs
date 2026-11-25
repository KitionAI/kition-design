//! Command-level round trips: a realistic edit made only with commands survives saving the
//! `.kdesign` file and opening it again, both from a path (`file.saveAs`, then `file.open {path}`, as
//! the desktop does) and from bytes (the kit's save, then `file.open {name, data}`, as the web
//! editor does). The whole document is compared with [`kition_design_kdm::same_content`], which leaves
//! out only what the file keeps for itself (the published library) and the editing person's own
//! state (the current page, the id counter).

use base64::Engine as _;
use kition_design_doc::NodeId;
use serde_json::{Value, json};

use crate::Session;
use crate::params::id_of;

fn ok(s: &mut Session, cmd: &str, p: Value) -> Value {
    s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"))
}

fn node(v: &Value) -> NodeId {
    id_of(&v["id"]).unwrap_or_else(|| panic!("no id in {v}"))
}

fn inspect(s: &Session, id: NodeId) -> Value {
    crate::inspect::node_json(s.doc().unwrap(), id, 2)
}

/// The first JSON pointer at which two documents differ, for a readable failure.
fn first_difference(a: &Value, b: &Value, at: &str) -> Option<String> {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => x
            .keys()
            .chain(y.keys())
            .find_map(|k| first_difference(x.get(k).unwrap_or(&Value::Null), y.get(k).unwrap_or(&Value::Null), &format!("{at}/{k}"))),
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() => {
            x.iter().zip(y).enumerate().find_map(|(i, (u, v))| first_difference(u, v, &format!("{at}/{i}")))
        }
        _ if a == b => None,
        _ => Some(format!("{at}: {a} != {b}")),
    }
}

fn assert_same(before: &kition_design_doc::Document, after: &Session, how: &str) {
    let after = after.doc().unwrap();
    if !kition_design_kdm::same_content(before, after) {
        let comparable = |d: &kition_design_doc::Document| {
            let mut d = d.clone();
            d.current_page = None;
            d.published = None;
            serde_json::to_value(d).unwrap()
        };
        let difference = first_difference(&comparable(before), &comparable(after), "");
        panic!("{how} changed the design at {}", difference.unwrap_or_else(|| "a field JSON does not show".into()));
    }
}

/// Save and reopen both ways; each reopened design must have the same content as the edited one.
/// Opening a file titles the design after the file (as upstream does), so the file is named after
/// the design.
fn survives_save_and_reopen(s: &mut Session, case: &str) {
    let before = s.doc().unwrap().clone();
    let name = before.name.clone();

    let bytes = crate::cmd::file::file_bytes(s.ds().unwrap()).unwrap();
    let mut from_bytes = Session::empty();
    let data = base64::engine::general_purpose::STANDARD.encode(&bytes);
    ok(&mut from_bytes, "file.open", json!({"name": format!("{name}.kdesign"), "data": data}));
    assert_same(&before, &from_bytes, "saving to bytes and opening them");

    let dir = std::env::temp_dir().join(format!("kition-design-roundtrip-{}-{case}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.kdesign")).to_string_lossy().into_owned();
    ok(s, "file.saveAs", json!({"path": path}));
    let mut from_path = Session::empty();
    ok(&mut from_path, "file.open", json!({"path": path}));
    assert_same(&before, &from_path, "file.saveAs then file.open");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn frames_shapes_and_text_survive_save_and_reopen() {
    let mut s = Session::new();
    let f = node(&ok(&mut s, "frame.add", json!({"x": 0, "y": 0, "width": 400, "height": 300})));
    ok(&mut s, "node.set", json!({"id": f, "props": {"name": "Card"}}));
    let r = node(&ok(&mut s, "shape.rectangle", json!({"parent": f, "x": 20, "y": 20, "width": 120, "height": 80})));
    let e = node(&ok(&mut s, "shape.ellipse", json!({"parent": f, "x": 200, "y": 20, "width": 80, "height": 80})));
    let t = node(&ok(&mut s, "text.add", json!({"text": "Hello, design", "x": 20, "y": 140, "parent": f})));
    ok(&mut s, "transform.move", json!({"ids": [e], "dx": 40, "dy": 10}));
    let card = inspect(&s, f);
    assert_eq!(card["name"], "Card");
    assert_eq!(card["children"].as_array().unwrap().len(), 3, "{card}");
    assert!(inspect(&s, t).to_string().contains("Hello, design"));
    assert_eq!((inspect(&s, r)["type"].clone(), inspect(&s, e)["x"].as_f64()), (json!("RECTANGLE"), Some(240.0)), "moved by 40");
    survives_save_and_reopen(&mut s, "frames-shapes-text");
}

#[test]
fn components_and_instances_survive_save_and_reopen() {
    let mut s = Session::new();
    let f = node(&ok(&mut s, "frame.add", json!({"x": 0, "y": 0, "width": 160, "height": 48})));
    ok(&mut s, "shape.rectangle", json!({"parent": f, "x": 0, "y": 0, "width": 160, "height": 48}));
    ok(&mut s, "text.add", json!({"text": "Button", "x": 16, "y": 12, "parent": f}));
    let comp = ok(&mut s, "component.add", json!({"ids": [f]}));
    let comp = id_of(&comp["id"]).or_else(|| id_of(&comp["ids"][0])).or_else(|| id_of(&comp[0])).unwrap_or(f);
    let a = node(&ok(&mut s, "instance.add", json!({"component": comp, "x": 0, "y": 200})));
    let b = node(&ok(&mut s, "instance.add", json!({"component": comp, "x": 200, "y": 200})));
    ok(&mut s, "node.set", json!({"id": b, "props": {"name": "Secondary"}}));
    assert_eq!(inspect(&s, a)["type"], "INSTANCE", "{}", inspect(&s, a));
    assert_eq!(inspect(&s, b)["name"], "Secondary");
    survives_save_and_reopen(&mut s, "components");
}

#[test]
fn auto_layout_survives_save_and_reopen() {
    let mut s = Session::new();
    let f = node(&ok(&mut s, "frame.add", json!({"x": 0, "y": 0, "width": 300, "height": 200})));
    for i in 0..3 {
        ok(&mut s, "shape.rectangle", json!({"parent": f, "x": i * 10, "y": 0, "width": 40, "height": 20}));
    }
    ok(&mut s, "layout.add", json!({"ids": [f]}));
    ok(&mut s, "node.set", json!({"id": f, "props": {"sizingH": "hug"}}));
    let frame = inspect(&s, f);
    assert_eq!((frame["layout"]["mode"].clone(), frame["sizing"]["horizontal"].clone()), (json!("horizontal"), json!("hug")), "{frame}");
    let xs: Vec<f64> = frame["children"].as_array().unwrap().iter().map(|c| c["x"].as_f64().unwrap()).collect();
    assert_eq!(xs, [0.0, 40.0, 80.0], "laid out in a row");
    survives_save_and_reopen(&mut s, "auto-layout");
}

#[test]
fn fills_strokes_effects_and_styles_survive_save_and_reopen() {
    let mut s = Session::new();
    let r = node(&ok(&mut s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 100, "height": 100})));
    let v = node(&ok(&mut s, "shape.rectangle", json!({"x": 200, "y": 0, "width": 100, "height": 100})));
    ok(&mut s, "fill.set", json!({"ids": [r], "color": "#FF0000"}));
    ok(&mut s, "stroke.add", json!({"ids": [r], "color": "#0000FF"}));
    ok(&mut s, "effect.add", json!({"ids": [r], "type": "dropShadow"}));
    ok(&mut s, "style.add", json!({"type": "paint", "name": "Brand/Blue", "value": "#0D6EFD"}));
    ok(&mut s, "style.apply", json!({"ids": [v], "style": "Brand/Blue"}));
    let rect = inspect(&s, r).to_string();
    assert!(rect.contains("dropShadow") || rect.contains("DROP_SHADOW"), "{rect}");
    assert!(inspect(&s, r).to_string().contains("#0000FF"), "the stroke");
    let styled = inspect(&s, v);
    assert_eq!((styled["styles"]["fill"].is_string(), styled["fills"][0]["color"].clone()), (true, json!("#0D6EFD")), "{styled}");
    survives_save_and_reopen(&mut s, "paints-and-styles");
}

#[test]
fn vectors_pages_and_guides_survive_save_and_reopen() {
    let mut s = Session::new();
    let f = node(&ok(&mut s, "frame.add", json!({"x": 0, "y": 0, "width": 400, "height": 400})));
    ok(&mut s, "vector.add", json!({"path": "M0 0 C50 -50 150 -50 200 0", "x": 40, "y": 100}));
    ok(&mut s, "guide.add", json!({"frame": f, "vertical": true, "offset": 40}));
    ok(&mut s, "page.add", json!({"name": "Two"}));
    ok(&mut s, "shape.ellipse", json!({"x": 10, "y": 10, "width": 20, "height": 20}));
    assert_eq!(s.doc().unwrap().pages.len(), 2);
    survives_save_and_reopen(&mut s, "vectors-pages");
}

#[test]
fn variables_survive_save_and_reopen() {
    let mut s = Session::new();
    let r = node(&ok(&mut s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 100, "height": 100})));
    ok(&mut s, "variable.addCollection", json!({"name": "Tokens"}));
    ok(&mut s, "variable.add", json!({"collection": "Tokens", "name": "radius/sm", "type": "number", "value": 4}));
    ok(&mut s, "variable.bind", json!({"ids": [r], "field": "cornerRadius", "variable": "radius/sm"}));
    let rect = inspect(&s, r);
    assert_eq!((rect["boundVariables"]["cornerRadius"].is_string(), rect["cornerRadius"].as_f64()), (true, Some(4.0)), "{rect}");
    survives_save_and_reopen(&mut s, "variables");
}
