//! Create and export (KHP A41, K5): every `creates` entry of `kit.json` makes a new design that
//! saves, reopens, and checks clean, and every `exports` entry's command, given only its format,
//! answers a file an independent reader reads back at the page's size (PNG and JPEG through the
//! `image` decoders, SVG through `usvg`, PDF by its structure and, where poppler is installed,
//! `pdfinfo`); exporting never changes the design.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use std::path::Path;

use base64::Engine as _;
use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;
use serde_json::{Value, json};

fn repo() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

fn manifest() -> Value {
    serde_json::from_str(&std::fs::read_to_string(repo().join("kit.json")).unwrap()).unwrap()
}

fn basic() -> DesignDocument {
    DesignDocument::open("basic.kdesign", &std::fs::read(repo().join("conformance/basic.kdesign")).unwrap()).unwrap()
}

#[test]
fn every_creates_entry_makes_a_clean_design() {
    for entry in manifest()["creates"].as_array().unwrap() {
        let name = format!("{}.{}", entry["baseName"].as_str().unwrap(), entry["extension"].as_str().unwrap());
        let size = &entry["size"];
        let mut doc = DesignDocument::create(&name, size["width"].as_u64().unwrap() as u32, size["height"].as_u64().unwrap() as u32).unwrap();
        let reopened = DesignDocument::open(&name, &doc.save_file(None).unwrap()).unwrap();
        assert!(reopened.check().is_empty(), "{name}: {:?}", reopened.check());
        assert_eq!(reopened.inspect().unwrap()["layers"], 1, "{name}: one frame");
        let ds = reopened.session().ds().unwrap();
        let frame = ds.doc.world_bounds(ds.doc.node(ds.page).unwrap().children[0]).unwrap();
        assert_eq!((frame.width(), frame.height()), (size["width"].as_f64().unwrap(), size["height"].as_f64().unwrap()), "{name}");
    }
}

fn exported(doc: &mut DesignDocument, command: &str, format: &str) -> Vec<u8> {
    let answer = doc.execute(command, json!({ "format": format })).unwrap_or_else(|error| panic!("{command} {format}: {error}"));
    assert_eq!(answer["format"], format, "{answer}");
    base64::engine::general_purpose::STANDARD.decode(answer["data"].as_str().unwrap()).unwrap()
}

fn read_pdf(bytes: &[u8]) {
    let text = String::from_utf8_lossy(bytes);
    assert!(bytes.starts_with(b"%PDF-1.") && text.trim_end().ends_with("%%EOF"), "a PDF file");
    assert!(text.contains("/MediaBox"), "a page with a size");
    let Ok(output) = std::process::Command::new("pdfinfo").arg("-").stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).spawn()
    else {
        return;
    };
    let mut child = output;
    std::io::Write::write_all(child.stdin.as_mut().unwrap(), bytes).unwrap();
    drop(child.stdin.take());
    let out = child.wait_with_output().unwrap();
    let info = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "pdfinfo reads it: {info}");
    assert!(info.lines().any(|line| line.starts_with("Pages:") && line.trim_end().ends_with(" 1")), "{info}");
    assert!(info.contains("400 x 300 pts"), "{info}");
}

#[test]
fn every_exports_entry_writes_a_file_a_reader_reads_back() {
    let manifest = manifest();
    let exports = manifest["exports"].as_array().unwrap();
    let extensions: Vec<&str> = exports.iter().map(|entry| entry["extension"].as_str().unwrap()).collect();
    assert_eq!(extensions, ["png", "jpg", "svg", "pdf"]);
    let mut doc = basic();
    let (revision, saved) = (doc.revision(), doc.save_file(None).unwrap());
    for entry in exports {
        let (extension, command) = (entry["extension"].as_str().unwrap(), entry["command"].as_str().unwrap());
        let listed = doc.commands().into_iter().find(|c| c.id == command).unwrap();
        assert!(!listed.mutates && listed.schema.is_some(), "{command}");
        let bytes = exported(&mut doc, command, extension);
        match extension {
            "png" | "jpg" => {
                let format = if extension == "png" { image::ImageFormat::Png } else { image::ImageFormat::Jpeg };
                let picture = image::load_from_memory_with_format(&bytes, format).unwrap();
                assert_eq!((picture.width(), picture.height()), (400, 300), "{extension}");
            }
            "svg" => {
                let tree = usvg::Tree::from_data(&bytes, &usvg::Options::default()).unwrap();
                assert_eq!((tree.size().width(), tree.size().height()), (400.0, 300.0));
                assert!(tree.root().has_children());
            }
            "pdf" => read_pdf(&bytes),
            other => panic!("no reader for {other}"),
        }
        assert!(entry["mime"].as_str().is_some(), "{extension} has a MIME type");
    }
    // Every engine command moves the view's revision; the design and its history stay as they were.
    assert!(doc.revision() >= revision && !doc.dirty(), "exports change nothing");
    assert_eq!(doc.inspect().unwrap()["canUndo"], false);
    assert_eq!(doc.save_file(None).unwrap(), saved);
}

#[test]
fn an_export_never_writes_a_file_itself() {
    let mut doc = basic();
    let dir = std::env::temp_dir().join(format!("kition-design-export-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("out.png");
    assert!(doc.execute("file.export", json!({ "format": "png", "path": path.to_string_lossy() })).is_err());
    assert!(!path.exists(), "the engine reaches no file system: the host writes exports");
    let _ = std::fs::remove_dir_all(dir);
}

/// KHP A28: no listed command takes a file to write by an untyped path; exports answer bytes.
#[test]
fn export_schemas_name_no_file_paths() {
    let doc = basic();
    for command in doc.commands().into_iter().filter(|c| c.id.starts_with("file.export")) {
        let schema = command.schema.unwrap_or_else(|| panic!("{} has a schema", command.id));
        for key in ["path", "dir", "file", "folder", "output"] {
            assert!(schema.get(key).is_none(), "{} takes `{key}`", command.id);
        }
    }
}
