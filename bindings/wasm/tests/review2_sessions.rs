//! C8-design review 2 item 3: writes whose inputs come from session state (vector edit mode, the
//! last duplicate, the clipboard) declare the document objects that state points at. A headless
//! run cannot keep that state across another program's save, so these check `under` directly.
#![allow(clippy::unwrap_used, clippy::panic)]

use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;
use serde_json::{Value, json};

fn setup() -> (DesignDocument, Value, Value) {
    let mut doc = DesignDocument::create("Sessions.kdesign", 320, 240).unwrap();
    let frame = doc.inspect().unwrap()["tree"][0]["id"].clone();
    let a = doc.execute("node.create", json!({"type":"rectangle","parent":frame,"x":10,"y":10,"width":20,"height":20})).unwrap()["id"].clone();
    (doc, frame, a)
}

fn whole(under: &kition_kit::Under, id: &Value) -> bool {
    under.premises.iter().any(|p| {
        p.id == *id
            && p.fields
                .as_ref()
                .is_none_or(|fields| kition_design_kdm::fields::NODE_REGISTERS.iter().all(|field| fields.iter().any(|held| held == field)))
            && p.positions.as_ref().is_none_or(|positions| positions.iter().any(|p| p == "frame"))
    })
}

fn holds(under: &kition_kit::Under, id: &Value, field: &str) -> bool {
    under.premises.iter().any(|p| p.id == *id && p.fields.as_ref().is_none_or(|f| f.iter().any(|f| f == field)))
}

#[test]
fn vector_edit_mode_holds_the_network_and_refit_geometry() {
    let (mut doc, frame, a) = setup();
    doc.execute("vector.edit.begin", json!({"ids":[a]})).unwrap();
    // A network edit keeps unrelated appearance. Only edits that refit need world geometry.
    for (command, params, refits) in [
        ("vector.edit.moveVertices", json!({"vertices":[0],"dx":1,"dy":0}), true),
        ("vector.edit.delete", json!({}), true),
        ("vector.edit.cornerRadius", json!({"radius":2}), false),
        ("vector.deleteHeal", json!({}), true),
    ] {
        let under = doc.under(command, &params).unwrap().unwrap_or_else(|| panic!("{command}: nothing held"));
        let need = under.premises.iter().find(|p| p.id == a).unwrap();
        assert_eq!(need.fields.as_ref().unwrap(), if refits { &["frame.transform", "kind"][..] } else { &["kind"][..] }, "{command}");
        assert_eq!(need.positions.as_ref().unwrap(), if refits { &["frame"][..] } else { &[][..] }, "{command}");
        assert_eq!(holds(&under, &frame, "frame.transform"), refits, "{command}: {:?}", under.premises);
    }
}

#[test]
fn a_repeated_duplicate_holds_the_last_sources_and_copies() {
    let (mut doc, _, a) = setup();
    let copy = doc.execute("edit.duplicate", json!({"ids":[a]})).unwrap()["ids"][0].clone();
    doc.execute("transform.move", json!({"ids":[copy],"dx":50})).unwrap();
    let under = doc.under("edit.duplicate", &json!({"ids":[copy]})).unwrap().unwrap();
    assert!(holds(&under, &a, "frame.transform"), "{:?}", under.premises);
    let under = doc.under("edit.duplicate", &json!({"ids":[copy],"dx":5,"dy":0})).unwrap().unwrap();
    assert!(!under.premises.iter().any(|p| p.id == a), "an explicit offset reads no sources: {:?}", under.premises);
}

#[test]
fn a_paste_holds_copied_mains_and_resources() {
    let (mut doc, frame, a) = setup();
    let main = doc.execute("frame.create", json!({"parent":frame,"x":100,"y":10,"width":30,"height":30})).unwrap()["id"].clone();
    doc.execute("component.create", json!({"ids":[main]})).unwrap();
    let style = doc.execute("style.create", json!({"type":"paint","name":"Accent","value":["#f00"]})).unwrap()["id"].clone();
    doc.execute("style.apply", json!({"ids":[a],"style":style})).unwrap();
    doc.execute("edit.copy", json!({"ids":[main, a]})).unwrap();
    let under = doc.under("edit.paste", &json!({})).unwrap().unwrap();
    assert!(whole(&under, &main), "{:?}", under.premises);
    assert!(under.premises.iter().any(|p| p.id == style), "{:?}", under.premises);
    assert!(!holds(&under, &json!("design"), "libraries"), "a local clipboard reads no library records: {:?}", under.premises);
}
