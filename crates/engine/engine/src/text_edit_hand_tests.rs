//! Typing a new text by hand (the hand walkthrough's D10 and D12): a text made with the Text tool
//! and typed into is one undo step with the layer, and an unnamed text takes its name from its
//! first line.

use serde_json::json;

use crate::Session;
use crate::tools::{PointerEvent, PointerKind, ToolId};
use kition_design_doc::NodeId;

/// A click with the Text tool at (`x`, `y`) and `typed` typed into the new text, still editing.
fn type_new_text(s: &mut Session, x: f64, y: f64, typed: &str) -> NodeId {
    s.set_tool(ToolId::Text);
    s.pointer(&PointerEvent::at(PointerKind::Down, x, y, 1.0)).unwrap();
    s.pointer(&PointerEvent::at(PointerKind::Up, x, y, 1.0)).unwrap();
    crate::textedit::insert(s, typed).unwrap();
    s.ds().unwrap().text_edit.as_ref().unwrap().id
}

fn chars(s: &Session, t: NodeId) -> String {
    s.doc().unwrap().node(t).unwrap().kind.text().unwrap().characters.clone()
}

fn layers(s: &Session) -> usize {
    s.doc().unwrap().nodes().count()
}

#[test]
fn undoing_a_new_text_takes_the_layer_away() {
    // D10 (case regressions/issue-hand-d10-empty-text-undo): T, a click, "Hello", Escape, Cmd+Z
    // left an empty text layer 1 px wide and nothing more to undo.
    let mut s = Session::new();
    let before = layers(&s);
    let (undo_before, _) = s.undo_depth();
    let t = type_new_text(&mut s, 100.0, 100.0, "Hello");
    crate::textedit::key(&mut s, "Escape", false, false, false).unwrap();
    assert_eq!(chars(&s, t), "Hello");
    assert_eq!(s.undo_depth().0, undo_before + 1, "the new text and its typing are one step");
    assert!(s.undo().unwrap());
    assert!(s.doc().unwrap().node(t).is_none(), "undo takes the new layer away");
    assert_eq!(layers(&s), before);
    assert_eq!(s.undo_depth().0, undo_before, "nothing of the text is left to undo");
    // Redo brings the text back with what was typed.
    assert!(s.redo().unwrap());
    assert_eq!(chars(&s, t), "Hello");
    // A click with nothing typed still leaves no layer and no step.
    let (depth, _) = s.undo_depth();
    let empty = type_new_text(&mut s, 300.0, 100.0, "");
    crate::textedit::key(&mut s, "Escape", false, false, false).unwrap();
    assert!(s.doc().unwrap().node(empty).is_none());
    assert_eq!(s.undo_depth().0, depth);
}

#[test]
fn an_unnamed_text_takes_its_first_line_as_its_name() {
    // D12: a new text's layer was called "" and showed as "Text" whatever it said.
    let mut s = Session::new();
    let t = type_new_text(&mut s, 100.0, 100.0, "Sign in\nto continue");
    let name = |s: &Session| s.doc().unwrap().node(t).unwrap().name.clone();
    assert_eq!(name(&s), "Sign in", "the first line");
    crate::textedit::key(&mut s, "Home", false, false, true).unwrap();
    crate::textedit::insert(&mut s, "Please ").unwrap();
    assert_eq!(name(&s), "Please Sign in", "the name follows the typing");
    // At most 64 characters.
    crate::textedit::insert(&mut s, &"x".repeat(80)).unwrap();
    assert_eq!(name(&s).chars().count(), 64);
    crate::textedit::key(&mut s, "Escape", false, false, false).unwrap();
    // A name the author chose stays.
    s.execute("object.rename", &json!({"ids": [t], "name": "Title"})).unwrap();
    crate::textedit::begin(&mut s, t, Some(0), false).unwrap();
    crate::textedit::insert(&mut s, "A").unwrap();
    assert_eq!(name(&s), "Title");
    // An edit that leaves the first line as it was leaves a layer called `Text` alone.
    let other = crate::params::id_of(&s.execute("node.add", &json!({"type": "text", "props": {"characters": "a"}})).unwrap()["id"]).unwrap();
    let before = s.doc().unwrap().node(other).unwrap().name.clone();
    crate::textedit::begin(&mut s, other, Some(1), false).unwrap();
    crate::textedit::key(&mut s, "Enter", false, false, false).unwrap();
    assert_eq!(s.doc().unwrap().node(other).unwrap().name, before);
}
