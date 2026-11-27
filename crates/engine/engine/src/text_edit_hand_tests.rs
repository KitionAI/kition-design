//! Typing a new text by hand (the hand walkthrough's D6, D8, D10 and D12): a text made with the
//! Text tool and typed into is one undo step with the layer, Select all, Copy, Cut and Paste act on
//! the characters while it is being edited, and an unnamed text takes its name from its first line.

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

fn rect(s: &mut Session, x: f64) -> NodeId {
    crate::params::id_of(&s.execute("shape.rectangle", &json!({"x": x, "y": 400, "width": 40, "height": 40})).unwrap()["id"]).unwrap()
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
fn select_all_while_typing_a_new_text_selects_its_characters() {
    // D6 (case regressions/issue-hand-d6-select-all-in-text): Cmd+A while typing a new text
    // selected every layer on the page, and Cmd+B then bolded them all.
    let mut s = Session::new();
    rect(&mut s, 0.0);
    rect(&mut s, 100.0);
    let t = type_new_text(&mut s, 100.0, 100.0, "Hello world");
    s.execute("edit.selectAll", &json!({})).unwrap();
    assert_eq!(s.selection(), vec![t], "the layer selection stays the text");
    let te = s.ds().unwrap().text_edit.clone().unwrap();
    assert_eq!((te.anchor.min(te.caret), te.anchor.max(te.caret)), (0, 11), "all the characters are selected");
    s.execute("format.bold", &json!({})).unwrap();
    assert_eq!(s.selection(), vec![t]);
    // Not editing, Select all still selects the layers.
    crate::textedit::key(&mut s, "Escape", false, false, false).unwrap();
    s.execute("edit.selectAll", &json!({})).unwrap();
    assert_eq!(s.selection().len(), 3);
}

#[test]
fn copy_and_paste_while_editing_act_on_the_characters() {
    // D8 (case regressions/issue-hand-d8-text-clipboard): a word selected, Cmd+C, End, Cmd+V left
    // the text as it was and pasted a second text layer over it.
    let mut s = Session::new();
    let t = type_new_text(&mut s, 100.0, 100.0, "Hello world");
    crate::textedit::set_caret(&mut s, 6, false).unwrap();
    crate::textedit::set_caret(&mut s, 11, true).unwrap();
    let before = layers(&s);
    let copied = s.execute("edit.copy", &json!({})).unwrap();
    assert_eq!(copied["copiedText"], 5);
    crate::textedit::key(&mut s, "End", false, false, false).unwrap();
    s.execute("edit.paste", &json!({})).unwrap();
    assert_eq!(chars(&s, t), "Hello worldworld", "the copied word is typed at the caret");
    assert_eq!(layers(&s), before, "no layer is pasted");
    // Cut takes the selected characters away; pasting types them back.
    crate::textedit::set_caret(&mut s, 0, false).unwrap();
    crate::textedit::set_caret(&mut s, 6, true).unwrap();
    s.execute("edit.cut", &json!({})).unwrap();
    assert_eq!(chars(&s, t), "worldworld");
    assert_eq!(layers(&s), before, "cut deletes characters, not the layer");
    crate::textedit::key(&mut s, "End", false, false, false).unwrap();
    s.execute("edit.paste", &json!({})).unwrap();
    assert_eq!(chars(&s, t), "worldworldHello ");
    // With nothing selected, copy keeps what was copied.
    crate::textedit::set_caret(&mut s, 2, false).unwrap();
    s.execute("edit.copy", &json!({})).unwrap();
    crate::textedit::key(&mut s, "End", false, false, false).unwrap();
    s.execute("edit.paste", &json!({})).unwrap();
    assert_eq!(chars(&s, t), "worldworldHello Hello ");
    // The whole session is still one undo step, which takes the new layer away.
    crate::textedit::key(&mut s, "Escape", false, false, false).unwrap();
    assert!(s.undo().unwrap());
    assert!(s.doc().unwrap().node(t).is_none());
}

#[test]
fn copied_text_layers_paste_as_their_characters_while_editing() {
    // Copying a text layer and pasting into another text being edited types its characters.
    let mut s = Session::new();
    let a = type_new_text(&mut s, 100.0, 100.0, "First");
    crate::textedit::key(&mut s, "Escape", false, false, false).unwrap();
    s.execute("edit.copy", &json!({"ids": [a]})).unwrap();
    let b = type_new_text(&mut s, 100.0, 300.0, "Second ");
    let before = layers(&s);
    s.execute("edit.paste", &json!({})).unwrap();
    assert_eq!(chars(&s, b), "Second First");
    assert_eq!(layers(&s), before);
    // Copied layers that are not all texts have no text to type: the paste changes nothing.
    crate::textedit::key(&mut s, "Escape", false, false, false).unwrap();
    let r = rect(&mut s, 0.0);
    s.execute("edit.copy", &json!({"ids": [r, a]})).unwrap();
    crate::textedit::begin(&mut s, b, Some(0), false).unwrap();
    s.execute("edit.paste", &json!({})).unwrap();
    assert_eq!(chars(&s, b), "Second First");
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
}
