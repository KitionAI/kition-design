//! TEMPORARY (task schema-freeze, history separation): rewrites the repository's `.kdesign`
//! fixtures, written with `document.json`, as content files and prints each one base64 for the
//! commit that replaces them. Delete with that commit.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::io::Read;
use std::path::Path;
use std::sync::Arc;

const FILES: &[&str] = &[
    "conformance/basic.kdesign",
    "conformance/check-absolute-in-auto-layout.kdesign",
    "conformance/check-broken-flow.kdesign",
    "conformance/check-broken-interaction.kdesign",
    "conformance/check-empty-component.kdesign",
    "conformance/check-empty-frame.kdesign",
    "conformance/check-empty-text.kdesign",
    "conformance/check-font-substituted.kdesign",
    "conformance/check-low-contrast.kdesign",
    "conformance/check-missing-component.kdesign",
    "conformance/check-missing-image.kdesign",
    "conformance/check-missing-style.kdesign",
    "conformance/check-missing-variable.kdesign",
    "conformance/check-off-pixel.kdesign",
    "conformance/check-outside-clip.kdesign",
    "conformance/check-text-ellipsis.kdesign",
    "conformance/check-text-overflow.kdesign",
    "conformance/check-truncated.kdesign",
    "conformance/check-unbound-color.kdesign",
    "conformance/check-unused-style.kdesign",
    "cases/design.agent.act/auto-layout/input/screen.kdesign",
    "cases/design.agent.context/two-pages-one-run/input/app.kdesign",
    "cases/design.agent.conversation/card-component/input/screen.kdesign",
    "cases/design.agent.observe/widest-layer/input/screen.kdesign",
    "cases/design.agent.recover/text-overflow/input/note.kdesign",
    "examples/glass-and-vectors.kdesign",
    "examples/ridgeline-app.kdesign",
    "examples/ridgeline-design-system.kdesign",
    "examples/ridgeline-onboarding.kdesign",
    "examples/ridgeline-studio-dashboard.kdesign",
    "web/public/sample/Sample.kdesign",
    "web/public/sample/libraries/Design System.kdesign",
    "web/tests/fixtures/Design System v2.kdesign",
];

fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], chunk.get(1).copied().unwrap_or(0), chunk.get(2).copied().unwrap_or(0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(T[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// An old file (`document.json`, `images/`, an optional thumbnail and published library) as a
/// content file: the same document, layers, ids and images; no saved versions.
fn convert(old: &[u8]) -> Vec<u8> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(old)).unwrap();
    let member = |z: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>, name: &str| -> Option<Vec<u8>> {
        let mut f = z.by_name(name).ok()?;
        let mut out = Vec::new();
        f.read_to_end(&mut out).unwrap();
        Some(out)
    };
    let json = member(&mut z, "document.json").expect("document.json");
    let (mut doc, _) = kition_design_format::from_json(std::str::from_utf8(&json).unwrap()).unwrap();
    let names: Vec<String> = z.file_names().map(str::to_owned).collect();
    let listed: Vec<String> = serde_json::from_slice::<serde_json::Value>(&json).unwrap()["imageIndex"]
        .as_array()
        .map(|a| a.iter().filter_map(|i| i.as_str().map(str::to_owned).or_else(|| i["hash"].as_str().map(str::to_owned))).collect())
        .unwrap_or_default();
    for name in &names {
        let Some(hash) = name.strip_prefix("images/") else { continue };
        if !listed.is_empty() && !listed.iter().any(|l| l == hash) {
            continue;
        }
        let bytes = member(&mut z, name).unwrap();
        let (got, ..) = kition_design_format::add_image(&mut doc, bytes).unwrap();
        assert_eq!(got, hash);
    }
    doc.published = kition_design_format::read_library(std::io::Cursor::new(old)).unwrap().map(Arc::new);
    let thumbnail = member(&mut z, "thumbnail.png");
    kition_design_format::save_bytes(&doc, thumbnail.as_deref()).unwrap()
}

#[test]
fn convert_the_fixtures() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let mut out = String::new();
    for file in FILES {
        let old = std::fs::read(root.join(file)).unwrap();
        let new = convert(&old);
        let (doc, issues) = kition_design_format::load_bytes(&new).unwrap();
        assert!(issues.is_empty(), "{file}: {issues:?}");
        assert!(doc.node_count() > 0, "{file}");
        out.push_str(&format!("FIXTURE\t{file}\t{}\n", base64(&new)));
    }
    std::fs::write(std::env::temp_dir().join("kdesign-fixtures.txt"), &out).unwrap();
    println!("{out}FIXTURES-END");
    panic!("TEMPORARY: the converted fixtures are printed above (delete this test with the commit that adds them)");
}
