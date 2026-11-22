//! The KHP Agent-ready phases (KHP `spec/conformance.md`) through kition-sdk's shared check
//! (`kition_headless::agent_ready`), against the headless kit's own backend
//! (`kition_headless::Headless<DesignDocument>`, what `kition-design` serves over the Kit Protocol) on the
//! fixtures in `conformance/`; then what only Design can say: the edit adds one layer, a bad node id is
//! refused, previews are framed to the content and to a diagnostic's target, and a new design checks
//! clean in the engine. The transport itself is Kition's `kition-kit-conformance`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use kition_design_kit::DesignDocument;
use kition_headless::{Headless, HostFolder, KitInfo};
use kition_kit::KitDocument;
use kition_kit_server::protocol::{Origin, ScreenshotScope};
use kition_kit_server::{ErrorReason, KitBackend, OpenRequest, ScreenshotRequest};
use serde_json::{Map, Value, json};

const MANIFEST: &str = include_str!("../../../kit.json");

fn repo() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

fn manifest() -> Value {
    serde_json::from_str(MANIFEST).unwrap()
}

/// A fresh workspace holding a copy of the conformance fixtures.
fn workspace(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kition-design-agent-ready-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for entry in std::fs::read_dir(repo().join("conformance")).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file() {
            std::fs::copy(entry.path(), dir.join(entry.file_name())).unwrap();
        }
    }
    std::fs::canonicalize(dir).unwrap()
}

/// The kit with the files of `root` served by a host folder.
fn kit(root: &Path) -> (Headless<DesignDocument>, HostFolder) {
    let info = KitInfo::from_manifest(MANIFEST, kition_design_kit::SANDBOXED, kition_design_kit::EDITOR_STATE).unwrap();
    let mut kit = Headless::<DesignDocument>::new(info);
    let folder = HostFolder::new(root).unwrap();
    kit.set_host(folder.host());
    (kit, folder)
}

fn open(kit: &mut Headless<DesignDocument>, folder: &HostFolder, name: &str) -> String {
    kit.open(OpenRequest { file: &folder.file(name).unwrap(), read_only: false, create: None }).unwrap().id
}

fn params(value: Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

#[test]
fn passes_the_shared_agent_ready_check() {
    if let Err(failures) = kition_headless::agent_ready::check(MANIFEST, repo(), kit_without_workspace) {
        panic!("{failures:#?}");
    }
}

fn kit_without_workspace() -> Headless<DesignDocument> {
    let info = KitInfo::from_manifest(MANIFEST, kition_design_kit::SANDBOXED, kition_design_kit::EDITOR_STATE).unwrap();
    Headless::<DesignDocument>::new(info)
}

#[test]
fn the_edit_adds_one_layer_and_a_bad_node_is_refused() {
    let root = workspace("act");
    let (mut kit, folder) = kit(&root);
    let file = root.join("basic.kdesign");
    let id = open(&mut kit, &folder, "basic.kdesign");
    let edit: Value = serde_json::from_str(&std::fs::read_to_string(root.join("edit.json")).unwrap()).unwrap();
    let layers = |kit: &mut Headless<DesignDocument>, id: &str| kit.describe(id).unwrap().state["layers"].as_u64().unwrap();
    let before = layers(&mut kit, &id);
    for step in edit["steps"].as_array().unwrap() {
        let executed = kit.execute(&id, step["command"].as_str().unwrap(), params(step["params"].clone()), Origin::Agent).unwrap();
        assert!(executed.changed);
    }
    kit.save(&id, None, None).unwrap();
    kit.close(&id, false).unwrap();
    let id = open(&mut kit, &folder, "basic.kdesign");
    assert_eq!(layers(&mut kit, &id), before + 1, "the edit survives a save and a reopen");

    let bytes = std::fs::read(&file).unwrap();
    let unknown = kit.execute(&id, "no.such.command", Map::new(), Origin::Agent).unwrap_err();
    assert_eq!(unknown.code, ErrorReason::CommandNotFound.code());
    let bad = kit.execute(&id, "node.set", params(json!({ "id": 999_999, "props": { "x": 1 } })), Origin::Agent).unwrap_err();
    assert!(!bad.message.is_empty());
    assert!(!kit.document(&id).unwrap().dirty, "failed calls leave the design as it was");
    assert_eq!(std::fs::read(&file).unwrap(), bytes, "and the file byte for byte");
    let _ = std::fs::remove_dir_all(root);
}

fn decode(png: &[u8]) -> image::RgbaImage {
    image::load_from_memory_with_format(png, image::ImageFormat::Png).unwrap().to_rgba8()
}

fn distinct(picture: &image::RgbaImage) -> usize {
    picture.pixels().map(|p| p.0).collect::<BTreeSet<_>>().len()
}

#[test]
fn previews_are_framed_and_targeted() {
    let root = workspace("preview");
    let (mut kit, folder) = kit(&root);
    let id = open(&mut kit, &folder, "basic.kdesign");
    let shot = kit.screenshot(&id, &ScreenshotRequest { scope: ScreenshotScope::Canvas, max_size: 512, scale: 1.0, target: None }).unwrap();
    assert!(distinct(&decode(&shot.png)) > 4, "view.screenshot is not uniform");
    let render = |kit: &mut Headless<DesignDocument>, p: Value| {
        let answer = kit.execute(&id, "file.render", params(p), Origin::Agent).unwrap().result.unwrap();
        decode(&base64::engine::general_purpose::STANDARD.decode(answer["png"].as_str().unwrap()).unwrap())
    };
    let whole = render(&mut kit, json!({}));
    assert_eq!(whole.dimensions(), (400, 300), "framed to the content");
    assert!(distinct(&whole) > 4);
    let expected = decode(&std::fs::read(repo().join("conformance/expected/basic.png")).unwrap());
    assert_eq!(expected.dimensions(), whole.dimensions());
    assert_eq!(render(&mut kit, json!({ "scale": 2 })).dimensions(), (800, 600));
    assert_eq!(render(&mut kit, json!({ "slide": 0 })).dimensions(), (400, 300), "a host's part preview");
    let diagnostics = {
        let fixture = open(&mut kit, &folder, "check-off-pixel.kdesign");
        let answer = kit.check(&fixture).unwrap();
        (fixture, answer["diagnostics"][0]["target"].clone())
    };
    let (fixture, target) = diagnostics;
    let answer = kit.execute(&fixture, "file.render", params(json!({ "target": target })), Origin::Agent).unwrap().result.unwrap();
    assert_eq!((answer["width"].as_u64(), answer["height"].as_u64()), (Some(121), Some(81)), "the diagnostic's layer, on whole pixels");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn create_and_export_write_new_files_only() {
    let root = workspace("files");
    let (mut kit, folder) = kit(&root);
    let manifest = manifest();
    for entry in manifest["creates"].as_array().unwrap() {
        let name = format!("{}.{}", entry["baseName"].as_str().unwrap(), entry["extension"].as_str().unwrap());
        let path = root.join(&name);
        let options = params(entry["size"].clone());
        let file = folder.file(&name).unwrap();
        let info = kit.open(OpenRequest { file: &file, read_only: false, create: Some((entry["id"].as_str().unwrap(), Some(&options))) }).unwrap();
        assert!(info.dirty, "a new design is unsaved");
        kit.save(&info.id, None, None).unwrap();
        let reopened = DesignDocument::open("new.kdesign", &std::fs::read(&path).unwrap()).unwrap();
        assert!(kit_check_is_clean(&reopened));
    }
    let id = open(&mut kit, &folder, "basic.kdesign");
    for entry in manifest["exports"].as_array().unwrap() {
        let (extension, command) = (entry["extension"].as_str().unwrap(), entry["command"].as_str().unwrap());
        let answer = kit.execute(&id, command, params(json!({ "format": extension })), Origin::Agent).unwrap().result.unwrap();
        let bytes = base64::engine::general_purpose::STANDARD.decode(answer["data"].as_str().unwrap()).unwrap();
        assert!(bytes.len() > 100, "{extension}");
    }
    // The kit writes no files of its own: a path is refused and nothing appears.
    let target = root.join("out.png");
    assert!(kit.execute(&id, "file.export", params(json!({ "format": "png", "path": target.to_string_lossy() })), Origin::Agent).is_err());
    assert!(!target.exists());
    // Files outside the folder get no handle, whether they exist or not.
    for outside in ["../escape.kdesign", "/etc/hosts.kdesign"] {
        assert!(folder.file(outside).is_err(), "{outside}");
    }
    let _ = std::fs::remove_dir_all(root);
}

fn kit_check_is_clean(doc: &DesignDocument) -> bool {
    kition_design_kit::check::check(&doc.session().ds().unwrap().doc).is_empty()
}
