//! Engine-side defects the independent reading of the write commands found (task card
//! C8-design-read): a handler acting on something other than what its parameter names, or an
//! agent's call losing what it made. Each was seen failing before its fix.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;
use kition_kit::kdm::Origin;
use serde_json::{Value, json};

fn agent() -> Origin {
    Origin::agent("agent:test")
}

fn id(answer: &Value) -> Value {
    answer.get("id").cloned().unwrap_or_else(|| panic!("no id in {answer}"))
}

fn names(doc: &DesignDocument) -> Vec<String> {
    doc.session().ds().unwrap().doc.nodes().map(|n| n.name.clone()).collect()
}

/// An agent's cut that names its layers keeps what it cut on the clipboard for the run's next
/// step: the call puts the editor back, but the clipboard is what the call made, not where the
/// editor was (`context.rs` `bracketed` restored it, so the cut layer was lost).
#[test]
fn an_agents_cut_by_id_can_be_pasted() {
    let mut doc = DesignDocument::create("Design.kdesign", 400, 300).unwrap();
    let page = doc.inspect().unwrap()["page"].clone();
    let card = id(&doc.execute("frame.add", json!({ "parent": page, "x": 500, "y": 0, "width": 50, "height": 50, "name": "Cut me" })).unwrap());
    doc.execute_as("edit.cut", json!({ "ids": [card] }), &agent()).unwrap();
    assert!(!names(&doc).contains(&"Cut me".to_owned()), "the cut layer is gone from the page");
    doc.execute_as("edit.paste", json!({}), &agent()).unwrap_or_else(|e| panic!("the cut layer is on the clipboard: {e}"));
    assert!(names(&doc).contains(&"Cut me".to_owned()), "pasting brings it back");
}

/// `text.onPath` takes a vector or a shape as the path and removes it; a frame, a component or a
/// section named as the path is refused, not deleted with its layers (`cmd/text.rs`).
#[test]
fn text_on_a_frame_path_is_refused_and_the_frame_kept() {
    let mut doc = DesignDocument::create("Design.kdesign", 400, 300).unwrap();
    let page = doc.inspect().unwrap()["page"].clone();
    let text = id(&doc.execute("text.add", json!({ "text": "Along", "x": 0, "y": 400, "parent": page })).unwrap());
    let frame = id(&doc.execute("frame.add", json!({ "parent": page, "x": 500, "y": 0, "width": 50, "height": 50, "name": "Screen" })).unwrap());
    doc.execute("node.add", json!({ "type": "rectangle", "parent": frame, "width": 10, "height": 10, "name": "Inside" })).unwrap();
    assert!(doc.execute("text.onPath", json!({ "text": text, "path": frame })).is_err(), "a frame is no path");
    assert!(names(&doc).contains(&"Screen".to_owned()) && names(&doc).contains(&"Inside".to_owned()), "the frame and its layers stay");
    let ellipse = id(&doc.execute("shape.ellipse", json!({ "x": 0, "y": 500, "width": 40, "height": 40, "parent": page })).unwrap());
    doc.execute("text.onPath", json!({ "text": text, "path": ellipse })).unwrap();
}

/// `page.duplicate` duplicates the page its `id` names; a layer's id is refused, not taken for a
/// page (`cmd/page.rs`).
#[test]
fn duplicating_a_layer_as_a_page_is_refused() {
    let mut doc = DesignDocument::create("Design.kdesign", 400, 300).unwrap();
    let page = doc.inspect().unwrap()["page"].clone();
    let layer = id(&doc.execute("shape.rectangle", json!({ "x": 0, "y": 0, "width": 10, "height": 10, "parent": page })).unwrap());
    let pages = doc.inspect().unwrap()["pages"].as_array().unwrap().len();
    assert!(doc.execute("page.duplicate", json!({ "id": layer })).is_err(), "a layer is no page");
    assert_eq!(doc.inspect().unwrap()["pages"].as_array().unwrap().len(), pages, "no page was added");
    doc.execute("page.duplicate", json!({ "id": page })).unwrap();
}

/// `proto.interaction.set` replaces the interaction at `index` with `interaction`, or patches its
/// `trigger`, `actions`, `transition`, `durationMs`, `easing`, `delayMs` (its notation says so);
/// the schema named only `id` and `index`, so every call that set something was refused.
#[test]
fn an_interaction_can_be_set_by_its_parameters() {
    let mut doc = DesignDocument::create("Design.kdesign", 400, 300).unwrap();
    let page = doc.inspect().unwrap()["page"].clone();
    let button = id(&doc.execute("shape.rectangle", json!({ "x": 0, "y": 0, "width": 10, "height": 10, "parent": page })).unwrap());
    let screen = id(&doc.execute("frame.add", json!({ "parent": page, "x": 500, "y": 0, "width": 50, "height": 50 })).unwrap());
    doc.execute("proto.interaction.add", json!({ "id": button, "dest": screen })).unwrap();
    let whole = json!({ "trigger": { "type": "onHover" }, "actions": [{ "type": "navigate", "dest": screen }], "durationMs": 200 });
    doc.execute("proto.interaction.set", json!({ "id": button, "index": 0, "interaction": whole })).unwrap();
    doc.execute(
        "proto.interaction.set",
        json!({ "id": button, "index": 0, "transition": { "type": "dissolve" }, "durationMs": 300, "easing": "easeIn" }),
    )
    .unwrap();
}
