//! C8-design-3: vector session inputs and file mutations, including conversion and cleanup.
#![allow(clippy::unwrap_used)]

use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;
use serde_json::json;

#[test]
fn tool_switches_declare_vector_cleanup_writes_and_inputs() {
    for (command, params) in [("tool.rectangle", json!({})), ("tool.set", json!({"tool":"ellipse"})), ("edit.pickColor", json!({}))] {
        let mut doc = DesignDocument::create("Vectors.kdesign", 320, 240).unwrap();
        assert!(!doc.commands().iter().find(|c| c.id == command).unwrap().mutates, "{command} has no cleanup in an inactive session");
        let vector = doc.execute("node.add", json!({"type":"vector"})).unwrap()["id"].clone();
        doc.execute("vector.edit.begin", json!({"ids":[vector]})).unwrap();
        let under = doc.under(command, &params).unwrap().unwrap_or_default();
        let cleanup = under.premises.iter().find(|p| p.id == vector).unwrap();
        assert_eq!(cleanup.fields.as_ref().unwrap(), &["kind"]);
        assert!(cleanup.positions.as_ref().unwrap().is_empty());
        assert!(doc.commands().iter().find(|c| c.id == command).unwrap().mutates, "{command} deletes empty edited layers");
        doc.execute(command, params).unwrap();
        assert!(!doc.session().doc().unwrap().contains(kition_design_doc::NodeId(vector.as_u64().unwrap())));
        assert!(!doc.commands().iter().find(|c| c.id == command).unwrap().mutates, "{command} finished cleanup");
    }
}

#[test]
fn vector_begin_and_end_are_file_writes() {
    let doc = DesignDocument::create("Vectors.kdesign", 320, 240).unwrap();
    let commands = doc.commands();
    for id in ["vector.edit.begin", "vector.edit.end"] {
        assert!(commands.iter().find(|c| c.id == id).unwrap().mutates, "{id} can change stored geometry");
    }
}

#[test]
fn vector_begin_holds_boolean_operands_and_end_holds_all_edited_layers() {
    let mut doc = DesignDocument::create("Vectors.kdesign", 320, 240).unwrap();
    let a = doc.execute("shape.rectangle", json!({"x":0,"y":0,"width":20,"height":20})).unwrap()["id"].clone();
    let b = doc.execute("shape.rectangle", json!({"x":10,"y":10,"width":20,"height":20})).unwrap()["id"].clone();
    let boolean = doc.execute("bool.union", json!({"ids":[a,b]})).unwrap()["id"].clone();
    let under = doc.under("vector.edit.begin", &json!({"ids":[boolean]})).unwrap().unwrap_or_default();
    for id in [&a, &b] {
        assert!(under.premises.iter().any(|p| &p.id == id && p.fields.is_none()), "conversion consumes operand {id}: {under:?}");
    }
    let other = doc.execute("node.add", json!({"type":"vector"})).unwrap()["id"].clone();
    doc.execute("vector.edit.begin", json!({"ids":[boolean,other]})).unwrap();
    let under = doc.under("vector.edit.end", &json!({})).unwrap().unwrap();
    let kept = under.premises.iter().find(|p| p.id == boolean).unwrap();
    assert_eq!(kept.fields.as_ref().unwrap(), &["kind"]);
    assert!(kept.positions.as_ref().unwrap().is_empty());
    let removed = under.premises.iter().find(|p| p.id == other).unwrap();
    assert_eq!(removed.fields.as_ref().unwrap(), &["kind"]);
    assert!(removed.positions.as_ref().unwrap().is_empty());
}
