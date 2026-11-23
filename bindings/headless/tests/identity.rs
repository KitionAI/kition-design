//! `document.identity` and `document.fork` (Kit Process Protocol section 5.6, KitModel M7a)
//! through the real `kition-design` process: a `.kdesign` embeds its replica's content (its `kdm/`
//! entries: `meta.json` and `content.bin`, `docs/features/kdm.md` section 3), so a byte copy of a
//! file is the same document. A host reads the identity a file carries without parsing the design,
//! and forks a file into a new document: a new docId (in the content and in `meta.json`), the same
//! layers, ids and values, no history, only the images it uses, and the thumbnail as it was.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;
use kition_kit_host::{ClientError, HeadlessKit, LaunchOptions};
use serde_json::{Value, json};

const LIMIT: Duration = Duration::from_secs(60);

/// The name of a layer deleted before the save: in the replica's history (the host's store), in
/// neither file.
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

/// A design saved twice as a host saves it (`prepare_save`, then `save`): first with three
/// rectangles (one named [`SECRET`], one with a picture), then with the secret one deleted.
fn design() -> Vec<u8> {
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
    doc.prepare_save().unwrap();
    let first = doc.save(None).unwrap();
    assert!(holds(&entry(&first, "kdm/content.bin").unwrap(), SECRET), "the layer is content before it is deleted");
    let mut doc = DesignDocument::open("Plan.kdesign", &first).unwrap();
    doc.execute("edit.delete", json!({ "ids": [secret] })).unwrap();
    doc.prepare_save().unwrap();
    let saved = doc.save(None).unwrap();
    assert_eq!(
        names(&saved).iter().filter(|name| name.starts_with("kdm/")).cloned().collect::<Vec<_>>(),
        ["kdm/content.bin", "kdm/meta.json"],
        "the content and what names it, no change log"
    );
    saved
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
    let bytes = design();
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

    // The fork's file: the new id in meta.json, its content, no branches, only the pictures it
    // uses; the thumbnail as it was.
    let fork_bytes = std::fs::read(dir.join("Fork.kdesign")).unwrap();
    let fork_meta = meta(&fork_bytes);
    assert_eq!(fork_meta["docId"], json!(fork_id), "meta.json's copy of the docId is the fork's");
    let fork_names = names(&fork_bytes);
    assert!(fork_names.iter().filter(|name| name.starts_with("kdm/")).eq(["kdm/content.bin", "kdm/meta.json"].iter()), "{fork_names:?}");
    assert!(!fork_names.iter().any(|name| name.starts_with("versions/") || name.starts_with("branches/")), "{fork_names:?}");
    let forked = kition_design_format::load_bytes_with_branches(&fork_bytes).unwrap();
    assert!(forked.branches.is_empty());
    let used: BTreeSet<String> = forked.doc.images.keys().cloned().collect();
    assert!(!used.is_empty(), "the placed picture");
    assert_eq!(images(&fork_bytes), used, "the pictures the design uses, and no others");
    assert_eq!(images(&source_bytes), used, "the source holds the same pictures");
    assert_eq!(entry(&fork_bytes, "thumbnail.png"), entry(&source_bytes, "thumbnail.png"), "the thumbnail is kept");

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

    // No history: the deleted layer's name is in neither file.
    assert!(!holds(&entry(&source_bytes, "kdm/content.bin").unwrap(), SECRET), "a file holds no deleted content");
    assert!(!holds(&entry(&fork_bytes, "kdm/content.bin").unwrap(), SECRET), "deleted content does not travel");

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
    // An empty `.kdesign` (a file made for the kit before it saved one) and bytes that are no
    // archive hold no replica: no identity, and their forks are the same bytes.
    let doc = kition_design_format::load_bytes(&DesignDocument::create("Plain.kdesign", 800, 600).unwrap().save(None).unwrap()).unwrap().0;
    let mut json = Vec::new();
    kition_design_format::write_json(&doc, &mut json).unwrap();
    for (name, bytes) in [("New.kdesign", Vec::new()), ("Bare.kdesign", json)] {
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
    let loaded = kition_design_format::load_bytes_with_branches(&saved).unwrap();
    let rewrite = |replica: Vec<(String, Vec<u8>)>| kition_design_format::save_bytes_with_replica(&loaded.doc, None, &[], &replica).unwrap();
    let with = |name: &str, bytes: &[u8]| -> Vec<(String, Vec<u8>)> {
        loaded.replica.iter().map(|(entry, data)| (entry.clone(), if entry == name { bytes.to_vec() } else { data.clone() })).collect()
    };
    let damaged = [
        ("Content.kdesign", rewrite(with("kdm/content.bin", b"not a replica"))),
        ("Meta.kdesign", rewrite(with("kdm/meta.json", br#"{"format": "something else"}"#))),
        ("Missing.kdesign", rewrite(loaded.replica.iter().filter(|(name, _)| name != "kdm/content.bin").cloned().collect())),
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

/// Written once by an agent's batch, overwritten by the next: in no file.
const OVERWRITTEN: &str = "Ug5Le1Fo0Dw4";

/// The headless `kition-design` saves on the direct path (feature `design.file.content-only`): it
/// seals the document (`prepare_save`) before every save, so the file an agent's batches write
/// holds the design's content and, of its history, only that seal.
#[tokio::test]
async fn an_agents_batches_save_sealed_content() {
    let dir = workspace("batches");
    let bytes = design();
    let kept =
        DesignDocument::open("Plan.kdesign", &bytes).unwrap().session().ds().unwrap().doc.nodes().find(|n| n.name == "Kept").map(|n| n.id.0).unwrap();
    std::fs::write(dir.join("Plan.kdesign"), bytes).unwrap();
    let kit = launch(&dir).await;
    let state = kit.open("Plan.kdesign", false, LIMIT).await.unwrap();
    let document = state["documentId"].clone();
    for name in [OVERWRITTEN, "Renamed by the agent"] {
        let ran = kit
            .request(
                "model.batch",
                json!({ "documentId": document, "ops": [{ "op": "set", "target": kept.to_string(), "field": "name", "value": name }] }),
                LIMIT,
            )
            .await
            .unwrap();
        assert!(ran.get("error").is_none(), "{ran}");
    }
    kit.request("document.save", json!({ "documentId": document }), LIMIT).await.unwrap();
    let saved = std::fs::read(dir.join("Plan.kdesign")).unwrap();
    let content = entry(&saved, "kdm/content.bin").unwrap();
    assert_eq!(
        kition_kit::kdm::Replica::snapshot_origins(&content).unwrap(),
        vec![Some(kition_kit::kdm::Origin::system("content-seal"))],
        "of the history, the file holds the seal"
    );
    assert!(!holds(&content, OVERWRITTEN), "the overwritten name is in no file");
    let reopened = DesignDocument::open("Plan.kdesign", &saved).unwrap();
    assert!(reopened.session().ds().unwrap().doc.nodes().any(|n| n.id.0 == kept && n.name == "Renamed by the agent"));
    kit.shutdown(Duration::from_secs(5)).await;
    let _ = std::fs::remove_dir_all(dir);
}
