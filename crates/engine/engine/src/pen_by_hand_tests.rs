//! The pen and vector edit mode as a person uses them with a mouse (plan 2026-11-26 stage 10,
//! walkthrough C7–C9): a double-click ends a path without doubling its last point, Enter ends a
//! path and edits its points with the Move tool (and Enter again leaves), undo while drawing takes
//! back one anchor at a time, a Bend click on a sharp point curves it, a click on another layer
//! leaves edit mode while a drag from there is a marquee, and a click on a point near a short
//! handle picks the point.

use kition_design_doc::{NodeId, NodeKind};
use serde_json::json;

use crate::Session;
use crate::tools::{PointerEvent, PointerKind, ToolId};
use crate::vectoredit::{VHit, VTool};

fn ev(kind: PointerKind, x: f64, y: f64) -> PointerEvent {
    PointerEvent::at(kind, x, y, 1.0)
}

fn click(s: &mut Session, x: f64, y: f64) {
    s.pointer(&ev(PointerKind::Down, x, y)).unwrap();
    s.pointer(&ev(PointerKind::Up, x, y)).unwrap();
}

/// A double-click as a browser gives it: two clicks, then the double-click.
fn double_click(s: &mut Session, x: f64, y: f64) {
    click(s, x, y);
    click(s, x, y);
    s.pointer(&ev(PointerKind::DoubleClick, x, y)).unwrap();
}

fn layers(s: &Session) -> usize {
    s.doc().unwrap().node(s.page().unwrap()).unwrap().children.len()
}

/// (points, segments) of vector `id`.
fn shape(s: &Session, id: NodeId) -> (usize, usize) {
    match &s.doc().unwrap().node(id).unwrap().kind {
        NodeKind::Vector { network } => (network.vertices.len(), network.segments.len()),
        k => panic!("not a vector: {k:?}"),
    }
}

fn edit_tool(s: &Session) -> Option<VTool> {
    s.ds().unwrap().vector_edit.as_ref().map(|ve| ve.tool)
}

fn undo(s: &mut Session) -> bool {
    s.execute("edit.undo", &json!({})).unwrap()["undone"] == true
}

#[test]
fn a_double_click_ends_the_path_without_doubling_its_last_point() {
    let mut s = Session::new();
    s.set_tool(ToolId::Pen);
    click(&mut s, 0.0, 0.0);
    click(&mut s, 100.0, 0.0);
    double_click(&mut s, 150.0, 50.0);
    let id = s.ds().unwrap().vector_edit.as_ref().map(|ve| ve.id).expect("the path's points are edited");
    assert!(s.tool.pen.is_none(), "the path ended");
    assert_eq!(shape(&s, id), (3, 2), "the second press of the double-click adds no point");
    assert_eq!(edit_tool(&s), Some(VTool::Move), "its points are edited with the Move tool");
    assert_eq!(layers(&s), 1);
}

#[test]
fn a_double_click_before_a_segment_edits_the_layer_under_it() {
    let mut s = Session::new();
    let r = s.execute("shape.rectangle", &json!({"x": 200, "y": 200, "width": 100, "height": 100})).unwrap();
    let rect = crate::params::id_of(&r["id"]).unwrap();
    s.set_selection(vec![]);
    s.set_tool(ToolId::Pen);
    double_click(&mut s, 250.0, 250.0);
    assert!(s.tool.pen.is_none(), "the one point the double-click began goes");
    assert_eq!(layers(&s), 1, "no new layer");
    let ve = s.ds().unwrap().vector_edit.clone().expect("the layer under the pointer is edited");
    assert_eq!((ve.id, ve.tool), (rect, VTool::Move));
}

#[test]
fn enter_ends_the_path_with_the_move_tool_and_enter_again_leaves() {
    let mut s = Session::new();
    s.set_tool(ToolId::Pen);
    click(&mut s, 0.0, 0.0);
    click(&mut s, 100.0, 0.0);
    let undo_depth = s.undo_depth().0;
    s.pen_enter().unwrap();
    let ve = s.ds().unwrap().vector_edit.clone().expect("still editing the path");
    assert_eq!((ve.tool, ve.anchor), (VTool::Move, None));
    assert_eq!(s.undo_depth().0, undo_depth + 1, "the path is one undo step");
    assert!(s.tool_key("Enter", false, false, false).unwrap());
    assert!(s.ds().unwrap().vector_edit.is_none(), "the next Enter leaves edit mode");
    assert_eq!(s.ds().unwrap().selection, vec![ve.id]);
}

#[test]
fn enter_with_the_pen_in_edit_mode_goes_to_the_move_tool() {
    let mut s = Session::new();
    let r = s.execute("vector.add", &json!({"points": [[0, 0], [100, 0]], "closed": false})).unwrap();
    let id = crate::params::id_of(&r["id"]).unwrap();
    s.execute("vector.edit.begin", &json!({"id": id})).unwrap();
    s.execute("vector.edit.tool", &json!({"tool": "pen"})).unwrap();
    click(&mut s, 100.0, 0.0);
    click(&mut s, 100.0, 100.0);
    assert!(s.tool_key("Enter", false, false, false).unwrap());
    let ve = s.ds().unwrap().vector_edit.clone().expect("still in edit mode");
    assert_eq!((ve.tool, ve.anchor), (VTool::Move, None), "the path ended, the Move tool edits its points");
    assert!(s.tool_key("Enter", false, false, false).unwrap());
    assert!(s.ds().unwrap().vector_edit.is_none());
}

#[test]
fn undo_while_drawing_takes_back_one_anchor_at_a_time() {
    let mut s = Session::new();
    s.execute("shape.rectangle", &json!({"x": 400, "y": 400, "width": 50, "height": 50})).unwrap();
    s.set_tool(ToolId::Pen);
    for (x, y) in [(0.0, 0.0), (100.0, 0.0), (100.0, 100.0)] {
        click(&mut s, x, y);
    }
    let points = |s: &Session| s.tool.pen.as_ref().map(|p| p.points.len());
    let id = s.tool.pen.as_ref().and_then(|p| p.id).unwrap();
    assert_eq!((points(&s), shape(&s, id), layers(&s)), (Some(3), (3, 2), 2));
    assert_eq!(s.pen_history(), (true, false));
    assert!(undo(&mut s));
    assert_eq!((points(&s), shape(&s, id), layers(&s)), (Some(2), (2, 1), 2), "only the last anchor goes");
    assert!(undo(&mut s));
    assert_eq!((points(&s), layers(&s)), (Some(1), 1), "one anchor left: its layer goes, the rectangle stays");
    assert_eq!(s.pen_history(), (true, true));
    s.execute("edit.redo", &json!({})).unwrap();
    s.execute("edit.redo", &json!({})).unwrap();
    let id = s.tool.pen.as_ref().and_then(|p| p.id).unwrap();
    assert_eq!((points(&s), shape(&s, id), layers(&s)), (Some(3), (3, 2), 2), "redo puts them back in order");
    // A new anchor drops what was taken back; the path still ends as one undo step.
    assert!(undo(&mut s));
    click(&mut s, 0.0, 100.0);
    assert_eq!(s.pen_history(), (true, false));
    s.pen_finish().unwrap();
    assert_eq!(layers(&s), 2);
    assert!(undo(&mut s));
    assert_eq!(layers(&s), 1, "one undo takes back the finished path");
    // Undoing the only anchor ends the path; the next undo is the document's.
    s.set_tool(ToolId::Pen);
    click(&mut s, 0.0, 0.0);
    assert!(undo(&mut s));
    assert!(s.tool.pen.is_none());
    assert!(undo(&mut s));
    assert_eq!(layers(&s), 0, "then the rectangle");
}

#[test]
fn a_pen_double_click_in_edit_mode_adds_nothing_and_goes_to_the_move_tool() {
    let mut s = Session::new();
    let r = s.execute("vector.add", &json!({"points": [[0, 0], [100, 0]], "closed": false})).unwrap();
    let id = crate::params::id_of(&r["id"]).unwrap();
    s.execute("vector.edit.begin", &json!({"id": id})).unwrap();
    s.execute("vector.edit.tool", &json!({"tool": "pen"})).unwrap();
    double_click(&mut s, 300.0, 300.0);
    assert_eq!(shape(&s, id), (2, 1), "the new path's first point is taken back");
    assert_eq!(edit_tool(&s), Some(VTool::Move));
}

#[test]
fn a_bend_click_on_a_sharp_point_curves_it() {
    let mut s = Session::new();
    let r = s.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 100, "height": 100})).unwrap();
    let id = crate::params::id_of(&r["id"]).unwrap();
    s.execute("vector.edit.begin", &json!({"id": id})).unwrap();
    s.execute("vector.edit.tool", &json!({"tool": "bend"})).unwrap();
    let undo_depth = s.undo_depth().0;
    click(&mut s, 100.0, 0.0);
    let net = crate::vectoredit::network(s.doc().unwrap(), id).unwrap().clone();
    let w = s.doc().unwrap().world_transform(id);
    let v = net.vertices.iter().position(|p| (w * p.point() - kition_design_geom::Point::new(100.0, 0.0)).hypot() < 1e-6).unwrap();
    assert!(net.incident(v).iter().all(|(sg, st)| net.handle(*sg, *st).is_some_and(|h| h != (0.0, 0.0))), "the corner got handles");
    assert_eq!(s.undo_depth().0, undo_depth + 1, "one undo step");
    // Another click sharpens it again.
    click(&mut s, 100.0, 0.0);
    let net = crate::vectoredit::network(s.doc().unwrap(), id).unwrap().clone();
    assert!(net.incident(v).iter().all(|(sg, st)| net.handle(*sg, *st) == Some((0.0, 0.0))));
}

#[test]
fn a_click_on_another_layer_leaves_and_a_drag_from_there_marquees() {
    let mut s = Session::new();
    let a = crate::params::id_of(&s.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 100, "height": 100})).unwrap()["id"]).unwrap();
    let b = crate::params::id_of(&s.execute("shape.rectangle", &json!({"x": 300, "y": 0, "width": 100, "height": 100})).unwrap()["id"]).unwrap();
    s.execute("vector.edit.begin", &json!({"id": a})).unwrap();
    // A drag that starts on the other layer selects the edited layer's points it covers.
    s.pointer(&ev(PointerKind::Down, 350.0, 50.0)).unwrap();
    s.pointer(&ev(PointerKind::Move, 200.0, 50.0)).unwrap();
    s.pointer(&ev(PointerKind::Move, 50.0, 150.0)).unwrap();
    s.pointer(&ev(PointerKind::Up, 50.0, 150.0)).unwrap();
    let ve = s.ds().unwrap().vector_edit.clone().expect("a drag stays in edit mode");
    assert_eq!(ve.id, a);
    assert_eq!(ve.vertices.len(), 1, "the marquee took the point it covers (100, 100): {:?}", ve.vertices);
    // A click there leaves edit mode and selects that layer.
    click(&mut s, 350.0, 50.0);
    assert!(s.ds().unwrap().vector_edit.is_none());
    assert_eq!(s.ds().unwrap().selection, vec![b]);
}

#[test]
fn a_click_on_a_point_beside_a_short_handle_picks_the_point() {
    let mut s = Session::new();
    let r = s.execute("vector.add", &json!({"points": [[0, 0], [100, 0]], "closed": false})).unwrap();
    let id = crate::params::id_of(&r["id"]).unwrap();
    s.execute("vector.edit.begin", &json!({"id": id})).unwrap();
    // A handle 3 units long on the first point, which is selected (so its handles show).
    s.execute("vector.edit.setHandle", &json!({"segment": 0, "start": true, "x": 3, "y": 0})).unwrap();
    s.execute("vector.edit.select", &json!({"vertices": [0]})).unwrap();
    let doc = s.doc().unwrap().clone();
    let ve = s.ds().unwrap().vector_edit.clone().unwrap();
    let w = doc.world_transform(id);
    let p0 = w * kition_design_geom::Point::new(0.0, 0.0);
    assert_eq!(crate::vectoredit::hit(&doc, &ve, p0, 6.0), Some(VHit::Vertex { index: 0 }), "on the point: the point");
    let end = w * kition_design_geom::Point::new(3.0, 0.0);
    assert_eq!(crate::vectoredit::hit(&doc, &ve, end, 6.0), Some(VHit::Handle { seg: 0, start: true }), "on the handle's end: the handle");
}
