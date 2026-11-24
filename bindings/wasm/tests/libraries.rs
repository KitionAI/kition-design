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
    let f = ex(&mut s, "frame.create", json!({ "x": 0, "y": 0, "width": 120, "height": 40, "name": "Button" }))["id"].clone();
    ex(&mut s, "node.set", json!({ "id": f, "props": { "fill": "#0000ff" } }));
    ex(&mut s, "style.create", json!({ "type": "paint", "name": "Brand", "from": f }));
    let button = ex(&mut s, "component.create", json!({ "ids": [f] }))["id"].clone();
    let theme = ex(&mut s, "variable.createCollection", json!({ "name": "Theme", "modes": ["Light", "Dark"] }))["id"].clone();
    ex(
        &mut s,
        "variable.create",
        json!({ "collection": theme, "name": "Primary", "type": "color", "values": { "Light": "#ffffff", "Dark": "#000000" } }),
    );
    ex(&mut s, "library.publish", json!({ "description": "Synthetic test library" }));
    let v1 = kition_design_format::save_bytes(s.doc().unwrap(), None).unwrap();
    ex(&mut s, "node.set", json!({ "id": button, "props": { "fill": "#ff0000" } }));
    ex(&mut s, "library.publish", json!({}));
    let v2 = kition_design_format::save_bytes(s.doc().unwrap(), None).unwrap();
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
    let saved = doc.seal_and_save(None).unwrap();
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
    doc.execute("component.create", json!({ "ids": [frame] })).unwrap();
    let published = doc.execute("library.publish", json!({ "description": "Cards" })).unwrap();
    assert_eq!(published["version"], 1, "{published}");
    let saved = doc.seal_and_save(None).unwrap();
    // Another design uses it.
    let (mut other, _) = design();
    other.lend_library("Cards.kdesign", Some(&saved)).unwrap();
    let added = other.execute("library.add", json!({ "path": "Cards.kdesign" })).unwrap();
    assert_eq!(added["components"][0]["name"], "Card", "{added}");
}

/// Exchanges every change between `a` and `b`, both ways.
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

/// Regression (`docs/features/kdm.md` F16, V10, V11, V12): two people place the same library component
/// at once in a design that has no library copies yet. Neither side's copies turn into a page,
/// the two copies of the component become one with one record (V12), the library style and
/// variable both copied are one copy each, and every instance still follows its library.
#[test]
fn two_people_placing_library_components_at_once_keep_one_store_and_one_copy() {
    let _turn = turn();
    kition_kit::host::set_root(None);
    let (v1, v2) = library_versions();
    let (mut a, frame) = design();
    a.lend_library("Design System.kdesign", Some(&v1)).unwrap();
    let library = a.execute("library.add", json!({ "path": "Design System.kdesign" })).unwrap()["id"].as_str().unwrap().to_owned();
    let snapshot = a.snapshot().unwrap();
    let mut b = DesignDocument::open_with_replica("Design.kdesign", &[], &snapshot, &a.blobs().unwrap()).unwrap();
    b.lend_library("Design System.kdesign", Some(&v1)).unwrap();
    assert_eq!(a.state_json().unwrap(), b.state_json().unwrap());

    // At the same time: each places the button and binds the library variable.
    let picker = a.query("variables.pickFor", &json!({ "ids": [frame], "field": "fill", "kind": "color" })).unwrap();
    let variable = picker["groups"].as_array().unwrap().iter().find(|g| !g["library"].is_null()).unwrap()["variables"][0]["id"].clone();
    let mut instances = Vec::new();
    for (doc, x) in [(&mut a, 100), (&mut b, 300)] {
        instances.push(doc.execute("library.place", json!({ "library": library, "component": "Button", "x": x, "y": 100 })).unwrap()["id"].clone());
        doc.execute("library.bindVariable", json!({ "library": library, "variable": variable, "field": "fill", "ids": [frame] })).unwrap();
    }
    sync(&mut a, &mut b);
    assert_eq!(a.state_json().unwrap(), b.state_json().unwrap());

    for doc in [&a, &b] {
        assert_eq!(page_names(doc), ["Page 1"], "the library store never shows as a page");
        let ds = doc.session().ds().unwrap();
        let libs = &ds.doc.libraries;
        assert_eq!(libs.components.len(), 1, "the two copies are one, with one record: {libs:?}");
        let store = ds.doc.library_store().unwrap();
        assert_eq!(ds.doc.node(store).unwrap().children, [libs.components[0].root], "the one copy is in the one store");
        for instance in &instances {
            assert_eq!(main_of(doc, instance), Some(libs.components[0].root), "every instance uses the one copy");
        }
        for r in &libs.components {
            assert!(ds.doc.is_remote(r.root), "{r:?}");
        }
        let copies: Vec<&str> = ds.doc.styles.iter().filter(|s| s.name == "Brand").map(|s| s.id.as_str()).collect();
        assert_eq!(copies.len(), 1, "one copy of the library style: {copies:?}");
        assert_eq!(libs.styles.len(), 1, "{:?}", libs.styles);
        assert!(libs.style_import(copies[0]).is_some());
        let themes = ds.doc.variables.collections.iter().filter(|c| c.name == "Theme").count();
        assert_eq!((themes, libs.collections.len(), libs.variables.len()), (1, 1, 1), "one copy of the library variable and collection");
        assert!(missing_components(doc).is_empty());
    }

    // The library is published again: one acceptance updates both people's copies (once the
    // repairs both replicas made have reached each other, V12).
    sync(&mut a, &mut b);
    for doc in [&mut a, &mut b] {
        doc.lend_library("Design System.kdesign", Some(&v2)).unwrap();
    }
    assert_eq!(a.query("library.updates", &json!({})).unwrap()["count"], 1, "the component counts once");
    a.execute("library.acceptUpdates", json!({})).unwrap();
    sync(&mut a, &mut b);
    assert_eq!(a.state_json().unwrap(), b.state_json().unwrap());
    for doc in [&a, &b] {
        for instance in &instances {
            assert_eq!(fill_of(doc, instance), "ff0000", "every instance follows its library");
        }
        assert_eq!(doc.session().ds().unwrap().doc.libraries.components.len(), 1);
        assert_eq!(doc.session().ds().unwrap().doc.libraries.components.len(), 1);
    }
}

/// A second library file: the component "Icon".
fn icons() -> Vec<u8> {
    let mut s = Session::with_document(kition_design_doc::Document::new("Icons"));
    let f = ex(&mut s, "frame.create", json!({ "x": 0, "y": 0, "width": 24, "height": 24, "name": "Icon" }))["id"].clone();
    ex(&mut s, "component.create", json!({ "ids": [f] }));
    ex(&mut s, "library.publish", json!({}));
    kition_design_format::save_bytes(s.doc().unwrap(), None).unwrap()
}

/// Regression (`docs/features/kdm.md` V10): the person who made the library store undoes it after another
/// person added a copy of another component to it. The other copy keeps a store (never a page),
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

/// One library file published twice: the component "Card" with a child "Dot" (blue, then red).
fn card_versions() -> (Vec<u8>, Vec<u8>) {
    let mut s = Session::with_document(kition_design_doc::Document::new("Cards"));
    let f = ex(&mut s, "frame.create", json!({ "x": 0, "y": 0, "width": 120, "height": 40, "name": "Card" }))["id"].clone();
    let dot =
        ex(&mut s, "shape.rectangle", json!({ "x": 10, "y": 10, "width": 20, "height": 20, "parent": f, "props": { "fill": "#0000ff" } }))["id"]
            .clone();
    ex(&mut s, "node.set", json!({ "id": dot, "props": { "name": "Dot" } }));
    ex(&mut s, "component.create", json!({ "ids": [f] }));
    ex(&mut s, "library.publish", json!({}));
    let v1 = kition_design_format::save_bytes(s.doc().unwrap(), None).unwrap();
    ex(&mut s, "node.set", json!({ "id": dot, "props": { "fill": "#ff0000" } }));
    ex(&mut s, "library.publish", json!({}));
    let v2 = kition_design_format::save_bytes(s.doc().unwrap(), None).unwrap();
    (v1, v2)
}

/// Sends every change `from` has that `to` lacks (one way).
fn send(from: &mut DesignDocument, to: &mut DesignDocument) {
    let v = to.version().unwrap();
    to.add_blobs(&from.blobs_since(&v).unwrap()).unwrap();
    to.import_changes(&from.export_changes(&v).unwrap()).unwrap();
}

/// Two replicas of a design that uses the "Cards" library (version 1 lent to both).
fn card_pair() -> (DesignDocument, DesignDocument, String, Vec<u8>) {
    kition_kit::host::set_root(None);
    let (v1, v2) = card_versions();
    let (mut a, _) = design();
    a.lend_library("Cards.kdesign", Some(&v1)).unwrap();
    let library = a.execute("library.add", json!({ "path": "Cards.kdesign" })).unwrap()["id"].as_str().unwrap().to_owned();
    let mut b = DesignDocument::open_with_replica("Design.kdesign", &[], &a.snapshot().unwrap(), &a.blobs().unwrap()).unwrap();
    b.lend_library("Cards.kdesign", Some(&v1)).unwrap();
    (a, b, library, v2)
}

fn place_card(doc: &mut DesignDocument, library: &str, x: i64) -> Value {
    doc.execute("library.place", json!({ "library": library, "component": "Card", "x": x, "y": 100 })).unwrap()["id"].clone()
}

/// The main component of instance `id`, if `id` is an instance.
fn main_of(doc: &DesignDocument, id: &Value) -> Option<kition_design_doc::NodeId> {
    let ds = doc.session().ds().unwrap();
    match &ds.doc.node(kition_design_engine::params::id_of(id)?)?.kind {
        kition_design_doc::NodeKind::Instance { component, .. } => Some(*component),
        _ => None,
    }
}

/// One copy of "Card" in the store, with one record, and every instance of `instances` uses it:
/// its main is the copy's root and the layers inside it mirror the copy's layers.
fn assert_one_card_copy(doc: &DesignDocument, instances: &[&Value]) {
    let ds = doc.session().ds().unwrap();
    let d = &ds.doc;
    assert_eq!(page_names(doc), ["Page 1"], "the library store never shows as a page");
    assert_eq!(d.libraries.components.len(), 1, "one copy record: {:?}", d.libraries.components);
    let record = &d.libraries.components[0];
    let store = d.library_store().unwrap();
    assert_eq!(d.node(store).unwrap().children, [record.root], "one copy in the store");
    let copy: std::collections::BTreeSet<_> = d.subtree(record.root).into_iter().collect();
    assert_eq!(copy.len(), record.ids.len(), "the record names every layer of the copy");
    for instance in instances {
        let id = kition_design_engine::params::id_of(instance).unwrap();
        assert_eq!(main_of(doc, instance), Some(record.root), "instance {instance} uses the one copy");
        for layer in d.subtree(id).into_iter().skip(1) {
            let source = d.node(layer).unwrap().source;
            assert!(source.is_some_and(|s| copy.contains(&s)), "layer {layer} of {instance} mirrors the copy: {source:?}");
        }
    }
    assert!(missing_components(doc).is_empty(), "{:?}", missing_components(doc));
}

fn dot_of(doc: &DesignDocument, instance: &Value) -> Value {
    let ds = doc.session().ds().unwrap();
    let id = kition_design_engine::params::id_of(instance).unwrap();
    json!(ds.doc.node(id).unwrap().children[0].0)
}

fn overrides_of(doc: &DesignDocument, id: &Value) -> Vec<String> {
    let ds = doc.session().ds().unwrap();
    ds.doc.node(kition_design_engine::params::id_of(id).unwrap()).unwrap().overrides.iter().cloned().collect()
}

/// Regression (`docs/features/kdm.md` V12): two people place the same library component at once. The two
/// copies become one, every instance uses it and keeps its overrides, and the update count names
/// the component once.
#[test]
fn two_people_placing_one_component_at_once_keep_one_copy() {
    let _turn = turn();
    let (mut a, mut b, library, v2) = card_pair();
    let ia = place_card(&mut a, &library, 100);
    let ib = place_card(&mut b, &library, 300);
    // B overrides its instance's dot.
    let dot_b = dot_of(&b, &ib);
    b.execute("node.set", json!({ "id": dot_b, "props": { "fill": "#00ff00" } })).unwrap();
    sync(&mut a, &mut b);
    assert_eq!(a.state_json().unwrap(), b.state_json().unwrap());
    // Each replica made the merged copy in its own repair; those repairs reach the other side
    // before the copy is edited (an edit of it made before then can be lost, `docs/features/kdm.md` V12).
    sync(&mut a, &mut b);
    for doc in [&a, &b] {
        assert_one_card_copy(doc, &[&ia, &ib]);
        assert_eq!(fill_of(doc, &dot_b), "00ff00", "B's override is kept");
        assert_eq!(overrides_of(doc, &dot_b), ["fills"]);
        assert_eq!(fill_of(doc, &dot_of(doc, &ia)), "0000ff");
    }

    // Published again: the update names the component once, and one acceptance updates it.
    for doc in [&mut a, &mut b] {
        doc.lend_library("Cards.kdesign", Some(&v2)).unwrap();
    }
    let updates = b.query("library.updates", &json!({})).unwrap();
    assert_eq!(updates["count"], 1, "{updates}");
    assert_eq!(updates["libraries"][0]["components"].as_array().unwrap().len(), 1, "{updates}");
    let listed = b.query("library.list", &json!({})).unwrap();
    assert_eq!(listed["libraries"][0]["updates"], 1, "{listed}");
    let accepted = b.execute("library.acceptUpdates", json!({})).unwrap();
    assert_eq!(accepted["components"], 1, "{accepted}");
    sync(&mut a, &mut b);
    assert_eq!(a.state_json().unwrap(), b.state_json().unwrap());
    for doc in [&a, &b] {
        assert_one_card_copy(doc, &[&ia, &ib]);
        assert_eq!(fill_of(doc, &dot_of(doc, &ia)), "ff0000", "A's instance follows the library");
        assert_eq!(fill_of(doc, &dot_b), "00ff00", "B's override survives the update");
        assert_eq!(doc.query("library.updates", &json!({})).unwrap()["count"], 0);
    }
}

/// V12 and undo: after the merge, either person takes their placement back (and puts it back
/// again). The other person's instance keeps its main, and one copy is left.
#[test]
fn undoing_a_placement_after_the_merge_keeps_the_other_instance() {
    let _turn = turn();
    for undoer in 0..2 {
        let (mut a, mut b, library, _) = card_pair();
        let ia = place_card(&mut a, &library, 100);
        let ib = place_card(&mut b, &library, 300);
        sync(&mut a, &mut b);
        let (kept, undone) = if undoer == 0 { (&ib, &ia) } else { (&ia, &ib) };
        {
            let doc = if undoer == 0 { &mut a } else { &mut b };
            assert!(doc.undo().unwrap().0, "person {undoer} takes the placement back");
        }
        sync(&mut a, &mut b);
        assert_eq!(a.state_json().unwrap(), b.state_json().unwrap(), "undoer {undoer}");
        for doc in [&a, &b] {
            assert!(main_of(doc, undone).is_none(), "the undone instance is gone");
            assert_one_card_copy(doc, &[kept]);
        }
        {
            let doc = if undoer == 0 { &mut a } else { &mut b };
            assert!(doc.redo().unwrap().0, "person {undoer} puts it back");
        }
        sync(&mut a, &mut b);
        assert_eq!(a.state_json().unwrap(), b.state_json().unwrap(), "undoer {undoer} redo");
        for doc in [&a, &b] {
            assert_one_card_copy(doc, &[kept, undone]);
        }
    }
}

/// V12: a reference to a merged copy that arrives after the merge (B duplicated its instance
/// before it heard of A's placement and of the merge A's replica made) follows the kept copy.
#[test]
fn a_late_reference_to_a_merged_copy_follows_the_kept_copy() {
    let _turn = turn();
    let (mut a, mut b, library, _) = card_pair();
    let ia = place_card(&mut a, &library, 100);
    let ib = place_card(&mut b, &library, 300);
    send(&mut b, &mut a);
    assert_one_card_copy(&a, &[&ia, &ib]);
    let copy = b.execute("edit.duplicate", json!({ "ids": [ib] })).unwrap();
    let ic = copy["ids"][0].clone();
    assert!(main_of(&b, &ic).is_some(), "{copy}");
    sync(&mut a, &mut b);
    assert_eq!(a.state_json().unwrap(), b.state_json().unwrap());
    for doc in [&a, &b] {
        assert_one_card_copy(doc, &[&ia, &ib, &ic]);
    }
}

/// V12 and a concurrent edit of a layer the merge rewrote: A's replica merged the copies and
/// pointed B's instance at the kept copy, while B (not yet hearing of it) changed a layer of its
/// instance (its blend mode, a `props` register; the merge writes the `mirrors` register, F18).
/// The instance ends on the kept copy, and an undo of B's edit after the merge leaves it there.
#[test]
fn an_edit_that_crosses_the_merge_keeps_the_instance_on_the_kept_copy() {
    let _turn = turn();
    let (mut a, mut b, library, _) = card_pair();
    let ia = place_card(&mut a, &library, 100);
    let ib = place_card(&mut b, &library, 300);
    send(&mut b, &mut a);
    assert_one_card_copy(&a, &[&ia, &ib]);
    let dot_b = dot_of(&b, &ib);
    b.execute("node.set", json!({ "id": dot_b, "props": { "blend": "multiply" } })).unwrap();
    sync(&mut a, &mut b);
    sync(&mut a, &mut b);
    assert_eq!(a.state_json().unwrap(), b.state_json().unwrap());
    for doc in [&a, &b] {
        assert_one_card_copy(doc, &[&ia, &ib]);
    }
    assert!(b.undo().unwrap().0, "B takes the blend mode back");
    sync(&mut a, &mut b);
    assert_eq!(a.state_json().unwrap(), b.state_json().unwrap());
    for doc in [&a, &b] {
        assert_one_card_copy(doc, &[&ia, &ib]);
    }
}

fn blend_of(doc: &DesignDocument, id: &Value) -> kition_design_doc::color::BlendMode {
    let ds = doc.session().ds().unwrap();
    ds.doc.node(kition_design_engine::params::id_of(id).unwrap()).unwrap().blend
}

/// Regression (`docs/features/kdm.md` F18, schema v4): B sets a blend mode on a layer of its instance (a
/// layer that mirrors the copy's), then the copies merge (V12), which points that layer at the
/// kept copy. The pointer is its own register (`mirrors`), so the repair leaves the blend mode's
/// register alone and B's undo of the blend mode still takes it back, on both replicas.
#[test]
fn undoing_an_edit_of_a_mirrored_layer_made_before_the_merge_reverts_it() {
    let _turn = turn();
    let (mut a, mut b, library, _) = card_pair();
    let ia = place_card(&mut a, &library, 100);
    let ib = place_card(&mut b, &library, 300);
    let dot_b = dot_of(&b, &ib);
    let before = blend_of(&b, &dot_b);
    b.execute("node.set", json!({ "id": dot_b, "props": { "blend": "multiply" } })).unwrap();
    assert_ne!(blend_of(&b, &dot_b), before);
    sync(&mut a, &mut b);
    sync(&mut a, &mut b);
    assert_eq!(a.state_json().unwrap(), b.state_json().unwrap());
    for doc in [&a, &b] {
        assert_one_card_copy(doc, &[&ia, &ib]);
        assert_eq!(blend_of(doc, &dot_b), kition_design_doc::color::BlendMode::Multiply);
    }
    assert!(b.undo().unwrap().0, "B takes the blend mode back");
    sync(&mut a, &mut b);
    assert_eq!(a.state_json().unwrap(), b.state_json().unwrap());
    for doc in [&a, &b] {
        assert_eq!(blend_of(doc, &dot_b), before, "the blend mode set before the merge is undone");
        assert_one_card_copy(doc, &[&ia, &ib]);
    }
}
