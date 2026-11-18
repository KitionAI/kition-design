//! Regressions for cases regressions/issue-1, issue-2 and issue-3 (answers): variables (a `fill`
//! binding is a binding for the check, `node.get` shows a layer's variable modes, `variable.list`
//! answers colours as hex), key styles (answers use the input's camelCase keys, no float noise),
//! and the layer tree (inspect has one; `node.get` always counts children).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;
use serde_json::{Value, json};

fn run(doc: &mut DesignDocument, command: &str, params: Value) -> Value {
    doc.execute(command, params).unwrap_or_else(|error| panic!("{command}: {error}"))
}

fn page(doc: &DesignDocument) -> Value {
    doc.inspect().unwrap()["page"].clone()
}

fn frame(doc: &mut DesignDocument, name: &str) -> Value {
    let parent = page(doc);
    run(doc, "frame.create", json!({ "x": 0, "y": 0, "width": 200, "height": 100, "parent": parent, "name": name }))["id"].clone()
}

#[test]
fn issue_1_variables() {
    let mut doc = DesignDocument::create("theme.fig", 800, 600).unwrap();
    run(&mut doc, "variable.createCollection", json!({ "name": "Theme", "modes": ["Light", "Dark"] }));
    let surface = run(
        &mut doc,
        "variable.create",
        json!({ "collection": "Theme", "name": "surface", "type": "color", "values": { "Light": "#FFFFFF", "Dark": "#162032" } }),
    )["id"]
        .clone();
    let card = frame(&mut doc, "Card");
    run(&mut doc, "node.set", json!({ "ids": [card], "props": { "fills": ["#FFFFFF"] } }));
    let unbound = |doc: &DesignDocument| doc.check().into_iter().filter(|d| d.code == "design.unbound-color").count();
    let before = unbound(&doc);
    assert!(before >= 1, "the white fill equals surface");
    // `fill` is the documented alias of the first fill's colour: bound, nothing to report for it.
    run(&mut doc, "variable.bind", json!({ "ids": [card], "field": "fill", "variable": surface }));
    assert_eq!(unbound(&doc), before - 1, "{:?}", doc.check());
    // A layer's mode shows in node.get.
    run(&mut doc, "variable.setMode", json!({ "ids": [card], "collection": "Theme", "mode": "Dark" }));
    let node = run(&mut doc, "node.get", json!({ "id": card }));
    assert_eq!(node["variableModes"], json!({ "Theme": "Dark" }), "{node}");
    // Colours made from hex read back with their hex (the editor keeps the rgba value).
    let list = run(&mut doc, "variable.list", json!({}));
    let variable = list["variables"].as_array().unwrap().iter().find(|v| v["name"] == "surface").cloned().unwrap();
    let hexes: Vec<String> = variable["values"].as_object().unwrap().values().map(|v| v["hex"].as_str().unwrap_or_default().to_owned()).collect();
    assert!(hexes.contains(&"#162032".to_owned()) && hexes.contains(&"#FFFFFF".to_owned()), "{variable}");
}

#[test]
fn issue_2_answers_use_the_inputs_keys() {
    let mut doc = DesignDocument::create("proto.fig", 800, 600).unwrap();
    let a = frame(&mut doc, "Start");
    let b = frame(&mut doc, "Next");
    run(
        &mut doc,
        "proto.interaction.add",
        json!({ "id": a, "trigger": { "type": "onClick" }, "action": { "type": "navigate", "dest": b }, "transition": { "type": "dissolve" }, "durationMs": 300 }),
    );
    let node = run(&mut doc, "node.get", json!({ "id": a })).to_string();
    assert!(node.contains("durationMs") && !node.contains("duration_ms"), "{node}");
    let text = run(
        &mut doc,
        "text.create",
        json!({ "text": "Title", "x": 10, "y": 10, "parent": a, "props": { "fontFamily": "Inter", "fontSize": 24 } }),
    )["id"]
        .clone();
    run(&mut doc, "style.create", json!({ "type": "text", "name": "Title", "from": text }));
    let styles = run(&mut doc, "style.list", json!({})).to_string();
    assert!(styles.contains("fontFamily") && !styles.contains("font_family") && !styles.contains("font_size"), "{styles}");
    let parent = page(&doc);
    let ring = run(
        &mut doc,
        "node.create",
        json!({ "type": "ellipse", "parent": parent, "x": 0, "y": 0, "width": 100, "height": 100, "props": { "arcStart": -90, "arcSweep": 250 } }),
    )["id"]
        .clone();
    let arc = run(&mut doc, "node.get", json!({ "id": ring }))["arc"].clone();
    assert_eq!(arc["sweep"], json!(250.0), "{arc}");
    assert_eq!(arc["start"], json!(-90.0), "{arc}");
}

#[test]
fn issue_3_layer_tree() {
    let mut doc = DesignDocument::create("tree.fig", 800, 600).unwrap();
    let screen = frame(&mut doc, "Screen");
    let row = run(&mut doc, "frame.create", json!({ "x": 0, "y": 0, "width": 100, "height": 40, "parent": screen, "name": "Row" }))["id"].clone();
    run(&mut doc, "text.create", json!({ "text": "Hi", "x": 0, "y": 0, "parent": row }));
    let inspected = doc.inspect().unwrap();
    let tree = inspected["tree"].as_array().unwrap_or_else(|| panic!("inspect has a layer tree: {inspected}"));
    let top = tree.iter().find(|n| n["id"] == screen).unwrap();
    assert_eq!(top["name"], json!("Screen"));
    assert_eq!(top["children"][0]["name"], json!("Row"), "{top}");
    assert_eq!(top["children"][0]["children"][0]["type"], json!("TEXT"), "{top}");
    // node.get: the node's children, and every node's childCount.
    let got = run(&mut doc, "node.get", json!({ "id": screen }));
    assert_eq!(got["childCount"], json!(1), "{got}");
    assert_eq!(got["children"][0]["childCount"], json!(1), "{got}");
    assert_eq!(got["children"][0]["name"], json!("Row"));
}
