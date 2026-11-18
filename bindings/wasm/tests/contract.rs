//! The kit engine contract, opening each import kind from bytes, creating, rendering and saving.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::Write;

use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;
use kition_kit::testing::check_contract;
use serde_json::{Value, json};

/// A 400 × 300 design: the created frame and a rectangle inside it.
fn fixture() -> Vec<u8> {
    let mut doc = DesignDocument::create("Design.kdesign", 400, 300).unwrap();
    doc.execute("shape.rectangle", json!({ "x": 20, "y": 20, "width": 120, "height": 80 })).unwrap();
    doc.save(None).unwrap()
}

fn layers(doc: &DesignDocument) -> u64 {
    doc.inspect().unwrap()["layers"].as_u64().unwrap()
}

#[test]
fn keeps_the_kit_contract() {
    check_contract::<DesignDocument>(&fixture(), "Design.kdesign", |doc| {
        doc.execute(
            "shape.ellipse",
            json!({ "x": 200, "y": 120, "width": 80, "height": 60, "props": { "fills": [{ "type": "solid", "color": "#ff0000" }] } }),
        )
        .map(|_| ())
    })
    .unwrap();
}

#[test]
fn keeps_the_contract_on_a_real_example_file() {
    let bytes = include_bytes!("../../../examples/glass-and-vectors.kdesign");
    check_contract::<DesignDocument>(bytes, "glass-and-vectors.kdesign", |doc| {
        // A rectangle at the frame's top-left content corner, where the page shows.
        let area = doc.area();
        doc.execute(
            "shape.rectangle",
            json!({ "x": area.x + 110.0, "y": area.y + 110.0, "width": 64, "height": 64, "props": { "fills": [{ "type": "solid", "color": "#ff00ff" }] } }),
        )
        .map(|_| ())
    })
    .unwrap();
}

#[test]
fn create_makes_one_page_with_one_frame_and_is_clean() {
    let doc = DesignDocument::create("Untitled design.kdesign", 1440, 1024).unwrap();
    let inspected = doc.inspect().unwrap();
    assert_eq!(inspected["pages"].as_array().unwrap().len(), 1);
    assert_eq!(inspected["layers"], 1, "{inspected}");
    assert_eq!(inspected["name"], "Untitled design");
    assert_eq!(inspected["canUndo"], false, "creating is not an undo step");
    assert!(!doc.dirty());
    // The frame plus the margin, out to whole 1000-unit chunks.
    assert_eq!((doc.width(), doc.height()), (3000, 3000));
    assert_eq!(inspected["area"], json!({ "x": -1000.0, "y": -1000.0, "width": 3000, "height": 3000 }));
    assert_eq!(inspected["initialArea"], json!({ "x": 1000.0, "y": 1000.0, "width": 1440.0, "height": 1024.0 }));
    let small = DesignDocument::create("Small.kdesign", 100, 50).unwrap();
    assert_eq!((small.width(), small.height()), (2000, 2000));
}

#[test]
fn renders_the_page_background_and_the_art_opaque_in_straight_alpha() {
    let doc = DesignDocument::open("Design.kdesign", &fixture()).unwrap();
    let (w, h) = (doc.width(), doc.height());
    let pixels = doc.render(0.0, 0.0, 1.0, w, h).unwrap();
    assert_eq!(pixels.len(), (w * h * 4) as usize);
    let at = |x: u32, y: u32| &pixels[((y * w + x) * 4) as usize..((y * w + x) * 4 + 4) as usize];
    // The page (#F5F5F5) around the frame, the white frame, the rectangle's grey (#D9D9D9).
    assert_eq!(at(5, 5), &[0xF5, 0xF5, 0xF5, 0xFF]);
    assert_eq!(at(1000 + 300, 1000 + 200), &[0xFF, 0xFF, 0xFF, 0xFF], "the frame is white");
    let inside = at(1000 + 60, 1000 + 60);
    assert_eq!(inside[3], 0xFF);
    assert!(inside[0] < 0xF0, "the rectangle's fill: {inside:?}");
    assert!(pixels.chunks_exact(4).all(|p| p[3] == 0xFF), "the page is opaque");
    // Zoomed in, drawn at the zoom; zoomed out, an exact reduction.
    assert_eq!(doc.render(1000.0, 1000.0, 4.0, 64, 48).unwrap().len(), 64 * 48 * 4);
    assert_eq!(doc.render(0.0, 0.0, 0.25, 64, 48).unwrap().len(), 64 * 48 * 4);
    assert_eq!(doc.render_area(0, 0, 4000, 4000, 100, 100).unwrap().len(), 100 * 100 * 4);
    assert_eq!(doc.render_area(0, 0, 40_000, 20_000, 200, 100).unwrap().len(), 200 * 100 * 4, "a huge area is drawn at its scale");
}

#[test]
fn saving_writes_kdesign_and_leaves_the_document_dirty_until_marked() {
    let mut doc = DesignDocument::open("Design.kdesign", &fixture()).unwrap();
    assert!(!doc.dirty());
    doc.execute("object.toggleVisible", json!({})).ok();
    doc.execute("select.all", json!({})).unwrap();
    doc.execute("object.group", json!({})).unwrap();
    assert!(doc.dirty());
    let bytes = doc.save(Some("Design.kdesign")).unwrap();
    assert_eq!(&bytes[..2], b"PK", "a .kdesign is a zip");
    assert!(doc.dirty());
    doc.mark_saved();
    assert!(!doc.dirty());
    assert!(doc.save(Some("Design.svg")).is_err(), "other formats are exports");
    let reopened = DesignDocument::open("Design.kdesign", &bytes).unwrap();
    assert_eq!(layers(&reopened), layers(&doc));
}

#[test]
fn opens_a_fig_file_from_bytes() {
    let mut doc = DesignDocument::open("Design.kdesign", &fixture()).unwrap();
    let exported = doc.execute("file.export", json!({ "format": "fig", "ids": [] })).unwrap();
    let data = exported["data"].as_str().expect("the .fig bytes as base64");
    let bytes = base64_decode(data);
    let fig = DesignDocument::open("Design.fig", &bytes).unwrap();
    assert!(layers(&fig) >= 2, "{}", fig.inspect().unwrap());
    assert!(!fig.dirty(), "an import opens clean");
}

#[test]
fn opens_a_rest_api_file_json() {
    let at = |x: f64, y: f64| json!([[1, 0, x], [0, 1, y]]);
    let file = json!({"name": "Kit", "document": {"id": "0:0", "type": "DOCUMENT", "children": [
        {"id": "0:1", "name": "Main", "type": "CANVAS", "backgroundColor": {"r": 0.9, "g": 0.9, "b": 0.9, "a": 1}, "children": [
            {"id": "1:1", "name": "Card", "type": "FRAME", "relativeTransform": at(0.0, 0.0), "size": {"x": 200, "y": 120},
             "fills": [{"type": "SOLID", "color": {"r": 1, "g": 1, "b": 1, "a": 1}}], "children": [
                {"id": "1:2", "name": "Box", "type": "RECTANGLE", "relativeTransform": at(10.0, 10.0), "size": {"x": 50, "y": 50},
                 "fills": [{"type": "SOLID", "color": {"r": 0.2, "g": 0.4, "b": 1, "a": 1}}]}
            ]}
        ]}
    ]}, "components": {}, "styles": {}});
    let doc = DesignDocument::open("Kit.json", file.to_string().as_bytes()).unwrap();
    assert_eq!(layers(&doc), 2);
    let pages = doc.query("pages", &json!({})).unwrap();
    assert_eq!(pages["pages"][0]["name"], "Main");
}

#[test]
fn opens_a_sketch_file() {
    let mut out = Vec::new();
    {
        let mut z = zip::ZipWriter::new(std::io::Cursor::new(&mut out));
        let o = zip::write::SimpleFileOptions::default();
        z.start_file("document.json", o).unwrap();
        z.write_all(json!({"_class": "document", "pages": [{"_class": "MSJSONFileReference", "_ref": "pages/P1"}]}).to_string().as_bytes()).unwrap();
        z.start_file("meta.json", o).unwrap();
        z.write_all(json!({"app": "com.bohemiancoding.sketch3", "appVersion": "70", "version": 136}).to_string().as_bytes()).unwrap();
        z.start_file("pages/P1.json", o).unwrap();
        z.write_all(json!({"_class": "page", "do_objectID": "P1", "name": "Screens", "layers": []}).to_string().as_bytes()).unwrap();
        z.finish().unwrap();
    }
    let doc = DesignDocument::open("App.sketch", &out).unwrap();
    assert_eq!(doc.query("pages", &json!({})).unwrap()["pages"][0]["name"], "Screens");
}

#[test]
fn opens_svg_and_pictures() {
    let svg =
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80"><rect x="10" y="10" width="60" height="40" fill="#336699"/></svg>"##;
    let doc = DesignDocument::open("Icon.svg", svg).unwrap();
    assert!(layers(&doc) >= 1);
    let mut png = Vec::new();
    image::RgbaImage::from_pixel(32, 16, image::Rgba([10, 200, 30, 255]))
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    let picture = DesignDocument::open("Photo.png", &png).unwrap();
    assert_eq!(layers(&picture), 1);
    let pixels = picture.render(1000.0, 1000.0, 1.0, 32, 16).unwrap();
    assert_eq!(&pixels[..4], &[10, 200, 30, 255], "the picture is drawn at the canvas origin");
}

#[test]
fn bad_input_is_an_error_not_a_panic() {
    for (name, bytes) in [
        ("Broken.kdesign", &b"not a design"[..]),
        ("Broken.fig", b"fig-kiwi\x00\x00\x00\x00garbage"),
        ("Broken.sketch", b"PK\x03\x04broken zip"),
        ("Broken.json", b"{\"document\": 3}"),
        ("Broken.png", b"\x89PNG\r\n\x1a\nnope"),
        ("Empty.kdesign", b""),
        ("Design.docx", b"PK\x03\x04"),
    ] {
        assert!(DesignDocument::open(name, bytes).is_err(), "{name}");
    }
    let mut doc = DesignDocument::open("Design.kdesign", &fixture()).unwrap();
    assert!(doc.execute("no.such.command", json!({})).is_err());
    assert!(doc.execute("file.open", json!({ "path": "/etc/passwd" })).is_err(), "shell commands are not offered");
    assert!(doc.execute("shape.rectangle", json!([1, 2])).is_err());
    assert!(doc.execute("edit.undo", json!({ "scope": "everyone" })).is_err());
    assert!(doc.execute("file.export", json!({ "path": "/tmp/kition-design-escape.png" })).is_err(), "no file system");
    assert!(!std::path::Path::new("/tmp/kition-design-escape.png").exists());
    assert!(doc.query("no-such-query", &json!({})).is_err());
    assert!(doc.render(f64::NAN, f64::INFINITY, f64::NAN, 0, 0).is_ok());
}

#[test]
fn undo_and_redo_take_a_scope() {
    let mut doc = DesignDocument::open("Design.kdesign", &fixture()).unwrap();
    let before = layers(&doc);
    doc.execute("shape.rectangle", json!({ "x": 300, "y": 10, "width": 10, "height": 10 })).unwrap();
    assert_eq!(layers(&doc), before + 1);
    assert_eq!(doc.inspect().unwrap()["canUndo"], true);
    // Engine contract 2 (E14a): the agent's undo takes back only the agent's steps; this one is
    // the person's.
    assert_eq!(doc.execute("edit.undo", json!({ "scope": "agent" })).unwrap()["done"], false);
    assert_eq!(layers(&doc), before + 1);
    assert_eq!(doc.execute("edit.undo", json!({ "scope": "person" })).unwrap()["done"], true);
    assert_eq!(layers(&doc), before);
    assert_eq!(doc.execute("edit.redo", json!({ "scope": "agent" })).unwrap()["done"], false);
    assert_eq!(doc.execute("edit.redo", json!({ "scope": "person" })).unwrap()["done"], true);
    assert_eq!(layers(&doc), before + 1);
    assert!(doc.disabled_commands().contains(&"edit.redo".to_owned()));
}

#[test]
fn check_reports_missing_components_and_empty_layers() {
    let mut doc = DesignDocument::create("Design.kdesign", 400, 300).unwrap();
    assert!(doc.check().is_empty() || doc.check().iter().all(|d| d.code != "design.missing-component"));
    doc.execute("text.create", json!({ "text": " ", "x": 10, "y": 10 })).unwrap();
    doc.execute("frame.create", json!({ "x": 600, "y": 0, "width": 50, "height": 50, "props": { "fills": [] } })).unwrap();
    let codes: Vec<String> = doc.check().into_iter().map(|d| d.code).collect();
    assert!(codes.contains(&"design.empty-text".to_owned()), "{codes:?}");
    assert!(codes.contains(&"design.empty-frame".to_owned()), "{codes:?}");
    let mut missing = kition_design_doc::Document::new("Broken");
    let page = missing.first_page().unwrap();
    missing
        .create(
            page,
            kition_design_doc::NodeKind::Instance { component: kition_design_doc::NodeId(9999), props: Default::default() },
            "Orphan",
            0.0,
            0.0,
            10.0,
            10.0,
        )
        .unwrap();
    let found = kition_design_kit::check::check_with(&missing, &mut |_| true);
    assert_eq!(found.first().map(|d| d.code.as_str()), Some("design.missing-component"));
    kition_kit::diagnostics::check_diagnostics(&found).unwrap();
}

fn base64_decode(text: &str) -> Vec<u8> {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let (mut out, mut buffer, mut bits) = (Vec::new(), 0u32, 0u32);
    for byte in text.trim_end_matches('=').bytes() {
        let value = ALPHABET.iter().position(|known| *known == byte).expect("base64") as u32;
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buffer >> bits) & 0xff) as u8);
        }
    }
    out
}

#[allow(dead_code)]
fn show(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap()
}
