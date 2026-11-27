//! Stage 3 of making the design usable by hand (R2): the engine's answers carry what the editor
//! used to ask for after each one, and only when it changed: the hovered layer (`canvas.hover`),
//! and during a gesture the chrome and the selection's position and size (`canvas.chrome`,
//! `canvas.gesture`), once per change; `inspect().gesture` says a gesture runs.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;
use kition_kit::tools::{KitTools, PointerAction, ToolMods, ToolPointer, ToolRequest, ToolResponse};
use serde_json::{Value, json};

/// A 400 × 300 design with a 120 × 80 rectangle at (20, 20) in its frame; the frame's top-left
/// is document pixel (1000, 1000).
fn design() -> DesignDocument {
    let mut doc = DesignDocument::create("Design.kdesign", 400, 300).unwrap();
    let rows = doc.query("layers", &json!({})).unwrap();
    let frame = rows["rows"].as_array().unwrap().iter().find(|r| r["type"] == "FRAME").unwrap()["id"].clone();
    doc.execute("shape.rectangle", json!({ "x": 20, "y": 20, "width": 120, "height": 80, "parent": frame })).unwrap();
    doc.execute("edit.deselectAll", json!({})).unwrap();
    doc.mark_saved();
    doc
}

fn event(action: PointerAction, x: f64, y: f64) -> ToolPointer {
    ToolPointer { action, x, y, mods: ToolMods::default(), pressure: 1.0, zoom: 1.0 }
}

/// The answer's `canvas.*` requests, by name.
fn live(response: &ToolResponse) -> Vec<(String, Value)> {
    response
        .requests
        .iter()
        .filter_map(|r| match r {
            ToolRequest::Dialog { name, params } if name.starts_with("canvas.") && name != "canvas.renameLabel" => {
                Some((name.clone(), params.clone()))
            }
            _ => None,
        })
        .collect()
}

fn names(requests: &[(String, Value)]) -> Vec<&str> {
    requests.iter().map(|(name, _)| name.as_str()).collect()
}

#[test]
fn a_hover_tells_the_hovered_layer_once_with_the_chrome() {
    let mut doc = design();
    let first = live(&doc.pointer(&event(PointerAction::Move, 1080.0, 1060.0)).unwrap());
    let hovered = doc.inspect().unwrap()["hover"].clone();
    assert!(hovered.is_u64(), "the rectangle is hovered: {hovered}");
    assert_eq!(names(&first), ["canvas.hover", "canvas.chrome"], "{first:?}");
    assert_eq!(first[0].1, json!({ "id": hovered }));
    assert!(first[1].1["labels"].is_array(), "the chrome as `chrome(zoom)` answers it: {}", first[1].1);
    // The same layer under the pointer again: nothing for the editor to update.
    for x in [1081.0, 1090.0, 1100.0] {
        let again = live(&doc.pointer(&event(PointerAction::Move, x, 1061.0)).unwrap());
        assert!(again.is_empty(), "{again:?}");
    }
    assert!(live(&doc.state(1100.0, 1061.0, ToolMods::default(), 1.0)).is_empty());
    // Off the rectangle and the frame: the hover goes, once.
    let gone = live(&doc.pointer(&event(PointerAction::Move, 1600.0, 1600.0)).unwrap());
    assert_eq!(names(&gone), ["canvas.hover", "canvas.chrome"], "{gone:?}");
    assert_eq!(gone[0].1, json!({ "id": null }));
    assert!(live(&doc.pointer(&event(PointerAction::Move, 1610.0, 1600.0)).unwrap()).is_empty());
}

#[test]
fn a_drag_carries_the_chrome_and_the_selection_geometry_once_per_change() {
    let mut doc = design();
    doc.pointer(&event(PointerAction::Move, 1080.0, 1060.0)).unwrap();
    assert_eq!(doc.inspect().unwrap()["gesture"], json!(false));
    doc.pointer(&event(PointerAction::Down, 1080.0, 1060.0)).unwrap();
    assert_eq!(doc.inspect().unwrap()["gesture"], json!(true), "a press on a layer starts a gesture");
    let id = doc.inspect().unwrap()["selection"][0].clone();
    let mut last_x = 20.0;
    for step in 1..=4 {
        let at = f64::from(step) * 7.0;
        let answer = live(&doc.pointer(&event(PointerAction::Drag, 1080.0 + at, 1060.0 + at / 2.0)).unwrap());
        assert_eq!(names(&answer), ["canvas.gesture", "canvas.chrome"], "step {step}: {answer:?}");
        let nodes = answer[0].1["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 1);
        let shown = &doc.query("selection", &json!({})).unwrap()["nodes"][0];
        for key in ["x", "y", "width", "height"] {
            assert_eq!(nodes[0][key], shown[key], "step {step}: {key} is the Design panel's");
        }
        assert_eq!(nodes[0]["id"], id);
        assert_eq!(nodes[0]["rotation"], json!(0.0));
        let x = nodes[0]["x"].as_f64().unwrap();
        assert!(x > last_x, "step {step}: the layer moved right ({last_x} to {x})");
        last_x = x;
        // The editor asking for the tool's state again in the same revision gets none of it again.
        assert!(live(&doc.state(1080.0 + at, 1060.0, ToolMods::default(), 1.0)).is_empty());
    }
    let up = live(&doc.pointer(&event(PointerAction::Up, 1108.0, 1074.0)).unwrap());
    assert!(!names(&up).contains(&"canvas.gesture"), "{up:?}");
    assert_eq!(doc.inspect().unwrap()["gesture"], json!(false), "the release ends it");
    // A nudge is no gesture: its answer carries none of it.
    let selected = json!({ "ids": [id] });
    doc.execute("select.set", selected).unwrap();
    doc.state(0.0, 0.0, ToolMods::default(), 1.0);
    let nudged = doc.key("ArrowRight", ToolMods::default(), 1.0).unwrap();
    assert!(live(&nudged).is_empty(), "{:?}", nudged.requests);
}
