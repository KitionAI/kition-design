//! What the kit's editor relies on: the command catalog, the engine's tools through pointer and
//! key events, overlays, the panels' queries and the canvas chrome.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeSet;

use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;
use kition_kit::tools::{KitTools, PointerAction, ToolMods, ToolOverlay, ToolPointer};
use serde_json::{Value, json};

/// A 400 × 300 design with a rectangle in its frame; the frame is at canvas (0, 0), which is
/// document pixel (1000, 1000).
fn design() -> DesignDocument {
    let mut doc = DesignDocument::create("Design.kdesign", 400, 300).unwrap();
    let frame = frame_id(&doc);
    doc.execute("shape.rectangle", json!({ "x": 20, "y": 20, "width": 120, "height": 80, "parent": frame })).unwrap();
    doc.execute("select.none", json!({})).unwrap();
    doc.mark_saved();
    doc
}

fn layers(doc: &DesignDocument) -> u64 {
    doc.inspect().unwrap()["layers"].as_u64().unwrap()
}

fn event(action: PointerAction, x: f64, y: f64) -> ToolPointer {
    ToolPointer { action, x, y, mods: ToolMods::default(), pressure: 1.0, zoom: 1.0 }
}

fn frame_id(doc: &DesignDocument) -> Value {
    let rows = doc.query("layers", &json!({})).unwrap();
    rows["rows"].as_array().unwrap().iter().find(|r| r["type"] == "FRAME").unwrap()["id"].clone()
}

#[test]
fn the_catalog_has_undo_redo_unique_ids_menus_and_mutates() {
    let doc = design();
    let commands = doc.commands();
    assert!(commands.len() > 300, "{} commands", commands.len());
    let ids: BTreeSet<&str> = commands.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids.len(), commands.len(), "ids are unique");
    let find = |id: &str| commands.iter().find(|c| c.id == id).unwrap_or_else(|| panic!("{id} is a command"));
    assert_eq!(find("edit.undo").shortcut.as_deref(), Some("Cmd+Z"));
    assert_eq!(find("edit.redo").shortcut.as_deref(), Some("Cmd+Shift+Z"));
    assert_eq!(find("edit.undo").menu, Some(vec!["Edit".to_owned()]));
    assert_eq!(find("text.alignLeft").menu, Some(vec!["Text".to_owned(), "Alignment".to_owned()]));
    assert_eq!(find("layout.add").shortcut.as_deref(), Some("Shift+A"));
    assert_eq!(find("layout.add").title, "Add auto layout");
    assert_eq!(find("component.create").shortcut.as_deref(), Some("Cmd+Alt+K"));
    assert_eq!(find("object.group").shortcut.as_deref(), Some("Cmd+G"));
    assert_eq!(find("object.removeFill").shortcut.as_deref(), Some("Alt+/"));
    assert!(find("edit.undo").enabled, "the rectangle can be undone");
    assert!(!find("edit.redo").enabled, "nothing to redo");
    for id in [
        "shape.rectangle",
        "frame.create",
        "text.create",
        "layout.add",
        "component.create",
        "object.group",
        "edit.undo",
        "edit.redo",
        "edit.cut",
        "command.batch",
        "node.set",
    ] {
        assert!(find(id).mutates, "{id} changes the design");
    }
    for id in [
        "select.all",
        "select.none",
        "select.set",
        "edit.copy",
        "view.outlines",
        "view.rulers",
        "tool.rectangle",
        "file.export",
        "inspect.properties",
        "pref.keepTool",
        "page.select",
    ] {
        assert!(!find(id).mutates, "{id} does not change the design");
    }
    for id in ["file.open", "file.save", "file.close", "doc.nextTab", "window.new", "collab.join", "help.about", "view.zoomIn", "app.actions"] {
        assert!(!ids.contains(id), "{id} belongs to the upstream app's shell");
    }
    assert!(find("tool.rectangle").shortcut.is_none(), "tool keys are the tool rail's");
    assert!(find("shape.rectangle").schema.is_some() || find("shape.rectangle").params.is_some());
    let menus: BTreeSet<&str> = commands.iter().filter_map(|c| c.menu.as_ref()).filter_map(|m| m.first()).map(String::as_str).collect();
    let manifest: Value = serde_json::from_str(include_str!("../../../kit.json")).unwrap();
    let declared: BTreeSet<&str> = manifest["ui"]["menus"].as_array().unwrap().iter().map(|m| m.as_str().unwrap()).collect();
    assert_eq!(menus, declared, "kit.json names the menus the commands fill");
}

#[test]
fn the_tools_are_the_upstream_toolbar() {
    let doc = design();
    let tools = doc.tools();
    let ids: Vec<&str> = tools.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "move",
            "hand",
            "scale",
            "frame",
            "section",
            "slice",
            "rectangle",
            "line",
            "arrow",
            "ellipse",
            "polygon",
            "star",
            "pen",
            "pencil",
            "shapeBuilder",
            "text",
            "textOnPath",
            "comment"
        ]
    );
    let rect = tools.iter().find(|t| t.id == "rectangle").unwrap();
    assert_eq!((rect.label.as_str(), rect.shortcut.as_deref(), rect.group), ("Rectangle", Some("R"), 2));
    assert_eq!(tools.iter().find(|t| t.id == "section").unwrap().shortcut.as_deref(), Some("Shift+S"));
    assert_eq!(doc.tool(), "move");
}

#[test]
fn the_rectangle_tool_draws_through_pointer_events_and_undo_removes_it() {
    let mut doc = design();
    let before = layers(&doc);
    doc.select_tool("rectangle", 1.0).unwrap();
    assert_eq!(doc.tool(), "rectangle");
    let revision = doc.revision();
    doc.pointer(&event(PointerAction::Down, 1400.0, 1300.0)).unwrap();
    let dragging = doc.pointer(&event(PointerAction::Drag, 1460.0, 1350.0)).unwrap();
    assert!(dragging.busy, "mid-drag");
    assert!(dragging.overlays.iter().any(|o| matches!(o, ToolOverlay::Path { .. })), "the shape being drawn shows: {:?}", dragging.overlays);
    let done = doc.pointer(&event(PointerAction::Up, 1460.0, 1350.0)).unwrap();
    assert!(doc.revision() > revision);
    assert_eq!(layers(&doc), before + 1);
    assert!(
        done.requests.iter().any(|r| matches!(r, kition_kit::tools::ToolRequest::SwitchTool { id } if id == "move")),
        "back to Move: {:?}",
        done.requests
    );
    let selected = doc.inspect().unwrap()["selectionBounds"].clone();
    assert_eq!(selected, json!({ "x": 1400.0, "y": 1300.0, "width": 60.0, "height": 50.0 }), "drawn where the pointer went, in document pixels");
    doc.execute("edit.undo", json!({})).unwrap();
    assert_eq!(layers(&doc), before);
    assert!(doc.select_tool("no-such-tool", 1.0).is_err());
}

#[test]
fn the_selection_shows_outlines_handles_and_hover() {
    let mut doc = design();
    let frame = frame_id(&doc);
    doc.execute("select.set", json!({ "ids": [frame] })).unwrap();
    let shown = doc.state(0.0, 0.0, ToolMods::default(), 1.0);
    let anchors: Vec<(f64, f64)> =
        shown.overlays.iter().filter_map(|o| if let ToolOverlay::Anchor { x, y, .. } = o { Some((*x, *y)) } else { None }).collect();
    assert_eq!(anchors, [(1000.0, 1000.0), (1400.0, 1000.0), (1400.0, 1300.0), (1000.0, 1300.0)], "four corner handles in document pixels");
    assert!(shown.overlays.iter().any(|o| matches!(o, ToolOverlay::Path { color, .. } if color[..3] == [0x00, 0x87, 0x5F])), "the green outline");
    assert!(shown.busy, "a selection takes Delete, arrows and Escape");
    // Arrow keys nudge the selection; Escape clears it.
    let x0 = doc.inspect().unwrap()["selectionBounds"]["x"].as_f64().unwrap();
    let nudged = doc.key("ArrowRight", ToolMods { shift: true, ..ToolMods::default() }, 1.0).unwrap();
    assert!(nudged.requests.is_empty(), "{:?}", nudged.requests);
    assert_eq!(doc.inspect().unwrap()["selectionBounds"]["x"].as_f64().unwrap(), x0 + 10.0);
    doc.key("Escape", ToolMods::default(), 1.0).unwrap();
    assert_eq!(doc.inspect().unwrap()["selection"], json!([]));
    // Hovering the rectangle outlines it.
    let hovering = doc.pointer(&event(PointerAction::Move, 1080.0, 1060.0)).unwrap();
    assert_eq!(hovering.overlays.iter().filter(|o| matches!(o, ToolOverlay::Path { .. })).count(), 1, "{:?}", hovering.overlays);
}

#[test]
fn the_text_tool_types_through_text_events() {
    let mut doc = design();
    doc.select_tool("text", 1.0).unwrap();
    doc.pointer(&event(PointerAction::Down, 1700.0, 1150.0)).unwrap();
    let up = doc.pointer(&event(PointerAction::Up, 1700.0, 1150.0)).unwrap();
    assert!(up.wants_text, "a click with the Text tool starts a text");
    let typed = KitTools::text(&mut doc, "Hello", 1.0).unwrap();
    assert!(typed.overlays.iter().any(|o| matches!(o, ToolOverlay::Line { .. })), "the caret");
    doc.key("Backspace", ToolMods::default(), 1.0).unwrap();
    let editing = doc.inspect().unwrap()["textEdit"]["id"].clone();
    let text = doc.query("node", &json!({ "id": editing })).unwrap();
    assert_eq!(text["characters"], "Hell", "{text}");
    let done = doc.key("Escape", ToolMods::default(), 1.0).unwrap();
    assert!(!done.wants_text);
    let rows = doc.query("layers", &json!({})).unwrap();
    assert!(rows.to_string().contains("\"TEXT\""), "{rows}");
}

#[test]
fn queries_answer_the_panels() {
    let mut doc = design();
    let pages = doc.query("pages", &json!({})).unwrap();
    assert_eq!(pages["pages"].as_array().unwrap().len(), 1);
    assert_eq!(pages["pages"][0]["current"], true);
    let layers = doc.query("layers", &json!({})).unwrap();
    let frame = &layers["rows"][0];
    assert_eq!((frame["type"].as_str(), frame["icon"].as_str(), frame["visible"].as_bool()), (Some("FRAME"), Some("frame"), Some(true)));
    assert_eq!(frame["expanded"], false, "collapsed until expanded");
    let id = frame["id"].clone();
    let open = doc.query("layers", &json!({ "expanded": [id] })).unwrap();
    assert_eq!(open["rows"][0]["children"][0]["type"], "RECTANGLE");
    // The Design panel: shared values and mixed ones.
    let nothing = doc.query("selection", &json!({})).unwrap();
    assert_eq!(nothing["page"]["background"], "#F5F5F5");
    doc.execute("select.all", json!({})).unwrap();
    doc.execute("select.set", json!({ "ids": [id] })).unwrap();
    doc.execute("layout.add", json!({ "direction": "vertical" })).unwrap();
    let one = doc.query("selection", &json!({})).unwrap();
    assert_eq!(one["kind"], "Frame");
    assert!(one["common"]["width"].as_f64().is_some(), "{one}");
    assert_eq!(one["nodes"][0]["layout"]["mode"], "vertical");
    let rows = doc.query("layers", &json!({})).unwrap();
    assert_eq!(rows["rows"][0]["icon"], "layoutVertical");
    let rect = doc.execute("shape.rectangle", json!({ "x": 600, "y": 0, "width": 50, "height": 50 })).unwrap()["id"].clone();
    doc.execute("select.set", json!({ "ids": [id, rect] })).unwrap();
    let two = doc.query("selection", &json!({})).unwrap();
    assert_eq!(two["count"], 2);
    assert_eq!(two["kind"], "Mixed");
    assert!(two["mixed"].as_array().unwrap().iter().any(|k| k == "type"), "{two}");
    for name in [
        "components",
        "styles",
        "variables",
        "framePresets",
        "versions",
        "prototype",
        "menus",
        "checked",
        "devInspect",
        "selectionColors",
        "exportSettings",
        "vectorEdit",
        "document",
    ] {
        doc.query(name, &json!({})).unwrap_or_else(|e| panic!("{name}: {e}"));
    }
    let menus = doc.query("menus", &json!({})).unwrap();
    assert!(menus.as_array().unwrap().iter().any(|r| r["command"] == "---"));
    // Queries change nothing.
    let revision = doc.revision();
    doc.query("devInspect", &json!({})).unwrap();
    doc.query("components", &json!({})).unwrap();
    assert_eq!(doc.revision(), revision);
}

#[test]
fn chrome_shows_frame_labels_the_size_pill_and_auto_layout_handles() {
    let mut doc = design();
    let id = frame_id(&doc);
    let chrome = doc.chrome(1.0);
    let label = &chrome["labels"][0];
    assert_eq!(label["kind"], "frame");
    assert_eq!((label["x"].as_f64(), label["y"].as_f64()), (Some(1000.0), Some(1000.0)), "the frame's top-left, in document pixels");
    assert_eq!(label["color"], Value::Null, "not selected: the secondary text colour");
    assert_eq!(chrome["sizeLabel"], Value::Null);
    doc.execute("select.set", json!({ "ids": [id] })).unwrap();
    doc.execute("layout.add", json!({})).unwrap();
    let chrome = doc.chrome(2.0);
    assert_eq!(chrome["labels"][0]["selected"], true);
    assert_eq!(chrome["labels"][0]["color"], "#00875f");
    let size = &chrome["sizeLabel"];
    assert!(size["text"].as_str().unwrap().contains(" × "), "{size}");
    assert_eq!(size["color"], "#00875f");
    let al = &chrome["autoLayout"];
    assert_eq!(al["id"], id);
    assert_eq!(al["padding"].as_array().unwrap().len(), 4);
    assert_eq!(al["padding"][0]["key"], "paddingTop");
    // The label's area is the engine's: a press on it selects the frame.
    doc.execute("select.none", json!({})).unwrap();
    let quad = doc.chrome(1.0)["labels"][0]["quad"].clone();
    let (x, y) = (quad[0][0].as_f64().unwrap() + 2.0, quad[0][1].as_f64().unwrap() + 2.0);
    doc.pointer(&event(PointerAction::Down, x, y)).unwrap();
    doc.pointer(&event(PointerAction::Up, x, y)).unwrap();
    assert_eq!(doc.inspect().unwrap()["selection"], json!([id]));
}

#[test]
fn export_node_and_thumbnail_make_files_without_changing_the_design() {
    let doc = design();
    let id = frame_id(&doc).to_string();
    let png = doc.export_node(&id, "png", 2.0).unwrap();
    assert_eq!(&png[1..4], b"PNG");
    let svg = doc.export_node(&id, "svg", 1.0).unwrap();
    assert!(String::from_utf8_lossy(&svg).contains("<svg"));
    assert!(doc.export_node("999999", "png", 1.0).is_err());
    assert!(doc.export_node(&id, "doc", 1.0).is_err());
    assert_eq!(&doc.thumbnail().unwrap()[1..4], b"PNG");
    assert!(!doc.dirty());
}

#[test]
fn hovering_and_queries_do_not_move_the_revision() {
    let mut doc = design();
    let revision = doc.revision();
    for x in [1050.0, 1080.0, 1500.0, 2500.0] {
        doc.pointer(&event(PointerAction::Move, x, 1060.0)).unwrap();
    }
    doc.chrome(1.0);
    doc.query("selection", &json!({})).unwrap();
    assert_eq!(doc.revision(), revision, "hover feedback repaints nothing");
}

#[test]
fn enter_edits_a_shapes_points_with_anchor_overlays() {
    let mut doc = design();
    let rows = doc.query("layers", &json!({ "all": true })).unwrap();
    let rect = rows["rows"][0]["children"][0]["id"].clone();
    doc.execute("select.set", json!({ "ids": [rect] })).unwrap();
    let editing = doc.key("Enter", ToolMods::default(), 1.0).unwrap();
    assert_eq!(doc.inspect().unwrap()["editing"], "vector");
    let points = editing.overlays.iter().filter(|o| matches!(o, ToolOverlay::Anchor { .. })).count();
    assert_eq!(points, 4, "the rectangle's four points: {:?}", editing.overlays);
    assert!(editing.busy);
    let vector = doc.query("vectorEdit", &json!({})).unwrap();
    assert!(vector["state"].is_object() && vector["tools"].as_array().is_some_and(|t| !t.is_empty()), "{vector}");
    doc.key("Escape", ToolMods::default(), 1.0).unwrap();
    doc.key("Escape", ToolMods::default(), 1.0).unwrap();
    assert_eq!(doc.inspect().unwrap()["editing"], "none");
}

#[test]
fn the_pen_draws_a_path_click_by_click() {
    let mut doc = design();
    let before = layers(&doc);
    doc.select_tool("pen", 1.0).unwrap();
    for (x, y) in [(1600.0, 1100.0), (1700.0, 1100.0), (1700.0, 1200.0)] {
        doc.pointer(&event(PointerAction::Down, x, y)).unwrap();
        doc.pointer(&event(PointerAction::Up, x, y)).unwrap();
    }
    let drawing = doc.pointer(&event(PointerAction::Move, 1650.0, 1250.0)).unwrap();
    assert!(drawing.busy, "a pen path is open");
    assert!(drawing.overlays.iter().filter(|o| matches!(o, ToolOverlay::Anchor { .. })).count() >= 3, "{:?}", drawing.overlays);
    doc.key("Enter", ToolMods::default(), 1.0).unwrap();
    assert_eq!(layers(&doc), before + 1, "Enter ends the path");
}

/// The pixel size a PNG's header gives.
fn png_size(png: &[u8]) -> (u64, u64) {
    let word = |at: usize| u64::from(u32::from_be_bytes(png[at..at + 4].try_into().unwrap()));
    (word(16), word(20))
}

/// Review #5: a raster export's pixel count is capped (the scale is lowered to fit), so a large
/// layer at the largest scale cannot take gigabytes; small exports keep their scale.
#[test]
fn a_raster_export_keeps_within_the_pixel_budget() {
    let doc = DesignDocument::create("Design.kdesign", 300, 300).unwrap();
    let id = frame_id(&doc).to_string();
    let budget = 32 * 1024 * 1024;
    for png in [doc.export_node(&id, "png", 32.0).unwrap(), doc.export_node_with(&id, &json!({ "scale": 32 })).unwrap()] {
        let (w, h) = png_size(&png);
        assert!(w * h <= budget, "{w} x {h} is over the budget");
        assert!(w * h > budget * 9 / 10, "{w} x {h}: the scale is lowered only as far as it has to be");
        assert_eq!(w, h, "the aspect ratio is kept");
    }
    assert_eq!(png_size(&doc.export_node(&id, "png", 2.0).unwrap()), (600, 600));
}
