//! The agent's preview (`file.render`, KHP A39) of `conformance/basic.kdesign`: not uniform, framed to
//! the page's content, of a diagnostic's target (a layer, a page), at a scale up to 2 and a size
//! budget up to 4096, never a change to the design, and equal to `conformance/golden/` within
//! `golden/tolerance.json`. The whole-document picture hosts take headless (`render` of the
//! document frame) is real too. `KITION_DESIGN_WRITE_GOLDEN=1 cargo test -p kition-design-kit --test
//! preview` rewrites the goldens.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;
use serde_json::{Value, json};

fn repo() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

fn basic() -> DesignDocument {
    DesignDocument::open("basic.kdesign", &std::fs::read(repo().join("conformance/basic.kdesign")).unwrap()).unwrap()
}

struct Image {
    rgba: Vec<u8>,
    width: u32,
    height: u32,
}

fn decode(png: &[u8]) -> Image {
    let image = image::load_from_memory_with_format(png, image::ImageFormat::Png).unwrap().to_rgba8();
    Image { width: image.width(), height: image.height(), rgba: image.into_raw() }
}

fn render(doc: &mut DesignDocument, params: Value) -> (Value, Image, Vec<u8>) {
    let answer = doc.execute("file.render", params).unwrap();
    assert_eq!(answer["mimeType"], "image/png");
    let bytes = base64::engine::general_purpose::STANDARD.decode(answer["png"].as_str().unwrap()).unwrap();
    let image = decode(&bytes);
    assert_eq!((answer["width"].as_u64().unwrap(), answer["height"].as_u64().unwrap()), (u64::from(image.width), u64::from(image.height)));
    (answer, image, bytes)
}

fn colours(image: &Image) -> BTreeSet<[u8; 4]> {
    image.rgba.chunks_exact(4).map(|p| [p[0], p[1], p[2], p[3]]).collect()
}

fn at(image: &Image, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * image.width + x) * 4) as usize;
    [image.rgba[i], image.rgba[i + 1], image.rgba[i + 2], image.rgba[i + 3]]
}

/// The basic fixture's frame and its first rectangle.
fn ids(doc: &DesignDocument) -> (u64, u64) {
    let ds = doc.session().ds().unwrap();
    let frame = ds.doc.node(ds.page).unwrap().children[0];
    let rect = ds.doc.node(frame).unwrap().children[0];
    (frame.0, rect.0)
}

#[test]
fn the_preview_is_real_and_framed_to_the_content() {
    let mut doc = basic();
    let (answer, image, _) = render(&mut doc, json!({}));
    // The page holds one 400 x 300 frame at the canvas origin.
    assert_eq!((image.width, image.height), (400, 300));
    assert_eq!(answer["bounds"], json!({ "x": 0.0, "y": 0.0, "width": 400.0, "height": 300.0 }));
    assert_eq!((answer["page"].as_u64(), answer["scale"].as_f64()), (Some(1), Some(1.0)));
    assert!(colours(&image).len() > 4, "a uniform preview: {:?}", colours(&image).len());
    // The rectangle at (40, 40) is the brand purple; the frame around it is white.
    assert_eq!(at(&image, 100, 80), [0x56, 0x45, 0xD4, 255]);
    assert_eq!(at(&image, 10, 10), [255, 255, 255, 255]);
    // Content grows the frame of the preview.
    doc.execute("shape.rectangle", json!({ "x": 500, "y": 0, "width": 100, "height": 50 })).unwrap();
    let (_, grown, _) = render(&mut doc, json!({}));
    assert_eq!((grown.width, grown.height), (600, 300));
}

#[test]
fn a_target_draws_that_layer_or_page() {
    let mut doc = basic();
    let (frame, rect) = ids(&doc);
    let (answer, layer, _) = render(&mut doc, json!({ "target": { "path": format!("layer:{rect}"), "page": 1 } }));
    assert_eq!((layer.width, layer.height), (160, 100));
    assert_eq!(answer["bounds"], json!({ "x": 40.0, "y": 40.0, "width": 160.0, "height": 100.0 }));
    assert_eq!(colours(&layer), BTreeSet::from([[0x56, 0x45, 0xD4, 255]]), "the rectangle fills its own preview");
    let (_, by_object, _) = render(&mut doc, json!({ "target": { "objectId": rect.to_string() } }));
    assert_eq!(by_object.rgba, layer.rgba);
    let (_, whole, _) = render(&mut doc, json!({ "target": { "path": format!("layer:{frame}") } }));
    let (_, page, _) = render(&mut doc, json!({ "target": { "page": 1 } }));
    let (_, numbered, _) = render(&mut doc, json!({ "page": 1 }));
    let (_, part, _) = render(&mut doc, json!({ "slide": 0 }));
    assert_eq!(whole.rgba, page.rgba);
    assert_eq!(page.rgba, numbered.rgba);
    assert_eq!(page.rgba, part.rgba);
}

#[test]
fn another_page_previews_without_switching_to_it() {
    let mut doc = basic();
    doc.execute("page.add", json!({ "name": "Second" })).unwrap();
    let current = doc.session().ds().unwrap().page;
    let second = doc.session().ds().unwrap().doc.pages[1];
    doc.execute("page.select", json!({ "id": doc.session().ds().unwrap().doc.pages[0] })).unwrap();
    let (answer, empty, _) = render(&mut doc, json!({ "page": 2 }));
    assert_eq!(answer["page"], 2);
    assert_eq!((empty.width, empty.height), (1920, 1080), "an empty page shows one view of its background");
    assert_eq!(colours(&empty).len(), 1);
    assert_ne!(doc.session().ds().unwrap().page, second);
    let _ = current;
}

#[test]
fn scale_and_size_budget() {
    let mut doc = basic();
    let (answer, image, _) = render(&mut doc, json!({ "scale": 2 }));
    assert_eq!((image.width, image.height, answer["scale"].as_f64().unwrap()), (800, 600, 2.0));
    let (answer, small, _) = render(&mut doc, json!({ "scale": 2, "maxSize": 200 }));
    assert_eq!((small.width, small.height, answer["scale"].as_f64().unwrap()), (200, 150, 0.5));
    for bad in [
        json!({ "scale": 3 }),
        json!({ "scale": 0 }),
        json!({ "scale": "big" }),
        json!({ "maxSize": 5000 }),
        json!({ "maxSize": 0 }),
        json!({ "page": 9 }),
        json!({ "page": 0 }),
        json!({ "slide": -1 }),
        json!({ "target": { "path": "style:S:1" } }),
        json!({ "target": { "path": "layer:999999" } }),
        json!({ "target": {} }),
        json!({ "target": "layer:1" }),
    ] {
        let error = doc.execute("file.render", bad.clone()).unwrap_err();
        assert!(error.starts_with("invalid-params:") || error.starts_with("not-found:"), "{bad}: {error}");
    }
}

#[test]
fn previews_change_nothing_and_are_listed_for_agents() {
    let mut doc = basic();
    let (revision, saved) = (doc.revision(), doc.save(None).unwrap());
    render(&mut doc, json!({ "scale": 2 }));
    assert_eq!((doc.revision(), doc.dirty()), (revision, false));
    assert_eq!(doc.save(None).unwrap(), saved);
    let command = doc.commands().into_iter().find(|c| c.id == "file.render").unwrap();
    assert!(!command.mutates && command.enabled && command.schema.is_some());
    let schema = kition_design_kit::agent::output();
    kition_kit::schema::validate_output(&schema).unwrap();
    let answer = doc.execute("file.render", json!({})).unwrap();
    let declared: BTreeSet<&String> = schema["properties"].as_object().unwrap().keys().collect();
    assert_eq!(declared, answer.as_object().unwrap().keys().collect::<BTreeSet<_>>());
}

/// What hosts draw headless for the whole document (`view.screenshot`: `render` of the document
/// frame, the page's content with a margin) shows the design too.
#[test]
fn the_whole_document_picture_is_real() {
    let doc = basic();
    let (w, h) = (doc.width(), doc.height());
    let scale = 512.0 / f64::from(w.max(h));
    let (pw, ph) = ((f64::from(w) * scale).round() as u32, (f64::from(h) * scale).round() as u32);
    let rgba = doc.render(0.0, 0.0, scale, pw, ph).unwrap();
    let distinct: BTreeSet<&[u8]> = rgba.chunks_exact(4).collect();
    assert!(distinct.len() > 4, "{} colours", distinct.len());
}

fn golden(name: &str) -> PathBuf {
    repo().join("conformance/golden").join(name)
}

/// Pixels differ by at most `maxChannelDelta` per channel, on at most `maxDifferentPixels`.
fn compare(name: &str, image: &Image, png: &[u8]) {
    if std::env::var_os("KITION_DESIGN_WRITE_GOLDEN").is_some() {
        std::fs::create_dir_all(golden("")).unwrap();
        std::fs::write(golden(name), png).unwrap();
        return;
    }
    let tolerance: Value = serde_json::from_str(&std::fs::read_to_string(golden("tolerance.json")).unwrap()).unwrap();
    let (delta, allowed) = (tolerance["maxChannelDelta"].as_u64().unwrap(), tolerance["maxDifferentPixels"].as_u64().unwrap());
    let expected = decode(&std::fs::read(golden(name)).unwrap());
    assert_eq!((image.width, image.height), (expected.width, expected.height), "{name}");
    let different = image
        .rgba
        .chunks_exact(4)
        .zip(expected.rgba.chunks_exact(4))
        .filter(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| u64::from(x.abs_diff(*y)) > delta))
        .count();
    assert!(different as u64 <= allowed, "{name}: {different} pixels differ");
}

#[test]
fn the_previews_match_the_goldens() {
    let mut doc = basic();
    let (_, rect) = ids(&doc);
    let (_, image, png) = render(&mut doc, json!({}));
    compare("basic.png", &image, &png);
    let (_, image, png) = render(&mut doc, json!({ "scale": 2 }));
    compare("basic-scale-2.png", &image, &png);
    let (_, image, png) = render(&mut doc, json!({ "target": { "path": format!("layer:{rect}") } }));
    compare("basic-target.png", &image, &png);
}

/// Review #21: a page number at the ends of the integer range is "no such page", never an overflow.
#[test]
fn extreme_page_numbers_are_not_found() {
    let mut doc = basic();
    for params in [json!({ "page": i64::MIN }), json!({ "target": { "page": i64::MIN } }), json!({ "slide": i64::MAX }), json!({ "page": 0 })] {
        let error = doc.execute("file.render", params.clone()).unwrap_err();
        assert!(error.starts_with("not-found:"), "{params}: {error}");
    }
}
