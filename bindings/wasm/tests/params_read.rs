//! Every parameter the engine reads for a command is in that command's schema, so an agent can
//! learn every key it takes (a key the notation leaves out is a key no agent sends). Generalizes
//! `layout_add_offers_its_direction`: the engine's command table (`spec!`) is read from source,
//! and the keys each command's run function reads straight from its params are collected.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;

const HELPERS: &[&str] = &["string", "get", "num", "num_or", "boolean", "id", "row", "indices", "color", "handle_list", "shape", "line", "blend"];

/// Keys a run function reads that the kit leaves out of the command's schema on purpose: the kit
/// reaches no file system, so a `path` naming a file to read or write is not offered (KHP A28;
/// `commands.rs` `WRITES_TO_PATH`); images and imports come as `data`.
const NOT_PARAMS: &[(&str, &str)] = &[
    ("file.exportFramesPdf", "path"),
    ("variable.export", "path"),
    ("image.place", "path"),
    ("file.placeImage", "path"),
    ("file.export", "path"),
    ("file.exportLayers", "dir"),
    ("component.playground", "path"),
    ("codeConnect.export", "path"),
];

fn sources(dir: &Path, out: &mut Vec<(String, String)>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") && !path.to_string_lossy().contains("test") {
            out.push((path.with_extension("").to_string_lossy().into_owned(), std::fs::read_to_string(&path).unwrap()));
        }
    }
}

/// The text from `open` (an opening bracket) to its matching close, skipping string literals.
fn balanced(text: &str, open: usize) -> &str {
    let bytes = text.as_bytes();
    let (mut depth, mut i, mut in_str) = (0i32, open, false);
    while i < bytes.len() {
        let c = bytes[i];
        if in_str {
            if c == b'\\' {
                i += 1;
            } else if c == b'"' {
                in_str = false;
            }
        } else if c == b'"' {
            in_str = true;
        } else if c == b'(' || c == b'{' || c == b'[' {
            depth += 1;
        } else if c == b')' || c == b'}' || c == b']' {
            depth -= 1;
            if depth == 0 {
                return &text[open..=i];
            }
        }
        i += 1;
    }
    &text[open..]
}

/// The top-level comma-separated arguments of `spec!( … )`.
fn arguments(call: &str) -> Vec<&str> {
    let inner = &call[1..call.len() - 1];
    let (mut parts, mut depth, mut start, mut in_str, mut escape) = (Vec::new(), 0i32, 0, false, false);
    for (i, c) in inner.char_indices() {
        if in_str {
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            '(' | '{' | '[' => depth += 1,
            ')' | '}' | ']' => depth -= 1,
            '|' if depth == 0 => {}
            ',' if depth == 0 => {
                parts.push(inner[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(inner[start..].trim());
    parts
}

fn keys_read(body: &str) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    let mut take = |rest: &str| {
        if let Some(end) = rest.find('"') {
            let key = &rest[..end];
            if !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                keys.insert(key.to_owned());
            }
        }
    };
    for helper in HELPERS {
        for pattern in [format!("{helper}(p, \""), format!("{helper}(&p, \"")] {
            for (at, _) in body.match_indices(&pattern) {
                let before = body[..at].chars().last();
                if before.is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_' || c == '.')) {
                    take(&body[at + pattern.len()..]);
                }
            }
        }
    }
    for pattern in ["p[\"", "p.get(\""] {
        for (at, _) in body.match_indices(pattern) {
            let before = body[..at].chars().last();
            if before.is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_' || c == '.')) {
                take(&body[at + pattern.len()..]);
            }
        }
    }
    keys
}

#[test]
fn every_parameter_the_engine_reads_is_in_the_schema() {
    let mut files = Vec::new();
    sources(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/engine/engine/src"), &mut files);
    let index = |text: &'static str| {
        let mut functions: BTreeMap<String, &'static str> = BTreeMap::new();
        for (at, _) in text.match_indices("fn ") {
            let name: String = text[at + 3..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
            if let Some(open) = text[at..].find('{').map(|o| at + o) {
                functions.entry(name).or_insert_with(|| balanced(text, open));
            }
        }
        functions
    };
    let paths: Vec<String> = files.iter().map(|(path, _)| path.clone()).collect();
    let files: Vec<&'static str> = files.into_iter().map(|(_, f)| &*Box::leak(f.into_boxed_str())).collect();
    let local: Vec<BTreeMap<String, &'static str>> = files.iter().map(|f| index(f)).collect();
    // A file's own submodules (`cmd/component.rs` and `cmd/component/*.rs`), for run functions
    // they define.
    let nearby = |at: usize, name: &str| {
        let module = format!("{}/", paths[at]);
        paths.iter().zip(&local).filter(|(path, _)| path.starts_with(&module)).find_map(|(_, functions)| functions.get(name).copied())
    };
    let mut global: BTreeMap<String, &'static str> = BTreeMap::new();
    for functions in &local {
        for (name, body) in functions {
            global.entry(name.clone()).or_insert(body);
        }
    }
    let schemas: BTreeMap<String, BTreeSet<String>> = DesignDocument::create("Design.kdesign", 400, 300)
        .unwrap()
        .commands()
        .into_iter()
        .map(|c| (c.id, c.schema.and_then(|s| s.as_object().map(|o| o.keys().cloned().collect())).unwrap_or_default()))
        .collect();
    let mut missing = Vec::new();
    for (file_at, (file, functions)) in files.iter().zip(&local).enumerate() {
        for (at, _) in file.match_indices("spec!(") {
            let args = arguments(balanced(file, at + 5));
            let (Some(id), Some(run)) = (args.first(), args.last()) else { continue };
            let id = id.trim_matches('"');
            let Some(schema) = schemas.get(id) else { continue };
            let run = run.trim();
            let name = run.rsplit("::").next().unwrap_or(run);
            let body = if run.starts_with('|') {
                run
            } else {
                functions.get(name).copied().or_else(|| nearby(file_at, name)).or_else(|| global.get(name).copied()).unwrap_or("")
            };
            for key in keys_read(body) {
                if !schema.contains(&key) && !NOT_PARAMS.contains(&(id, key.as_str())) {
                    missing.push(format!("{id}: reads `{key}`, which its schema lacks"));
                }
            }
        }
    }
    assert!(missing.is_empty(), "{} keys missing:\n{}", missing.len(), missing.join("\n"));
}
