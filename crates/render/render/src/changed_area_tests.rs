//! [`crate::changed_area`]: a canvas that keeps its image and draws again only the area an edit
//! changed shows what a whole render would, after any run of edits (stage 4 of the hand plan).

use kition_design_color::{Color, Paint};
use kition_design_doc::{Document, Effect, EffectKind, NodeId, NodeKind, Shadow};
use kition_design_geom::Affine;
use tiny_skia::{IntRect, Pixmap, PixmapPaint, Transform};

use crate::retained_tests::assert_same;
use crate::{Options, View, changed_area, render_window};

fn solid(d: &mut Document, id: NodeId, hex: &str) {
    d.get_mut(id).unwrap().fills = vec![Paint::solid(Color::from_hex(hex).unwrap())];
}

/// Shapes on the page and in a clipping frame, a group with a drop shadow, an auto-sized parent.
fn scene() -> (Document, NodeId, Vec<NodeId>) {
    let mut d = Document::new("damage");
    let page = d.first_page().unwrap();
    let rect = d.create(page, NodeKind::Rectangle, "R", 20.0, 20.0, 80.0, 50.0).unwrap();
    solid(&mut d, rect, "E03030");
    let ell = d.create(page, NodeKind::Ellipse { start: 0.0, sweep: std::f64::consts::TAU, inner: 0.0 }, "E", 140.0, 30.0, 60.0, 60.0).unwrap();
    solid(&mut d, ell, "3070E0");
    let frame = d.create(page, NodeKind::Frame, "F", 40.0, 120.0, 200.0, 120.0).unwrap();
    solid(&mut d, frame, "FFFFFF");
    let child = d.create(frame, NodeKind::Rectangle, "C", 150.0, 20.0, 90.0, 40.0).unwrap();
    solid(&mut d, child, "20A060");
    let group = d.create(page, NodeKind::Group, "G", 280.0, 40.0, 60.0, 60.0).unwrap();
    let g1 = d.create(group, NodeKind::Rectangle, "G1", 0.0, 0.0, 60.0, 60.0).unwrap();
    solid(&mut d, g1, "F0B020");
    d.get_mut(group)
        .unwrap()
        .effects
        .push(Effect { kind: EffectKind::DropShadow(Shadow { offset: (12.0, 16.0), radius: 6.0, ..Shadow::default() }), visible: true });
    (d, page, vec![rect, ell, child, g1, frame])
}

/// `damage` (page coordinates) as `view`'s pixels, rounded out with a pixel of anti-aliasing, drawn
/// again into `image` from `doc`, as a canvas patches its kept frame.
fn patch(image: &mut Pixmap, doc: &Document, page: NodeId, view: &View, damage: kition_design_geom::Rect) {
    let s = view.scale;
    let px = |v: f64, o: f64| ((v - o) * s).clamp(0.0, 1e6);
    let x0 = (px(damage.x0, view.origin.0).floor() - 1.0).max(0.0) as u32;
    let y0 = (px(damage.y0, view.origin.1).floor() - 1.0).max(0.0) as u32;
    let x1 = ((px(damage.x1, view.origin.0).ceil() + 1.0) as u32).min(view.width);
    let y1 = ((px(damage.y1, view.origin.1).ceil() + 1.0) as u32).min(view.height);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let sub = View { origin: (view.origin.0 + f64::from(x0) / s, view.origin.1 + f64::from(y0) / s), scale: s, width: x1 - x0, height: y1 - y0 };
    let part = render_window(doc, page, &sub, &Options::default()).unwrap();
    let all = part.clone_rect(IntRect::from_xywh(0, 0, x1 - x0, y1 - y0).unwrap()).unwrap();
    let copy = PixmapPaint { blend_mode: tiny_skia::BlendMode::Source, ..Default::default() };
    image.draw_pixmap(x0 as i32, y0 as i32, all.as_ref(), &copy, Transform::identity(), None);
}

/// A run of edits (moves, resizes, recolours, opacity, a layer in a clipping frame, one under a
/// drop shadow) at 1:1, at 2× and zoomed out: each time, a whole render of the page before the
/// edit patched with the changed area is the whole render after it, as the retained renderer's
/// patches are ([`assert_same`]: a few levels of anti-aliasing where a curved edge crosses the
/// area's border); patched all along, the image never drifts far from it.
#[test]
fn a_kept_image_patched_with_the_changed_area_is_a_whole_render() {
    for scale in [1.0, 2.0, 0.5] {
        let (mut d, page, ids) = scene();
        let view = View { origin: (0.0, 0.0), scale, width: (400.0 * scale) as u32, height: (300.0 * scale) as u32 };
        let mut kept = render_window(&d, page, &view, &Options::default()).unwrap();
        for step in 0..24_u32 {
            let before = d.clone();
            let mut image = render_window(&d, page, &view, &Options::default()).unwrap();
            let id = ids[step as usize % ids.len()];
            let n = d.get_mut(id).unwrap();
            match step % 6 {
                0 => n.transform = Affine::translate((7.5, 4.25)) * n.transform,
                1 => n.width += 13.0,
                2 => n.fills = vec![Paint::solid(Color::from_hex(if step % 4 == 1 { "102030" } else { "C0C0F0" }).unwrap())],
                3 => n.transform *= Affine::translate((-11.0, 9.0)),
                4 => n.opacity = 0.5,
                _ => n.height -= 5.0,
            }
            let damage = changed_area(&before, &d, page, scale).expect("a layer edit is a partial repaint");
            if let Some(damage) = damage {
                patch(&mut image, &d, page, &view, damage);
                patch(&mut kept, &d, page, &view, damage);
            }
            let whole = render_window(&d, page, &view, &Options::default()).unwrap();
            assert_same(&image, &whole, &format!("scale {scale}, step {step}"));
            let drift = kept.data().iter().zip(whole.data()).filter(|(a, b)| a.abs_diff(**b) > 96).count();
            assert_eq!(drift, 0, "scale {scale}, step {step}: patched all along, the image drifted");
        }
    }
}

#[test]
fn nothing_changed_is_no_area_and_a_page_change_is_everything() {
    let (mut d, page, _) = scene();
    assert_eq!(changed_area(&d, &d.clone(), page, 1.0), Some(None));
    let before = d.clone();
    // A layer added on the page itself changes the page's children: only a whole render is right.
    d.create(page, NodeKind::Rectangle, "New", 300.0, 200.0, 20.0, 20.0).unwrap();
    assert_eq!(changed_area(&before, &d, page, 1.0), None);
    // Nothing on another page shows.
    let other = d.add_page("Other");
    let before = d.clone();
    d.create(other, NodeKind::Rectangle, "Elsewhere", 0.0, 0.0, 10.0, 10.0).unwrap();
    assert_eq!(changed_area(&before, &d, page, 1.0), Some(None));
}

#[test]
fn a_moved_layer_s_area_is_its_old_and_new_place() {
    let (mut d, page, ids) = scene();
    let before = d.clone();
    let rect = ids[0];
    let n = d.get_mut(rect).unwrap();
    n.transform = Affine::translate((30.0, 0.0)) * n.transform;
    let area = changed_area(&before, &d, page, 1.0).unwrap().unwrap();
    assert!(area.x0 <= 20.0 && area.x1 >= 130.0 && area.y0 <= 20.0 && area.y1 >= 70.0, "{area:?}");
    assert!(area.width() < 130.0 && area.height() < 70.0, "only the rectangle's places: {area:?}");
}
