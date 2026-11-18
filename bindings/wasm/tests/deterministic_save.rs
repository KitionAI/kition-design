//! A document built by the same commands saves the same bytes when `SOURCE_DATE_EPOCH` is set, as
//! a showcase sample or a gate case builds it: the replica's actor and the document id come from
//! a counter in the process instead of randomness. Each build runs in a process of its own: this
//! test starts itself several times.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::process::Command;

use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;
use serde_json::json;

const OUT: &str = "KITION_DESIGN_DETERMINISM_OUT";

fn build() -> Vec<u8> {
    let mut doc = DesignDocument::create("cards.kdesign", 400, 300).unwrap();
    let page = doc.inspect().unwrap()["page"].clone();
    let card =
        doc.execute("frame.create", json!({ "x": 0, "y": 0, "width": 200, "height": 120, "parent": page, "name": "Card" })).unwrap()["id"].clone();
    doc.execute("text.create", json!({ "text": "Hello", "x": 16, "y": 16, "parent": card })).unwrap();
    doc.execute("variable.createCollection", json!({ "name": "Theme", "modes": ["Light", "Dark"] })).unwrap();
    doc.save(Some("cards.kdesign")).unwrap()
}

/// The child's half: builds the drawing into the file `OUT` names (nothing without it).
#[test]
fn child_builds_a_document() {
    if let Ok(path) = std::env::var(OUT) {
        std::fs::write(path, build()).unwrap();
    }
}

fn build_in_a_process(path: &PathBuf) -> Vec<u8> {
    let status = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child_builds_a_document", "--test-threads=1", "--quiet"])
        .env("SOURCE_DATE_EPOCH", "1794182400")
        .env(OUT, path)
        .status()
        .unwrap();
    assert!(status.success(), "the child build failed");
    std::fs::read(path).unwrap()
}

#[test]
fn the_same_commands_save_the_same_bytes() {
    let dir = std::env::temp_dir().join(format!("kition-design-determinism-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let first = build_in_a_process(&dir.join("0.kdesign"));
    for build in 1..4 {
        // A second apart, so a clock read would show.
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let again = build_in_a_process(&dir.join(format!("{build}.kdesign")));
        assert!(first == again, "build {build} saved other bytes than build 0");
    }
    std::fs::remove_dir_all(&dir).ok();
}
