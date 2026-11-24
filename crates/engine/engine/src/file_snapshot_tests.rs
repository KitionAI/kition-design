//! A file encoded on another thread is saved as of its snapshot (`FileSnapshot`). (Automatic
//! checkpoints are the host's, recorded with its saves: Kit UI protocol section 7m.)

use serde_json::json;

use crate::Session;

fn rect(s: &mut Session, x: f64) {
    s.execute("shape.rectangle", &json!({"x": x, "y": 0, "width": 10, "height": 10})).unwrap();
}

#[test]
fn a_file_encoded_elsewhere_is_saved_as_of_its_snapshot() {
    use crate::cmd::file::FileSnapshot;
    let dir = std::env::temp_dir().join(format!("kition-design-snapshot-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("snap.kdesign").to_string_lossy().to_string();
    let mut s = Session::new();
    s.execute("file.save", &json!({"path": path})).unwrap();
    rect(&mut s, 0.0);
    let file = FileSnapshot::of(s.ds().unwrap()).unwrap();
    let bytes = file.encode().unwrap();
    rect(&mut s, 50.0);
    assert!(file.save(&mut s, &bytes).unwrap());
    assert!(s.ds().unwrap().is_dirty(), "the edit since the snapshot is still unsaved");
    let mut t = Session::empty();
    t.execute("file.open", &json!({"path": path})).unwrap();
    assert_eq!(t.ds().unwrap().doc.page_content_bounds(t.ds().unwrap().page).unwrap().x1, 10.0, "the file is the snapshot");
    // Saved meanwhile: the older bytes never overwrite the newer file.
    let file = FileSnapshot::of(s.ds().unwrap()).unwrap();
    let bytes = file.encode().unwrap();
    rect(&mut s, 100.0);
    s.execute("file.save", &json!({})).unwrap();
    assert!(!file.save(&mut s, &bytes).unwrap());
    assert!(!s.ds().unwrap().is_dirty());
    let mut t = Session::empty();
    t.execute("file.open", &json!({"path": path})).unwrap();
    assert_eq!(t.ds().unwrap().doc.page_content_bounds(t.ds().unwrap().page).unwrap().x1, 110.0);
    let _ = std::fs::remove_dir_all(&dir);
}
