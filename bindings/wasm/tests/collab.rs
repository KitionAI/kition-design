//! Engine contract 2: the design as a KDM replica (`docs/features/kdm.md`), through `DesignDocument`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use kition_design_kit::DesignDocument;
use kition_design_kit::collab::{decode_blobs, encode_blobs};
use kition_kit::KitDocument;
use kition_kit::kdm::Origin;
use kition_kit::session::MemoryBlobs;
use kition_kit::session::model::{Exchange, ModelErrorCode};
use serde_json::{Value, json};

fn fixture() -> Vec<u8> {
    let mut doc = DesignDocument::create("Design.kdesign", 400, 300).unwrap();
    doc.execute("shape.rectangle", json!({ "x": 40, "y": 40, "width": 120, "height": 80 })).unwrap();
    doc.seal_and_save(None).unwrap()
}

fn layers(doc: &DesignDocument) -> u64 {
    doc.inspect().unwrap()["layers"].as_u64().unwrap()
}

fn rect(doc: &mut DesignDocument, x: f64) {
    doc.execute("shape.rectangle", json!({ "x": x, "y": 150, "width": 30, "height": 30 })).unwrap();
}

#[test]
fn a_design_has_its_replica_from_the_start() {
    let mut doc = DesignDocument::open("Design.kdesign", &fixture()).unwrap();
    assert!(doc.has_replica());
    assert!(doc.doc_id().is_some());
    let version = doc.version().unwrap();
    assert_eq!(doc.version().unwrap(), version, "asking makes no change");
    assert!(!doc.dirty(), "opening writes nothing the file lacks");
    rect(&mut doc, 200.0);
    assert_ne!(doc.version().unwrap(), version);
}

#[test]
fn the_kdesign_keeps_the_documents_identity() {
    let mut a = DesignDocument::open("Design.kdesign", &fixture()).unwrap();
    let id = a.doc_id().unwrap();
    rect(&mut a, 200.0);
    let bytes = a.seal_and_save(None).unwrap();
    a.mark_saved();
    let mut b = DesignDocument::open("Design.kdesign", &bytes).unwrap();
    assert_eq!(b.doc_id(), Some(id.clone()));
    assert_eq!(b.version().unwrap(), a.version().unwrap());
    assert!(!b.dirty());
    // A host's stored replica does not override the file's own.
    let other = DesignDocument::create("Other.kdesign", 100, 100).unwrap().snapshot_of();
    let c = DesignDocument::open_with_replica("Design.kdesign", &bytes, &other, &[]).unwrap();
    assert_eq!(c.doc_id(), Some(id));
}

trait SnapshotOf {
    fn snapshot_of(self) -> Vec<u8>;
}

impl SnapshotOf for DesignDocument {
    fn snapshot_of(mut self) -> Vec<u8> {
        self.snapshot().unwrap()
    }
}

#[test]
fn a_replica_opens_without_a_file_and_converges() {
    let mut a = DesignDocument::open("Design.kdesign", &fixture()).unwrap();
    rect(&mut a, 200.0);
    let changes = a.export_changes(&[]).unwrap();
    let mut b = DesignDocument::open_with_replica("Shared.kdesign", &[], &changes, &a.blobs().unwrap()).unwrap();
    assert_eq!(b.doc_id(), a.doc_id());
    assert_eq!(b.state_json().unwrap(), a.state_json().unwrap());
    assert!(DesignDocument::open("Shared.kdesign", &b.seal_and_save(None).unwrap()).is_ok());
    assert!(DesignDocument::open_with_replica("Shared.kdesign", &[], b"not a replica", &[]).is_err());

    // Edits on both sides, exchanged both ways.
    rect(&mut a, 260.0);
    b.execute("shape.ellipse", json!({ "x": 320, "y": 150, "width": 30, "height": 30 })).unwrap();
    let (va, vb) = (a.version().unwrap(), b.version().unwrap());
    let to_b = a.export_changes(&vb).unwrap();
    let to_a = b.export_changes(&va).unwrap();
    b.add_blobs(&a.blobs_since(&vb).unwrap()).unwrap();
    b.import_changes(&to_b).unwrap();
    a.import_changes(&to_a).unwrap();
    assert_eq!(a.state_json().unwrap(), b.state_json().unwrap());
    assert_eq!(layers(&a), layers(&b));
    let size = |d: &DesignDocument| (d.width(), d.height());
    assert_eq!(a.render(0.0, 0.0, 1.0, size(&a).0, size(&a).1).unwrap(), b.render(0.0, 0.0, 1.0, size(&b).0, size(&b).1).unwrap());
}

#[test]
fn a_stored_replica_that_does_not_open_never_stops_the_file() {
    let bytes = fixture();
    for (snapshot, blobs) in [(&b"not a snapshot"[..], &[][..]), (&[][..], &[1u8, 2, 3][..])] {
        let doc = DesignDocument::open_with_replica("Design.kdesign", &bytes, snapshot, blobs).unwrap();
        assert!(doc.has_replica());
    }
    assert!(DesignDocument::open_with_replica("Design.kdesign", b"not a design", b"not a snapshot", &[]).is_err());
}

#[test]
fn undo_and_redo_are_the_persons() {
    let mut doc = DesignDocument::open("Design.kdesign", &fixture()).unwrap();
    assert!(!doc.can_undo());
    let before = layers(&doc);
    doc.execute_as("shape.ellipse", json!({ "x": 200, "y": 150, "width": 20, "height": 20 }), &Origin::agent("agent:test")).unwrap();
    assert!(!doc.can_undo(), "the agent's step is not the person's to undo");
    assert!(!doc.undo().unwrap().0);
    rect(&mut doc, 300.0);
    assert!(doc.can_undo());
    assert!(doc.undo().unwrap().0);
    assert_eq!(layers(&doc), before + 1, "the person's undo kept the agent's ellipse");
    assert!(doc.redo().unwrap().0);
    // The agent's own undo, run as the agent, takes back its ellipse and keeps the rectangle.
    doc.execute_as("edit.undo", json!({}), &Origin::agent("agent:test")).unwrap();
    assert_eq!(layers(&doc), before + 1);
    assert!(doc.can_undo(), "the person's rectangle is still theirs to undo");
}

#[test]
fn blobs_round_trip_and_bad_bundles_are_errors() {
    let mut blobs = MemoryBlobs::default();
    blobs.0.insert("a".repeat(64), std::sync::Arc::from(b"hello".as_slice()));
    let back = decode_blobs(&encode_blobs(&blobs)).unwrap();
    assert_eq!(back.0.get(&"a".repeat(64)).map(|b| b.as_ref()), Some(b"hello".as_ref()));
    assert!(decode_blobs(&[1, 2, 3]).is_err());
    let mut lying = "a".repeat(64).into_bytes();
    lying.extend_from_slice(&u64::MAX.to_le_bytes());
    assert!(decode_blobs(&lying).is_err());
}

#[test]
fn images_travel_as_blobs() {
    let mut a = DesignDocument::open("Design.kdesign", &fixture()).unwrap();
    let mut b = DesignDocument::open_with_replica("Design.kdesign", &[], &a.snapshot().unwrap(), &a.blobs().unwrap()).unwrap();
    let mut png = Vec::new();
    image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 0, 0, 255])).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    let data = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &png);
    let since = b.version().unwrap();
    a.execute("file.placeImage", json!({ "data": data, "x": 10, "y": 10 })).unwrap();
    let changes = a.export_changes(&since).unwrap();
    // The changes first, the blob later: the image shows once it arrives.
    b.import_changes(&changes).unwrap();
    b.add_blobs(&a.blobs_since(&since).unwrap()).unwrap();
    assert_eq!(a.state_json().unwrap(), b.state_json().unwrap());
    let saved = b.seal_and_save(None).unwrap();
    assert!(DesignDocument::open("Design.kdesign", &saved).is_ok());
}

#[test]
fn serves_the_object_model_over_its_change_exchange() {
    let mut doc = DesignDocument::open("Design.kdesign", &fixture()).unwrap();
    let mut exchange = Exchange::new();
    let described = exchange.describe(&mut doc).unwrap();
    assert_eq!(described["capability"], "KitObjectModel 1.0");
    assert_eq!(described["kit"], "kit.design");
    assert!(described["commands"].as_array().unwrap().iter().any(|c| c["id"] == "shape.rectangle"));

    let command = json!({ "ops": [{ "op": "command", "id": "shape.ellipse", "params": { "x": 10, "y": 10, "width": 20, "height": 20 } }] });
    assert_eq!(exchange.batch(&mut doc, &command, true).unwrap_err().code, ModelErrorCode::ReadOnly);
    let before = doc.version().unwrap();
    exchange.batch(&mut doc, &command, false).unwrap();
    assert_ne!(doc.version().unwrap(), before, "the command ran in the engine");

    let load = |exchange: &mut Exchange, doc: &mut DesignDocument, target: &str, field: &str| -> Value {
        let answer = exchange.batch(doc, &json!({ "ops": [], "load": [{ "target": target, "fields": [field] }] }), false).unwrap();
        answer["loaded"][0]["fields"][field].clone()
    };
    let nodes = load(&mut exchange, &mut doc, "root", "nodes");
    let page = nodes[0]["object"].as_str().unwrap().to_owned();
    let rename = json!({ "ops": [{ "op": "set", "target": page, "field": "name", "value": "Renamed by a script" }] });
    exchange.batch(&mut doc, &rename, false).unwrap();
    assert_eq!(doc.inspect().unwrap()["pages"][0]["name"], "Renamed by a script");
}

/// Review #13: the `history` query and `inspect().undo` list one undo scope's steps (the person's
/// by default), as undo takes them; `scope: "agent"` lists the agent's.
#[test]
fn history_lists_one_scopes_steps() {
    let mut doc = DesignDocument::open("Design.kdesign", &fixture()).unwrap();
    let agent = Origin::agent("agent:test");
    rect(&mut doc, 200.0);
    doc.execute_as("shape.ellipse", json!({ "x": 200, "y": 150, "width": 20, "height": 20 }), &agent).unwrap();
    let person = doc.query("history", &json!({})).unwrap();
    assert_eq!(person["undo"], json!(["Rectangle"]), "{person}");
    assert_eq!(doc.inspect().unwrap()["undo"], "Rectangle", "the person's next undo");
    let agents = doc.query("history", &json!({ "scope": "agent" })).unwrap();
    assert_eq!(agents["undo"], json!(["Ellipse"]), "{agents}");
    doc.execute_as("edit.undo", json!({}), &agent).unwrap();
    assert_eq!(doc.query("history", &json!({})).unwrap()["redo"], json!([]), "the agent's undone step is not the person's redo");
    assert_eq!(doc.query("history", &json!({ "scope": "agent" })).unwrap()["redo"], json!(["Ellipse"]));
    assert!(doc.query("history", &json!({ "scope": "everyone" })).unwrap_err().starts_with("invalid-params:"));
}

/// A minimal `.sketch` zip (one empty page) with `extra` entries beside it.
fn sketch(extra: &[(&str, &[u8])]) -> Vec<u8> {
    use std::io::Write;
    let mut out = Vec::new();
    let mut z = zip::ZipWriter::new(std::io::Cursor::new(&mut out));
    let o = zip::write::SimpleFileOptions::default();
    let entries: [(&str, Vec<u8>); 3] = [
        (
            "document.json",
            json!({ "_class": "document", "pages": [{ "_class": "MSJSONFileReference", "_ref": "pages/p1" }] }).to_string().into_bytes(),
        ),
        ("meta.json", json!({ "app": "com.bohemiancoding.sketch3", "appVersion": "70", "version": 125 }).to_string().into_bytes()),
        ("pages/p1.json", json!({ "_class": "page", "do_objectID": "p1", "name": "Page", "layers": [] }).to_string().into_bytes()),
    ];
    for (name, bytes) in entries.iter().map(|(n, b)| (*n, b.as_slice())).chain(extra.iter().copied()) {
        z.start_file(name, o).unwrap();
        z.write_all(bytes).unwrap();
    }
    z.finish().unwrap();
    out
}

/// Review #25b: only a design whose file really holds its replica opens with it; an import that
/// merely names `kdm/meta.json` (a forged `.sketch`) opens on the host's stored replica.
#[test]
fn only_a_files_own_replica_overrides_the_stored_one() {
    let mut stored = DesignDocument::open("Design.kdesign", &fixture()).unwrap();
    let (id, snapshot, blobs) = (stored.doc_id().unwrap(), stored.snapshot().unwrap(), stored.blobs().unwrap());
    let plain = sketch(&[]);
    let forged = sketch(&[("kdm/meta.json", b"{\"format\":\"kit.design\"}")]);
    for bytes in [&plain, &forged] {
        let doc = DesignDocument::open_with_replica("Shared.sketch", bytes, &snapshot, &blobs).unwrap();
        assert_eq!(doc.doc_id(), Some(id.clone()), "the stored replica is the design's");
    }
    // A .kdesign with replica entries that do not open falls back to the stored replica too.
    let mut damaged = Vec::new();
    {
        use std::io::{Read, Write};
        let mut source = zip::ZipArchive::new(std::io::Cursor::new(fixture())).unwrap();
        let mut z = zip::ZipWriter::new(std::io::Cursor::new(&mut damaged));
        for i in 0..source.len() {
            let mut entry = source.by_index(i).unwrap();
            let name = entry.name().to_owned();
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            z.start_file(name.as_str(), zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(if name == "kdm/meta.json" { b"not json" } else { &bytes }).unwrap();
        }
        z.finish().unwrap();
    }
    let doc = DesignDocument::open_with_replica("Design.kdesign", &damaged, &snapshot, &blobs).unwrap();
    assert_eq!(doc.doc_id(), Some(id), "a damaged file replica gives way to the stored one");
}

/// The fixture's rectangle and where its left edge is on the canvas.
fn the_rect(doc: &DesignDocument) -> Value {
    let ds = doc.session().ds().unwrap();
    let id = ds.doc.nodes().find(|n| matches!(n.kind, kition_design_doc::NodeKind::Rectangle)).unwrap().id;
    json!(id)
}

fn x_of(doc: &DesignDocument, id: &Value) -> f64 {
    let ds = doc.session().ds().unwrap();
    ds.doc.world_bounds(kition_design_doc::NodeId(id.as_u64().unwrap())).unwrap().x0
}

fn set_x(doc: &mut DesignDocument, id: &Value, x: f64) {
    doc.execute("node.set", json!({ "ids": [id], "props": { "x": x } })).unwrap();
}

/// Review #3: an agent's `edit.undo` while the person scrubs takes back the agent's own step and
/// leaves the person's interaction open, with what it changed so far.
#[test]
fn an_agents_undo_leaves_the_persons_interaction_alone() {
    let mut doc = DesignDocument::open("Design.kdesign", &fixture()).unwrap();
    let agent = Origin::agent("agent:test");
    let rect = the_rect(&doc);
    let before = layers(&doc);
    doc.execute_as("shape.ellipse", json!({ "x": 200, "y": 150, "width": 20, "height": 20 }), &agent).unwrap();
    assert!(doc.begin_interaction("Scrub").unwrap());
    set_x(&mut doc, &rect, 50.0);
    doc.execute_as("edit.undo", json!({}), &agent).unwrap();
    assert_eq!(layers(&doc), before, "the agent's ellipse is undone");
    assert_eq!(x_of(&doc, &rect), 50.0, "the person's scrub so far stays");
    assert_eq!(doc.inspect().unwrap()["interaction"], "Scrub", "and stays open");
    assert!(!doc.can_undo(), "nothing of the person's was recorded yet");
    set_x(&mut doc, &rect, 60.0);
    assert!(doc.commit_interaction().unwrap());
    assert_eq!(doc.query("history", &json!({})).unwrap()["undo"], json!(["Scrub"]));
    assert!(doc.undo().unwrap().0);
    assert_eq!(x_of(&doc, &rect), 40.0, "one step takes the whole scrub back");
    assert_eq!(layers(&doc), before);
}

/// Review #4: an agent's edit while the person scrubs is its own step: it survives the person's
/// cancel, and the person's commit records only the person's changes.
#[test]
fn an_agents_edit_during_the_persons_interaction_is_its_own_step() {
    let agent = Origin::agent("agent:test");
    for cancel in [false, true] {
        let mut doc = DesignDocument::open("Design.kdesign", &fixture()).unwrap();
        let rect = the_rect(&doc);
        let before = layers(&doc);
        assert!(doc.begin_interaction("Scrub").unwrap());
        set_x(&mut doc, &rect, 50.0);
        doc.execute_as("shape.ellipse", json!({ "x": 200, "y": 150, "width": 20, "height": 20 }), &agent).unwrap();
        assert_eq!(x_of(&doc, &rect), 50.0, "the scrub so far stays on screen");
        assert_eq!(doc.inspect().unwrap()["interaction"], "Scrub");
        set_x(&mut doc, &rect, 60.0);
        if cancel {
            assert!(doc.cancel_interaction().unwrap());
            assert_eq!(x_of(&doc, &rect), 40.0, "cancel puts back the person's start");
            assert_eq!(layers(&doc), before + 1, "and keeps the agent's ellipse");
            assert!(!doc.can_undo());
            continue;
        }
        assert!(doc.commit_interaction().unwrap());
        assert_eq!(doc.query("history", &json!({})).unwrap()["undo"], json!(["Scrub"]));
        assert_eq!(doc.query("history", &json!({ "scope": "agent" })).unwrap()["undo"], json!(["Ellipse"]));
        assert!(doc.undo().unwrap().0);
        assert_eq!(x_of(&doc, &rect), 40.0, "the person's undo takes back the scrub");
        assert_eq!(layers(&doc), before + 1, "and keeps the agent's ellipse");
        assert!(doc.redo().unwrap().0);
        doc.execute_as("edit.undo", json!({}), &agent).unwrap();
        assert_eq!(layers(&doc), before, "the agent's undo takes back its ellipse");
        assert_eq!(x_of(&doc, &rect), 60.0, "and keeps the scrub");
    }
}

/// Review #10: a collaborator's change arriving while the person scrubs keeps the scrub one
/// interaction: one undo step, which cancel and undo take back whole, keeping the peer's change.
#[test]
fn a_peers_change_during_an_interaction_keeps_it_one_step() {
    for cancel in [false, true] {
        let mut a = DesignDocument::open("Design.kdesign", &fixture()).unwrap();
        let mut b = DesignDocument::open_with_replica("Design.kdesign", &[], &a.snapshot().unwrap(), &a.blobs().unwrap()).unwrap();
        let target = the_rect(&a);
        let before = layers(&a);
        let since = a.version().unwrap();
        assert!(a.begin_interaction("Scrub").unwrap());
        set_x(&mut a, &target, 50.0);
        rect(&mut b, 300.0);
        let changes = b.export_changes(&since).unwrap();
        a.add_blobs(&b.blobs().unwrap()).unwrap();
        a.import_changes(&changes).unwrap();
        assert_eq!(layers(&a), before + 1, "the peer's rectangle arrived");
        assert_eq!(x_of(&a, &target), 50.0, "the scrub so far stays on screen");
        assert_eq!(a.inspect().unwrap()["interaction"], "Scrub", "the interaction is still open");
        assert!(!a.can_undo(), "nothing was recorded yet");
        set_x(&mut a, &target, 60.0);
        if cancel {
            assert!(a.cancel_interaction().unwrap());
            assert_eq!(x_of(&a, &target), 40.0);
            assert_eq!(layers(&a), before + 1, "cancel keeps the peer's change");
            assert!(!a.can_undo());
            continue;
        }
        assert!(a.commit_interaction().unwrap(), "the commit records the step");
        assert_eq!(a.query("history", &json!({})).unwrap()["undo"], json!(["Scrub"]), "one step");
        assert!(a.undo().unwrap().0);
        assert_eq!(x_of(&a, &target), 40.0);
        assert_eq!(layers(&a), before + 1, "undo keeps the peer's change");
    }
}

/// Review #25a: `edit.undo` inside a batch while an interaction is open abandons the
/// interaction (its snapshot comes back) before it undoes, so nothing of it hides in another step.
#[test]
fn an_undo_in_a_batch_abandons_the_open_interaction() {
    let mut doc = DesignDocument::open("Design.kdesign", &fixture()).unwrap();
    let rect = the_rect(&doc);
    set_x(&mut doc, &rect, 10.0);
    assert!(doc.begin_interaction("Scrub").unwrap());
    set_x(&mut doc, &rect, 50.0);
    doc.execute("command.batch", json!({ "commands": [{ "command": "edit.undo" }] })).unwrap();
    assert_eq!(x_of(&doc, &rect), 40.0, "the scrub is abandoned and the step before it undone");
    assert!(!doc.commit_interaction().unwrap(), "the interaction is gone");
    assert!(doc.undo().unwrap().0);
    assert_eq!(x_of(&doc, &rect), 10.0, "undoing the batch brings back the step, not the abandoned scrub");
}
