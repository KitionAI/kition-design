//! One id space (task card C4/C5/C6-design): the numeric layer id is what inspect lists, commands
//! take and answer, and the object model answers and takes (its tool id, `Node.key`), for pages
//! too; and it survives saving and opening the file again.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::{BTreeMap, BTreeSet};

use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;
use kition_kit::session::model::Exchange;
use serde_json::{Value, json};

/// A design of two pages: Frame 1 with a rectangle on the first, Frame 2 on the second.
fn two_pages() -> DesignDocument {
    let mut doc = DesignDocument::create("Design.kdesign", 400, 300).unwrap();
    let frame = doc.inspect().unwrap()["tree"][0]["id"].as_u64().unwrap();
    doc.execute("shape.rectangle", json!({ "x": 40, "y": 40, "width": 120, "height": 80, "parent": frame })).unwrap();
    doc.execute("page.add", json!({ "name": "Second" })).unwrap();
    let second = doc.inspect().unwrap()["pages"][1]["id"].as_u64().unwrap();
    doc.execute("node.create", json!({ "type": "frame", "parent": second, "x": 0, "y": 0, "width": 100, "height": 100, "name": "Frame 2" })).unwrap();
    doc
}

/// Every layer and page by id, with its name.
fn layers(doc: &DesignDocument) -> BTreeMap<u64, String> {
    let ds = doc.session().ds().unwrap();
    ds.doc.nodes().map(|node| (node.id.0, node.name.clone())).collect()
}

#[test]
fn inspect_lists_every_page_with_its_id_name_and_index() {
    let doc = two_pages();
    let pages = doc.inspect().unwrap()["pages"].clone();
    let pages = pages.as_array().unwrap();
    assert_eq!(pages.len(), 2);
    for (index, page) in pages.iter().enumerate() {
        assert!(page["id"].as_u64().is_some(), "{page}");
        assert_eq!(page["index"], json!(index), "a page's index counts from 0, as page.select takes it: {page}");
        assert!(page["name"].is_string(), "{page}");
    }
    assert_eq!(pages[1]["name"], "Second");
}

#[test]
fn layer_and_page_ids_survive_saving_and_opening_again() {
    let mut doc = two_pages();
    let before = layers(&doc);
    let pages: Vec<Value> = doc.inspect().unwrap()["pages"].as_array().unwrap().iter().map(|p| p["id"].clone()).collect();
    let mut again = DesignDocument::open("Design.kdesign", &doc.save_file(None).unwrap()).unwrap();
    assert_eq!(layers(&again), before, "every layer and page keeps its id");
    let reopened: Vec<Value> = again.inspect().unwrap()["pages"].as_array().unwrap().iter().map(|p| p["id"].clone()).collect();
    assert_eq!(reopened, pages);
    // A command takes them after the reopening.
    let rect = *before.iter().find(|(_, name)| name.starts_with("Rectangle")).unwrap().0;
    again.execute("node.set", json!({ "id": rect, "props": { "name": "Kept" } })).unwrap();
    assert_eq!(layers(&again)[&rect], "Kept");
}

#[test]
fn the_object_model_answers_and_takes_the_layer_ids() {
    let mut doc = two_pages();
    let mut doc = DesignDocument::open("Design.kdesign", &doc.save_file(None).unwrap()).unwrap();
    let before = layers(&doc);
    let mut exchange = Exchange::new();
    let answer = exchange.batch(&mut doc, &json!({ "ops": [], "load": [{ "target": "root", "fields": ["nodes"] }] }), false).unwrap();
    let nodes = answer["loaded"][0]["fields"]["nodes"].as_array().unwrap().clone();
    let named: BTreeSet<u64> =
        nodes.iter().map(|entry| entry["object"].as_str().unwrap().parse::<u64>().unwrap_or_else(|_| panic!("not a layer id: {entry}"))).collect();
    assert_eq!(named, before.keys().copied().collect::<BTreeSet<_>>(), "the model names every layer and page by its layer id");
    // A batch names a layer by it.
    let frame = *before.iter().find(|(_, name)| name.as_str() == "Frame 2").unwrap().0;
    exchange.batch(&mut doc, &json!({ "ops": [{ "op": "set", "target": frame.to_string(), "field": "name", "value": "Renamed" }] }), false).unwrap();
    assert_eq!(layers(&doc)[&frame], "Renamed");
}
