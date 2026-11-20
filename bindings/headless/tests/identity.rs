//! `document.identity` and `document.fork` (Kit Process Protocol section 5.6, KitModel M7a)
//! through the real `kition-design` process: a `.kdesign` embeds its replica (its `kdm/` entries:
//! `meta.json`, `snapshot.bin` and the change entries after it, `docs/features/kdm.md` section 3),
//! so a byte copy of a file is the same document. A host reads the identity a file carries without
//! parsing the design, and forks a file into a new document: a new docId (in the replica and in
//! `meta.json`'s copy), the same layers, ids and values, no change entries, no history (no saved
//! versions either), only the images it uses, and `document.json` and the thumbnail as they were.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;
use kition_kit_host::{ClientError, HeadlessKit, LaunchOptions};
use serde_json::{Value, json};

const LIMIT: Duration = Duration::from_secs(60);

/// The name of a layer deleted before the fork: in the source's replica (its history) and saved
/// version, not the fork's.
const SECRET: &str = "SECRET-DELETED-BEFORE-THE-FORK";

/// A 2 x 2 PNG, placed on a layer the fork keeps.
const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAFElEQVR4nGP8z8DwnwEJMDGgAQBVBgICyX5rtAAAAABJRU5ErkJggg==";

fn workspace(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kition-design-identity-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::canonicalize(dir).unwrap()
}

async fn launch(dir: &Path) -> HeadlessKit {
    let mut options = LaunchOptions::new(env!("CARGO_BIN_EXE_kition-design"));
    options.workspace = Some(dir.to_path_buf());
    HeadlessKit::launch(options).await.unwrap()
}

/// The archive's entry `name`, unpacked, or `None`.
fn entry(bytes: &[u8], name: &str) -> Option<Vec<u8>> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut file = zip.by_name(name).ok()?;
    let mut out = Vec::new();
    file.read_to_end(&mut out).unwrap();
    Some(out)
}

fn names(bytes: &[u8]) -> BTreeSet<String> {
    zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap().file_names().map(str::to_owned).collect()
}

fn images(bytes: &[u8]) -> BTreeSet<String> {
    names(bytes).iter().filter_map(|name| name.strip_prefix("images/")).map(str::to_owned).collect()
}

fn meta(bytes: &[u8]) -> Value {
    serde_json::from_slice(&entry(bytes, "kdm/meta.json").expect("kdm/meta.json")).unwrap()
}

/// A picture no layer of the design uses: only a saved version holds it.
fn stray_png() -> Vec<u8> {
    let mut out = Vec::new();
    image::RgbaImage::from_pixel(3, 3, image::Rgba([10, 200, 30, 255]))
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .unwrap();
    out
}

/// A design saved twice: first with three rectangles (one named [`SECRET`], one with a picture)
/// and a saved version, then with the secret one deleted (one change entry); then a second saved
/// version that holds a picture nothing else uses.
fn design() -> (Vec<u8>, String) {
    let mut doc = DesignDocument::create("Plan.kdesign", 800, 600).unwrap();
    let frame = doc.inspect().unwrap()["tree"][0]["id"].as_u64().unwrap();
    let mut rect = |name: &str, x: u64| {
        doc.execute("node.create", json!({ "type": "rectangle", "parent": frame, "name": name, "x": x, "y": 20, "width": 40, "height": 40 })).unwrap()
            ["id"]
            .as_u64()
            .unwrap()
    };
    let kept = rect("Kept", 20);
    rect("Also kept", 100);
    let secret = rect(SECRET, 180);
    doc.execute("image.place", json!({ "data": PNG, "target": kept })).unwrap();
    doc.execute("file.saveVersion", json!({ "name": "Before the deletion" })).unwrap();
    let first = doc.save(None).unwrap();
    let mut doc = DesignDocument::open("Plan.kdesign", &first).unwrap();
    doc.execute("edit.delete", json!({ "ids": [secret] })).unwrap();
    let saved = doc.save(None).unwrap();
    assert_eq!(meta(&saved)["changes"].as_array().unwrap().len(), 1, "the deletion is a change entry after the snapshot");

    let mut loaded = kition_design_format::load_bytes_with_versions(&saved).unwrap();
    assert_eq!(loaded.versions.len(), 1, "the saved version");
    let mut old = (*loaded.versions[0].document().unwrap()).clone();
    let (stray, _, _) = kition_design_format::add_image(&mut old, stray_png()).unwrap();
    loaded.versions.push(kition_design_format::Version::new("stray", "Stray picture", "", 1, Arc::new(old)));
    let thumbnail = entry(&saved, "thumbnail.png");
    let bytes = kition_design_format::save_bytes_with_replica(&loaded.doc, thumbnail.as_deref(), &loaded.versions, &loaded.branches, &loaded.replica)
        .unwrap();
    (bytes, stray)
}

/// Every object of the design with its fields, by id: what must be equal in a fork.
async fn objects(kit: &HeadlessKit, path: &str) -> Value {
    let state = kit.open(path, false, LIMIT).await.unwrap();
    let document = state["documentId"].clone();
    let batch = |request: Value| {
        let mut request = request;
        request["documentId"] = document.clone();
        kit.request("model.batch", request, LIMIT)
    };
    let inventory = batch(json!({ "ops": [], "objects": true })).await.unwrap();
    let ids: Vec<Value> = inventory["objects"].as_array().unwrap().iter().map(|object| object["id"].clone()).collect();
    assert!(ids.len() >= 4, "the design, a page, a frame and the rectangles: {inventory}");
    let load: Vec<Value> = ids.iter().map(|id| json!({ "target": id })).collect();
    let loaded = batch(json!({ "ops": [], "load": load })).await.unwrap();
    kit.request("document.close", json!({ "documentId": document }), LIMIT).await.unwrap();
    json!({ "objects": inventory["objects"], "loaded": loaded["loaded"] })
}

fn holds(bytes: &[u8], needle: &str) -> bool {
    bytes.windows(needle.len()).any(|window| window == needle.as_bytes())
}

/// What the kit said about a refusal (`error.data.detail`).
fn detail(error: &ClientError) -> String {
    error.data().and_then(|data| data["detail"].as_str()).unwrap_or_default().to_owned()
}

#[tokio::test]
async fn a_design_tells_its_identity_and_forks_into_a_new_document_without_history() {
    let dir = workspace("embedded");
    let (bytes, stray) = design();
    std::fs::write(dir.join("Plan.kdesign"), bytes).unwrap();
    let kit = launch(&dir).await;
    assert!(kit.serves("document.identity"), "{}", kit.capabilities());

    // The identity the file carries, read without opening it, and the same for a byte copy.
    let source_id = kit.identity("Plan.kdesign", LIMIT).await.unwrap().doc_id.expect("a design file has an identity");
    let source_bytes = std::fs::read(dir.join("Plan.kdesign")).unwrap();
    assert_eq!(meta(&source_bytes)["docId"], json!(source_id), "meta.json's copy names the same document");
    std::fs::copy(dir.join("Plan.kdesign"), dir.join("Copy.kdesign")).unwrap();
    assert_eq!(kit.identity("Copy.kdesign", LIMIT).await.unwrap().doc_id.as_deref(), Some(source_id.as_str()), "a byte copy is the same document");

    // A fork is a new document with the same objects, ids and values.
    let fork = kit.fork("Plan.kdesign", "Fork.kdesign", LIMIT).await.unwrap();
    let fork_id = fork.doc_id.clone().expect("the fork's identity");
    assert_ne!(fork_id, source_id);
    assert_eq!(fork.name, "Fork.kdesign");
    assert!(!fork.version.is_empty());
    assert_eq!(kit.identity("Fork.kdesign", LIMIT).await.unwrap().doc_id.as_deref(), Some(fork_id.as_str()), "the forked file carries the new id");
    assert_eq!(kit.identity("Plan.kdesign", LIMIT).await.unwrap().doc_id.as_deref(), Some(source_id.as_str()), "the source is unchanged");
    assert_eq!(std::fs::read(dir.join("Plan.kdesign")).unwrap(), source_bytes, "the source's bytes are unchanged");
    assert_eq!(objects(&kit, "Fork.kdesign").await, objects(&kit, "Plan.kdesign").await, "the same objects, ids and values");

    // The fork's replica travels as a whole (an agent's draft of it: `model.export`, then
    // `model.open`), though it starts at the fork and has no earlier changes.
    let state = kit.open("Fork.kdesign", true, LIMIT).await.unwrap();
    let exported = kit.request("model.export", json!({ "documentId": state["documentId"] }), LIMIT).await.unwrap();
    let draft = kit.request("model.open", json!({ "name": "Fork.kdesign", "snapshot": exported["changes"] }), LIMIT).await.unwrap();
    assert_ne!(draft["documentId"], state["documentId"], "{draft}");
    kit.request("document.close", json!({ "documentId": state["documentId"] }), LIMIT).await.unwrap();

    // The fork's file: the new id in meta.json, no change entries, no saved versions or branches,
    // only the pictures it uses; document.json and the thumbnail as they were.
    let fork_bytes = std::fs::read(dir.join("Fork.kdesign")).unwrap();
    let fork_meta = meta(&fork_bytes);
    assert_eq!(fork_meta["docId"], json!(fork_id), "meta.json's copy of the docId is the fork's");
    assert_eq!(fork_meta["changes"], json!([]), "a fork has no change log");
    let fork_names = names(&fork_bytes);
    assert!(!fork_names.iter().any(|name| name.starts_with("kdm/changes/")), "{fork_names:?}");
    assert!(!fork_names.iter().any(|name| name.starts_with("versions/") || name.starts_with("branches/")), "no saved versions: {fork_names:?}");
    assert!(names(&source_bytes).iter().any(|name| name.starts_with("versions/")), "the source keeps its versions");
    let forked = kition_design_format::load_bytes_with_versions(&fork_bytes).unwrap();
    assert!(forked.versions.is_empty() && forked.branches.is_empty());
    assert!(images(&source_bytes).contains(&stray), "the source holds the picture only a version uses");
    let used: BTreeSet<String> = forked.doc.images.keys().cloned().collect();
    assert!(!used.is_empty(), "the placed picture");
    assert_eq!(images(&fork_bytes), used, "the pictures the design uses, and no others");
    for name in ["document.json", "thumbnail.png"] {
        assert_eq!(entry(&fork_bytes, name), entry(&source_bytes, name), "{name} is kept");
    }

    // The fork edits, saves and opens again as any design, under its own id.
    let mut opened = DesignDocument::open("Fork.kdesign", &fork_bytes).unwrap();
    assert_eq!(opened.doc_id().as_deref(), Some(fork_id.as_str()));
    let frame = opened.inspect().unwrap()["tree"][0]["id"].as_u64().unwrap();
    opened
        .execute("node.create", json!({ "type": "rectangle", "parent": frame, "name": "New", "x": 20, "y": 200, "width": 40, "height": 40 }))
        .unwrap();
    let edited = opened.save(None).unwrap();
    let reopened = DesignDocument::open("Fork.kdesign", &edited).unwrap();
    assert_eq!(reopened.doc_id().as_deref(), Some(fork_id.as_str()));
    assert_eq!(reopened.inspect().unwrap()["tree"], opened.inspect().unwrap()["tree"]);

    // No history: the deleted layer's name is in the source's bytes and not in the fork's.
    assert!(holds(&source_bytes, SECRET), "the source keeps its history");
    assert!(!holds(&fork_bytes, SECRET), "deleted content does not travel");

    // A fork of a fork is a third document with the same content.
    let again = kit.fork("Fork.kdesign", "Again.kdesign", LIMIT).await.unwrap().doc_id.expect("the second fork's identity");
    assert!(again != source_id && again != fork_id, "{again}");
    assert_eq!(objects(&kit, "Again.kdesign").await, objects(&kit, "Plan.kdesign").await);

    // A fork never replaces a file.
    let taken = kit.fork("Plan.kdesign", "Fork.kdesign", LIMIT).await.unwrap_err();
    assert_eq!(taken.reason(), Some("file_conflict"), "{taken}");
    assert_eq!(std::fs::read(dir.join("Fork.kdesign")).unwrap(), fork_bytes, "the first fork is kept");
    kit.shutdown(Duration::from_secs(5)).await;
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn a_design_without_a_replica_has_no_identity_and_a_damaged_replica_is_refused() {
    let dir = workspace("plain");
    let kit = launch(&dir).await;
    // An empty `.kdesign` (a file made for the kit before it saved one), a `.kdesign` written
    // without the bridge, and a bare `document.json` hold no replica: no identity, and their forks
    // are the same bytes.
    let doc = kition_design_format::load_bytes(&DesignDocument::create("Plain.kdesign", 800, 600).unwrap().save(None).unwrap()).unwrap().0;
    let plain = kition_design_format::save_bytes(&doc, None).unwrap();
    let mut json = Vec::new();
    kition_design_format::write_json(&doc, &mut json).unwrap();
    for (name, bytes) in [("New.kdesign", Vec::new()), ("Plain.kdesign", plain), ("Bare.kdesign", json)] {
        std::fs::write(dir.join(name), &bytes).unwrap();
        assert_eq!(kit.identity(name, LIMIT).await.unwrap().doc_id, None, "{name}");
        let copy = format!("Copy{name}");
        let fork = kit.fork(name, &copy, LIMIT).await.unwrap();
        assert_eq!(fork.doc_id, None, "{name}");
        assert_eq!(std::fs::read(dir.join(&copy)).unwrap(), bytes, "{name}: a file without a replica forks unchanged");
    }

    // A replica that does not load, and replica entries that are not a design's, are refused,
    // never answered with no identity.
    let saved = DesignDocument::create("Plan.kdesign", 800, 600).unwrap().save(None).unwrap();
    let loaded = kition_design_format::load_bytes_with_versions(&saved).unwrap();
    let rewrite = |replica: Vec<(String, Vec<u8>)>| kition_design_format::save_bytes_with_replica(&loaded.doc, None, &[], &[], &replica).unwrap();
    let with = |name: &str, bytes: &[u8]| -> Vec<(String, Vec<u8>)> {
        loaded.replica.iter().map(|(entry, data)| (entry.clone(), if entry == name { bytes.to_vec() } else { data.clone() })).collect()
    };
    let damaged = [
        ("Snapshot.kdesign", rewrite(with("kdm/snapshot.bin", b"not a replica"))),
        ("Meta.kdesign", rewrite(with("kdm/meta.json", br#"{"format": "something else"}"#))),
        ("Missing.kdesign", rewrite(loaded.replica.iter().filter(|(name, _)| name != "kdm/snapshot.bin").cloned().collect())),
    ];
    for (name, bytes) in damaged {
        std::fs::write(dir.join(name), bytes).unwrap();
        let copy = format!("Copy{name}");
        for refused in [kit.identity(name, LIMIT).await.unwrap_err(), kit.fork(name, &copy, LIMIT).await.unwrap_err()] {
            assert_eq!(refused.reason(), Some("unsupported_format"), "{name}: {refused}: {}", detail(&refused));
        }
        assert!(!dir.join(copy).exists());
    }
    kit.shutdown(Duration::from_secs(5)).await;
    let _ = std::fs::remove_dir_all(dir);
}
