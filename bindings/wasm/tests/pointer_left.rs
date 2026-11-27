//! S3 of making the design usable by hand (regressions/issue-hand-a16-pointer-left): when the
//! pointer leaves the canvas with no button held (kit-editor calls `pointerLeft`), what hovering
//! showed goes: the hovered layer's outline and the distances Alt measured. A hover a Layers row
//! set since stays, and so does all of it while a gesture runs.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;
use kition_kit::tools::{KitTools, PointerAction, ToolMods, ToolOverlay, ToolPointer, ToolRequest, ToolResponse};
use serde_json::{Value, json};

/// A 400 × 300 design with two 100 × 80 rectangles at (20, 20) and (220, 20) in its frame; the
/// frame's top-left is document pixel (1000, 1000). The first one is selected.
fn design() -> (DesignDocument, Value, Value) {
    let mut doc = DesignDocument::create("Design.kdesign", 400, 300).unwrap();
    let rows = doc.query("layers", &json!({})).unwrap();
    let frame = rows["rows"].as_array().unwrap().iter().find(|r| r["type"] == "FRAME").unwrap()["id"].clone();
    doc.execute("shape.rectangle", json!({ "x": 20, "y": 20, "width": 100, "height": 80, "parent": frame })).unwrap();
    let a = doc.inspect().unwrap()["selection"][0].clone();
    doc.execute("shape.rectangle", json!({ "x": 220, "y": 20, "width": 100, "height": 80, "parent": frame })).unwrap();
    let b = doc.inspect().unwrap()["selection"][0].clone();
    doc.execute("select.set", json!({ "ids": [a.clone()] })).unwrap();
    doc.mark_saved();
    (doc, a, b)
}

fn event(action: PointerAction, x: f64, y: f64, alt: bool) -> ToolPointer {
    ToolPointer { action, x, y, mods: ToolMods { alt, ..ToolMods::default() }, pressure: 1.0, zoom: 1.0 }
}

/// The measurements Alt draws (the numbers on its red lines), as the answer's overlays give them.
fn measured(response: &ToolResponse) -> usize {
    response.overlays.iter().filter(|o| matches!(o, ToolOverlay::Measure { .. })).count()
}

fn hover_told(response: &ToolResponse) -> Option<Value> {
    response.requests.iter().find_map(|r| match r {
        ToolRequest::Dialog { name, params } if name == "canvas.hover" => Some(params["id"].clone()),
        _ => None,
    })
}

#[test]
fn leaving_the_canvas_takes_the_hover_and_the_alt_measurement_away() {
    let (mut doc, _a, b) = design();
    // Alt over the other rectangle: it is hovered and measured to.
    let over = doc.pointer(&event(PointerAction::Move, 1270.0, 1060.0, true)).unwrap();
    assert_eq!(doc.inspect().unwrap()["hover"], b, "the other rectangle is hovered");
    assert!(measured(&over) > 0, "Alt measures to it: {:?}", over.overlays);
    let left = doc.pointer_left(1.0);
    assert_eq!(doc.inspect().unwrap()["hover"], Value::Null, "the hover goes");
    assert_eq!(hover_told(&left), Some(Value::Null), "and the editor is told: {:?}", left.requests);
    assert_eq!(measured(&left), 0, "the measurement goes: {:?}", left.overlays);
    // Leaving again changes nothing more.
    let again = doc.pointer_left(1.0);
    assert_eq!(hover_told(&again), None);
}

#[test]
fn a_hover_a_layers_row_set_since_stays() {
    let (mut doc, a, b) = design();
    doc.pointer(&event(PointerAction::Move, 1270.0, 1060.0, false)).unwrap();
    assert_eq!(doc.inspect().unwrap()["hover"], b);
    // The pointer crosses onto a Layers row, which outlines the first rectangle before the
    // canvas's leave arrives.
    doc.set_hover(Some(&a.to_string())).unwrap();
    doc.pointer_left(1.0);
    assert_eq!(doc.inspect().unwrap()["hover"], a, "the row's hover stays");
}

#[test]
fn a_gesture_keeps_what_it_shows() {
    let (mut doc, _a, _b) = design();
    doc.pointer(&event(PointerAction::Move, 1070.0, 1060.0, false)).unwrap();
    doc.pointer(&event(PointerAction::Down, 1070.0, 1060.0, false)).unwrap();
    doc.pointer(&event(PointerAction::Drag, 1090.0, 1070.0, false)).unwrap();
    let hover = doc.inspect().unwrap()["hover"].clone();
    let left = doc.pointer_left(1.0);
    assert!(left.busy, "the drag still runs");
    assert_eq!(doc.inspect().unwrap()["hover"], hover);
    doc.pointer(&event(PointerAction::Up, 1090.0, 1070.0, false)).unwrap();
}
