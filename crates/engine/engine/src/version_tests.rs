//! Saved versions are the host's checkpoints, each with its own snapshot: the engine reads the
//! design a snapshot holds to restore it (one undo step, a change of the replica), preview it and
//! compare with it; nothing about versions is kept in the session's document or its file; bad
//! snapshots and another design's are refused.

use serde_json::json;

use crate::cmd::file::saved_version;
use crate::params::id_of;
use crate::{EngineError, Session};

fn rect(s: &mut Session, x: f64) -> kition_design_doc::NodeId {
    id_of(&s.execute("shape.rectangle", &json!({"x": x, "y": 0, "width": 10, "height": 10})).unwrap()["id"]).unwrap()
}

fn count(s: &Session) -> usize {
    s.doc().unwrap().node_count()
}

#[test]
fn restore_takes_the_content_at_a_version_as_one_undo_step() {
    let mut s = Session::new();
    s.execute("file.rename", &json!({"name": "Shop"})).unwrap();
    rect(&mut s, 0.0);
    let one = count(&s);
    let first = saved_version(&mut s);
    rect(&mut s, 20.0);
    let second = saved_version(&mut s);
    rect(&mut s, 40.0);
    let three = count(&s);
    // The panel opens on request; the engine keeps no list of versions.
    s.execute("file.versionHistory", &json!({})).unwrap();
    assert!(s.ui_requests.iter().any(|r| r.kind == "panel.versionHistory"));

    // The content goes back, the file name stays; one undo step brings it back.
    s.execute("file.rename", &json!({"name": "Shop v2"})).unwrap();
    let before = s.version().unwrap();
    let r = s.execute("version.restore", &json!({"snapshot": first})).unwrap();
    assert_eq!(r["restored"], true);
    assert_eq!(count(&s), one);
    assert_eq!(s.doc().unwrap().name, "Shop v2");
    assert!(s.selection().is_empty());
    assert!(s.version().unwrap().includes(&before) && s.version().unwrap() != before, "a restore is a new change of the replica");
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(count(&s), three);
    s.execute("edit.redo", &json!({})).unwrap();
    assert_eq!(count(&s), one);
    // A later version restores its own content.
    s.execute("version.restore", &json!({"snapshot": second})).unwrap();
    assert_eq!(count(&s), one + 1);
}

#[test]
fn save_version_only_opens_the_form() {
    let mut s = Session::new();
    rect(&mut s, 0.0);
    let doc = s.ds().unwrap().doc.clone();
    s.execute("file.saveVersion", &json!({})).unwrap();
    assert!(s.ui_requests.iter().any(|r| r.kind == "panel.versionHistory" && r.data["save"] == true));
    assert!(std::sync::Arc::ptr_eq(&doc, &s.ds().unwrap().doc), "saving a version is the host's: the design is unchanged");
}

#[test]
fn bad_versions_are_rejected() {
    let mut s = Session::new();
    rect(&mut s, 0.0);
    saved_version(&mut s);
    rect(&mut s, 20.0);
    let n = count(&s);
    for p in [json!({"snapshot": "nope"}), json!({"snapshot": "%%%"}), json!({"snapshot": 5}), json!({"index": 0}), json!({}), json!(null)] {
        for cmd in ["version.restore", "version.preview"] {
            assert!(s.execute(cmd, &p).is_err(), "{cmd} {p}");
        }
    }
    // A version of another design.
    let mut other = Session::new();
    rect(&mut other, 0.0);
    rect(&mut other, 30.0);
    let foreign = saved_version(&mut other);
    for cmd in ["version.restore", "version.preview"] {
        assert!(matches!(s.execute(cmd, &json!({"snapshot": foreign})), Err(EngineError::Disabled(..))), "{cmd}");
    }
    assert_eq!(count(&s), n);
    assert!(s.ds().unwrap().preview.is_none());
}

#[test]
fn versions_stay_out_of_the_file() {
    let dir = std::env::temp_dir().join(format!("kition-design-versions-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("history.kdesign").to_string_lossy().to_string();
    let mut s = Session::new();
    rect(&mut s, 0.0);
    let one = count(&s);
    let start = saved_version(&mut s);
    rect(&mut s, 20.0);
    s.execute("file.save", &json!({"path": path})).unwrap();
    assert!(!s.ds().unwrap().is_dirty());
    // The file is the content and its images, nothing about versions.
    let bytes = std::fs::read(&path).unwrap();
    let z = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
    let names: Vec<&str> = z.file_names().collect();
    assert!(names.iter().all(|n| n.starts_with("kdm/") || n.starts_with("images/")), "{names:?}");

    // The version carries its own snapshot: the file reopened alone restores it too.
    let mut t = Session::empty();
    t.enable_kdm();
    t.execute("file.open", &json!({"path": path})).unwrap();
    t.execute("version.restore", &json!({"snapshot": start})).unwrap();
    assert_eq!(count(&t), one);
    s.execute("version.restore", &json!({"snapshot": start})).unwrap();
    assert_eq!(count(&s), one);
    let _ = std::fs::remove_dir_all(&dir);
}
