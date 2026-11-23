//! TEMPORARY (task schema-freeze, content size): measures what `examples/ridgeline-app.kdesign`'s
//! `content.bin` holds and prints the breakdown. Delete with the commit that acts on it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::cast_precision_loss)]

use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use kition_design_doc::{Document, NodeKind};
use kition_design_kdm::kition_kit::kdm::{ActorId, Replica};
use serde_json::Value;

fn deflate(bytes: &[u8], level: u32) -> usize {
    let mut e = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::new(level));
    e.write_all(bytes).unwrap();
    e.finish().unwrap().len()
}

fn content_of(doc: &Document) -> Vec<u8> {
    let entries = kition_design_kdm::file::content_entries(&Arc::new(doc.clone())).unwrap();
    entries.into_iter().find(|(n, _)| n == kition_design_kdm::file::CONTENT).unwrap().1
}

fn is_default(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::Bool(b) => !b,
        Value::Number(n) => n.as_f64() == Some(0.0),
        Value::String(s) => s.is_empty(),
        Value::Array(a) => a.is_empty(),
        Value::Object(o) => o.is_empty(),
    }
}

#[test]
fn measure_ridgeline_app() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let file = std::fs::read(root.join("examples/ridgeline-app.kdesign")).unwrap();
    let loaded = kition_design_format::load_bytes_with_branches(&file).unwrap();
    let content = loaded.replica.iter().find(|(n, _)| n == kition_design_kdm::file::CONTENT).unwrap().1.clone();
    let doc = loaded.doc.clone();
    let mut out = String::from("MEASURE ridgeline-app\n");
    out.push_str(&format!("file bytes {}\n", file.len()));
    for level in [1, 3, 6, 9] {
        out.push_str(&format!("content.bin raw {} deflate(level {level}) {}\n", content.len(), deflate(&content, level)));
    }
    let mut json = Vec::new();
    kition_design_format::write_json(&doc, &mut json).unwrap();
    out.push_str(&format!("document.json raw {} deflate6 {}\n", json.len(), deflate(&json, 6)));
    let schema = kition_design_kdm::schema().unwrap();
    let replica = Replica::load(schema.clone(), &content, ActorId::random()).unwrap();
    let state = replica.state_json().unwrap();
    let state_text = serde_json::to_vec(&state).unwrap();
    out.push_str(&format!("state_json compact raw {} deflate6 {}\n", state_text.len(), deflate(&state_text, 6)));
    out.push_str(&format!("origins in content {:?}\n", Replica::snapshot_origins(&content).unwrap()));

    // Fresh: the same document written in one transaction, sealed.
    let fresh = content_of(&doc);
    out.push_str(&format!("fresh content (one transaction) raw {} deflate6 {}\n", fresh.len(), deflate(&fresh, 6)));
    let mut fresh_replica = Replica::load(schema.clone(), &fresh, ActorId::random()).unwrap();
    let again = fresh_replica.content_snapshot().unwrap();
    out.push_str(&format!("fresh content re-sealed raw {}\n", again.len()));
    let full = fresh_replica.snapshot().unwrap();
    out.push_str(&format!("fresh full snapshot raw {}\n", full.len()));

    // Objects by type, and the JSON bytes of each field by type.
    let objects = state["objects"].as_object().unwrap();
    let mut count: BTreeMap<String, usize> = BTreeMap::new();
    let mut field_bytes: BTreeMap<(String, String), (usize, usize, usize)> = BTreeMap::new();
    let mut id_bytes = 0;
    for (id, object) in objects {
        let ty = object["$type"].as_str().unwrap_or("?").to_owned();
        *count.entry(ty.clone()).or_default() += 1;
        id_bytes += id.len();
        for (field, value) in object.as_object().unwrap() {
            if field.starts_with('$') {
                continue;
            }
            let bytes = serde_json::to_vec(value).unwrap().len() + field.len();
            let e = field_bytes.entry((ty.clone(), field.clone())).or_default();
            e.0 += 1;
            e.1 += bytes;
            if is_default(value) {
                e.2 += 1;
            }
        }
    }
    out.push_str(&format!("objects {} (ids {} bytes)\n", objects.len(), id_bytes));
    for (ty, n) in &count {
        out.push_str(&format!("  type {ty:<14} {n}\n"));
    }
    out.push_str("fields (type.field count json-bytes defaults):\n");
    let mut sorted: Vec<_> = field_bytes.into_iter().collect();
    sorted.sort_by(|a, b| b.1.1.cmp(&a.1.1));
    for ((ty, field), (n, bytes, defaults)) in sorted {
        out.push_str(&format!("  {ty}.{field:<16} n={n:<6} bytes={bytes:<9} default-values={defaults}\n"));
    }
    // Text: runs and the marks they carry.
    let mut runs = 0;
    let mut mark_bytes = 0;
    let mut chars = 0;
    for object in objects.values() {
        if let Some(Value::Array(rs)) = object.get("text") {
            for r in rs {
                runs += 1;
                chars += r["insert"].as_str().map_or(0, str::len);
                mark_bytes += r.get("attributes").map_or(0, |a| serde_json::to_vec(a).unwrap().len());
            }
        }
    }
    out.push_str(&format!("text: runs {runs} chars {chars} marks-json {mark_bytes}\n"));
    // Keys of node registers that hold what the lists or the text hold too.
    let mut dup = BTreeMap::<String, usize>::new();
    for object in objects.values() {
        if object["$type"] != "Node" {
            continue;
        }
        for reg in ["kind", "props", "bindings", "instance", "dev", "prototype"] {
            if let Some(Value::Object(m)) = object.get(reg) {
                for k in m.keys() {
                    if ["fills", "strokes", "effects", "characters", "runs", "paragraphs", "text"].contains(&k.as_str()) {
                        *dup.entry(format!("{reg}.{k}")).or_default() += 1;
                    }
                }
            }
        }
    }
    out.push_str(&format!("duplicated keys in registers: {dup:?}\n"));

    // Ablations: the content of the document without one kind of thing.
    let base = fresh.len();
    let mut ablate = |label: &str, f: &dyn Fn(&mut Document)| {
        let mut d = doc.clone();
        f(&mut d);
        let c = content_of(&d);
        out.push_str(&format!("ablation {label:<40} raw {:>8} (-{:>7}) deflate6 {:>8}\n", c.len(), base.saturating_sub(c.len()), deflate(&c, 6)));
    };
    let ids: Vec<_> = doc.ids().collect();
    let each = |d: &mut Document, g: &dyn Fn(&mut kition_design_doc::Node)| {
        for id in &ids {
            if let Some(n) = d.node_mut(*id) {
                g(n);
            }
        }
    };
    ablate("no paints/strokes/effects", &|d| {
        each(d, &|n| {
            n.fills.clear();
            n.strokes.clear();
            n.effects.clear();
        })
    });
    ablate("texts emptied", &|d| {
        each(d, &|n| {
            if let NodeKind::Text(t) = &mut n.kind {
                t.characters.clear();
                t.runs.clear();
                t.paragraphs.clear();
            }
        })
    });
    ablate("names emptied", &|d| each(d, &|n| n.name.clear()));
    ablate("no bindings/styles", &|d| {
        each(d, &|n| {
            n.bound.clear();
            n.styles = Default::default();
        })
    });
    ablate("no images", &|d| d.images.clear());
    ablate("no comments/styles/variables", &|d| {
        d.comments.clear();
        d.styles.clear();
        d.variables = Default::default();
    });
    std::fs::write(std::env::temp_dir().join("measure.txt"), &out).unwrap();
    println!("{out}MEASURE-END");
    panic!("TEMPORARY: the measurements are printed above");
}
