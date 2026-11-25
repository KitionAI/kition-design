//! The second round of engine requests from the editor's panels: the node keys the Design panel
//! reads, variable modes, the shared hover, interaction-named undo steps, document colours with
//! alpha, the paste state, images from bytes, brush art, Dev Mode's tool state, export options,
//! and the catalog's presenter commands.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use base64::Engine as _;
use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;
use kition_kit::tools::{KitTools, ToolMods};
use serde_json::{Value, json};

/// A 400 × 300 design with a red 120 × 80 rectangle at (20, 20) in its frame.
fn design() -> (DesignDocument, Value, Value) {
    let mut doc = DesignDocument::create("Design.kdesign", 400, 300).unwrap();
    let frame = doc.query("layers", &json!({})).unwrap()["rows"][0]["id"].clone();
    let rect = doc.execute("shape.rectangle", json!({ "x": 20, "y": 20, "width": 120, "height": 80, "parent": frame })).unwrap()["id"].clone();
    doc.execute("fill.set", json!({ "ids": [rect], "color": "#ff0000" })).unwrap();
    doc.execute("edit.deselectAll", json!({})).unwrap();
    doc.mark_saved();
    (doc, frame, rect)
}

fn id_str(id: &Value) -> String {
    id.as_str().map_or_else(|| id.to_string(), str::to_owned)
}

fn node(doc: &DesignDocument, id: &Value) -> Value {
    doc.query("node", &json!({ "id": id })).unwrap()
}

fn png_b64(w: u32, h: u32) -> String {
    let img = image::RgbaImage::from_pixel(w, h, image::Rgba([0, 0, 255, 255]));
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(img).write_to(&mut out, image::ImageFormat::Png).unwrap();
    base64::engine::general_purpose::STANDARD.encode(out.into_inner())
}

#[test]
fn node_json_carries_the_keys_the_design_panel_reads() {
    let (mut doc, frame, rect) = design();
    let n = node(&doc, &rect);
    for key in
        ["constrainProportions", "overflow", "scrollPosition", "startCap", "endCap", "varModes", "propRefs", "parent", "insideInstance", "hasStatus"]
    {
        assert!(n.get(key).is_some(), "node lacks {key}: {n}");
    }
    assert_eq!(n["constrainProportions"], false);
    assert_eq!(n["parent"], frame);
    assert_eq!(n["parentAutoLayout"], false);
    assert_eq!(n["hasStatus"], false);
    assert_eq!(node(&doc, &frame)["hasStatus"], true, "a top-level frame can carry a status");
    assert_eq!(node(&doc, &frame)["parent"], Value::Null);

    doc.execute("node.set", json!({ "ids": [rect], "props": { "constrainProportions": true, "overflow": "vertical", "scrollPosition": "fixed", "startCap": "round", "endCap": "square" } })).unwrap();
    let n = node(&doc, &rect);
    assert_eq!(n["constrainProportions"], true);
    assert_eq!(n["overflow"], "vertical");
    assert_eq!(n["scrollPosition"], "fixed");
    assert_eq!(n["startCap"], "round");
    assert_eq!(n["endCap"], "square");

    let text = doc.execute("text.add", json!({ "text": "Hello", "x": 10, "y": 150, "parent": frame })).unwrap()["id"].clone();
    doc.execute(
        "node.set",
        json!({ "ids": [text], "props": { "textAlignVertical": "bottom", "paragraphSpacing": 12, "paragraphIndent": 8, "features": { "liga": false } } }),
    )
    .unwrap();
    let t = node(&doc, &text);
    assert_eq!(t["textAlignVertical"], "bottom");
    assert_eq!(t["paragraphSpacing"], 12.0);
    assert_eq!(t["paragraphIndent"], 8.0);
    assert_eq!(t["features"], json!({ "liga": false }));

    // The selection's facts and the per-node keys there.
    doc.execute("select.set", json!({ "ids": [rect, text] })).unwrap();
    let sel = doc.query("selection", &json!({})).unwrap();
    assert_eq!(sel["parents"], json!([frame, frame]));
    assert_eq!(sel["allParentsAutoLayout"], false);
    assert_eq!(sel["insideInstance"], false);
    assert_eq!(sel["hasStatus"], false);
    assert_eq!(sel["canAddVariant"], false);
    assert_eq!(sel["canCombineVariants"], false);
    assert_eq!(sel["common"]["parent"], Value::Null, "parent is left out of common");
    assert_eq!(sel["nodes"][0]["constrainProportions"], true);

    // Components: a set's description, variants on offer, property bindings, inside an instance.
    doc.execute("layout.add", json!({ "ids": [frame] })).unwrap();
    doc.execute("select.set", json!({ "ids": [rect] })).unwrap();
    assert_eq!(doc.query("selection", &json!({})).unwrap()["allParentsAutoLayout"], true);
    let a = doc.execute("component.add", json!({ "ids": [rect] })).unwrap();
    let a = a.get("id").cloned().unwrap_or_else(|| doc.inspect().unwrap()["selection"][0].clone());
    doc.execute("select.set", json!({ "ids": [a] })).unwrap();
    let sel = doc.query("selection", &json!({})).unwrap();
    assert_eq!(sel["canAddVariant"], true, "a main component can become a set: {sel}");
    doc.execute("component.addVariant", json!({ "id": a })).unwrap();
    let set = doc.ds_parent(&a);
    let set_node = node(&doc, &set);
    assert_eq!(set_node["type"], "COMPONENT_SET");
    assert_eq!(set_node["description"], "");
}

/// The parent of a layer, through the `node` query.
trait Parent {
    fn ds_parent(&self, id: &Value) -> Value;
}

impl Parent for DesignDocument {
    fn ds_parent(&self, id: &Value) -> Value {
        self.query("node", &json!({ "id": id })).unwrap()["parent"].clone()
    }
}

#[test]
fn variable_mode_rows_list_collections_with_several_modes() {
    let (mut doc, frame, rect) = design();
    doc.execute("select.set", json!({ "ids": [frame] })).unwrap();
    assert_eq!(doc.query("variable.modeRows", &json!({})).unwrap()["show"], false, "no collection yet");
    let c = doc.execute("variable.addCollection", json!({ "name": "Theme", "modes": ["Light", "Dark"] })).unwrap()["id"].clone();
    doc.execute("variable.addCollection", json!({ "name": "Single" })).unwrap();
    let rows = doc.query("variable.modeRows", &json!({})).unwrap();
    assert_eq!(rows["show"], true, "{rows}");
    assert_eq!(rows["rows"].as_array().unwrap().len(), 1, "one-mode collections are left out");
    let row = &rows["rows"][0];
    assert_eq!(row["collection"], c);
    assert_eq!(row["name"], "Theme");
    assert_eq!(row["modes"].as_array().unwrap().len(), 2);
    assert_eq!(row["own"], Value::Null, "Auto");
    assert_eq!(row["inherited"]["name"], "Light", "Auto is the default mode");
    let dark = row["modes"][1]["id"].clone();
    doc.execute("variable.setMode", json!({ "ids": [frame], "collection": c, "mode": dark })).unwrap();
    let row = doc.query("variable.modeRows", &json!({})).unwrap()["rows"][0].clone();
    assert_eq!(row["own"], dark);
    assert_eq!(node(&doc, &frame)["varModes"][c.as_str().unwrap()], dark);
    // The rectangle inherits Dark from its frame, but a rectangle has no mode picker.
    let rows = doc.query("variable.modeRows", &json!({ "ids": [rect] })).unwrap();
    assert_eq!(rows["show"], false);
    assert_eq!(rows["rows"][0]["inherited"]["name"], "Dark");
    assert!(doc.query("variable.modeRows", &json!({ "ids": "x" })).is_err());
}

#[test]
fn hover_is_shared_without_moving_the_revision() {
    let (mut doc, frame, rect) = design();
    let revision = doc.revision();
    assert_eq!(doc.inspect().unwrap()["hover"], Value::Null);
    assert!(doc.set_hover(Some(&id_str(&rect))).unwrap());
    assert!(!doc.set_hover(Some(&id_str(&rect))).unwrap(), "the same hover again");
    assert_eq!(doc.inspect().unwrap()["hover"], rect);
    assert_eq!(doc.revision(), revision, "hover is no edit");
    let overlays = doc.state(0.0, 0.0, ToolMods::default(), 1.0).overlays;
    assert!(!overlays.is_empty(), "the hover outline shows on the canvas");
    assert!(doc.set_hover(None).unwrap());
    assert_eq!(doc.inspect().unwrap()["hover"], Value::Null);
    assert!(doc.state(0.0, 0.0, ToolMods::default(), 1.0).overlays.len() < overlays.len());
    assert!(doc.set_hover(Some("nope")).is_err());
    assert!(doc.set_hover(Some("99999")).is_err());
    let page = doc.inspect().unwrap()["page"].clone();
    assert!(doc.set_hover(Some(&id_str(&page))).is_err(), "a page is not hovered");
    assert!(doc.set_hover(Some("")).is_ok(), "an empty id clears");
    // The canvas's hover reads back the same way.
    let at = doc.inspect().unwrap()["area"].clone();
    let (ax, ay) = (at["x"].as_f64().unwrap(), at["y"].as_f64().unwrap());
    let _ = frame;
    doc.pointer(&kition_kit::tools::ToolPointer {
        action: kition_kit::tools::PointerAction::Move,
        x: 50.0 - ax,
        y: 50.0 - ay,
        mods: ToolMods::default(),
        pressure: 0.0,
        zoom: 1.0,
    })
    .unwrap();
    assert_eq!(doc.inspect().unwrap()["hover"], rect, "the pointer's hover is the rectangle");
    assert_eq!(doc.revision(), revision);
}

#[test]
fn interaction_steps_are_named_after_their_label() {
    let (mut doc, _, rect) = design();
    doc.begin_interaction("Color").unwrap();
    for c in ["#00ff00", "#00ee00", "#00dd00"] {
        doc.execute("fill.set", json!({ "ids": [rect], "color": c })).unwrap();
    }
    doc.commit_interaction().unwrap();
    doc.begin_interaction("Edit gradient").unwrap();
    doc.execute(
        "command.batch",
        json!({ "commands": [{ "command": "fill.set", "params": { "ids": [rect], "color": "#123456" } }, { "command": "node.set", "params": { "ids": [rect], "props": { "x": 30 } } }] }),
    )
    .unwrap();
    doc.commit_interaction().unwrap();
    let h = doc.query("history", &json!({})).unwrap();
    assert_eq!(h["undo"][0], "Edit gradient", "{h}");
    assert_eq!(h["undo"][1], "Color", "{h}");
    assert_eq!(doc.inspect().unwrap()["undo"], "Edit gradient");
    doc.execute("edit.undo", json!({})).unwrap();
    let h = doc.query("history", &json!({ "limit": 1 })).unwrap();
    assert_eq!(h["undo"], json!(["Color"]));
    assert_eq!(h["redo"], json!(["Edit gradient"]));
}

#[test]
fn document_colors_keep_alpha_and_the_paste_state_follows_copies() {
    let (mut doc, frame, rect) = design();
    let other = doc.execute("shape.rectangle", json!({ "x": 200, "y": 20, "width": 40, "height": 40, "parent": frame })).unwrap()["id"].clone();
    doc.execute("fill.set", json!({ "ids": [other], "color": "#00ff0080" })).unwrap();
    let colors = doc.query("documentColors", &json!({})).unwrap()["colors"].clone();
    let translucent = colors.as_array().unwrap().iter().find(|c| c["color"] == "00FF00").cloned().unwrap_or_else(|| panic!("{colors}"));
    assert_eq!(translucent["css"], "#00FF0080");
    assert!((translucent["alpha"].as_f64().unwrap() - 0.502).abs() < 0.01);
    let red = colors.as_array().unwrap().iter().find(|c| c["color"] == "FF0000").unwrap();
    assert_eq!(red["css"], "#FF0000");
    assert_eq!(doc.query("documentColors", &json!({ "limit": 1 })).unwrap()["colors"].as_array().unwrap().len(), 1);

    let state = doc.query("clipboard.paintState", &json!({})).unwrap();
    assert_eq!(state, json!({ "fill": false, "stroke": false, "effect": false, "properties": false, "layers": false }));
    doc.execute("fill.copy", json!({ "id": rect })).unwrap();
    assert_eq!(doc.query("clipboard.paintState", &json!({})).unwrap()["fill"], true);
    doc.execute("effect.add", json!({ "ids": [rect], "type": "dropShadow" })).unwrap();
    doc.execute("effect.copy", json!({ "id": rect })).unwrap();
    doc.execute("stroke.add", json!({ "ids": [rect] })).unwrap();
    doc.execute("stroke.copy", json!({ "id": rect })).unwrap();
    let state = doc.query("clipboard.paintState", &json!({})).unwrap();
    assert_eq!(state["effect"], true);
    assert_eq!(state["stroke"], true);
    assert_eq!(state["fill"], false, "one paint row at a time, as upstream's clipboard");
}

#[test]
fn an_image_from_bytes_goes_into_a_fill_in_place_and_reports_its_info() {
    let (mut doc, _, rect) = design();
    let placed = doc.execute("image.place", json!({ "data": png_b64(6, 4), "target": rect, "index": 0 })).unwrap();
    let hash = placed["image"].as_str().unwrap().to_owned();
    let n = node(&doc, &rect);
    assert_eq!(n["fills"].as_array().unwrap().len(), 1, "replaced, not added: {n}");
    let info = doc.query("image.info", &json!({ "hash": hash })).unwrap();
    assert_eq!(info["width"], 6);
    assert_eq!(info["height"], 4);
    assert_eq!(info["format"], "png");
    assert_eq!(info["uses"], 1);
    assert_eq!(info["profile"], Value::Null);
    assert!(info["bytes"].as_u64().unwrap() > 0);
    assert!(doc.query("image.info", &json!({ "hash": "nope" })).is_err());
    assert!(doc.query("image.info", &json!({})).is_err());
}

#[test]
fn stroke_preview_outline_traces_every_brush_as_upstream_brush_art() {
    let (doc, _, _) = design();
    for preset in kition_design_pathops::brush::PRESETS {
        let d = doc.stroke_preview_outline(&json!({ "preset": preset.id, "width": 64, "height": 10 })).unwrap();
        assert!(d.starts_with('M') && d.ends_with('Z'), "{}: {d}", preset.id);
        let numbers: Vec<f64> =
            d.split(|c: char| c.is_ascii_alphabetic() || c == ' ').filter(|s| !s.is_empty()).map(|s| s.parse().unwrap()).collect();
        assert!(numbers.len() >= 6 && numbers.len().is_multiple_of(2), "{}", preset.id);
        let ys: Vec<f64> = numbers.iter().skip(1).step_by(2).copied().collect();
        let (lo, hi) = ys.iter().fold((f64::MAX, f64::MIN), |(lo, hi), y| (lo.min(*y), hi.max(*y)));
        // Centred on the line at y = 5 and as tall as the weight allows (upstream's art).
        assert!(lo >= -1.0 && hi <= 11.0 && hi - lo >= 4.0, "{}: {lo}..{hi}", preset.id);
    }
    // A scatter brush is many closed shapes along the line, each its own subpath.
    let stars = doc.stroke_preview_outline(&json!({ "preset": "stars", "width": 64, "height": 10 })).unwrap();
    assert!(stars.matches('M').count() > 2, "{stars}");
    assert!(doc.stroke_preview_outline(&json!({ "preset": "nope" })).is_err());
    assert!(doc.stroke_preview_outline(&json!({})).is_err());
    assert!(doc.stroke_preview_outline(&json!("x")).is_err());
}

#[test]
fn dev_mode_tool_state_pixel_preview_and_content_bounds() {
    let (mut doc, frame, _) = design();
    assert_eq!(doc.query("toolbar", &json!({})).unwrap()["devMode"], false);
    let text = doc.execute("text.add", json!({ "text": "Hi", "x": 10, "y": 150, "parent": frame })).unwrap()["id"].clone();
    doc.execute("select.set", json!({ "ids": [text] })).unwrap();
    doc.key("Enter", ToolMods::default(), 1.0).unwrap();
    assert_eq!(doc.inspect().unwrap()["editing"], "text");
    assert!(doc.set_dev_mode(true).unwrap());
    assert!(!doc.set_dev_mode(true).unwrap());
    assert_eq!(doc.query("toolbar", &json!({})).unwrap()["devMode"], true);
    assert_eq!(doc.inspect().unwrap()["editing"], "none", "Dev Mode ends text editing");
    assert!(doc.set_dev_mode(false).unwrap());
    assert_eq!(doc.query("toolbar", &json!({})).unwrap()["devMode"], false);

    let checked = doc.query("checked", &json!({})).unwrap();
    assert_eq!(checked["pixelPreview"], json!({ "on": false, "scale": 1 }));
    doc.execute("view.pixelPreview", json!({ "scale": 2 })).unwrap();
    assert_eq!(doc.query("checked", &json!({})).unwrap()["pixelPreview"], json!({ "on": true, "scale": 2 }));

    let b = doc.inspect().unwrap()["contentBounds"].clone();
    assert_eq!(b["width"], 400.0, "{b}");
    assert_eq!(b["height"], 300.0);
}

#[test]
fn export_node_with_takes_the_export_settings_options() {
    let (mut doc, frame, rect) = design();
    let id = id_str(&rect);
    let png = doc.export_node_with(&id, &json!({ "scale": 2 })).unwrap();
    let img = image::load_from_memory(&png).unwrap();
    assert_eq!((img.width(), img.height()), (240, 160));
    let by_width = doc.export_node_with(&id, &json!({ "setting": { "size": { "by": "width", "value": 60 } } })).unwrap();
    assert_eq!(image::load_from_memory(&by_width).unwrap().width(), 60);
    let svg = String::from_utf8(doc.export_node_with(&id, &json!({ "format": "svg", "setting": { "svgIdAttribute": true } })).unwrap()).unwrap();
    assert!(svg.contains("<svg") && svg.contains("id="), "{svg}");
    let pdf = doc.export_node_with(&id, &json!({ "format": "pdf", "setting": { "print": { "bleed": 9, "cropMarks": true } } })).unwrap();
    assert!(pdf.starts_with(b"%PDF"));
    // Contents only off: what overlaps the layer's area shows too.
    let other = doc.execute("shape.rectangle", json!({ "x": 30, "y": 30, "width": 20, "height": 20, "parent": frame })).unwrap()["id"].clone();
    doc.execute("fill.set", json!({ "ids": [other], "color": "#0000ff" })).unwrap();
    let alone = image::load_from_memory(&doc.export_node_with(&id, &json!({})).unwrap()).unwrap().to_rgba8();
    let with = image::load_from_memory(&doc.export_node_with(&id, &json!({ "setting": { "contentsOnly": false } })).unwrap()).unwrap().to_rgba8();
    assert_eq!(alone.get_pixel(15, 15)[0], 255, "only the red rectangle");
    assert_eq!(with.get_pixel(15, 15)[2], 255, "the blue square over it");
    // The layer's own export settings by index.
    doc.execute("export.add", json!({ "ids": [rect], "setting": { "size": "3x" } })).unwrap();
    let three = image::load_from_memory(&doc.export_node_with(&id, &json!({ "index": 0 })).unwrap()).unwrap();
    assert_eq!(three.width(), 360);
    assert!(doc.export_node_with(&id, &json!({ "index": 9 })).is_err());
    assert!(doc.export_node_with(&id, &json!({ "format": "doc" })).is_err());
    assert!(doc.export_node_with(&id, &json!({ "setting": { "size": 3 } })).is_err());
    assert!(doc.export_node_with(&id, &json!({ "scale": -1 })).is_err());
    assert!(doc.export_node_with("nope", &json!({})).is_err());
}

#[test]
fn the_presenter_commands_are_in_the_catalog_and_ask_for_the_player() {
    let (mut doc, frame, _) = design();
    let ids: Vec<String> = doc.commands().into_iter().map(|c| c.id).collect();
    for id in ["proto.present", "proto.presentOptions", "proto.preview"] {
        assert!(ids.iter().any(|c| c == id), "{id} is offered");
    }
    let r = doc.execute("proto.present", json!({})).unwrap();
    assert_eq!(r["start"], frame);
    assert_eq!(r["uiRequests"][0]["kind"], "present");
    let r = doc.execute("proto.presentOptions", json!({ "scale": "fitWidth", "deviceFrame": false })).unwrap();
    assert_eq!(r["uiRequests"][0]["kind"], "present.options");
    assert_eq!(r["uiRequests"][0]["data"], json!({ "scale": "fitWidth", "deviceFrame": false }));
}
