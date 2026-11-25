//! What the editor's panels and canvas layer rely on beyond `execute`: interactions as one undo
//! step, the view, the panels' newer queries, and the pictures only the engine draws.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;
use kition_kit::tools::{KitTools, PointerAction, ToolMods, ToolPointer};
use serde_json::{Value, json};

/// A 400 × 300 design with a red 120 × 80 rectangle at (20, 20) in its frame; the frame is at
/// canvas (0, 0), which is document pixel (1000, 1000).
fn design() -> (DesignDocument, Value, Value) {
    let mut doc = DesignDocument::create("Design.kdesign", 400, 300).unwrap();
    let frame = doc.query("layers", &json!({})).unwrap()["rows"][0]["id"].clone();
    let rect = doc.execute("shape.rectangle", json!({ "x": 20, "y": 20, "width": 120, "height": 80, "parent": frame })).unwrap()["id"].clone();
    doc.execute("fill.set", json!({ "ids": [rect], "color": "#ff0000" })).unwrap();
    doc.execute("edit.deselectAll", json!({})).unwrap();
    doc.mark_saved();
    (doc, frame, rect)
}

/// A layer id as the extras take it.
fn id_str(id: &Value) -> String {
    id.as_str().map_or_else(|| id.to_string(), str::to_owned)
}

fn x_of(doc: &DesignDocument, id: &Value) -> f64 {
    doc.query("node", &json!({ "id": id })).unwrap()["x"].as_f64().unwrap()
}

fn undo_depth(doc: &DesignDocument) -> usize {
    doc.session().undo_depth().0
}

fn is_png(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x89, b'P', b'N', b'G'])
}

fn png_size(bytes: &[u8]) -> (u32, u32) {
    let image = image::load_from_memory(bytes).unwrap();
    (image.width(), image.height())
}

#[test]
fn an_interaction_is_one_undo_step_and_renders_live() {
    let (mut doc, _, rect) = design();
    let steps = undo_depth(&doc);
    assert!(doc.begin_interaction("Scrub").unwrap(), "begin opens the interaction");
    assert!(!doc.begin_interaction("Again").unwrap(), "a second begin joins it");
    assert_eq!(doc.inspect().unwrap()["interaction"], "Scrub");
    let mut revision = doc.revision();
    for x in [25, 30, 40, 55] {
        doc.execute("node.set", json!({ "ids": [rect], "props": { "x": x } })).unwrap();
        assert!(doc.revision() > revision, "every scrub step moves the revision for the canvas");
        revision = doc.revision();
        assert_eq!(x_of(&doc, &rect), f64::from(x));
    }
    assert_eq!(undo_depth(&doc), steps, "no undo step while it runs");
    assert!(doc.commit_interaction().unwrap(), "commit records a step");
    assert_eq!(undo_depth(&doc), steps + 1, "the scrub is one undo step");
    assert_eq!(doc.inspect().unwrap()["undo"], "Scrub");
    assert_eq!(doc.inspect().unwrap()["interaction"], Value::Null);
    assert!(!doc.commit_interaction().unwrap(), "nothing left to commit");
    doc.execute("edit.undo", json!({})).unwrap();
    assert_eq!(x_of(&doc, &rect), 20.0, "one undo takes the whole scrub back");
}

#[test]
fn cancel_restores_and_an_empty_interaction_records_nothing() {
    let (mut doc, _, rect) = design();
    let steps = undo_depth(&doc);
    doc.begin_interaction("Color").unwrap();
    doc.execute("node.set", json!({ "ids": [rect], "props": { "x": 90 } })).unwrap();
    assert!(doc.cancel_interaction().unwrap());
    assert_eq!(x_of(&doc, &rect), 20.0, "cancel puts the document back");
    assert_eq!(undo_depth(&doc), steps);
    doc.begin_interaction("Nothing").unwrap();
    assert!(!doc.commit_interaction().unwrap(), "no change, no step");
    assert_eq!(undo_depth(&doc), steps);
    assert!(!doc.cancel_interaction().unwrap(), "nothing open");
}

#[test]
fn undo_a_press_or_a_key_end_an_open_interaction_first() {
    let (mut doc, _, rect) = design();
    let steps = undo_depth(&doc);
    doc.begin_interaction("Padding").unwrap();
    doc.execute("node.set", json!({ "ids": [rect], "props": { "x": 70 } })).unwrap();
    doc.execute("edit.undo", json!({})).unwrap();
    assert_eq!(x_of(&doc, &rect), 20.0, "undo commits the open drag, then undoes it");
    assert_eq!(undo_depth(&doc), steps);
    assert_eq!(doc.inspect().unwrap()["interaction"], Value::Null);

    doc.begin_interaction("Scrub").unwrap();
    doc.execute("node.set", json!({ "ids": [rect], "props": { "x": 33 } })).unwrap();
    let press = ToolPointer { action: PointerAction::Down, x: 0.0, y: 0.0, mods: ToolMods::default(), pressure: 1.0, zoom: 1.0 };
    doc.pointer(&press).unwrap();
    doc.pointer(&ToolPointer { action: PointerAction::Up, ..press }).unwrap();
    assert_eq!(undo_depth(&doc), steps + 1, "a press committed the scrub");
    assert!(!doc.commit_interaction().unwrap(), "the editor no longer holds it");

    doc.begin_interaction("Scrub").unwrap();
    doc.execute("node.set", json!({ "ids": [rect], "props": { "x": 44 } })).unwrap();
    doc.key("Escape", ToolMods::default(), 1.0).unwrap();
    assert_eq!(undo_depth(&doc), steps + 2, "a key committed it");
}

#[test]
fn text_editing_keeps_its_own_interaction() {
    let (mut doc, frame, _) = design();
    let text = doc.execute("text.add", json!({ "text": "Hello", "x": 10, "y": 150, "parent": frame })).unwrap()["id"].clone();
    doc.execute("select.set", json!({ "ids": [text] })).unwrap();
    doc.key("Enter", ToolMods::default(), 1.0).unwrap();
    assert_eq!(doc.inspect().unwrap()["editing"], "text");
    assert!(!doc.begin_interaction("Font size").unwrap(), "the text edit holds the interaction");
    assert!(!doc.commit_interaction().unwrap(), "commit leaves it to the text edit");
    assert_eq!(doc.inspect().unwrap()["editing"], "text", "still editing");
}

#[test]
fn view_target_answers_where_zoom_commands_go() {
    let (mut doc, frame, rect) = design();
    let view = json!({ "zoom": 1, "x": 0, "y": 0, "width": 1000, "height": 800 });
    let fit = doc.view_target("view.zoomFit", &view).unwrap();
    assert_eq!(fit["rect"], json!({ "x": 1000.0, "y": 1000.0, "width": 400.0, "height": 300.0 }), "the page's content");
    // The engine's fit: the view less a 64 px margin on every side.
    let zoom = ((1000.0f64 - 128.0) / 400.0).min((800.0 - 128.0) / 300.0);
    assert!((fit["zoom"].as_f64().unwrap() - zoom).abs() < 1e-9, "{fit}");
    assert_eq!((fit["centerX"].as_f64().unwrap(), fit["centerY"].as_f64().unwrap()), (1200.0, 1150.0));
    assert!((fit["x"].as_f64().unwrap() - (1200.0 - 500.0 / zoom)).abs() < 1e-9);

    let nothing = doc.view_target("view.zoomSelection", &view).unwrap();
    assert_eq!(nothing["rect"], Value::Null, "nothing selected");
    doc.execute("select.set", json!({ "ids": [rect] })).unwrap();
    let sel = doc.view_target("view.zoomSelection", &view).unwrap();
    assert_eq!(sel["rect"], json!({ "x": 1020.0, "y": 1020.0, "width": 120.0, "height": 80.0 }));

    let hundred = doc.view_target("view.zoom100", &json!({ "zoom": 0.5, "x": 0, "y": 0, "width": 1000, "height": 800 })).unwrap();
    assert_eq!(hundred["zoom"], 1.0);
    assert_eq!((hundred["centerX"].as_f64().unwrap(), hundred["centerY"].as_f64().unwrap()), (1000.0, 800.0), "about the view's centre");
    let zoom_in = doc.view_target("view.zoomIn", &json!({ "zoom": 1 })).unwrap();
    assert_eq!(zoom_in["zoom"], 2.0, "the next zoom step");
    let at = doc.view_target("view.zoomAt", &json!({ "zoom": 1, "x": 0, "y": 0, "width": 1000, "height": 800, "atX": 100, "atY": 100 })).unwrap();
    assert_eq!(at["zoom"], 2.0);

    let next = doc.view_target("view.nextFrame", &view).unwrap();
    assert_eq!(next["selection"], json!([frame]), "the frame step selects the frame");
    assert_eq!(next["rect"], json!({ "x": 1000.0, "y": 1000.0, "width": 400.0, "height": 300.0 }));
    assert_eq!(doc.inspect().unwrap()["selection"], json!([rect]), "but only in its answer");
    assert!(doc.view_target("view.pan", &view).is_err());
    assert!(doc.view_target("edit.delete", &view).is_err());
}

#[test]
fn set_viewport_places_shapes_in_the_view() {
    let (mut doc, _, _) = design();
    let revision = doc.revision();
    // Document pixels 1500..1700 × 1500..1700 at zoom 2: the centre is canvas (600, 600).
    doc.set_viewport(1500.0, 1500.0, 400.0, 400.0, 2.0);
    assert_eq!(doc.revision(), revision, "the view is no edit");
    doc.select_tool("rectangle", 2.0).unwrap();
    // A hover elsewhere no longer moves where Enter places.
    let hover = ToolPointer { action: PointerAction::Move, x: 10.0, y: 10.0, mods: ToolMods::default(), pressure: 1.0, zoom: 2.0 };
    doc.pointer(&hover).unwrap();
    doc.key("Enter", ToolMods::default(), 2.0).unwrap();
    let bounds = doc.inspect().unwrap()["selectionBounds"].clone();
    let cx = bounds["x"].as_f64().unwrap() + bounds["width"].as_f64().unwrap() / 2.0;
    let cy = bounds["y"].as_f64().unwrap() + bounds["height"].as_f64().unwrap() / 2.0;
    assert_eq!((cx, cy), (1600.0, 1600.0), "{bounds}");
}

#[test]
fn board_commands_are_disabled_on_designs() {
    let (doc, _, _) = design();
    let disabled = doc.disabled_commands();
    for id in ["board.sticky", "board.timer", "tool.sticky", "tool.connector"] {
        assert!(disabled.iter().any(|d| d == id), "{id} is disabled on a design");
    }
    assert!(!disabled.iter().any(|d| d == "tool.section" || d == "tool.rectangle"));
    let commands = doc.commands();
    assert!(!commands.iter().find(|c| c.id == "board.sticky").unwrap().enabled);
}

#[test]
fn font_queries_list_families_with_styles_and_one_family_in_full() {
    let (doc, _, _) = design();
    let families = doc.query("fontFamilies", &json!({})).unwrap();
    let list = families["families"].as_array().unwrap();
    assert!(!list.is_empty(), "the engine draws some family");
    for f in list {
        assert!(!f["styles"].as_array().unwrap().is_empty(), "{f}");
    }
    let family = list[0]["family"].as_str().unwrap();
    let one = doc.query("font", &json!({ "family": family })).unwrap();
    assert_eq!(one["found"], true, "{one}");
    assert_eq!(one["styles"], list[0]["styles"]);
    for key in ["features", "axes", "instances"] {
        assert!(one[key].is_array(), "{key}: {one}");
    }
    let missing = doc.query("font", &json!({ "family": "No Such Family 12345" })).unwrap();
    assert_eq!(missing["found"], false);
    assert_eq!(missing["features"], json!([]));
    let filtered = doc.query("fontFamilies", &json!({ "query": family.to_uppercase() })).unwrap();
    assert!(filtered["families"].as_array().unwrap().iter().any(|f| f["family"] == family));
    assert!(doc.query("font", &json!({})).is_err());
    assert!(doc.query("missingFonts", &json!({})).is_ok());
}

#[test]
fn text_styles_and_text_previews() {
    let (mut doc, _, _) = design();
    let family = doc.query("fontFamilies", &json!({})).unwrap()["families"][0]["family"].as_str().unwrap().to_owned();
    let row = doc.text_preview(&json!({ "family": family, "scale": 2 })).unwrap();
    assert!(is_png(&row));
    let (w, h) = png_size(&row);
    assert!(w > 10 && h > 10 && w <= 800 && h <= 120, "{w} × {h}");
    let style =
        doc.execute("style.add", json!({ "type": "text", "name": "Headings/Title", "value": { "fontFamily": family, "fontSize": 32 } })).unwrap();
    let styles = doc.query("textStyles", &json!({})).unwrap();
    let title = styles["styles"].as_array().unwrap().iter().find(|s| s["name"] == "Headings/Title").unwrap().clone();
    assert_eq!((title["folder"].as_str(), title["label"].as_str()), (Some("Headings"), Some("Title")));
    assert_eq!(title["style"]["fontSize"], 32.0, "{title}");
    let id = title["id"].as_str().unwrap();
    assert!(style.to_string().contains(id) || !id.is_empty());
    let sample = doc.text_preview(&json!({ "styleId": id, "color": "#5645d4" })).unwrap();
    assert!(is_png(&sample));
    assert!(png_size(&sample).1 > png_size(&doc.text_preview(&json!({ "family": family })).unwrap()).1, "32 px type is taller than 13 px");
    assert!(doc.text_preview(&json!({})).is_err());
    assert!(doc.text_preview(&json!({ "styleId": "nope" })).is_err());
    assert!(doc.text_preview(&json!({ "family": family, "color": 3 })).is_err());
}

#[test]
fn node_thumbnails_and_the_eyedropper_sample() {
    let (doc, frame, rect) = design();
    let thumb = doc.node_thumbnail(&id_str(&rect), 64.0).unwrap();
    assert!(is_png(&thumb));
    assert_eq!(png_size(&thumb), (64, 43), "120 × 80 fits 64 px");
    let big = doc.node_thumbnail(&id_str(&frame), 4000.0).unwrap();
    assert_eq!(png_size(&big), (800, 600), "at most 2x");
    assert!(doc.node_thumbnail("nope", 64.0).is_err());
    assert!(doc.node_thumbnail("999999", 64.0).is_err());
    // The rectangle is at canvas (20, 20)..(140, 100): document pixels (1020, 1020)..
    assert_eq!(doc.sample_pixel(1050.0, 1050.0, 1.0).unwrap(), "#ff0000");
    assert_eq!(doc.sample_pixel(1050.0, 1050.0, 4.0).unwrap(), "#ff0000");
    assert_eq!(doc.sample_pixel(1300.0, 1250.0, 1.0).unwrap(), "#ffffff", "the frame's white");
    assert!(doc.sample_pixel(f64::NAN, 0.0, 1.0).is_err());
    let colors = doc.query("recentColors", &json!({})).unwrap();
    assert!(colors["colors"].is_array() && colors["picked"].is_null(), "{colors}");
}

#[test]
fn comment_pins_sit_in_document_pixels_and_crowded_ones_cluster() {
    let (mut doc, _, _) = design();
    doc.execute("comment.add", json!({ "x": 50, "y": 60, "text": "First" })).unwrap();
    doc.execute("comment.add", json!({ "x": 55, "y": 62, "text": "Close by" })).unwrap();
    doc.execute("comment.add", json!({ "x": 300, "y": 200, "text": "Far" })).unwrap();
    let pins = doc.query("commentPins", &json!({ "zoom": 1 })).unwrap();
    assert_eq!(pins["pins"].as_array().unwrap().len(), 1, "{pins}");
    assert_eq!(pins["clusters"].as_array().unwrap().len(), 1);
    assert_eq!(pins["clusters"][0]["ids"].as_array().unwrap().len(), 2);
    let far = &pins["pins"][0];
    assert_eq!((far["x"].as_f64(), far["y"].as_f64()), (Some(1300.0), Some(1200.0)));
    assert!(far["color"].as_str().unwrap().starts_with('#'));
    assert!(!far["initials"].as_str().unwrap().is_empty());
    // Zoomed in, the close pair stands apart.
    let zoomed = doc.query("commentPins", &json!({ "zoom": 8 })).unwrap();
    assert_eq!(zoomed["pins"].as_array().unwrap().len(), 3);
    let resolved = doc.query("commentPins", &json!({ "filter": "resolved" })).unwrap();
    assert!(resolved["pins"].as_array().unwrap().is_empty());
    assert!(doc.query("commentPins", &json!({ "filter": "junk" })).is_err());
    assert!(doc.query("commentPeople", &json!({})).unwrap()["people"].is_array());
}

#[test]
fn prototype_connections_run_from_layers_to_frames() {
    let (mut doc, frame, rect) = design();
    let other = doc.execute("frame.add", json!({ "x": 600, "y": 0, "width": 200, "height": 300 })).unwrap()["id"].clone();
    doc.execute("proto.interaction.add", json!({ "id": rect, "dest": other })).unwrap();
    doc.execute("select.set", json!({ "ids": [rect] })).unwrap();
    let all = doc.query("protoConnections", &json!({ "zoom": 1 })).unwrap();
    let c = &all["connections"][0];
    assert_eq!((c["source"].clone(), c["dest"].clone(), c["index"].clone()), (rect.clone(), other.clone(), json!(0)));
    let area = doc.inspect().unwrap()["area"].clone();
    let (ax, ay) = (area["x"].as_f64().unwrap(), area["y"].as_f64().unwrap());
    // From the rectangle's right edge middle (140, 60) to the frame's left edge middle (600, 150).
    assert_eq!(c["points"][0], json!([140.0 - ax, 60.0 - ay]));
    assert_eq!(c["points"][3], json!([600.0 - ax, 150.0 - ay]));
    assert_eq!(c["arrow"][0], c["points"][3]);
    assert_eq!(c["selected"], true);
    assert_eq!(all["handles"][0]["id"], rect);
    let handle = all["handles"][0].clone();
    let over = doc.query("protoConnections", &json!({ "zoom": 1, "x": handle["x"], "y": handle["y"] })).unwrap();
    assert_eq!(over["handleHit"], rect);
    let start = doc.query("protoConnections", &json!({ "zoom": 1, "x": 141.0 - ax, "y": 60.0 - ay })).unwrap();
    assert_eq!(start["hit"], json!({ "source": rect, "index": 0 }));
    let drop = doc.query("protoConnections", &json!({ "x": 700.0 - ax, "y": 100.0 - ay })).unwrap();
    assert_eq!(drop["frameAt"], other);
    let none = doc.query("protoConnections", &json!({ "x": 380.0 - ax, "y": 280.0 - ay })).unwrap();
    assert_eq!((none["hit"].clone(), none["frameAt"].clone()), (Value::Null, frame));
}

#[test]
fn paint_and_blur_handles_map_the_unit_box_to_document_pixels() {
    let (mut doc, _, rect) = design();
    assert_eq!(doc.query("paintHandles", &json!({ "id": rect, "index": 0 })).unwrap()["kind"], "none", "a solid has no handles");
    let gradient = json!({ "type": "gradient", "kind": "linear",
        "stops": [{ "position": 0, "color": "#ff0000" }, { "position": 1, "color": "#0000ff" }],
        "from": [0, 0.5], "to": [1, 0.5], "width": [0, 1] });
    doc.execute("fill.set", json!({ "ids": [rect], "index": 0, "paint": gradient })).unwrap();
    let h = doc.query("paintHandles", &json!({ "id": rect, "index": 0 })).unwrap();
    assert_eq!(h["kind"], "gradient", "{h}");
    assert_eq!(h["gradientType"], "linear");
    // The rectangle spans document pixels (1020, 1020)..(1140, 1100).
    assert_eq!(h["from"], json!([1020.0, 1060.0]));
    assert_eq!(h["to"], json!([1140.0, 1060.0]));
    assert_eq!(h["stops"][1]["point"], json!([1140.0, 1060.0]));
    assert_eq!(h["unit"], json!([120.0, 0.0, 0.0, 80.0, 1020.0, 1020.0]));
    assert!(doc.query("paintHandles", &json!({ "id": rect, "index": 4 })).is_err());
    // Review #24: an index past 32 bits names no paint (it wrapped to 0 on wasm32).
    for index in [1u64 << 32, u64::MAX] {
        assert!(doc.query("paintHandles", &json!({ "id": rect, "index": index })).unwrap_err().starts_with("not-found:"));
        assert_eq!(doc.query("blurHandles", &json!({ "id": rect, "index": index })).unwrap(), Value::Null);
    }
    assert!(doc.query("paintHandles", &json!({})).is_err());
    assert_eq!(doc.query("blurHandles", &json!({ "id": rect, "index": 0 })).unwrap(), Value::Null);
    doc.execute(
        "effect.add",
        json!({ "ids": [rect], "effect": { "type": "layerBlur", "radius": 12, "blurType": "progressive", "startOffset": [0.5, 0.0], "endOffset": [0.5, 1.0] } }),
    )
    .unwrap();
    let b = doc.query("blurHandles", &json!({ "id": rect, "index": 0 })).unwrap();
    assert_eq!((b["start"].clone(), b["end"].clone()), (json!([1080.0, 1020.0]), json!([1080.0, 1100.0])), "{b}");
}

#[test]
fn measurements_and_layers_at_answer_in_document_pixels() {
    let (mut doc, frame, rect) = design();
    doc.execute("select.set", json!({ "ids": [rect] })).unwrap();
    let page = doc.query("measurements", &json!({})).unwrap();
    assert!(page.is_array());
    let lines = doc.query("distances", &json!({})).unwrap();
    let first = &lines["lines"][0];
    assert!(first["x0"].as_f64().unwrap() >= 1000.0 && first["y0"].as_f64().unwrap() >= 1000.0, "{lines}");
    assert!(doc.query("redlines", &json!({})).unwrap().is_array());
    let at = doc.query("layersAt", &json!({ "x": 1050, "y": 1050 })).unwrap();
    assert!(at["layers"].as_array().unwrap().iter().any(|l| l["id"] == rect), "{at}");
    assert!(doc.query("layersAt", &json!({})).is_err());
    assert!(doc.query("paddingLines", &json!({})).is_err(), "not in an auto layout frame");
    doc.execute("layout.add", json!({ "ids": [frame] })).unwrap();
    doc.execute("node.set", json!({ "ids": [frame], "props": { "paddingTop": 24 } })).unwrap();
    let padding = doc.query("paddingLines", &json!({ "id": frame })).unwrap();
    let top = padding["lines"].as_array().unwrap().iter().find(|l| l["label"] == "24").unwrap_or_else(|| panic!("{padding}"));
    assert!(top["y0"].as_f64().unwrap() >= 1000.0, "{top}");
}

#[test]
fn keymap_prefs_and_the_toolbar() {
    let (doc, _, _) = design();
    let keys = doc.query("keymap", &json!({})).unwrap();
    let undo = keys["bindings"].as_array().unwrap().iter().find(|b| b["id"] == "edit.undo").unwrap().clone();
    assert_eq!(undo["keys"][0], "Cmd+Z");
    assert!(!undo["title"].as_str().unwrap().is_empty());
    assert!(!keys["bindings"].as_array().unwrap().iter().any(|b| b["id"] == "file.save"), "the host's commands are left out");
    assert!(!keys["gestures"].as_array().unwrap().is_empty());
    let other = doc.query("keymap", &json!({ "platform": "other" })).unwrap();
    let other_undo = other["bindings"].as_array().unwrap().iter().find(|b| b["id"] == "edit.undo").unwrap().clone();
    assert_ne!(other_undo["shown"], undo["shown"], "each platform's own display: {other_undo} {undo}");
    assert!(doc.query("keymap", &json!({ "platform": "amiga" })).is_err());
    let prefs = doc.query("prefs", &json!({})).unwrap();
    assert!(prefs["nudgeBig"].as_f64().unwrap() > prefs["nudgeSmall"].as_f64().unwrap());
    let bar = doc.query("toolbar", &json!({})).unwrap();
    assert_eq!((bar["tool"].as_str(), bar["readOnly"].clone(), bar["isBoard"].clone()), (Some("move"), Value::Null, json!(false)));
}
