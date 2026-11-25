//! Explicit call context (Kit Process Protocol section 7a; task card C4/C5/C6-design): the editor
//! state a command reads besides its parameters (the current page, the selection on it, the view,
//! the edit modes, the tool) is taken and put back without an edit, set for one call from a part
//! (`{"pageId": id}` or `{"page": n}`) and the layers a call acts on (`on`), and an agent's command
//! that names what it acts on leaves it as it was.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use kition_design_kit::{DesignDocument, EDITOR_STATE};
use kition_kit::kdm::Origin;
use kition_kit::{CallContext, KitDocument};
use serde_json::{Value, json};

/// Ids of a two-page design.
struct Design {
    doc: DesignDocument,
    page1: u64,
    page2: u64,
    frame1: u64,
    a: u64,
    b: u64,
    frame2: u64,
    c: u64,
}

fn id_of(answer: &Value) -> u64 {
    answer["id"].as_u64().unwrap_or_else(|| panic!("no id in {answer}"))
}

/// Page 1: Frame 1 holding rectangles A and B. Page 2: Frame 2 holding rectangle C. The person is
/// on page 1 with A selected, zoomed to 2 and the Rectangle tool chosen.
fn design() -> Design {
    let mut doc = DesignDocument::create("Design.kdesign", 400, 300).unwrap();
    let inspected = doc.inspect().unwrap();
    let page1 = inspected["pages"][0]["id"].as_u64().unwrap();
    let frame1 = inspected["tree"][0]["id"].as_u64().unwrap();
    let rect = |doc: &mut DesignDocument, parent: u64, name: &str, x: f64| {
        id_of(
            &doc.execute("node.add", json!({ "type": "rectangle", "parent": parent, "x": x, "y": 20.5, "width": 40, "height": 40, "name": name }))
                .unwrap(),
        )
    };
    let a = rect(&mut doc, frame1, "A", 20.0);
    let b = rect(&mut doc, frame1, "B", 80.0);
    doc.execute("page.add", json!({ "name": "Second" })).unwrap();
    let page2 = doc.inspect().unwrap()["pages"][1]["id"].as_u64().unwrap();
    let frame2 = id_of(
        &doc.execute("node.add", json!({ "type": "frame", "parent": page2, "x": 0, "y": 0, "width": 200, "height": 200, "name": "Frame 2" }))
            .unwrap(),
    );
    let c = rect(&mut doc, frame2, "C", 10.0);
    doc.execute("page.select", json!({ "id": page1 })).unwrap();
    doc.execute("select.set", json!({ "ids": [a] })).unwrap();
    doc.execute("tool.rectangle", json!({})).unwrap();
    doc.set_viewport(0.0, 0.0, 800.0, 600.0, 2.0);
    Design { doc, page1, page2, frame1, a, b, frame2, c }
}

/// The editor as the person sees it: current page, selection, zoom, tool and edit mode.
fn editor(doc: &DesignDocument) -> Value {
    let inspected = doc.inspect().unwrap();
    let ds = doc.session().ds().unwrap();
    json!({
        "page": inspected["page"],
        "selection": inspected["selection"],
        "tool": inspected["tool"],
        "editing": inspected["editing"],
        "zoom": ds.view.zoom,
        "centre": [ds.view.cx, ds.view.cy],
    })
}

fn agent() -> Origin {
    Origin::agent("agent:test")
}

fn context(on: &[u64], part: Option<Value>) -> CallContext {
    CallContext { on: on.iter().map(|id| json!(id)).collect(), part, time: None }
}

/// What the document holds, without the editor state.
fn content(doc: &DesignDocument) -> Value {
    kition_kit::document_content(doc.inspect().unwrap(), EDITOR_STATE)
}

#[test]
fn editor_state_changes_nothing_in_what_an_agent_reads() {
    let mut d = design();
    let before = content(&d.doc);
    d.doc.execute("page.select", json!({ "id": d.page2 })).unwrap();
    d.doc.execute("select.set", json!({ "ids": [d.c] })).unwrap();
    d.doc.execute("tool.ellipse", json!({})).unwrap();
    d.doc.set_viewport(30.0, 40.0, 640.0, 480.0, 0.5);
    d.doc.execute("text.add", json!({ "text": "Hi", "x": 0, "y": 0, "parent": d.frame2 })).unwrap();
    let after_text = content(&d.doc);
    // Only the layer count and the page's layer list moved with the new text, never the editor.
    assert_ne!(before, after_text);
    let state = d.doc.editor_state();
    d.doc.execute("page.select", json!({ "id": d.page1 })).unwrap();
    d.doc.execute("select.set", json!({ "ids": [d.a, d.b] })).unwrap();
    assert_eq!(content(&d.doc), after_text, "the current page, selection, tool and view are editor state");
    d.doc.restore_editor_state(&state).unwrap();
    assert_eq!(content(&d.doc), after_text);
}

#[test]
fn check_context_with_a_move_to_another_page() {
    let mut d = design();
    let on = context(&[d.c], Some(json!({ "pageId": d.page2 })));
    kition_kit::testing::check_context(&mut d.doc, &on, EDITOR_STATE).unwrap();
    let numbered = context(&[d.c], Some(json!({ "page": 2 })));
    kition_kit::testing::check_context(&mut d.doc, &numbered, EDITOR_STATE).unwrap();
}

#[test]
fn hooks_are_no_edit() {
    let mut d = design();
    let (revision, dirty, undo) = (d.doc.revision(), d.doc.dirty(), d.doc.inspect().unwrap()["canUndo"].clone());
    let saved = d.doc.editor_state();
    assert!(!saved.is_null());
    d.doc.set_context(&context(&[d.c], Some(json!({ "pageId": d.page2 })))).unwrap();
    assert_eq!(d.doc.inspect().unwrap()["page"], json!(d.page2));
    assert_eq!(d.doc.inspect().unwrap()["selection"], json!([d.c]));
    d.doc.restore_editor_state(&saved).unwrap();
    assert_eq!((d.doc.revision(), d.doc.dirty(), d.doc.inspect().unwrap()["canUndo"].clone()), (revision, dirty, undo));
}

#[test]
fn on_alone_names_its_page() {
    let mut d = design();
    d.doc.set_context(&context(&[d.c], None)).unwrap();
    assert_eq!(d.doc.inspect().unwrap()["page"], json!(d.page2), "a layer's page follows from its id");
    assert_eq!(d.doc.inspect().unwrap()["selection"], json!([d.c]));
}

#[test]
fn refused_contexts_change_nothing() {
    let mut d = design();
    let before = editor(&d.doc);
    let refusals = [
        (context(&[999_999], None), "no layer"),
        (context(&[d.a, d.c], None), "one page"),
        (context(&[d.a], Some(json!({ "pageId": d.page2 }))), "on page"),
        (context(&[], Some(json!({ "pageId": d.a }))), "not a page"),
        (context(&[], Some(json!({ "pageId": 999_999 }))), "no page"),
        (context(&[], Some(json!({ "page": 3 }))), "2 page"),
        (context(&[], Some(json!({ "slide": 1 }))), "pageId"),
        (context(&[d.page2], None), "part"),
        (CallContext { on: vec![json!("A")], part: None, time: None }, "layer id"),
    ];
    for (context, says) in refusals {
        let error = d.doc.set_context(&context).expect_err(&format!("{context:?} is refused"));
        assert!(error.contains(says), "{context:?}: {error}");
        assert!(!error.contains("not available"), "an invalid context is invalid_params, not command_disabled: {error}");
        assert_eq!(editor(&d.doc), before, "{context:?} changed the editor");
    }
    let time = d.doc.set_context(&CallContext { on: Vec::new(), part: None, time: Some(1.0) }).unwrap_err();
    assert!(time.contains("not available"), "{time}");
}

#[test]
fn a_selection_command_driven_by_on_changes_exactly_those_layers() {
    let mut d = design();
    let before = editor(&d.doc);
    let saved = d.doc.editor_state();
    // What a host does for a step `{command: "arrange.roundToPixels", on: [B, C]}`, with B and C
    // on two pages: one step per page.
    for (on, page) in [(d.b, d.page1), (d.c, d.page2)] {
        d.doc.set_context(&context(&[on], Some(json!({ "pageId": page })))).unwrap();
        d.doc.execute_as("arrange.roundToPixels", json!({}), &agent()).unwrap();
    }
    d.doc.restore_editor_state(&saved).unwrap();
    let y = |doc: &DesignDocument, id: u64| doc.session().doc().unwrap().node(kition_design_doc::NodeId(id)).unwrap().transform.translation().y;
    assert_eq!((y(&d.doc, d.b), y(&d.doc, d.c)), (21.0, 21.0), "B and C are on whole pixels");
    assert_eq!(y(&d.doc, d.a), 20.5, "A, the person's selection, was not touched");
    assert_eq!(editor(&d.doc), before, "selection, current page, view and tool are the person's");
}

#[test]
fn an_agents_create_on_another_page_leaves_the_current_page() {
    let mut d = design();
    let before = editor(&d.doc);
    let made =
        d.doc.execute_as("node.add", json!({ "type": "ellipse", "parent": d.frame2, "x": 5, "y": 5, "width": 10, "height": 10 }), &agent()).unwrap();
    let made = id_of(&made);
    assert_eq!(d.doc.session().doc().unwrap().page_of(kition_design_doc::NodeId(made)), Some(kition_design_doc::NodeId(d.page2)));
    assert_eq!(editor(&d.doc), before);
    // An agent's edit by id on another page too.
    d.doc.execute_as("node.set", json!({ "id": d.c, "props": { "name": "C2" } }), &agent()).unwrap();
    assert_eq!(editor(&d.doc), before);
}

#[test]
fn an_agents_failing_command_leaves_the_editor() {
    let mut d = design();
    let before = editor(&d.doc);
    let revision = d.doc.revision();
    d.doc.execute_as("node.set", json!({ "id": d.c, "props": { "width": "wide" } }), &agent()).unwrap_err();
    assert_eq!(editor(&d.doc), before);
    assert_eq!(d.doc.revision(), revision);
}

#[test]
fn a_persons_and_an_agents_navigation_keep_their_effect() {
    let mut d = design();
    d.doc.execute_as("select.set", json!({ "ids": [d.c] }), &agent()).unwrap();
    assert_eq!(d.doc.inspect().unwrap()["page"], json!(d.page2), "select.set is navigation: it goes to the layers' page");
    d.doc.execute_as("page.select", json!({ "id": d.page1 }), &agent()).unwrap();
    assert_eq!(d.doc.inspect().unwrap()["page"], json!(d.page1));
    // A person's command runs as it always did.
    d.doc.execute("node.set", json!({ "id": d.c, "props": { "name": "C3" } })).unwrap();
}

#[test]
fn create_then_act_in_one_run() {
    let mut d = design();
    let saved = d.doc.editor_state();
    d.doc.set_context(&context(&[], Some(json!({ "pageId": d.page2 })))).unwrap();
    let made = id_of(
        &d.doc
            .execute_as("node.add", json!({ "type": "rectangle", "parent": d.frame2, "x": 50.5, "y": 50, "width": 10, "height": 10 }), &agent())
            .unwrap(),
    );
    assert_eq!(d.doc.inspect().unwrap()["selection"], json!([made]), "what it made on the current page is the selection for the next step");
    d.doc.set_context(&context(&[], Some(json!({ "pageId": d.page2 })))).unwrap();
    d.doc.execute_as("arrange.roundToPixels", json!({}), &agent()).unwrap();
    let x = d.doc.session().doc().unwrap().node(kition_design_doc::NodeId(made)).unwrap().transform.translation().x;
    assert_eq!(x, 51.0);
    d.doc.restore_editor_state(&saved).unwrap();
    assert_eq!(d.doc.inspect().unwrap()["page"], json!(d.page1));
}

#[test]
fn page_inspect_lists_any_page_without_going_there() {
    let mut d = design();
    let before = editor(&d.doc);
    let by_index = d.doc.execute_as("page.inspect", json!({ "index": 1 }), &agent()).unwrap();
    let by_id = d.doc.execute_as("page.inspect", json!({ "id": d.page2 }), &agent()).unwrap();
    assert_eq!(by_index, by_id);
    assert_eq!((by_id["id"].clone(), by_id["index"].clone(), by_id["name"].clone()), (json!(d.page2), json!(1), json!("Second")));
    let frame = &by_id["layers"][0];
    assert_eq!(frame["id"], json!(d.frame2));
    assert_eq!(frame["children"][0]["id"], json!(d.c));
    // A layer's subtree by its id.
    let subtree = d.doc.execute_as("page.inspect", json!({ "id": d.frame1 }), &agent()).unwrap();
    assert_eq!(subtree["page"], json!(d.page1));
    let names: Vec<&str> = subtree["layers"][0]["children"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["B", "A"], "the topmost layer first, as the Layers panel lists them");
    let z: Vec<u64> = subtree["layers"][0]["children"].as_array().unwrap().iter().map(|c| c["z"].as_u64().unwrap()).collect();
    assert_eq!(z, [1, 0], "z counts from the bottom of the paint order");
    assert_eq!(editor(&d.doc), before, "listing a page does not go to it");
    let error = d.doc.execute_as("page.inspect", json!({ "index": 7 }), &agent()).unwrap_err();
    assert!(error.contains("2 pages"), "{error}");
}

#[test]
fn page_go_answers_the_index_of_a_page_by_id() {
    let mut d = design();
    let answer = d.doc.execute_as("page.go", json!({ "id": d.page2 }), &agent()).unwrap();
    assert_eq!((answer["index"].clone(), answer["id"].clone()), (json!(1), json!(d.page2)));
    assert_eq!(d.doc.inspect().unwrap()["page"], json!(d.page2), "page.go is navigation");
    let answer = d.doc.execute_as("page.go", json!({ "index": 0 }), &agent()).unwrap();
    assert_eq!(answer["id"], json!(d.page1));
}
