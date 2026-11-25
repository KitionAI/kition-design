//! Library files an app lends a session (`library::Cache::lend`): read by the path the document
//! stores (relative to its folder), parsed again only when their stamp changes, and never through
//! the file system.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use crate::Session;
use crate::library::{LentLibrary, LibraryFiles, lent_path};

/// An in-memory workspace: path → bytes; the stamp is the content's hash.
#[derive(Default)]
struct Files {
    files: Mutex<HashMap<String, Vec<u8>>>,
}

impl Files {
    fn put(&self, path: &str, bytes: Vec<u8>) {
        self.files.lock().unwrap().insert(path.to_string(), bytes);
    }

    fn remove(&self, path: &str) {
        self.files.lock().unwrap().remove(path);
    }
}

impl LibraryFiles for Files {
    fn read(&self, path: &str) -> Result<LentLibrary, String> {
        let files = self.files.lock().unwrap();
        let bytes = files.get(path).ok_or_else(|| format!("{path} is not in the workspace"))?;
        Ok(LentLibrary { stamp: kition_design_format::image_hash(bytes), bytes: Arc::new(bytes.clone()) })
    }
}

fn ex(s: &mut Session, cmd: &str, p: Value) -> Value {
    match s.execute(cmd, &p) {
        Ok(v) => v,
        Err(e) => panic!("{cmd} {p}: {e}"),
    }
}

/// One library file's bytes after each publish: a "Button" component in each colour of `colors`.
fn library_versions(colors: &[&str]) -> Vec<Vec<u8>> {
    let mut s = Session::new();
    let f = ex(&mut s, "frame.add", json!({"x": 0, "y": 0, "width": 100, "height": 40, "name": "Button"}))["id"].clone();
    let c = ex(&mut s, "component.add", json!({"ids": [f]}))["id"].clone();
    let mut out = Vec::new();
    for color in colors {
        ex(&mut s, "node.set", json!({"id": c, "props": {"fill": color}}));
        ex(&mut s, "library.publish", json!({}));
        out.push(kition_design_format::save_bytes(s.doc().unwrap()).unwrap());
    }
    out
}

fn library_bytes(color: &str) -> Vec<u8> {
    library_versions(&[color]).remove(0)
}

fn lending(files: &Arc<Files>) -> Session {
    let mut s = Session::new();
    s.libraries.lend(Some(files.clone() as Arc<dyn LibraryFiles>));
    s
}

#[test]
fn lent_paths_stay_in_the_workspace() {
    assert_eq!(lent_path("Design System.kdesign").unwrap(), "Design System.kdesign");
    assert_eq!(lent_path(" ./libs/../libs/Icons.kdesign ").unwrap(), "libs/Icons.kdesign");
    assert_eq!(lent_path("../shared/Icons.KDESIGN").unwrap(), "../shared/Icons.KDESIGN");
    assert_eq!(lent_path("/libraries/Icons.kdesign").unwrap(), "/libraries/Icons.kdesign");
    for bad in [
        "",
        "Icons.png",
        "libs/",
        "/../Icons.kdesign",
        r"C:\libs\Icons.kdesign",
        "C:/libs/Icons.kdesign",
        "https://example.com/Icons.kdesign",
        "//host/share/Icons.kdesign",
        r"\\host\share\Icons.kdesign",
        "file:///Icons.kdesign",
        "a\u{0}b.kdesign",
        ".kdesign",
    ] {
        assert!(lent_path(bad).is_err(), "{bad:?} must be refused");
    }
}

#[test]
fn a_lent_library_is_added_by_its_workspace_path_and_placed() {
    let files = Arc::new(Files::default());
    files.put("libraries/Design System.kdesign", library_bytes("#0000ff"));
    let mut s = lending(&files);
    let added = ex(&mut s, "library.add", json!({"path": "./libraries/Design System.kdesign"}));
    assert_eq!(added["path"], "libraries/Design System.kdesign", "kept relative to the document's folder");
    let doc = s.doc().unwrap();
    assert_eq!(doc.libraries.enabled[0].path, "libraries/Design System.kdesign");
    let lib = added["id"].as_str().unwrap().to_string();
    let placed = ex(&mut s, "library.place", json!({"library": lib, "component": "Button", "x": 0, "y": 0}));
    assert!(placed["component"].is_u64(), "{placed}");
    // An absolute path of a computer is not a workspace path.
    let e = s.execute("library.add", &json!({"path": "/Users/alice/Design System.png"})).unwrap_err();
    assert!(e.to_string().contains(".kdesign"), "{e}");
}

#[test]
fn a_lent_library_is_parsed_again_only_when_it_changed() {
    let files = Arc::new(Files::default());
    let mut versions = library_versions(&["#0000ff", "#ff0000"]);
    files.put("Kit.kdesign", versions.remove(0));
    let mut s = lending(&files);
    let id = ex(&mut s, "library.add", json!({"path": "Kit.kdesign"}))["id"].as_str().unwrap().to_string();
    ex(&mut s, "library.place", json!({"library": id, "component": "Button"}));
    let first = s.load_library("Kit.kdesign").unwrap();
    // A copy of the session (a panel's scratch session) shares what was parsed.
    let mut scratch = Session::new();
    scratch.libraries = s.libraries.clone();
    assert!(Arc::ptr_eq(&first, &scratch.load_library("Kit.kdesign").unwrap()), "the same stamp reads the parsed library");
    assert_eq!(ex(&mut s, "library.updates", json!({}))["count"], 0);
    // The library file is published again: the update shows and is accepted.
    files.put("Kit.kdesign", versions.remove(0));
    let updates = ex(&mut s, "library.updates", json!({}));
    assert_eq!(updates["count"], 1, "{updates}");
    let accepted = ex(&mut s, "library.acceptUpdates", json!({}));
    assert_eq!(accepted["components"], 1, "{accepted}");
    assert_eq!(ex(&mut s, "library.updates", json!({}))["count"], 0);
    // A missing file is an error naming it, and the copies stay.
    files.remove("Kit.kdesign");
    let listed = ex(&mut s, "library.list", json!({}));
    assert!(listed["libraries"][0]["error"].as_str().unwrap().contains("Kit.kdesign"), "{listed}");
    assert!(!s.doc().unwrap().libraries.components.is_empty());
}

/// Two copies of one library component (two people placed it at once, `docs/features/kdm.md` V12, before
/// the repair makes them one): the update names the component once and updates both copies.
#[test]
fn two_copies_of_one_component_count_as_one_update() {
    let files = Arc::new(Files::default());
    let mut versions = library_versions(&["#0000ff", "#ff0000"]);
    files.put("Kit.kdesign", versions.remove(0));
    let mut s = lending(&files);
    let id = ex(&mut s, "library.add", json!({"path": "Kit.kdesign"}))["id"].as_str().unwrap().to_string();
    ex(&mut s, "library.place", json!({"library": id, "component": "Button"}));
    // A second copy of the same component, with its own record, as a merge of two placements
    // leaves it.
    let doc = s.doc_mut().unwrap();
    let first = doc.libraries.components[0].clone();
    let store = doc.library_store().unwrap();
    let layers: Vec<kition_design_doc::Node> = doc.subtree(first.root).into_iter().filter_map(|i| doc.node(i).cloned()).collect();
    let map: std::collections::BTreeMap<_, _> = layers.iter().map(|n| (n.id, doc.alloc_id())).collect();
    doc.paste_nodes(&layers, &map, first.root, store, None).unwrap();
    let second = kition_design_doc::ImportedComponent {
        root: map[&first.root],
        ids: first.ids.iter().map(|(lib, local)| (*lib, map[local])).collect(),
        ..first.clone()
    };
    doc.libraries_mut().components.push(second.clone());
    files.put("Kit.kdesign", versions.remove(0));
    let updates = ex(&mut s, "library.updates", json!({}));
    assert_eq!(updates["count"], 1, "the component, not each copy: {updates}");
    assert_eq!(updates["libraries"][0]["components"].as_array().unwrap().len(), 1, "{updates}");
    assert_eq!(ex(&mut s, "library.list", json!({}))["libraries"][0]["updates"], 1);
    let accepted = ex(&mut s, "library.acceptUpdates", json!({}));
    assert_eq!(accepted["components"], 1, "{accepted}");
    assert_eq!(ex(&mut s, "library.updates", json!({}))["count"], 0, "every copy was updated");
}

#[test]
fn lent_libraries_never_reach_the_file_system() {
    let dir = std::env::temp_dir().join(format!("kition-design-lent-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("Kit.kdesign");
    std::fs::write(&file, library_bytes("#0000ff")).unwrap();
    let files = Arc::new(Files::default());
    let mut s = lending(&files);
    let e = s.execute("library.add", &json!({"path": file.to_string_lossy()})).unwrap_err();
    assert!(e.to_string().contains("not in the workspace"), "only the lent files answer: {e}");
    let _ = std::fs::remove_dir_all(dir);
}
