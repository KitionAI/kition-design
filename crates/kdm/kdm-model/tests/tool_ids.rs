//! Tool ids (KitObjectModel OM9, task card C4/C5/C6-design): the object model names every layer and
//! every page by its numeric layer id (`Node.key`, the id the kit's inspect lists and its commands
//! take), so receipts, queries, batches and commands speak one id space. The ids are written by
//! the kit's own write path and survive a rename, a reorder, a reparent and a new replica made from
//! the saved state.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::sync::Arc;

use kition_design_doc::{Document, NodeId, NodeKind};
use kition_design_kdm::{DesignSession, schema};
use kition_kit::kdm::tool_ids::ToolIds;
use kition_kit::kdm::{Origin, Replica};

fn sample() -> Document {
    let mut doc = Document::new("Sample");
    let page = doc.first_page().unwrap();
    let frame = doc.create(page, NodeKind::Frame, "Frame 1", 0.0, 0.0, 200.0, 100.0).unwrap();
    doc.create(frame, NodeKind::Rectangle, "Rectangle 1", 10.0, 10.0, 50.0, 40.0).unwrap();
    doc.create(page, NodeKind::Ellipse { start: 0.0, sweep: std::f64::consts::TAU, inner: 0.0 }, "Ellipse", 300.0, 0.0, 40.0, 40.0).unwrap();
    let second = doc.add_page("Page 2");
    doc.create(second, NodeKind::Frame, "Frame 2", 0.0, 0.0, 100.0, 100.0).unwrap();
    doc.take_dirty();
    doc
}

/// Every layer's tool id, by its layer id.
fn named(session: &DesignSession) -> BTreeMap<NodeId, String> {
    let replica: &Replica = session.replica();
    let tools = ToolIds::of_replica(replica);
    session
        .ids()
        .nodes
        .iter()
        .map(|(id, (object, _))| (*id, tools.tool_id(object).unwrap_or_else(|| panic!("layer {id} ({object}) has no tool id")).to_owned()))
        .collect()
}

/// What `doc`'s layers and pages should be named by: their numeric ids.
fn expected(doc: &Document) -> BTreeMap<NodeId, String> {
    doc.nodes().map(|n| (n.id, n.id.0.to_string())).collect()
}

#[test]
fn the_schema_declares_the_layer_id_as_the_tool_id() {
    let schema = schema().unwrap();
    let node = schema.types.get("Node").expect("a Node type");
    let tool = node.tool_id.as_ref().expect("Node declares a tool id");
    assert_eq!((tool.field.as_str(), tool.prefix.as_str(), tool.parent.as_deref()), ("key", "", None));
    let key = node.fields.get("key").expect("Node.key");
    assert!(key.read_only && key.optional, "Node.key is read-only and optional: only the kit writes it");
}

#[test]
fn layers_and_pages_are_named_by_the_ids_the_kit_lists() {
    let doc = Arc::new(sample());
    let session = DesignSession::create(&doc, &Origin::system("create")).unwrap();
    let names = named(&session);
    assert_eq!(names, expected(&doc));
    for page in &doc.pages {
        assert_eq!(names.get(page), Some(&page.0.to_string()), "page {page} is named by its id");
    }
}

#[test]
fn the_ids_survive_a_rename_a_reorder_a_reparent_and_a_new_replica() {
    let doc = Arc::new(sample());
    let mut session = DesignSession::create(&doc, &Origin::system("create")).unwrap();
    let mut after = (**session.document()).clone();
    let rect = after.nodes().find(|n| n.name == "Rectangle 1").map(|n| n.id).unwrap();
    let ellipse = after.nodes().find(|n| n.name == "Ellipse").map(|n| n.id).unwrap();
    let frame2 = after.nodes().find(|n| n.name == "Frame 2").map(|n| n.id).unwrap();
    after.get_mut(rect).unwrap().name = "Renamed".into();
    after.reparent(ellipse, frame2, None).unwrap();
    after.pages.reverse();
    session.record(&Origin::person("person:test"), &Arc::new(after)).unwrap();
    assert_eq!(named(&session), expected(session.document()));
    let reopened = DesignSession::open(&session.snapshot().unwrap(), &[], session.blobs().clone()).unwrap();
    assert_eq!(named(&reopened), expected(&doc), "a new replica names every layer and page as before");
}
