//! Every committed `.kdesign` sample (`conformance/`, `cases/`, `examples/`, `web/public/sample/`,
//! `web/tests/fixtures/`) loads and saves to its own bytes (kition-sdk data model 7.4; feature
//! `design.file.content-only`): its content is the canonical `kdm/content.json` the kit writes, so
//! the samples are the kit's own files. The content is written again from the bytes alone, without
//! the repair on open, so a sample of a broken design (the `check-*` fixtures) stays broken.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use kition_design_kdm::file::{CONTENT, fork_entries};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The committed samples, by their path from the repository root, sorted.
fn samples() -> Vec<String> {
    let root = repo();
    let mut found = Vec::new();
    let mut stack: Vec<PathBuf> =
        ["conformance", "cases", "examples", "web/public/sample", "web/tests/fixtures"].iter().map(|dir| root.join(dir)).collect();
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|extension| extension == "kdesign") {
                found.push(path.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/"));
            }
        }
    }
    found.sort();
    found
}

/// The file the kit saves for `bytes` (a `.kdesign` whose `kdm/content.json` is `content`): the
/// content written by the KDM (`Replica::content_snapshot`, through `fork_entries`, which keeps
/// the docId and repairs nothing), with the images and the published library the file holds.
fn kit_save(bytes: &[u8], content: &[u8]) -> Vec<u8> {
    let loaded = kition_design_format::load_with_replica(bytes).unwrap();
    let (entries, _) = fork_entries(content).unwrap();
    kition_design_format::save_bytes_with_replica(&loaded.doc, &entries).unwrap()
}

fn entry(bytes: &[u8], name: &str) -> Vec<u8> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut out = Vec::new();
    zip.by_name(name).unwrap().read_to_end(&mut out).unwrap();
    out
}

#[test]
fn every_committed_sample_loads_and_saves_to_its_own_bytes() {
    let samples = samples();
    assert!(samples.len() >= 33, "the samples: {samples:?}");
    for sample in samples {
        let bytes = std::fs::read(repo().join(&sample)).unwrap();
        let content = entry(&bytes, CONTENT);
        assert!(kition_design_format::load_with_replica(&bytes).is_ok(), "{sample} opens");
        assert_eq!(kit_save(&bytes, &content), bytes, "{sample} saves to its own bytes");
    }
}

/// TEMPORARY (plain model sample conversion, deleted in the next commit): writes each converted
/// `content.json` (`tests/regen/<path with / as __>.json`, written by kition-sdk's
/// `tools/convert-content`) into its sample in place of the old `kdm/content.bin`, saves it
/// through the kit and prints the file as base64, then fails so CI shows the output.
#[test]
fn regenerate_samples() {
    let regen = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/regen");
    let mut printed = 0;
    for sample in samples() {
        let content = std::fs::read(regen.join(format!("{}.json", sample.replace('/', "__")))).unwrap();
        let old = std::fs::read(repo().join(&sample)).unwrap();
        // The old file with the converted content in place of its old one.
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&old)).unwrap();
        let mut out = std::io::Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut out);
            let options = zip::write::SimpleFileOptions::default();
            for index in 0..zip.len() {
                let mut file = zip.by_index(index).unwrap();
                let name = file.name().to_owned();
                let mut data = Vec::new();
                file.read_to_end(&mut data).unwrap();
                if name == "kdm/content.bin" {
                    writer.start_file(CONTENT, options).unwrap();
                    writer.write_all(&content).unwrap();
                } else {
                    writer.start_file(name, options).unwrap();
                    writer.write_all(&data).unwrap();
                }
            }
            writer.finish().unwrap();
        }
        let bytes = kit_save(&out.into_inner(), &content);
        println!("REGEN {sample} {}", base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes));
        printed += 1;
    }
    panic!("REGEN printed {printed} samples");
}
