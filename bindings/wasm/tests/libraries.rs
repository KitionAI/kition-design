//! Team libraries from files (`bindings/wasm/src/libraries.rs`): a design uses the components,
//! styles and variables another `.kdesign` file of the workspace publishes, read through the host's
//! lent files (as `kition-design` reads them headless) or through bytes the editor lent; the document
//! names the file by a workspace path relative to its folder.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};

use kition_design_engine::Session;
use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;
use serde_json::{Value, json};

/// The host's lent root is one per process: the tests that lend files take turns.
static HOST: Mutex<()> = Mutex::new(());

fn turn() -> MutexGuard<'static, ()> {
    HOST.lock().unwrap_or_else(PoisonError::into_inner)
}

fn ex(s: &mut Session, cmd: &str, p: Value) -> Value {
    s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"))
}

/// One library file published twice: "Button" (blue, then red), the paint style "Brand", and the
/// collection "Theme" (modes Light and Dark) with the colour variable "Primary". The bytes after
/// each publish.
fn library_versions() -> (Vec<u8>, Vec<u8>) {
    let mut s = Session::with_document(kition_design_doc::Document::new("Design System"));
    let f = ex(&mut s, "frame.add", json!({ "x": 0, "y": 0, "width": 120, "height": 40, "name": "Button" }))["id"].clone();
    ex(&mut s, "node.set", json!({ "id": f, "props": { "fill": "#0000ff" } }));
    ex(&mut s, "style.add", json!({ "type": "paint", "name": "Brand", "from": f }));
    let button = ex(&mut s, "component.add", json!({ "ids": [f] }))["id"].clone();
    let theme = ex(&mut s, "variable.addCollection", json!({ "name": "Theme", "modes": ["Light", "Dark"] }))["id"].clone();
    ex(
        &mut s,
        "variable.add",
        json!({ "collection": theme, "name": "Primary", "type": "color", "values": { "Light": "#ffffff", "Dark": "#000000" } }),
    );
    ex(&mut s, "library.publish", json!({ "description": "Synthetic test library" }));
    let v1 = kition_design_format::save_bytes(s.doc().unwrap()).unwrap();
    ex(&mut s, "node.set", json!({ "id": button, "props": { "fill": "#ff0000" } }));
    ex(&mut s, "library.publish", json!({}));
    let v2 = kition_design_format::save_bytes(s.doc().unwrap()).unwrap();
    (v1, v2)
}

/// A workspace folder lent as the host lends it: the design is in `designs/`, the library in
/// `libraries/`.
fn workspace(name: &str, library: &[u8]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kition-design-libraries-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("designs")).unwrap();
    std::fs::create_dir_all(dir.join("libraries")).unwrap();
    std::fs::write(dir.join("libraries/Design System.kdesign"), library).unwrap();
    kition_kit::host::set_root(Some(dir.clone()));
    kition_kit::host::set_folder("designs");
    dir
}

fn unlend(dir: &PathBuf) {
    kition_kit::host::set_root(None);
    kition_kit::host::set_folder("");
    let _ = std::fs::remove_dir_all(dir);
}

fn design() -> (DesignDocument, Value) {
    let doc = DesignDocument::create("Design.kdesign", 800, 600).unwrap();
    let frame = doc.query("layers", &json!({})).unwrap()["rows"][0]["id"].clone();
    (doc, frame)
}

fn fill_of(doc: &DesignDocument, id: &Value) -> String {
    let id = kition_design_engine::params::id_of(id).unwrap();
    let ds = doc.session().ds().unwrap();
    ds.doc.node(id).and_then(|n| n.fills.first()).and_then(|p| p.solid_color()).map(|c| c.to_hex().to_ascii_lowercase()).unwrap_or_default()
}

fn missing_components(doc: &DesignDocument) -> Vec<String> {
    doc.check().into_iter().filter(|d| d.code == "design.missing-component").map(|d| d.message).collect()
}

#[test]
fn a_lent_library_is_listed_placed_updated_and_reported_missing() {
    let _turn = turn();
    let (v1, v2) = library_versions();
    let dir = workspace("lent", &v1);
    let (mut doc, frame) = design();

    // Added by its path from the design's folder: the document keeps it relative.
    let added = doc.execute("library.add", json!({ "path": "../libraries/Design System.kdesign" })).unwrap();
    assert_eq!(added["path"], "../libraries/Design System.kdesign", "{added}");
    let library = added["id"].as_str().unwrap().to_owned();
    let files = doc.query("library.files", &json!({})).unwrap();
    assert_eq!(files["files"], json!([{ "id": library, "name": "Design System", "path": "../libraries/Design System.kdesign" }]));

    // library.list, assets.list, the variable picker and the mode rows read it.
    let list = doc.query("library.list", &json!({ "assets": true })).unwrap();
    let row = &list["libraries"][0];
    assert_eq!((row["version"].as_u64(), row["updates"].as_u64(), row.get("error")), (Some(1), Some(0), None), "{list}");
    let assets = doc.query("assets.list", &json!({})).unwrap();
    let section = &assets["libraries"][0];
    assert_eq!(section["id"], json!(library));
    assert!(section["error"].is_null(), "{section}");
    let tile = &section["folders"][0]["tiles"][0];
    assert_eq!(tile["name"], "Button", "{section}");
    assert_eq!(section["styles"][0]["name"], "Brand");
    assert!(section["styles"][0]["paints"].is_array(), "a library paint style carries its swatch");
    let thumb = doc.library_thumbnail(&library, &tile["id"].to_string(), 64.0).unwrap();
    assert!(thumb.starts_with(&[0x89, b'P', b'N', b'G']));
    assert!(
        doc.query("assets.list", &json!({ "query": "nothing like it" })).unwrap()["libraries"].as_array().unwrap().is_empty(),
        "no match: left out"
    );
    let picker = doc.query("variables.pickFor", &json!({ "ids": [frame], "field": "fill", "kind": "color" })).unwrap();
    let group = picker["groups"].as_array().unwrap().iter().find(|g| !g["library"].is_null()).cloned().unwrap_or_else(|| panic!("{picker}"));
    assert_eq!(
        (group["name"].as_str(), group["library"]["name"].as_str(), group["variables"][0]["name"].as_str()),
        (Some("Theme"), Some("Design System"), Some("Primary"))
    );
    let modes = doc.query("variable.modeRows", &json!({ "ids": [frame] })).unwrap();
    let theme = modes["rows"].as_array().unwrap().iter().find(|r| r["name"] == "Theme").cloned().unwrap_or_else(|| panic!("{modes}"));
    assert!(theme["collection"].is_null() && theme["key"].is_string() && theme["library"]["id"] == json!(library), "{theme}");

    // Bind the library variable and choose its mode: both copy it in.
    let variable = group["variables"][0]["id"].clone();
    doc.execute("library.bindVariable", json!({ "library": library, "variable": variable, "field": "fill", "ids": [frame] })).unwrap();
    doc.execute("library.setVariableMode", json!({ "library": library, "collection": theme["key"], "mode": "Dark", "ids": [frame] })).unwrap();
    let modes = doc.query("variable.modeRows", &json!({ "ids": [frame] })).unwrap();
    let theme = modes["rows"].as_array().unwrap().iter().find(|r| r["name"] == "Theme").cloned().unwrap();
    assert!(theme["collection"].is_string() && !theme["own"].is_null(), "the copy now holds the mode: {theme}");

    // Insert an instance of the library's component.
    let placed = doc.execute("library.place", json!({ "library": library, "component": "Button", "x": 100, "y": 100 })).unwrap();
    let instance = placed["id"].clone();
    assert_eq!(fill_of(&doc, &instance), "0000ff");
    assert!(missing_components(&doc).is_empty());

    // The library file is published again: the update shows and is accepted.
    std::fs::write(dir.join("libraries/Design System.kdesign"), &v2).unwrap();
    let updates = doc.query("library.updates", &json!({})).unwrap();
    assert_eq!(updates["count"], 1, "{updates}");
    assert_eq!(updates["libraries"][0]["components"][0]["name"], "Button");
    let accepted = doc.execute("library.acceptUpdates", json!({})).unwrap();
    assert_eq!(accepted["components"], 1, "{accepted}");
    assert_eq!(fill_of(&doc, &instance), "ff0000", "the instance follows its main");
    assert_eq!(doc.query("library.updates", &json!({})).unwrap()["count"], 0);
    doc.execute("edit.undo", json!({})).unwrap();
    assert_eq!(fill_of(&doc, &instance), "0000ff", "accepting is one undo step");
    doc.execute("edit.redo", json!({})).unwrap();

    // The library file is gone: its instances are reported.
    std::fs::remove_file(dir.join("libraries/Design System.kdesign")).unwrap();
    let missing = missing_components(&doc);
    assert_eq!(missing.len(), 1, "{missing:?}");
    assert!(missing[0].contains("Design System"), "{missing:?}");
    assert!(doc.query("assets.list", &json!({})).unwrap()["libraries"][0]["error"].is_string());
    unlend(&dir);
}

#[test]
fn the_editor_lends_library_bytes_and_the_path_travels_with_the_file() {
    let _turn = turn();
    kition_kit::host::set_root(None);
    let (v1, v2) = library_versions();
    let (mut doc, _) = design();
    // Nothing lent and no host files: a clear error, no change.
    assert!(doc.execute("library.add", json!({ "path": "Design System.kdesign" })).is_err());
    assert!(doc.lend_library("Design System.kdesign", Some(&v1)).unwrap(), "first lend changes what the path reads");
    assert!(!doc.lend_library("./Design System.kdesign", Some(&v1)).unwrap(), "the same bytes again change nothing");
    let library = doc.execute("library.add", json!({ "path": "Design System.kdesign" })).unwrap()["id"].as_str().unwrap().to_owned();
    let instance = doc.execute("library.place", json!({ "library": library, "component": "Button" })).unwrap()["id"].clone();
    let revision = doc.revision();
    assert!(doc.lend_library("Design System.kdesign", Some(&v2)).unwrap());
    assert_eq!(doc.revision(), revision, "lending is never an edit");
    assert_eq!(doc.query("library.updates", &json!({})).unwrap()["count"], 1);
    doc.execute("library.acceptUpdates", json!({})).unwrap();
    assert_eq!(fill_of(&doc, &instance), "ff0000");

    // Saved and opened again: the link is still the relative path, read from what is lent.
    let saved = doc.save_file(None).unwrap();
    let mut again = DesignDocument::open("Design.kdesign", &saved).unwrap();
    assert_eq!(again.query("library.files", &json!({})).unwrap()["files"][0]["path"], "Design System.kdesign");
    assert!(again.query("library.list", &json!({})).unwrap()["libraries"][0]["error"].is_string(), "not lent to this document yet");
    again.lend_library("Design System.kdesign", Some(&v2)).unwrap();
    let list = again.query("library.list", &json!({})).unwrap();
    assert_eq!((list["libraries"][0]["version"].as_u64(), list["libraries"][0]["updates"].as_u64()), (Some(2), Some(0)), "{list}");
    assert!(missing_components(&again).is_empty());
    // The editor could not read it any more.
    again.lend_library("Design System.kdesign", None).unwrap();
    assert_eq!(missing_components(&again).len(), 1);

    // Paths that are not workspace paths are refused.
    for bad in ["/Users/alice/Design System.kdesign.png", "C:/libs/Design System.kdesign", "https://example.com/a.kdesign", "../.."] {
        assert!(doc.lend_library(bad, Some(&v1)).is_err(), "{bad}");
        assert!(doc.execute("library.add", json!({ "path": bad })).is_err(), "{bad}");
    }
}

#[test]
fn a_library_is_published_from_the_open_file() {
    let _turn = turn();
    let (mut doc, frame) = design();
    doc.execute("node.set", json!({ "id": frame, "props": { "name": "Card" } })).unwrap();
    doc.execute("component.add", json!({ "ids": [frame] })).unwrap();
    let published = doc.execute("library.publish", json!({ "description": "Cards" })).unwrap();
    assert_eq!(published["version"], 1, "{published}");
    let saved = doc.save_file(None).unwrap();
    // Another design uses it.
    let (mut other, _) = design();
    other.lend_library("Cards.kdesign", Some(&saved)).unwrap();
    let added = other.execute("library.add", json!({ "path": "Cards.kdesign" })).unwrap();
    assert_eq!(added["components"][0]["name"], "Card", "{added}");
}

/// Exchanges every change between `a` and `b`, both ways (only one of them changed since the last
/// exchange: one line of changes).
fn sync(a: &mut DesignDocument, b: &mut DesignDocument) {
    let (va, vb) = (a.version().unwrap(), b.version().unwrap());
    let to_b = a.export_changes(&vb).unwrap();
    let to_a = b.export_changes(&va).unwrap();
    b.add_blobs(&a.blobs_since(&vb).unwrap()).unwrap();
    a.add_blobs(&b.blobs_since(&va).unwrap()).unwrap();
    b.import_changes(&to_b).unwrap();
    a.import_changes(&to_a).unwrap();
}

fn page_names(doc: &DesignDocument) -> Vec<String> {
    let ds = doc.session().ds().unwrap();
    ds.doc.pages.iter().filter_map(|p| ds.doc.node(*p)).map(|n| n.name.clone()).collect()
}

/// A second library file: the component "Icon".
fn icons() -> Vec<u8> {
    let mut s = Session::with_document(kition_design_doc::Document::new("Icons"));
    let f = ex(&mut s, "frame.add", json!({ "x": 0, "y": 0, "width": 24, "height": 24, "name": "Icon" }))["id"].clone();
    ex(&mut s, "component.add", json!({ "ids": [f] }));
    ex(&mut s, "library.publish", json!({}));
    kition_design_format::save_bytes(s.doc().unwrap()).unwrap()
}

/// Regression (`docs/features/kdm.md` V10): the person who made the library store undoes it after another
/// person added a copy of another component to it (and that change reached them). The other copy keeps a store (never a page),
/// its record, and its instance's main.
#[test]
fn undoing_the_store_keeps_the_copies_another_person_added() {
    let _turn = turn();
    kition_kit::host::set_root(None);
    let (v1, _) = library_versions();
    let icons = icons();
    let lend = |doc: &mut DesignDocument| {
        doc.lend_library("Design System.kdesign", Some(&v1)).unwrap();
        doc.lend_library("Icons.kdesign", Some(&icons)).unwrap();
    };
    let (mut a, _) = design();
    lend(&mut a);
    let system = a.execute("library.add", json!({ "path": "Design System.kdesign" })).unwrap()["id"].as_str().unwrap().to_owned();
    let icon_lib = a.execute("library.add", json!({ "path": "Icons.kdesign" })).unwrap()["id"].as_str().unwrap().to_owned();
    let mut b = DesignDocument::open_with_replica("Design.kdesign", &[], &a.snapshot().unwrap(), &a.blobs().unwrap()).unwrap();
    lend(&mut b);
    a.execute("library.place", json!({ "library": system, "component": "Button", "x": 100, "y": 100 })).unwrap();
    sync(&mut a, &mut b);
    let instance = b.execute("library.place", json!({ "library": icon_lib, "component": "Icon", "x": 300, "y": 100 })).unwrap()["id"].clone();
    sync(&mut a, &mut b);
    assert!(a.undo().unwrap().0, "A takes its placement back");
    sync(&mut a, &mut b);
    assert_eq!(a.state_json().unwrap(), b.state_json().unwrap());
    for doc in [&a, &b] {
        assert_eq!(page_names(doc), ["Page 1"], "the library store never shows as a page");
        let ds = doc.session().ds().unwrap();
        assert!(ds.doc.library_store().is_some(), "a store holds the copy that is left");
        let main = match &ds.doc.node(kition_design_engine::params::id_of(&instance).unwrap()).unwrap().kind {
            kition_design_doc::NodeKind::Instance { component, .. } => *component,
            other => panic!("{other:?}"),
        };
        assert!(ds.doc.is_remote(main), "B's instance still uses its library copy");
        let records: Vec<&str> = ds.doc.libraries.components.iter().map(|r| r.library.as_str()).collect();
        assert_eq!(records, [icon_lib.as_str()]);
        assert!(missing_components(doc).is_empty());
    }
}

/// The style picker's and the Local variables window's library sections (`styles.libraries`,
/// `variables.libraries`): each enabled library's published styles, collections and variables,
/// and which of this file's are linked copies of them.
#[test]
fn the_pickers_list_library_styles_and_collections_and_their_copies() {
    let _turn = turn();
    kition_kit::host::set_root(None);
    let (v1, _) = library_versions();
    let (mut doc, frame) = design();
    doc.lend_library("Design System.kdesign", Some(&v1)).unwrap();
    let library = doc.execute("library.add", json!({ "path": "Design System.kdesign" })).unwrap()["id"].as_str().unwrap().to_owned();

    let styles = doc.query("styles.libraries", &json!({ "type": "paint" })).unwrap();
    let section = &styles["libraries"][0];
    assert_eq!(
        (section["id"].as_str(), section["name"].as_str(), section["error"].is_null()),
        (Some(library.as_str()), Some("Design System"), true),
        "{styles}"
    );
    let brand = &section["styles"][0];
    assert_eq!((brand["name"].as_str(), brand["type"].as_str()), (Some("Brand"), Some("paint")), "{styles}");
    assert!(brand["paints"].is_array(), "a paint style carries its paints: {brand}");
    assert!(doc.query("styles.libraries", &json!({ "type": "text" })).unwrap()["libraries"][0]["styles"].as_array().unwrap().is_empty());
    assert_eq!(styles["copies"], json!({}));

    // Applied from the library: the copy is linked to its library style.
    doc.execute("library.applyStyle", json!({ "library": library, "style": brand["id"], "ids": [frame], "target": "fill" })).unwrap();
    let copies = doc.query("styles.libraries", &json!({})).unwrap()["copies"].clone();
    let (local, link) = copies.as_object().unwrap().iter().next().unwrap();
    assert_eq!(link, &json!({ "library": library, "style": brand["id"] }));
    assert!(doc.session().ds().unwrap().doc.style(local).is_some());

    let vars = doc.query("variables.libraries", &json!({})).unwrap();
    let section = &vars["libraries"][0];
    assert_eq!(section["collections"][0]["name"], "Theme", "{vars}");
    assert_eq!(section["collections"][0]["modes"].as_array().unwrap().len(), 2);
    assert_eq!(section["variables"][0]["name"], "Primary");
    assert_eq!(vars["copies"], json!({ "collections": [], "variables": [] }));
    let collection = section["collections"][0]["id"].clone();
    let extended = doc.execute("library.extendCollection", json!({ "library": library, "collection": collection })).unwrap()["id"].clone();
    let copies = doc.query("variables.libraries", &json!({})).unwrap()["copies"].clone();
    assert_eq!((copies["collections"].as_array().unwrap().len(), copies["variables"].as_array().unwrap().len()), (1, 1), "{copies}");
    assert!(!copies["collections"].as_array().unwrap().contains(&extended), "the extension is this file's own");
}

/// The main component of instance `id`, if `id` is an instance.
fn main_of(doc: &DesignDocument, id: &Value) -> Option<kition_design_doc::NodeId> {
    let ds = doc.session().ds().unwrap();
    match &ds.doc.node(kition_design_engine::params::id_of(id)?)?.kind {
        kition_design_doc::NodeKind::Instance { component, .. } => Some(*component),
        _ => None,
    }
}

/// Regression (`docs/features/kdm.md` V11, V12): an agent's draft places the library component
/// "Button" and binds the library variable while the person does the same meanwhile (the store
/// already exists). The merge applies (each side added objects of its own to the store's tree and
/// the lists of copies, styles, collections and variables) and the repair makes the two copies of
/// the component one, with one record, used by both instances, and the library style, collection
/// and variable copied on both sides one copy each. A new publication is then one update, and
/// accepting it updates both instances.
#[test]
fn a_draft_placing_the_component_the_person_placed_meanwhile_keeps_one_copy() {
    let _turn = turn();
    kition_kit::host::set_root(None);
    let (v1, v2) = library_versions();
    let icons = icons();
    let lend = |doc: &mut DesignDocument| {
        doc.lend_library("Design System.kdesign", Some(&v1)).unwrap();
        doc.lend_library("Icons.kdesign", Some(&icons)).unwrap();
    };
    let (mut a, frame) = design();
    lend(&mut a);
    let system = a.execute("library.add", json!({ "path": "Design System.kdesign" })).unwrap()["id"].as_str().unwrap().to_owned();
    let icon_lib = a.execute("library.add", json!({ "path": "Icons.kdesign" })).unwrap()["id"].as_str().unwrap().to_owned();
    // The store exists before the draft is made.
    a.execute("library.place", json!({ "library": icon_lib, "component": "Icon", "x": 500, "y": 100 })).unwrap();
    let other = a.execute("shape.rectangle", json!({ "x": 600, "y": 300, "width": 40, "height": 40 })).unwrap();
    let other = other.get("id").cloned().unwrap_or_else(|| other["ids"][0].clone());
    let base = a.version().unwrap();
    let mut draft = DesignDocument::open_with_replica("Design.kdesign", &[], &a.snapshot().unwrap(), &a.blobs().unwrap()).unwrap();
    lend(&mut draft);

    let picker = a.query("variables.pickFor", &json!({ "ids": [frame], "field": "fill", "kind": "color" })).unwrap();
    let variable = picker["groups"].as_array().unwrap().iter().find(|g| !g["library"].is_null()).unwrap()["variables"][0]["id"].clone();
    let mut instances = Vec::new();
    for (doc, x, target) in [(&mut draft, 300, &other), (&mut a, 100, &frame)] {
        instances.push(doc.execute("library.place", json!({ "library": system, "component": "Button", "x": x, "y": 100 })).unwrap()["id"].clone());
        doc.execute("library.bindVariable", json!({ "library": system, "variable": variable, "field": "fill", "ids": [target] })).unwrap();
    }
    let changes = draft.export_changes(&base).unwrap();
    a.add_blobs(&draft.blobs_since(&base).unwrap()).unwrap();
    a.merge_changes(&changes, &kition_kit::kdm::Origin::agent("agent:drafter")).unwrap();

    assert_eq!(page_names(&a), ["Page 1"], "the library store never shows as a page");
    {
        let ds = a.session().ds().unwrap();
        let libs = &ds.doc.libraries;
        let buttons: Vec<_> = libs.components.iter().filter(|r| r.library == system).collect();
        assert_eq!(buttons.len(), 1, "the two copies are one, with one record: {libs:?}");
        let root = buttons[0].root;
        let store = ds.doc.library_store().unwrap();
        let children = &ds.doc.node(store).unwrap().children;
        assert_eq!(children.iter().filter(|c| **c == root).count(), 1, "{children:?}");
        assert_eq!(children.len(), 2, "the icon's copy and the one button copy: {children:?}");
        for instance in &instances {
            assert_eq!(main_of(&a, instance), Some(root), "every instance uses the one copy");
        }
        let copies: Vec<&str> = ds.doc.styles.iter().filter(|s| s.name == "Brand").map(|s| s.id.as_str()).collect();
        assert_eq!(copies.len(), 1, "one copy of the library style: {copies:?}");
        assert_eq!(libs.styles.len(), 1, "{:?}", libs.styles);
        assert!(libs.style_import(copies[0]).is_some());
        let themes = ds.doc.variables.collections.iter().filter(|c| c.name == "Theme").count();
        assert_eq!((themes, libs.collections.len(), libs.variables.len()), (1, 1, 1), "one copy of the library variable and collection");
    }
    assert!(missing_components(&a).is_empty(), "{:?}", missing_components(&a));

    // The library is published again: one update, and one acceptance updates both instances.
    a.lend_library("Design System.kdesign", Some(&v2)).unwrap();
    assert_eq!(a.query("library.updates", &json!({})).unwrap()["count"], 1, "the component counts once");
    a.execute("library.acceptUpdates", json!({})).unwrap();
    for instance in &instances {
        assert_eq!(fill_of(&a, instance), "ff0000", "every instance follows its library");
    }
    assert_eq!(a.session().ds().unwrap().doc.libraries.components.iter().filter(|r| r.library == system).count(), 1);
}

/// A library whose component "Label" holds the text layer "Sign in".
fn label_library() -> Vec<u8> {
    let mut s = Session::with_document(kition_design_doc::Document::new("Labels"));
    let f = ex(&mut s, "frame.add", json!({ "x": 0, "y": 0, "width": 120, "height": 40, "name": "Label" }))["id"].clone();
    ex(&mut s, "text.add", json!({ "text": "Sign in", "x": 8, "y": 8, "parent": f }));
    ex(&mut s, "component.add", json!({ "ids": [f] }));
    ex(&mut s, "library.publish", json!({}));
    kition_design_format::save_bytes(s.doc().unwrap()).unwrap()
}

/// An agent's draft places the library component "Label" while the person places it meanwhile
/// (the store already exists): the design, the draft's changes and both instances. Merging them
/// leaves two copies of the component that the repair (V12) makes one, copying the kept copy's
/// text, which it reads in the merge's transaction.
fn a_draft_placing_the_label_twice() -> (DesignDocument, Vec<u8>, Vec<Value>, String) {
    kition_kit::host::set_root(None);
    let (labels, icons) = (label_library(), icons());
    let lend = |doc: &mut DesignDocument| {
        doc.lend_library("Labels.kdesign", Some(&labels)).unwrap();
        doc.lend_library("Icons.kdesign", Some(&icons)).unwrap();
    };
    let (mut a, _) = design();
    lend(&mut a);
    let library = a.execute("library.add", json!({ "path": "Labels.kdesign" })).unwrap()["id"].as_str().unwrap().to_owned();
    let icon_lib = a.execute("library.add", json!({ "path": "Icons.kdesign" })).unwrap()["id"].as_str().unwrap().to_owned();
    a.execute("library.place", json!({ "library": icon_lib, "component": "Icon", "x": 500, "y": 100 })).unwrap();
    let base = a.version().unwrap();
    let mut draft = DesignDocument::open_with_replica("Design.kdesign", &[], &a.snapshot().unwrap(), &a.blobs().unwrap()).unwrap();
    lend(&mut draft);
    let mut instances = Vec::new();
    for (doc, x) in [(&mut draft, 300), (&mut a, 100)] {
        instances.push(doc.execute("library.place", json!({ "library": library, "component": "Label", "x": x, "y": 100 })).unwrap()["id"].clone());
    }
    let changes = draft.export_changes(&base).unwrap();
    a.add_blobs(&draft.blobs_since(&base).unwrap()).unwrap();
    (a, changes, instances, library)
}

/// The text layers of every copy of `library`'s component in the library store.
fn label_copy_text(doc: &DesignDocument, library: &str) -> Vec<String> {
    let ds = doc.session().ds().unwrap();
    let mut texts = Vec::new();
    for record in ds.doc.libraries.components.iter().filter(|r| r.library == library) {
        for child in &ds.doc.node(record.root).unwrap().children {
            if let Some(text) = ds.doc.node(*child).and_then(|n| n.kind.text()) {
                texts.push(text.characters.clone());
            }
        }
    }
    texts
}

/// Regression (`docs/features/kdm.md` section 5, V12; kition-sdk regressions/issue-merge-repair-reads-text):
/// the repair reads the merged copies' text, so it ran as a separate `system` step after the merge
/// and the person's one Undo of the merge left the repair's copy behind. It now runs inside the
/// merge's step (`Replica::merge_changes_repaired`, reading text through `Tx::text_delta`): one
/// copy with the library's text, one Undo gives back the exact state before the merge and one Redo
/// the exact repaired state.
#[test]
fn a_merge_whose_repair_copies_text_undoes_in_one_step_exactly() {
    let _turn = turn();
    let (mut a, changes, instances, library) = a_draft_placing_the_label_twice();
    let hash = |doc: &DesignDocument| doc.session().kdm().unwrap().state_hash().unwrap();
    let before = hash(&a);
    let merged = a.merge_changes(&changes, &kition_kit::kdm::Origin::agent("agent:drafter")).unwrap();
    assert_eq!(merged["scope"], "person", "{merged}");
    assert_eq!(label_copy_text(&a, &library), ["Sign in"], "one copy, with the text the repair copied");
    let root = a.session().ds().unwrap().doc.libraries.components.iter().find(|r| r.library == library).map(|r| r.root);
    assert!(root.is_some());
    for instance in &instances {
        assert_eq!(main_of(&a, instance), root, "every instance uses the one copy");
    }
    let repaired = hash(&a);
    assert!(a.undo().unwrap().0);
    assert_eq!(hash(&a), before, "one undo: the exact state before the merge");
    assert!(a.redo().unwrap().0);
    assert_eq!(hash(&a), repaired, "one redo: the exact repaired state");
    assert_eq!(label_copy_text(&a, &library), ["Sign in"]);
}

/// Regression (kition-sdk regressions/issue-draft-preview-repaired): a draft's preview
/// (`compareChanges`) read the merge without the repair, so it showed two copies of "Label" where
/// accepting the draft gives one. It now runs the repair on its copy: the comparison is what the
/// merge changed and the previewed state is the merged one; comparing changes nothing.
#[test]
fn a_drafts_preview_shows_the_repaired_merge() {
    let _turn = turn();
    let (mut a, changes, _, library) = a_draft_placing_the_label_twice();
    let kdm = a.session().kdm().unwrap();
    let (version, hash) = (kdm.version(), kdm.state_hash().unwrap());
    let unrepaired = kdm.replica().merged_state(&changes).unwrap();
    let previewed = kdm.merged_state(&changes).unwrap();
    assert_ne!(previewed, unrepaired, "the repair changes this merge");
    let compared = a.compare_changes(&changes).unwrap();
    let kdm = a.session().kdm().unwrap();
    assert_eq!((kdm.version(), kdm.state_hash().unwrap()), (version, hash), "a preview changes nothing");
    let merged = a.merge_changes(&changes, &kition_kit::kdm::Origin::agent("agent:drafter")).unwrap();
    assert_eq!(merged["summary"], compared, "the comparison is what accepting changed: {merged}");
    assert_eq!(a.session().kdm().unwrap().replica().state_json().unwrap(), previewed, "the preview is the merge");
    assert_eq!(label_copy_text(&a, &library), ["Sign in"]);
}
