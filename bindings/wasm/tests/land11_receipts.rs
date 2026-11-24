//! The command bridge keeps its identity and tells every deleted descendant.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;

use kition_design_doc::NodeId;
use kition_design_kdm::DesignSession;
use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;
use kition_kit::kdm::Origin;
use serde_json::json;

fn model(doc: &mut DesignDocument) -> serde_json::Value {
    kition_kit::session::model::Exchange::new().replica(doc).unwrap().state_json().unwrap()
}

#[test]
fn deleting_a_parent_records_its_children_without_resetting_the_replica() {
    let mut doc = DesignDocument::create("Delete.kdesign", 320, 240).unwrap();
    let frame = doc.inspect().unwrap()["tree"][0]["id"].as_u64().unwrap();
    for x in 0..5 {
        doc.execute("node.create", json!({"type":"rectangle","parent":frame,"x":x*20,"y":10,"width":10,"height":10})).unwrap();
    }
    let before = doc.session().ds().unwrap().doc.clone();
    let mut replica = DesignSession::create(&before, &Origin::system("fixture")).unwrap();
    let mut after = (*before).clone();
    after.remove(NodeId(frame)).unwrap();
    replica.record(&Origin::agent("delete"), &Arc::new(after)).unwrap();
    let identity = doc.doc_id();
    doc.execute("edit.delete", json!({"ids":[frame]})).unwrap();
    assert_eq!(doc.session().kdm_resets(), 0);
    assert_eq!(doc.doc_id(), identity);
}

#[test]
fn command_receipts_equal_the_saved_files_difference_on_fixtures() {
    use kition_kit::kdm::reread::diff_paired;
    let fixtures: &[(&str, &[u8])] = &[
        ("glass-and-vectors.kdesign", include_bytes!("../../../examples/glass-and-vectors.kdesign")),
        ("ridgeline-studio-dashboard.kdesign", include_bytes!("../../../examples/ridgeline-studio-dashboard.kdesign")),
        ("screen.kdesign", include_bytes!("../../../cases/design.agent.act/auto-layout/input/screen.kdesign")),
    ];
    let mut tested = 0;
    for (name, bytes) in fixtures {
        let mut doc = DesignDocument::open(name, bytes).unwrap();
        let page = doc.inspect().unwrap()["page"].clone();
        let frame =
            doc.execute("frame.create", json!({"parent":page,"name":"Invariant","x":0,"y":0,"width":200,"height":200})).unwrap()["id"].clone();
        let layer = doc.execute("node.create", json!({"type":"rectangle","parent":frame,"width":30,"height":20})).unwrap()["id"].clone();
        let commands = [
            ("node.set", json!({"id":layer,"props":{"name":"Saved"}})),
            ("node.set", json!({"id":layer,"props":{"opacity":0.123456789}})),
            ("node.set", json!({"id":layer,"props":{"x":0.1,"width":31.333333333}})),
            ("transform.move", json!({"ids":[layer],"dx":0.1,"dy":0.2})),
            ("transform.rotate", json!({"ids":[layer],"degrees":13.333333333})),
            ("fill.set", json!({"ids":[layer],"index":0,"color":"#fc237a7e"})),
            ("text.create", json!({"parent":frame,"text":"Saved words","x":1.25,"y":12.5})),
            ("layout.add", json!({"ids":[frame],"direction":"horizontal"})),
            ("node.reparent", json!({"ids":[layer],"parent":page,"index":0})),
            ("edit.delete", json!({"ids":[frame]})),
        ];
        let schema = kition_design_kdm::schema().unwrap();
        for (command, params) in commands {
            let before = model(&mut doc);
            let mut exchange = kition_kit::session::model::Exchange::new();
            let since = exchange.replica(&mut doc).unwrap().version();
            exchange.batch(&mut doc, &json!({"ops":[{"op":"command","id":command,"params":params}]}), false).unwrap();
            // Public receipts omit the journal's private $was deletion snapshots.
            let receipt = exchange.replica(&mut doc).unwrap().changes_since(&since).unwrap().without_texts().to_json();
            let memory = model(&mut doc);
            let engine_memory = doc.session().doc().unwrap().clone();
            let bytes = doc.seal_and_save(None).unwrap();
            let mut reopened = DesignDocument::open(name, &bytes).unwrap();
            let saved = model(&mut reopened);
            for node in engine_memory.ids() {
                assert_eq!(engine_memory.node(node), reopened.session().doc().unwrap().node(node), "{name} {command}: engine node {node}");
            }
            assert!(diff_paired(&schema, &memory, &saved).is_empty(), "{name} {command}: memory differs from saved");
            let saved_change = diff_paired(&schema, &before, &saved).without_texts().to_json();
            assert_eq!(receipt, saved_change, "{name} {command}");
            doc = reopened;
            tested += 1;
        }
    }
    println!("INVARIANT {tested} commands across {} fixture files", fixtures.len());
}
