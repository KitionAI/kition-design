//! Stage 6 of making the design usable by hand (Chinese input, walkthrough D9,
//! regressions/issue-hand-d9-ime-composition): what an input method composes shows at the caret
//! without changing the document, the `textCaret` query says where the editor's text field goes
//! (the candidate window opens there), and the text keys move as the reported platform's do.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;
use kition_kit::tools::{KitTools, ToolMods};
use serde_json::{Value, json};

/// A design with a text "one two" in its frame, being edited with the caret at `caret`.
fn editing(caret: usize) -> (DesignDocument, Value) {
    let mut doc = DesignDocument::create("Design.kdesign", 400, 300).unwrap();
    let rows = doc.query("layers", &json!({})).unwrap();
    let frame = rows["rows"].as_array().unwrap().iter().find(|r| r["type"] == "FRAME").unwrap()["id"].clone();
    doc.execute("text.add", json!({ "text": "one two", "x": 40, "y": 40, "parent": frame, "props": { "fontSize": 16 } })).unwrap();
    let id = doc.inspect().unwrap()["selection"][0].clone();
    doc.execute("text.edit.begin", json!({ "id": id, "caret": caret })).unwrap();
    doc.mark_saved();
    (doc, id)
}

fn characters(doc: &DesignDocument, id: &Value) -> String {
    doc.query("node", &json!({ "id": id })).unwrap()["characters"].as_str().unwrap().to_owned()
}

fn caret(doc: &DesignDocument) -> (f64, f64, f64, f64) {
    let c = doc.query("textCaret", &json!({})).unwrap();
    (c["x0"].as_f64().unwrap(), c["y0"].as_f64().unwrap(), c["x1"].as_f64().unwrap(), c["y1"].as_f64().unwrap())
}

fn undo_depth(doc: &DesignDocument) -> usize {
    doc.query("history", &json!({})).map(|h| h["undo"].as_array().map_or(0, Vec::len)).unwrap_or(0)
}

#[test]
fn a_composition_shows_at_the_caret_and_leaves_the_document_alone() {
    let (mut doc, id) = editing(3);
    let before = caret(&doc);
    assert!(before.3 > before.1, "the caret has a height: {before:?}");
    let undo = undo_depth(&doc);
    for typed in ["n", "ni", "ni hao"] {
        doc.execute("text.edit.preedit", json!({ "text": typed })).unwrap();
        let selection = doc.query("textSelection", &json!({})).unwrap();
        assert_eq!(selection["preedit"]["text"], typed, "the composition is held: {selection}");
        assert_eq!(characters(&doc, &id), "one two", "the document has not changed ({typed})");
    }
    let composing = caret(&doc);
    assert!(composing.0 > before.0 + 10.0, "the caret is after the composition: {before:?} -> {composing:?}");
    assert!((composing.1 - before.1).abs() < 1e-6, "on the same line");
    assert!(!doc.dirty(), "composing does not make the document unsaved");
    assert_eq!(undo_depth(&doc), undo, "nor an undo step");
    // The input method commits: the text sink types the finished text.
    KitTools::text(&mut doc, "你好", 1.0).unwrap();
    assert_eq!(characters(&doc, &id), "one你好 two");
    let selection = doc.query("textSelection", &json!({})).unwrap();
    assert_eq!(selection["preedit"], Value::Null, "the commit ends the composition: {selection}");
}

#[test]
fn an_empty_composition_cancels_it() {
    let (mut doc, id) = editing(7);
    doc.execute("text.edit.preedit", json!({ "text": "zhong" })).unwrap();
    doc.execute("text.edit.preedit", json!({ "text": "" })).unwrap();
    assert_eq!(doc.query("textSelection", &json!({})).unwrap()["preedit"], Value::Null);
    assert_eq!(characters(&doc, &id), "one two");
    assert!(!doc.dirty());
}

#[test]
fn the_caret_query_is_null_outside_text_editing() {
    let mut doc = DesignDocument::create("Design.kdesign", 400, 300).unwrap();
    assert_eq!(doc.query("textCaret", &json!({})).unwrap(), Value::Null);
    let (mut doc2, _) = editing(0);
    doc2.execute("text.edit.end", json!({})).unwrap();
    assert_eq!(doc2.query("textCaret", &json!({})).unwrap(), Value::Null);
    doc.mark_saved();
}

/// Ctrl+Right as the canvas sends it on Windows or Linux (`command` is Ctrl or Cmd).
fn ctrl() -> ToolMods {
    ToolMods { command: true, control: true, ..ToolMods::default() }
}

fn caret_index(doc: &DesignDocument) -> u64 {
    doc.query("textSelection", &json!({})).unwrap()["caret"].as_u64().unwrap()
}

#[test]
fn text_keys_move_as_the_reported_platform_does() {
    // Elsewhere Ctrl+Right goes a word on, and Ctrl+End to the end of the text.
    let (mut doc, _) = editing(0);
    doc.set_platform(false);
    doc.key("ArrowRight", ctrl(), 1.0).unwrap();
    assert_eq!(caret_index(&doc), 3, "Ctrl+Right ends the word on Windows and Linux");
    doc.key("ArrowLeft", ctrl(), 1.0).unwrap();
    assert_eq!(caret_index(&doc), 0, "and Ctrl+Left goes back to its start");
    doc.key("End", ctrl(), 1.0).unwrap();
    assert_eq!(caret_index(&doc), 7);
    // On a Mac (also before the editor tells) Cmd+Right goes to the line's end, Alt+Right a word.
    let (mut mac, _) = editing(0);
    mac.key("ArrowRight", ToolMods { command: true, ..ToolMods::default() }, 1.0).unwrap();
    assert_eq!(caret_index(&mac), 7, "Cmd+Right goes to the line's end");
    mac.set_platform(true);
    mac.key("ArrowLeft", ToolMods { alt: true, ..ToolMods::default() }, 1.0).unwrap();
    assert_eq!(caret_index(&mac), 4, "Alt+Left goes back a word");
}
