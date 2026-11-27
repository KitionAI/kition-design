//! Stage 5 of making the design usable by hand: what a press, a drag and a double-click on the
//! canvas pick and do. Each test is the engine half of one walkthrough finding's regression case
//! (`cases/regressions/issue-hand-*`); `web/tests/hand/gestures.mjs` does the same with a real
//! mouse in the editor.

use serde_json::json;

use crate::Session;
use crate::components::tests::{ex, healthy, idv, kids, node};
use crate::tools::{PointerEvent, PointerKind};
use kition_design_doc::*;

fn ev(kind: PointerKind, x: f64, y: f64, alt: bool) -> PointerEvent {
    PointerEvent { alt, ..PointerEvent::at(kind, x, y, 1.0) }
}

fn bounds(s: &Session, id: NodeId) -> (f64, f64, f64, f64) {
    let b = s.doc().unwrap().world_bounds(id).unwrap();
    (b.x0, b.y0, b.width(), b.height())
}

fn page_layers(s: &Session) -> usize {
    kids(s, s.page().unwrap()).len()
}

/// A press at `from`, moves to `to` in `steps` with Alt as `alt(step)` says (0 is the press,
/// `steps + 1` the release).
fn drag_with(s: &mut Session, from: (f64, f64), to: (f64, f64), steps: u32, alt: impl Fn(u32) -> bool) {
    s.pointer(&ev(PointerKind::Down, from.0, from.1, alt(0))).unwrap();
    for i in 1..=steps {
        let t = f64::from(i) / f64::from(steps);
        s.pointer(&ev(PointerKind::Move, from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t, alt(i))).unwrap();
    }
    s.pointer(&ev(PointerKind::Up, to.0, to.1, alt(steps + 1))).unwrap();
}

/// regressions/issue-hand-a24-alt-during-drag: Alt pressed during a move drags a copy and leaves
/// the original; let go again, the original moves and no copy is left. The release decides.
#[test]
fn alt_during_a_drag_switches_between_copy_and_move() {
    let mut s = Session::new();
    let r = idv(&ex(&mut s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 100, "height": 80})));
    ex(&mut s, "edit.deselectAll", json!({}));
    let before = page_layers(&s);
    // Alt goes down halfway: a copy lands, the rectangle stays.
    drag_with(&mut s, (50.0, 40.0), (50.0, 240.0), 8, |i| i >= 4);
    assert_eq!(page_layers(&s), before + 1, "Alt pressed mid-drag makes a copy");
    assert_eq!(bounds(&s, r), (0.0, 0.0, 100.0, 80.0), "the original stays");
    let [copy] = s.selection()[..] else { panic!("the copy is selected: {:?}", s.selection()) };
    assert_ne!(copy, r);
    assert_eq!(bounds(&s, copy), (0.0, 200.0, 100.0, 80.0));
    assert_eq!(s.undo_labels(1), ["Duplicate"]);
    // Cmd+D repeats that step.
    ex(&mut s, "edit.duplicate", json!({}));
    let third = s.selection()[0];
    assert_eq!(bounds(&s, third), (0.0, 400.0, 100.0, 80.0), "Duplicate repeats the Alt-drag's offset");
    ex(&mut s, "edit.undo", json!({}));
    ex(&mut s, "edit.undo", json!({}));
    assert_eq!(page_layers(&s), before);
    // Pressed with Alt, let go halfway: the rectangle itself moves, and no copy is left.
    ex(&mut s, "edit.deselectAll", json!({}));
    drag_with(&mut s, (50.0, 40.0), (250.0, 40.0), 8, |i| i < 4);
    assert_eq!(page_layers(&s), before, "no copy once Alt is let go");
    assert_eq!(bounds(&s, r), (200.0, 0.0, 100.0, 80.0), "the original moved");
    assert_eq!(s.selection(), [r]);
    assert_eq!(s.undo_labels(1), ["Move"]);
    // Alt let go after the last move, before the release: the release has it, a move.
    ex(&mut s, "edit.undo", json!({}));
    drag_with(&mut s, (50.0, 40.0), (50.0, 140.0), 4, |i| i <= 4);
    assert_eq!(page_layers(&s), before, "released without Alt: a move");
    assert_eq!(bounds(&s, r), (0.0, 100.0, 100.0, 80.0));
    healthy(&s);
}

/// An auto layout frame `Row` at (0, 0) holding two 40 × 30 rectangles, padding 10, gap 10, fixed
/// 300 × 100; the first rectangle.
fn row(s: &mut Session) -> (NodeId, NodeId) {
    let f = idv(&ex(s, "frame.add", json!({"x": 0, "y": 0, "width": 300, "height": 100, "name": "Row"})));
    let a = idv(&ex(s, "shape.rectangle", json!({"x": 10, "y": 10, "width": 40, "height": 30, "parent": f})));
    ex(s, "shape.rectangle", json!({"x": 60, "y": 10, "width": 40, "height": 30, "parent": f}));
    ex(s, "layout.add", json!({"ids": [f], "direction": "horizontal"}));
    ex(s, "node.set", json!({"id": f, "props": {"padding": 10, "gap": 10, "sizingH": "fixed", "sizingV": "fixed", "width": 300, "height": 100}}));
    (f, a)
}

fn double_click(s: &mut Session, x: f64, y: f64, alt: bool) {
    for kind in [PointerKind::Down, PointerKind::Up, PointerKind::Down, PointerKind::Up, PointerKind::DoubleClick] {
        s.pointer(&ev(kind, x, y, alt)).unwrap();
    }
}

fn sizing(s: &Session, id: NodeId) -> (Sizing, Sizing) {
    let n = node(s, id);
    (n.child_layout.sizing_h, n.child_layout.sizing_v)
}

/// regressions/issue-hand-f-double-click-edge: a double-click on the right edge of a selected
/// auto layout frame makes it hug its width, on the bottom edge its height; Alt+double-click on
/// a child's edge makes it fill the frame on that axis. Each is one undo step.
#[test]
fn a_double_click_on_an_edge_hugs_and_with_alt_fills() {
    let mut s = Session::new();
    let (f, a) = row(&mut s);
    assert_eq!(sizing(&s, f), (Sizing::Fixed, Sizing::Fixed));
    ex(&mut s, "select.set", json!({"ids": [f]}));
    let undo = s.undo_labels(50).len();
    // The right edge, halfway down.
    double_click(&mut s, 300.0, 50.0, false);
    assert_eq!(sizing(&s, f).0, Sizing::Hug, "the right edge hugs the width");
    assert_eq!(sizing(&s, f).1, Sizing::Fixed, "the height stays");
    assert_eq!(bounds(&s, f).2, 110.0, "10 + 40 + 10 + 40 + 10");
    assert_eq!(s.undo_labels(50).len(), undo + 1, "one undo step");
    // The bottom edge.
    let w = bounds(&s, f).2;
    double_click(&mut s, w / 2.0, 100.0, false);
    assert_eq!(sizing(&s, f), (Sizing::Hug, Sizing::Hug));
    assert_eq!(bounds(&s, f).3, 50.0);
    // Alt on a child's right edge: it fills the width. The frame goes back to fixed first.
    ex(&mut s, "node.set", json!({"id": f, "props": {"sizingH": "fixed", "width": 300}}));
    ex(&mut s, "select.set", json!({"ids": [a]}));
    let (x, y, w, h) = bounds(&s, a);
    double_click(&mut s, x + w, y + h / 2.0, true);
    assert_eq!(sizing(&s, a).0, Sizing::Fill, "Alt+double-click on a child's edge fills");
    // Not on an edge, or not auto layout: the double-click does what it did.
    ex(&mut s, "edit.undo", json!({}));
    let plain = idv(&ex(&mut s, "frame.add", json!({"x": 0, "y": 300, "width": 100, "height": 100})));
    ex(&mut s, "select.set", json!({"ids": [plain]}));
    double_click(&mut s, 100.0, 350.0, false);
    assert_eq!(sizing(&s, plain), (Sizing::Fixed, Sizing::Fixed), "a frame without auto layout keeps its size");
    healthy(&s);
}

/// regressions/issue-hand-a22-press-in-selection: with a frame selected, a press on a layer in it
/// drags the frame and a click selects that layer; with two layers selected, a press on one drags
/// both and a click keeps just it.
#[test]
fn a_press_inside_the_selection_drags_it_and_a_click_narrows_it() {
    let mut s = Session::new();
    let f = idv(&ex(&mut s, "frame.add", json!({"x": 0, "y": 0, "width": 300, "height": 200})));
    ex(&mut s, "node.set", json!({"id": f, "props": {"fill": "#ffffff"}}));
    let a = idv(&ex(&mut s, "shape.rectangle", json!({"x": 20, "y": 20, "width": 60, "height": 40, "parent": f})));
    let b = idv(&ex(&mut s, "shape.rectangle", json!({"x": 120, "y": 20, "width": 60, "height": 40, "parent": f})));
    // The frame selected: a drag that starts on its child moves the frame.
    ex(&mut s, "select.set", json!({"ids": [f]}));
    drag_with(&mut s, (50.0, 40.0), (90.0, 40.0), 4, |_| false);
    assert_eq!(bounds(&s, f).0, 40.0, "the selected frame moved");
    assert_eq!(bounds(&s, a).0, 60.0, "its child went with it");
    assert_eq!(s.selection(), [f]);
    ex(&mut s, "edit.undo", json!({}));
    // A click there selects the child.
    s.pointer(&ev(PointerKind::Down, 50.0, 40.0, false)).unwrap();
    assert_eq!(s.selection(), [f], "the press keeps the frame for a drag");
    s.pointer(&ev(PointerKind::Up, 50.0, 40.0, false)).unwrap();
    assert_eq!(s.selection(), [a], "the click selects the layer in it");
    // Two selected: a drag on one moves both, a click keeps just it.
    ex(&mut s, "select.set", json!({"ids": [a, b]}));
    drag_with(&mut s, (150.0, 40.0), (150.0, 80.0), 4, |_| false);
    assert_eq!((bounds(&s, a).1, bounds(&s, b).1), (60.0, 60.0), "both moved");
    ex(&mut s, "edit.undo", json!({}));
    ex(&mut s, "select.set", json!({"ids": [a, b]}));
    s.pointer(&ev(PointerKind::Down, 150.0, 40.0, false)).unwrap();
    s.pointer(&ev(PointerKind::Up, 150.0, 40.0, false)).unwrap();
    assert_eq!(s.selection(), [b], "a click on one of the selected layers keeps just it");
    healthy(&s);
}

/// regressions/issue-hand-g9-alt-drag-variant: Alt+dragging a variant out of its set onto a screen
/// gives an instance of it there; the set, the variant and their sizes stay. Alt+dragged within
/// the set, the copy is a new variant.
#[test]
fn alt_dragging_a_variant_out_of_its_set_places_an_instance() {
    let mut s = Session::new();
    let screen = idv(&ex(&mut s, "frame.add", json!({"x": 0, "y": 0, "width": 375, "height": 812, "name": "Screen"})));
    ex(&mut s, "node.set", json!({"id": screen, "props": {"fill": "#ffffff"}}));
    ex(&mut s, "text.add", json!({"text": "Title", "x": 24, "y": 24, "parent": screen}));
    ex(&mut s, "layout.add", json!({"ids": [screen], "direction": "vertical"}));
    ex(
        &mut s,
        "node.set",
        json!({"id": screen, "props": {"padding": 24, "gap": 16, "sizingH": "fixed", "sizingV": "fixed", "width": 375, "height": 812}}),
    );
    let btn = idv(&ex(&mut s, "frame.add", json!({"x": 500, "y": 0, "width": 327, "height": 48, "name": "Button"})));
    ex(&mut s, "node.set", json!({"id": btn, "props": {"fill": "#2563eb"}}));
    ex(&mut s, "text.add", json!({"text": "Log in", "x": 130, "y": 14, "parent": btn}));
    let btn = idv(&ex(&mut s, "component.add", json!({"ids": [btn]})));
    ex(&mut s, "select.set", json!({"ids": [btn]}));
    ex(&mut s, "component.addVariant", json!({}));
    let set = node(&s, btn).parent.unwrap();
    let (set_box, btn_box) = (bounds(&s, set), bounds(&s, btn));
    ex(&mut s, "select.set", json!({"ids": [btn]}));
    let (x, y, ..) = btn_box;
    drag_with(&mut s, (x + 20.0, y + 8.0), (190.0, 420.0), 24, |_| true);
    let [copy] = s.selection()[..] else { panic!("one copy selected: {:?}", s.selection()) };
    let n = node(&s, copy);
    let NodeKind::Instance { component, .. } = n.kind else { panic!("out of the set, the copy is an instance") };
    assert_eq!(component, btn, "of the dragged variant");
    assert_eq!(n.parent, Some(screen), "in the screen the pointer is over");
    assert_eq!(bounds(&s, copy).2, 327.0, "at the variant's width");
    assert_eq!(kids(&s, set).len(), 2, "the set keeps its two variants");
    assert_eq!((bounds(&s, set), bounds(&s, btn)), (set_box, btn_box), "the set and the variant stay as they were");
    assert_eq!(s.undo_labels(1), ["Duplicate"]);
    // Within the set (the pointer in the gap between the variants): a new variant.
    ex(&mut s, "edit.undo", json!({}));
    ex(&mut s, "select.set", json!({"ids": [btn]}));
    drag_with(&mut s, (x + 20.0, y + 8.0), (x + 20.0, y + 58.0), 8, |_| true);
    let [variant] = s.selection()[..] else { panic!("one copy selected") };
    assert!(matches!(node(&s, variant).kind, NodeKind::Component { .. }));
    assert_eq!(node(&s, variant).parent, Some(set), "a copy dropped in the set is a variant of it");
    healthy(&s);
}
