//! `image.place {target, index}` turns one fill into an image in place, without a file (the
//! editor's Image paint type and image fill swap send the bytes the person picked).

use base64::Engine as _;
use kition_design_color::{ImageScaleMode, PaintKind};
use kition_design_doc::NodeId;
use serde_json::{Value, json};

use crate::Session;

fn png(w: u32, h: u32) -> String {
    let img = image::RgbaImage::from_pixel(w, h, image::Rgba([10, 200, 30, 255]));
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(img).write_to(&mut out, image::ImageFormat::Png).unwrap();
    base64::engine::general_purpose::STANDARD.encode(out.into_inner())
}

fn ex(s: &mut Session, cmd: &str, p: Value) -> Value {
    s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"))
}

#[test]
fn an_image_replaces_the_fill_at_index_in_place() {
    let mut s = Session::new();
    let r = NodeId(ex(&mut s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 40, "height": 20}))["id"].as_u64().unwrap());
    ex(&mut s, "fill.add", json!({"ids": [r], "color": "#0000FF"}));
    let count = s.doc().unwrap().node(r).unwrap().fills.len();
    assert!(count >= 2);
    ex(&mut s, "fill.set", json!({"ids": [r], "index": 0, "opacity": 0.5}));
    let first = ex(&mut s, "image.place", json!({"data": png(4, 2), "target": r, "index": 0, "mode": "tile"}));
    let n = s.doc().unwrap().node(r).unwrap().clone();
    assert_eq!(n.fills.len(), count, "no fill added");
    let PaintKind::Image(img) = &n.fills[0].kind else { panic!("fill 0 is an image now: {:?}", n.fills[0]) };
    assert_eq!(img.image, first["image"].as_str().unwrap());
    assert_eq!(img.scale_mode, ImageScaleMode::Tile);
    assert!((n.fills[0].opacity - 0.5).abs() < 1e-6, "the fill keeps its opacity");

    // An image fill keeps its scale mode and adjustments; only the picture changes.
    let mode = ImageScaleMode::Tile;
    let second = ex(&mut s, "image.place", json!({"data": png(3, 3), "target": r, "index": 0}));
    assert_ne!(first["image"], second["image"]);
    let n = s.doc().unwrap().node(r).unwrap().clone();
    match &n.fills[0].kind {
        PaintKind::Image(img) => {
            assert_eq!(img.image, second["image"].as_str().unwrap());
            assert_eq!(img.scale_mode, mode);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(n.fills.len(), count);

    // One undo step each.
    ex(&mut s, "edit.undo", json!({}));
    match &s.doc().unwrap().node(r).unwrap().fills[0].kind {
        PaintKind::Image(img) => assert_eq!(img.image, first["image"].as_str().unwrap()),
        other => panic!("{other:?}"),
    }

    // Bad indexes change nothing.
    let before = s.doc().unwrap().node(r).unwrap().fills.clone();
    for index in [json!(9), json!(-1), json!(0.5)] {
        assert!(s.execute("image.place", &json!({"data": png(1, 1), "target": r, "index": index})).is_err(), "{index}");
    }
    assert!(s.execute("image.place", &json!({"data": png(1, 1), "index": 0})).is_err(), "index needs a target");
    assert_eq!(s.doc().unwrap().node(r).unwrap().fills, before);
}
