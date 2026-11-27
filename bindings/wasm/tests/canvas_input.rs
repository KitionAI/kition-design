//! Stage 8 of making the design usable by hand (design.editor.canvas-input): what the editor's
//! canvas input asks of the binding. A drag tells the editor when it pans at the canvas's edge
//! (`drag.autopan`): moving layers, a selection box, drawing and resizing do, rotating does not.
//! `guidePlace` says where a ruler guide dragged to a point lands (the frame under it or the page,
//! snapped to a layer's edge in reach), and a selected guide takes Delete and the arrows.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;
use kition_kit::tools::{KitTools, PointerAction, ToolMods, ToolPointer, ToolRequest, ToolResponse};
use serde_json::{Value, json};

/// A 400 × 300 design with a 120 × 80 rectangle at (20, 20) in its frame; the frame's top-left
/// is document pixel (1000, 1000), page point (0, 0).
fn design() -> (DesignDocument, Value) {
    let mut doc = DesignDocument::create("Design.kdesign", 400, 300).unwrap();
    let rows = doc.query("layers", &json!({})).unwrap();
    let frame = rows["rows"].as_array().unwrap().iter().find(|r| r["type"] == "FRAME").unwrap()["id"].clone();
    doc.execute("shape.rectangle", json!({ "x": 20, "y": 20, "width": 120, "height": 80, "parent": frame })).unwrap();
    doc.execute("edit.deselectAll", json!({})).unwrap();
    doc.mark_saved();
    (doc, frame)
}

fn event(action: PointerAction, x: f64, y: f64) -> ToolPointer {
    ToolPointer { action, x, y, mods: ToolMods::default(), pressure: 1.0, zoom: 1.0 }
}

/// The `drag.autopan` the answer tells, if it tells one.
fn autopan(response: &ToolResponse) -> Option<bool> {
    response.requests.iter().find_map(|r| match r {
        ToolRequest::Dialog { name, params } if name == "drag.autopan" => params["on"].as_bool(),
        _ => None,
    })
}

#[test]
fn a_drag_says_when_it_pans_at_the_edge() {
    let (mut doc, _) = design();
    // Moving the rectangle: not on the press, on once the drag leaves it, off on the release.
    assert_eq!(autopan(&doc.pointer(&event(PointerAction::Down, 1080.0, 1060.0)).unwrap()), None);
    assert_eq!(autopan(&doc.pointer(&event(PointerAction::Drag, 1081.0, 1060.0)).unwrap()), None, "within the drag slop");
    assert_eq!(autopan(&doc.pointer(&event(PointerAction::Drag, 1100.0, 1070.0)).unwrap()), Some(true));
    assert_eq!(autopan(&doc.pointer(&event(PointerAction::Drag, 1110.0, 1075.0)).unwrap()), None, "told once");
    assert_eq!(autopan(&doc.pointer(&event(PointerAction::Up, 1110.0, 1075.0)).unwrap()), Some(false));
    // A selection box from the empty canvas: once it is bigger than the slop.
    doc.pointer(&event(PointerAction::Down, 1600.0, 1600.0)).unwrap();
    assert_eq!(autopan(&doc.pointer(&event(PointerAction::Drag, 1601.0, 1601.0)).unwrap()), None);
    assert_eq!(autopan(&doc.pointer(&event(PointerAction::Drag, 1650.0, 1650.0)).unwrap()), Some(true));
    assert_eq!(autopan(&doc.pointer(&event(PointerAction::Up, 1650.0, 1650.0)).unwrap()), Some(false));
    // Drawing a rectangle: once it exists.
    doc.select_tool("rectangle", 1.0).unwrap();
    doc.pointer(&event(PointerAction::Down, 1700.0, 1700.0)).unwrap();
    assert_eq!(autopan(&doc.pointer(&event(PointerAction::Drag, 1760.0, 1750.0)).unwrap()), Some(true));
    assert_eq!(autopan(&doc.pointer(&event(PointerAction::Up, 1760.0, 1750.0)).unwrap()), Some(false));
}

#[test]
fn rotating_does_not_pan_at_the_edge() {
    let (mut doc, _) = design();
    let id = {
        doc.pointer(&event(PointerAction::Down, 1080.0, 1060.0)).unwrap();
        doc.pointer(&event(PointerAction::Up, 1080.0, 1060.0)).unwrap();
        doc.inspect().unwrap()["selection"][0].clone()
    };
    assert!(id.is_u64());
    // Just outside the bottom-right corner (140, 100) is the rotation zone.
    doc.state(1146.0, 1106.0, ToolMods::default(), 1.0);
    doc.pointer(&event(PointerAction::Move, 1146.0, 1106.0)).unwrap();
    doc.pointer(&event(PointerAction::Down, 1146.0, 1106.0)).unwrap();
    let mut told = Vec::new();
    for step in 1..=5 {
        let at = f64::from(step) * 8.0;
        told.extend(autopan(&doc.pointer(&event(PointerAction::Drag, 1146.0 + at, 1106.0 - at)).unwrap()));
    }
    doc.pointer(&event(PointerAction::Up, 1186.0, 1066.0)).unwrap();
    let rotation = doc.query("node", &json!({ "id": id })).unwrap()["rotation"].as_f64().unwrap_or(0.0);
    assert!(rotation.abs() > 1.0, "the drag rotated the rectangle ({rotation})");
    assert!(told.iter().all(|on| !on), "a rotation never pans: {told:?}");
}

#[test]
fn a_guide_out_of_a_ruler_lands_on_the_frame_under_it_and_snaps_to_an_edge() {
    let (doc, frame) = design();
    // Over the frame, 2 px right of the rectangle's left edge (page x 20): on its edge with
    // snapping, where the pointer is without.
    let snapped = doc.query("guidePlace", &json!({ "x": 1022.4, "y": 1150.0, "vertical": true, "snap": true, "zoom": 1.0 })).unwrap();
    assert_eq!(snapped["owner"], frame, "{snapped}");
    assert_eq!(snapped["page"], json!(false));
    assert_eq!(snapped["vertical"], json!(true));
    assert_eq!(snapped["offset"], json!(20.0));
    assert_eq!(snapped["snapped"], json!(true));
    assert_eq!(snapped["a"], json!([1020.0, 1000.0]), "a frame guide spans its frame: {snapped}");
    assert_eq!(snapped["b"], json!([1020.0, 1300.0]));
    let free = doc.query("guidePlace", &json!({ "x": 1022.4, "y": 1150.0, "vertical": true, "zoom": 1.0 })).unwrap();
    assert_eq!(free["offset"], json!(22.0), "whole pixels: {free}");
    assert_eq!(free["snapped"], json!(false));
    // Far out of reach at 100 %, in reach zoomed out to 25 % (5 screen px = 20 document px).
    let far = json!({ "x": 1035.0, "y": 1150.0, "vertical": true, "snap": true, "zoom": 1.0 });
    assert_eq!(doc.query("guidePlace", &far).unwrap()["offset"], json!(35.0));
    let mut out = far.clone();
    out["zoom"] = json!(0.25);
    assert_eq!(doc.query("guidePlace", &out).unwrap()["offset"], json!(20.0));
    // Off the frame: a page guide, across the whole canvas.
    let page = doc.query("guidePlace", &json!({ "x": 1600.0, "y": 1700.0, "vertical": false })).unwrap();
    assert_eq!(page["page"], json!(true), "{page}");
    assert_eq!(page["offset"], json!(700.0));
    assert_eq!(page["a"], Value::Null);
    assert!(doc.query("guidePlace", &json!({ "y": 1 })).is_err());
    assert!(doc.query("guidePlace", &json!({ "x": 1, "y": 1, "owner": 999_999 })).is_err());
}

#[test]
fn a_selected_guide_takes_delete_and_the_arrows() {
    let (mut doc, frame) = design();
    doc.execute("guide.add", json!({ "vertical": true, "offset": 50, "frame": frame })).unwrap();
    assert!(!doc.state(0.0, 0.0, ToolMods::default(), 1.0).busy, "nothing selected");
    doc.execute("guide.select", json!({ "index": 0, "frame": frame })).unwrap();
    assert!(doc.state(0.0, 0.0, ToolMods::default(), 1.0).busy, "a selected guide takes the editing keys");
    let guides = |doc: &mut DesignDocument| doc.chrome(1.0)["guides"].as_array().cloned().unwrap_or_default();
    doc.key("ArrowRight", ToolMods { shift: true, ..ToolMods::default() }, 1.0).unwrap();
    assert_eq!(guides(&mut doc)[0]["offset"], json!(60.0), "Shift+Right nudges it by the big step");
    doc.key("Delete", ToolMods::default(), 1.0).unwrap();
    assert!(guides(&mut doc).is_empty(), "Delete removes it");
    assert!(!doc.state(0.0, 0.0, ToolMods::default(), 1.0).busy);
}

#[test]
fn moving_a_frame_guide_undoes_to_where_it_was() {
    let (mut doc, frame) = design();
    let list = |doc: &mut DesignDocument| -> Vec<f64> {
        doc.chrome(1.0)["guides"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|g| g["page"] == json!(false))
            .map(|g| g["offset"].as_f64().unwrap())
            .collect()
    };
    doc.execute("guide.add", json!({ "vertical": true, "offset": 70, "frame": frame })).unwrap();
    doc.execute("guide.move", json!({ "index": 0, "offset": 90, "frame": frame })).unwrap();
    assert_eq!(list(&mut doc), [90.0]);
    doc.execute("edit.undo", json!({})).unwrap();
    assert_eq!(list(&mut doc), [70.0], "the move's undo puts it back");
    doc.execute("edit.undo", json!({})).unwrap();
    assert!(list(&mut doc).is_empty(), "the add's undo removes it");
}
