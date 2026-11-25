//! The KDM bridge ([`crate::kdm`]): commands become transactions, undo works per scope, another
//! session takes the changes in order, and a `.kdesign` carries its replica's content.

use super::*;
use kition_design_kdm::kition_kit::kdm::{Origin, UndoScope, Version};

fn session() -> Session {
    let mut s = Session::new();
    s.enable_kdm();
    s
}

fn rect(s: &mut Session, x: f64, name: &str) -> NodeId {
    let v = s.execute("shape.rectangle", &json!({ "x": x, "y": 0, "width": 40, "height": 30 })).unwrap();
    let id: NodeId = v["id"].as_u64().map(NodeId).or_else(|| v["ids"][0].as_u64().map(NodeId)).unwrap_or_else(|| s.selection()[0]);
    s.execute("object.rename", &json!({ "ids": [id.0], "name": name })).unwrap();
    id
}

fn view_matches(s: &mut Session) {
    let ds = s.ds().unwrap();
    let kdm = ds.kdm().unwrap();
    let (read, _) = kition_design_kdm::read_document(kdm.replica(), kdm.blobs()).unwrap();
    assert!(kition_design_kdm::same_content(&read, &ds.doc), "the view differs from the replica");
}

fn peer(s: &mut Session) -> Session {
    let snapshot = s.snapshot().unwrap();
    let blobs = s.blobs().unwrap();
    let mut b = Session::empty();
    b.enable_kdm();
    b.open_snapshot(&snapshot, blobs).unwrap();
    b
}

/// `from`'s changes taken into `to`, which has every change before them (one line of changes).
fn relay(from: &mut Session, to: &mut Session) {
    let since = to.version().unwrap();
    let changes = from.export_changes(&since).unwrap();
    to.import_changes(&changes).unwrap();
}

#[test]
fn commands_reach_the_replica_and_undo_follows_it() {
    let mut s = session();
    let before = s.version().unwrap();
    let id = rect(&mut s, 10.0, "One");
    assert_ne!(s.version().unwrap(), before);
    view_matches(&mut s);
    assert!(s.undo().unwrap());
    assert!(s.undo().unwrap());
    assert!(s.doc().unwrap().node(id).is_none());
    view_matches(&mut s);
    assert!(s.redo().unwrap());
    assert!(s.doc().unwrap().node(id).is_some());
    view_matches(&mut s);
}

#[test]
fn the_agents_undo_takes_back_only_its_own_steps() {
    let mut s = session();
    let mine = rect(&mut s, 0.0, "Mine");
    let agent = Origin::agent("agent:test");
    let v = s.execute_as("shape.ellipse", &json!({ "x": 100, "y": 0, "width": 20, "height": 20 }), &agent).unwrap();
    let theirs = s.selection()[0];
    let _ = v;
    // The person edits after the agent.
    s.execute("object.rename", &json!({ "ids": [mine.0], "name": "Mine, renamed" })).unwrap();
    assert_eq!(s.undo_depth_in(UndoScope::Agent).0, 1);

    // The agent's undo removes its ellipse and keeps the person's later rename.
    s.execute("edit.undo", &json!({ "scope": "agent" })).unwrap();
    assert!(s.doc().unwrap().node(theirs).is_none());
    assert_eq!(s.doc().unwrap().node(mine).unwrap().name, "Mine, renamed");
    view_matches(&mut s);

    // The person's undo takes back the rename, not the agent's work.
    s.execute("edit.redo", &json!({ "scope": "agent" })).unwrap();
    assert!(s.doc().unwrap().node(theirs).is_some());
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(s.doc().unwrap().node(mine).unwrap().name, "Mine");
    assert!(s.doc().unwrap().node(theirs).is_some(), "the person's undo kept the agent's ellipse");
    view_matches(&mut s);

    // An undo run as the agent is the agent's.
    s.execute_as("edit.undo", &json!({}), &agent).unwrap();
    assert!(s.doc().unwrap().node(theirs).is_none());
    assert!(s.execute("edit.undo", &json!({ "scope": "robot" })).is_err());
}

#[test]
fn two_sessions_on_one_line_keep_each_others_edits() {
    let mut a = session();
    let one = rect(&mut a, 0.0, "One");
    let mut b = peer(&mut a);
    assert_eq!(a.state_hash().unwrap(), b.state_hash().unwrap());
    let two = rect(&mut a, 200.0, "Two");
    relay(&mut a, &mut b);
    b.execute("object.rename", &json!({ "ids": [one.0], "name": "From B" })).unwrap();
    let three = rect(&mut b, 400.0, "Three");
    relay(&mut b, &mut a);
    assert_eq!(a.state_hash().unwrap(), b.state_hash().unwrap());
    for s in [&mut a, &mut b] {
        let doc = s.doc().unwrap();
        assert_eq!(doc.node(one).unwrap().name, "From B");
        assert!(doc.node(two).is_some() && doc.node(three).is_some());
        assert_ne!(two, three, "each session makes ids in its own space");
        view_matches(s);
    }
    // B's undo of its rectangle keeps A's.
    assert!(b.undo().unwrap());
    assert!(b.undo().unwrap());
    relay(&mut b, &mut a);
    assert!(a.doc().unwrap().node(three).is_none());
    assert!(a.doc().unwrap().node(two).is_some());
    assert_eq!(a.state_hash().unwrap(), b.state_hash().unwrap());
}

/// The file of `s`'s document as a host saves it.
fn file_bytes(s: &mut Session) -> Vec<u8> {
    cmd::file::file_bytes(s.ds().unwrap()).unwrap()
}

#[test]
fn a_kdesign_carries_its_replicas_content() {
    let mut s = session();
    rect(&mut s, 0.0, "Saved");
    let id = s.doc_id().unwrap();
    let bytes = file_bytes(&mut s);
    s.ds_mut().unwrap().mark_saved();
    let state = s.state_hash().unwrap();

    let mut t = Session::empty();
    t.enable_kdm();
    let data = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
    // Opened under its own name (another name renames the document, an import of one change).
    t.execute("file.open", &json!({ "name": "Untitled.kdesign", "data": data })).unwrap();
    assert_eq!(t.doc_id(), Some(id.clone()), "the file's replica keeps the document's identity");
    assert_eq!(t.state_hash().unwrap(), state, "the same objects under the same ids");
    view_matches(&mut t);

    // A second save holds the content as it is now, and still no history.
    rect(&mut s, 100.0, "Later");
    let again = file_bytes(&mut s);
    let loaded = kition_design_format::load_with_replica(&again).unwrap();
    let names: Vec<&str> = loaded.replica.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, [kition_design_kdm::file::META, kition_design_kdm::file::CONTENT]);
    assert!(loaded.doc.nodes().any(|n| n.name == "Later"));
    let _ = Version::empty();
}

#[test]
fn the_examples_round_trip_through_a_replica() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../examples");
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("kdesign") {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        let mut s = Session::empty();
        s.enable_kdm();
        let data = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
        s.execute("file.open", &json!({ "name": "x.kdesign", "data": data })).unwrap();
        assert!(s.kdm().is_some(), "{} got no replica", path.display());
        view_matches(&mut s);
        let mut b = peer(&mut s);
        assert_eq!(s.state_hash().unwrap(), b.state_hash().unwrap(), "{}", path.display());
        assert!(kition_design_kdm::same_content(b.doc().unwrap(), s.doc().unwrap()), "{}: a peer reads another document", path.display());
    }
}
