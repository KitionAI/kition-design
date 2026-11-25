//! A `.kdesign` holds the design's content, not its history (kition-sdk data model 7.3 and 7.4;
//! feature `design.file.content-only`): its `kdm/content.json` holds the state alone (no version,
//! no change, none of the deleted or overwritten content); an unchanged design saves to the same
//! bytes, and opened again it is the same objects under the same ids; a file opened without a
//! store is the same document and takes edits; undo after a save takes back the last edit; and a
//! file opens on top of a stored replica, whose line of changes continues.
//!
//! The needles below are written once each and share no run of four characters with anything else
//! in the design, so each stays literal in the KDM bytes that hold it: every test checks the
//! session's changes hold them (the control) before it checks the file's content (unpacked from the
//! zip) does not.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use std::io::Read;

use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;
use kition_kit::kdm::Version;
use serde_json::{Value, json};

const FILE: &str = "Plan.kdesign";
const TYPED_THEN_DELETED: &str = "Jx4Hb8Ty3Nc6";
const DELETED_LAYER: &str = "Qv7Zp9Wm2Rk5";
const OVERWRITTEN: &str = "Ug5Le1Fo0Dw4";
const NEEDLES: [&str; 3] = [TYPED_THEN_DELETED, DELETED_LAYER, OVERWRITTEN];

fn contains(bytes: &[u8], needle: &str) -> bool {
    let utf16: Vec<u8> = needle.encode_utf16().flat_map(u16::to_le_bytes).collect();
    bytes.windows(needle.len()).any(|window| window == needle.as_bytes()) || bytes.windows(utf16.len()).any(|window| window == utf16.as_slice())
}

/// The file as a host saves it.
fn host_save(doc: &mut DesignDocument) -> Vec<u8> {
    doc.save(Some(FILE)).unwrap()
}

fn entry(bytes: &[u8], name: &str) -> Vec<u8> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut out = Vec::new();
    z.by_name(name).unwrap().read_to_end(&mut out).unwrap();
    out
}

fn content(bytes: &[u8]) -> Vec<u8> {
    entry(bytes, "kdm/content.json")
}

/// The file's content holds the state alone: JSON with no version, line of changes, actor or
/// origin (data model 7.4).
fn holds_the_state_alone(bytes: &[u8]) {
    let content = content(bytes);
    let value: Value = serde_json::from_slice(&content).unwrap();
    assert!(value.get("objects").is_some(), "the state");
    for key in ["version", "changes", "lineage", "actor", "origin"] {
        assert!(value.get(key).is_none(), "the content holds no {key}");
    }
}

fn id_of(answer: &Value, doc: &DesignDocument) -> u64 {
    answer["id"].as_u64().or_else(|| doc.session().selection().first().map(|n| n.0)).unwrap()
}

/// A design whose history holds typed-then-deleted text, a deleted layer and an overwritten
/// value; answers it and the id of the text layer that stays.
fn design_with_history() -> (DesignDocument, u64) {
    let (doc, text, _) = design_with_changes();
    (doc, text)
}

/// [`design_with_history`] and the replica version it was created at (its changes follow it).
fn design_with_changes() -> (DesignDocument, u64, Vec<u8>) {
    let mut doc = DesignDocument::create(FILE, 800, 600).unwrap();
    let created = doc.version().unwrap();
    let made = doc.execute("text.add", json!({ "text": "Kept", "x": 20, "y": 20 })).unwrap();
    let text = id_of(&made, &doc);
    doc.execute("object.rename", json!({ "ids": [text], "name": "Title" })).unwrap();
    doc.execute("text.set", json!({ "id": text, "characters": format!("Kept {TYPED_THEN_DELETED}") })).unwrap();
    doc.execute("text.set", json!({ "id": text, "characters": "Kept" })).unwrap();
    let made = doc.execute("shape.rectangle", json!({ "x": 200, "y": 20, "width": 40, "height": 40 })).unwrap();
    let gone = id_of(&made, &doc);
    doc.execute("object.rename", json!({ "ids": [gone], "name": DELETED_LAYER })).unwrap();
    doc.execute("edit.delete", json!({ "ids": [gone] })).unwrap();
    let made = doc.execute("shape.rectangle", json!({ "x": 300, "y": 20, "width": 40, "height": 40 })).unwrap();
    let box_ = id_of(&made, &doc);
    doc.execute("object.rename", json!({ "ids": [box_], "name": OVERWRITTEN })).unwrap();
    doc.execute("object.rename", json!({ "ids": [box_], "name": "Box" })).unwrap();
    (doc, text, created)
}

fn tree(doc: &DesignDocument) -> Value {
    doc.inspect().unwrap()["tree"].clone()
}

fn characters(doc: &DesignDocument, id: u64) -> String {
    match &doc.session().ds().unwrap().doc.node(kition_design_doc::NodeId(id)).unwrap().kind {
        kition_design_doc::NodeKind::Text(t) => t.characters.clone(),
        _ => panic!("not a text"),
    }
}

#[test]
fn the_file_holds_the_content_and_no_history() {
    let (mut doc, text, created) = design_with_changes();
    let full = doc.export_changes(&created).unwrap();
    for needle in NEEDLES {
        assert!(contains(&full, needle), "the control: the session's changes hold {needle}");
    }
    let bytes = host_save(&mut doc);
    for needle in NEEDLES {
        assert!(!contains(&content(&bytes), needle), "the file holds {needle}");
    }
    holds_the_state_alone(&bytes);
    // No `document.json`, no saved versions, no change log.
    let names: Vec<String> = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap().file_names().map(str::to_owned).collect();
    assert!(!names.iter().any(|n| n == "document.json" || n.starts_with("versions/") || n.starts_with("kdm/changes")), "{names:?}");
    // The state is all there.
    let back = DesignDocument::open(FILE, &bytes).unwrap();
    assert_eq!(tree(&back), tree(&doc));
    assert_eq!(characters(&back, text), "Kept");
}

#[test]
fn an_unchanged_design_saves_to_the_same_bytes() {
    let (mut doc, _) = design_with_history();
    let first = host_save(&mut doc);
    let version = doc.version().unwrap();
    assert_eq!(host_save(&mut doc), first, "two saves with nothing between");
    assert_eq!(doc.version().unwrap(), version, "a save writes no change");
    doc.mark_saved();
    assert_eq!(host_save(&mut doc), first, "and after the host stored the first");
    let mut opened = DesignDocument::open(FILE, &first).unwrap();
    assert_eq!(host_save(&mut opened), first, "and from the file opened again");
    assert_eq!(opened.state_json().unwrap(), doc.state_json().unwrap(), "reopened, the same objects under the same ids");
}

#[test]
fn a_file_opened_without_a_store_is_the_same_document_and_takes_edits() {
    let (mut doc, text) = design_with_history();
    let bytes = host_save(&mut doc);
    let mut opened = DesignDocument::open(FILE, &bytes).unwrap();
    assert_eq!(opened.doc_id(), doc.doc_id(), "the same document");
    assert_eq!(tree(&opened), tree(&doc), "the same layers under the same ids");
    opened.execute("text.set", json!({ "id": text, "characters": "Kept, edited" })).unwrap();
    let edited = host_save(&mut opened);
    let again = DesignDocument::open(FILE, &edited).unwrap();
    assert_eq!(again.doc_id(), doc.doc_id());
    assert_eq!(characters(&again, text), "Kept, edited");
    holds_the_state_alone(&edited);
}

#[test]
fn undo_after_a_save_takes_back_the_last_edit() {
    let (mut doc, _) = design_with_history();
    let before = tree(&doc);
    doc.execute("shape.ellipse", json!({ "x": 400, "y": 300, "width": 30, "height": 30 })).unwrap();
    let with = tree(&doc);
    let bytes = host_save(&mut doc);
    doc.mark_saved();
    assert!(doc.undo().unwrap().0, "a save is no undo step");
    assert_eq!(tree(&doc), before);
    assert!(doc.redo().unwrap().0);
    assert_eq!(tree(&doc), with);
    assert_eq!(tree(&DesignDocument::open(FILE, &bytes).unwrap()), with);
}

#[test]
fn a_file_opens_on_the_stored_replica_whose_line_of_changes_continues() {
    let (mut doc, text) = design_with_history();
    let bytes = host_save(&mut doc);
    let stored = doc.snapshot().unwrap();
    let blobs = doc.blobs().unwrap();
    // The file and the store agree: the store's version stays.
    let mut same = DesignDocument::open_with_replica(FILE, &bytes, &stored, &blobs).unwrap();
    assert_eq!(same.doc_id(), doc.doc_id());
    assert_eq!(same.version().unwrap(), doc.version().unwrap(), "the stored line of changes, nothing added");
    assert_eq!(tree(&same), tree(&doc));
    // The file was saved elsewhere from changes the store never saw: its state is taken in.
    let mut elsewhere = DesignDocument::open(FILE, &bytes).unwrap();
    elsewhere.execute("text.set", json!({ "id": text, "characters": "Saved elsewhere" })).unwrap();
    let newer = host_save(&mut elsewhere);
    let mut taken = DesignDocument::open_with_replica(FILE, &newer, &stored, &blobs).unwrap();
    assert_eq!(taken.doc_id(), doc.doc_id());
    assert_eq!(characters(&taken, text), "Saved elsewhere");
    let (now, was) = (Version::decode(&taken.version().unwrap()).unwrap(), Version::decode(&doc.version().unwrap()).unwrap());
    assert!(now.includes(&was) && now != was, "the stored line of changes continues with the file's change");
}
