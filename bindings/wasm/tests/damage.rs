//! Stage 4 of making the design usable by hand (M1, M2; gap R2 item 5): a tool's answer names the
//! area its event changed (`damage`), so the canvas repaints that area of its cached frame instead
//! of the whole view, and a zoomed-out frame is drawn straight at its scale, not at 1:1 and
//! reduced. A frame patched with each answer's area shows what a whole render of it shows.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;
use kition_kit::tools::{KitTools, PointerAction, ToolDamage, ToolMods, ToolPointer};
use serde_json::json;

/// A 400 × 300 design whose frame's top-left is document pixel (1000, 1000): a 100 × 80 rectangle
/// at (20, 20), an ellipse with a drop shadow at (200, 40) and a rotated rectangle at (60, 160).
fn design() -> DesignDocument {
    let mut doc = DesignDocument::create("Design.kdesign", 400, 300).unwrap();
    let rows = doc.query("layers", &json!({})).unwrap();
    let frame = rows["rows"].as_array().unwrap().iter().find(|r| r["type"] == "FRAME").unwrap()["id"].clone();
    let fill = |hex: &str| json!({ "fills": [{ "type": "solid", "color": hex }] });
    doc.execute("shape.rectangle", json!({ "x": 20, "y": 20, "width": 100, "height": 80, "parent": frame, "props": fill("#3070E0") })).unwrap();
    doc.execute("shape.ellipse", json!({ "x": 200, "y": 40, "width": 90, "height": 60, "parent": frame, "props": fill("#E03030") })).unwrap();
    doc.execute("effect.add", json!({ "type": "dropShadow" })).unwrap();
    doc.execute("shape.rectangle", json!({ "x": 60, "y": 160, "width": 120, "height": 50, "parent": frame, "props": fill("#20A060") })).unwrap();
    doc.execute("transform.rotate", json!({ "degrees": 30 })).unwrap();
    doc.execute("edit.deselectAll", json!({})).unwrap();
    doc.mark_saved();
    doc
}

fn event(action: PointerAction, x: f64, y: f64, zoom: f64) -> ToolPointer {
    ToolPointer { action, x, y, mods: ToolMods::default(), pressure: 1.0, zoom }
}

/// How far a patched frame is from a whole render: channels more than 2 levels off, and more than
/// 96 levels off (the engine's retained renderer's measure, `retained_tests::assert_same`).
fn distance(a: &[u8], b: &[u8]) -> (usize, usize) {
    a.iter().zip(b).fold((0, 0), |(slight, big), (x, y)| (slight + usize::from(x.abs_diff(*y) > 2), big + usize::from(x.abs_diff(*y) > 96)))
}

/// A canvas's cached frame: document area (`x`, `y`) on, `width` × `height` pixels of `k` document
/// pixels each (k = 1: at 1:1; k = 2: at 50 %), drawn and patched as kition-sdk's raster viewport
/// draws and patches it.
struct Frame {
    x: i32,
    y: i32,
    k: i32,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl Frame {
    fn render(doc: &DesignDocument, x: i32, y: i32, k: i32, width: u32, height: u32) -> Self {
        Self { x, y, k, width, height, rgba: Self::pixels(doc, x, y, k, width, height) }
    }

    fn pixels(doc: &DesignDocument, x: i32, y: i32, k: i32, width: u32, height: u32) -> Vec<u8> {
        if k == 1 {
            doc.render(f64::from(x), f64::from(y), 1.0, width, height).unwrap()
        } else {
            doc.render_area(x, y, x + width as i32 * k, y + height as i32 * k, width, height).unwrap()
        }
    }

    fn whole(&self, doc: &DesignDocument) -> Vec<u8> {
        Self::pixels(doc, self.x, self.y, self.k, self.width, self.height)
    }

    /// Draws `damage` again (`RasterViewport.renderArea`: the frame pixels it touches, rounded out);
    /// the number of pixels drawn.
    fn patch(&mut self, doc: &DesignDocument, damage: &ToolDamage) -> u64 {
        let k = f64::from(self.k);
        let px = |v: f64, origin: i32, limit: u32| ((v - f64::from(origin)) / k).clamp(0.0, f64::from(limit));
        let (px0, py0) = (px(damage.x0, self.x, self.width).floor() as u32, px(damage.y0, self.y, self.height).floor() as u32);
        let (px1, py1) = (px(damage.x1, self.x, self.width).ceil() as u32, px(damage.y1, self.y, self.height).ceil() as u32);
        if px1 <= px0 || py1 <= py0 {
            return 0;
        }
        let (w, h) = (px1 - px0, py1 - py0);
        let part = Self::pixels(doc, self.x + px0 as i32 * self.k, self.y + py0 as i32 * self.k, self.k, w, h);
        for row in 0..h {
            let from = (row * w * 4) as usize;
            let to = (((py0 + row) * self.width + px0) * 4) as usize;
            self.rgba[to..to + (w * 4) as usize].copy_from_slice(&part[from..from + (w * 4) as usize]);
        }
        u64::from(w) * u64::from(h)
    }
}

/// Drags the rectangle in `steps` moves at `zoom` over a frame at (`x`, `y`) of `k` document
/// pixels a pixel: each event's area patches a whole render of the frame before it, and the result
/// is the whole render after it as the engine's retained renderer measures it (a few levels of
/// anti-aliasing at most where a curved edge crosses the area's border); patched all along, the
/// frame never drifts far from it. The pixels each patch drew.
fn drag_patched(doc: &mut DesignDocument, at: (i32, i32, i32), size: (u32, u32), zoom: f64, steps: u32) -> Vec<u64> {
    let (x, y, k) = at;
    let mut kept = Frame::render(doc, x, y, k, size.0, size.1);
    let mut drawn = Vec::new();
    let mut moves = vec![(PointerAction::Down, 1060.0, 1060.0)];
    moves.extend((1..=steps).map(|i| (PointerAction::Drag, 1060.0 + f64::from(i) * 9.5, 1060.0 + f64::from(i) * 5.7)));
    moves.push((PointerAction::Up, 1060.0 + f64::from(steps) * 9.5, 1060.0 + f64::from(steps) * 5.7));
    for (i, (action, px, py)) in moves.into_iter().enumerate() {
        let mut fresh = Frame::render(doc, x, y, k, size.0, size.1);
        let revision = doc.revision();
        doc.pointer(&event(action, px, py, zoom)).unwrap();
        let damage = doc.take_damage();
        if doc.revision() == revision {
            continue;
        }
        let damage = damage.unwrap_or_else(|| panic!("event {i} ({action:?}) changed the document and names no area"));
        drawn.push(fresh.patch(doc, &damage));
        kept.patch(doc, &damage);
        let whole = fresh.whole(doc);
        let (slight, big) = distance(&fresh.rgba, &whole);
        assert!(
            slight <= 200 && big == 0,
            "event {i} ({action:?}) at 1/{k}: the patched frame is {slight} channels off by more than 2, {big} by more than 96"
        );
        let (_, drift) = distance(&kept.rgba, &whole);
        assert_eq!(drift, 0, "event {i}: patched all along, the frame drifted from a whole render");
    }
    drawn
}

#[test]
fn a_drag_names_the_area_it_changed_and_the_patched_frame_is_a_whole_render() {
    let mut doc = design();
    let drawn = drag_patched(&mut doc, (900, 900, 1), (600, 500), 1.0, 12);
    assert!(drawn.len() >= 12, "every move changed the document: {drawn:?}");
    // A 100 × 80 rectangle moved about 10 × 6 pixels a step: its old and new places, not the view.
    assert!(drawn.iter().skip(1).all(|n| *n <= 600 * 500 / 10), "each move repaints a small area: {drawn:?}");
    let id = doc.inspect().unwrap()["selection"][0].clone();
    let x = doc.query("node", &json!({ "id": id })).unwrap()["x"].as_f64().unwrap();
    assert!((x - 134.0).abs() < 1.5, "the rectangle moved: x {x}");
}

#[test]
fn a_zoomed_out_frame_is_patched_on_its_own_pixel_grid() {
    let mut doc = design();
    // 50 %: one frame pixel is 2 × 2 document pixels; the frame starts on an odd document pixel.
    let drawn = drag_patched(&mut doc, (901, 899, 2), (300, 250), 0.5, 8);
    assert!(drawn.iter().skip(1).all(|n| *n <= 300 * 250 / 10), "{drawn:?}");
}

#[test]
fn a_zoomed_out_area_is_drawn_straight_at_its_scale() {
    let doc = design();
    let ds = doc.session().ds().unwrap();
    let area = doc.area();
    // The canvas's whole-number reductions are the page drawn at that scale (with the canvas's
    // level of detail), not drawn at 1:1 and averaged: k² times fewer pixels drawn.
    for k in [2_i32, 3, 4] {
        let (w, h) = (120_u32, 90_u32);
        let reduced = doc.render_area(940, 960, 940 + w as i32 * k, 960 + h as i32 * k, w, h).unwrap();
        let scale = 1.0 / f64::from(k);
        let view = kition_design_render::View { origin: (area.x + 940.0, area.y + 960.0), scale, width: w, height: h };
        let options = kition_design_render::Options { lod: true, ..kition_design_render::Options::default() };
        let straight = kition_design_render::to_rgba8(&kition_design_render::render_window(&ds.doc, ds.page, &view, &options).unwrap());
        assert_eq!(reduced, straight, "1/{k}");
        assert_eq!(reduced, doc.render(940.0, 960.0, scale, w, h).unwrap(), "render at 1/{k} is the same");
    }
}

#[test]
fn a_click_that_only_selects_repaints_next_to_nothing_and_a_nudge_its_layer() {
    let mut doc = design();
    let revision = doc.revision();
    doc.pointer(&event(PointerAction::Down, 1060.0, 1060.0, 1.0)).unwrap();
    doc.take_damage();
    doc.pointer(&event(PointerAction::Up, 1060.0, 1060.0, 1.0)).unwrap();
    let after_click = doc.take_damage();
    assert_ne!(doc.revision(), revision, "the click selected the rectangle");
    if let Some(d) = after_click {
        assert!((d.x1 - d.x0) * (d.y1 - d.y0) <= 1.0, "nothing to repaint for a selection: {d:?}");
    }
    let mut frame = Frame::render(&doc, 900, 900, 1, 600, 500);
    doc.key("ArrowRight", ToolMods { shift: true, ..ToolMods::default() }, 1.0).unwrap();
    let nudge = doc.take_damage().expect("a nudge names its area");
    assert!((nudge.x1 - nudge.x0) <= 100.0 + 10.0 + 8.0 && (nudge.y1 - nudge.y0) <= 80.0 + 8.0, "{nudge:?}");
    frame.patch(&doc, &nudge);
    let (slight, big) = distance(&frame.rgba, &frame.whole(&doc));
    assert!(slight <= 200 && big == 0, "after a nudge: {slight} / {big}");
    // A hover changes no pixel and names nothing.
    doc.pointer(&event(PointerAction::Move, 1300.0, 1100.0, 1.0)).unwrap();
    assert_eq!(doc.take_damage(), None);
}

#[test]
fn outline_mode_repaints_everything() {
    let mut doc = design();
    doc.pointer(&event(PointerAction::Down, 1060.0, 1060.0, 1.0)).unwrap();
    doc.pointer(&event(PointerAction::Up, 1060.0, 1060.0, 1.0)).unwrap();
    doc.take_damage();
    doc.execute("view.outlines", json!({})).unwrap();
    // Outlines are hairlines whose anti-aliasing a patch cannot match: every edit is a whole repaint.
    doc.key("ArrowRight", ToolMods::default(), 1.0).unwrap();
    assert_eq!(doc.take_damage(), None, "outline mode");
    doc.execute("view.outlines", json!({})).unwrap();
    doc.key("ArrowRight", ToolMods::default(), 1.0).unwrap();
    assert!(doc.take_damage().is_some(), "outline mode off: a nudge names its area again");
}
