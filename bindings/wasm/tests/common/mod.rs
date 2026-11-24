//! Kition Design as the convergence suite drives it (kition-sdk `kition-convergence`, `KitAdapter`):
//! replicas are `DesignDocument`s edited through the engine's own commands.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use kition_convergence::{Blobs, KitAdapter, Result, Rng, Script};
use kition_design_doc::{Document, NodeId, NodeKind};
use kition_design_kit::DesignDocument;
use kition_design_kit::collab::{decode_blobs, encode_blobs};
use kition_kit::KitDocument;
use kition_kit::session::MemoryBlobs;
use serde_json::{Value, json};

pub const NAME: &str = "Design.kdesign";

/// The library files every replica is lent (synthetic: one component each), for the random
/// edits that add and remove library links (`Design.libraries`) and place their components (the
/// library store `Design.libraryStore` and the records `Design.libraryCopies`).
pub const LIBRARIES: &[&str] = &["libraries/Buttons.kdesign", "libraries/Icons.kdesign"];

fn library_files() -> &'static [Vec<u8>] {
    static FILES: std::sync::OnceLock<Vec<Vec<u8>>> = std::sync::OnceLock::new();
    FILES.get_or_init(|| {
        LIBRARIES
            .iter()
            .map(|path| {
                let name = path.trim_start_matches("libraries/").trim_end_matches(".kdesign");
                let mut s = kition_design_engine::Session::with_document(Document::new(name));
                let run = |s: &mut kition_design_engine::Session, cmd: &str, p: Value| s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd}: {e}"));
                let f = run(&mut s, "frame.create", json!({ "x": 0, "y": 0, "width": 40, "height": 40, "name": name }))["id"].clone();
                run(&mut s, "component.create", json!({ "ids": [f] }));
                run(&mut s, "library.publish", json!({}));
                kition_design_format::save_bytes(s.doc().unwrap()).unwrap()
            })
            .collect()
    })
}

/// Lends every replica the same library files, as an editor lends what it read from the host.
fn lend_libraries(d: &mut DesignDocument) -> Result<()> {
    for (path, bytes) in LIBRARIES.iter().zip(library_files()) {
        d.lend_library(path, Some(bytes)).map_err(err)?;
    }
    Ok(())
}

pub struct DesignKit;

fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn to_memory(blobs: &Blobs) -> MemoryBlobs {
    MemoryBlobs(blobs.iter().map(|(hash, bytes)| (hash.clone(), Arc::from(bytes.as_slice()))).collect())
}

fn doc(d: &DesignDocument) -> Arc<Document> {
    d.session().ds().map(|ds| ds.doc.clone()).unwrap_or_default()
}

/// The layers of the current page's tree, pages left out, in id order (the same on every replica
/// with the same state).
fn layers(d: &DesignDocument) -> Vec<NodeId> {
    let doc = doc(d);
    let Ok(page) = d.session().page() else { return Vec::new() };
    let mut out: Vec<NodeId> = doc.subtree(page).into_iter().filter(|id| *id != page).collect();
    out.sort();
    out
}

fn named(d: &DesignDocument, name: &str) -> Option<NodeId> {
    let doc = doc(d);
    let mut found: Vec<NodeId> = doc.nodes().filter(|n| n.name == name && !n.is_page()).map(|n| n.id).collect();
    found.sort();
    found.first().copied()
}

/// The object a scripted step acts on: the layer named "Probe", else the first layer.
fn probe(d: &DesignDocument) -> Option<NodeId> {
    named(d, "Probe").or_else(|| layers(d).first().copied())
}

/// The probe text: the text layer named "Title" and its characters.
fn title(d: &DesignDocument) -> Option<(NodeId, String)> {
    let id = named(d, "Title")?;
    match &doc(d).node(id)?.kind {
        NodeKind::Text(t) => Some((id, t.characters.clone())),
        _ => None,
    }
}

/// Sets the probe text to `edit` of its characters (as Unicode scalar values) through the
/// engine's `text.set`; the bridge writes the difference as text edits.
fn edit_title(d: &mut DesignDocument, edit: impl FnOnce(&mut Vec<char>) -> bool) -> bool {
    let Some((id, characters)) = title(d) else { return false };
    let mut chars: Vec<char> = characters.chars().collect();
    if !edit(&mut chars) {
        return false;
    }
    run(d, "text.set", json!({ "id": id.0, "characters": chars.into_iter().collect::<String>() }))
}

fn frames(d: &DesignDocument) -> Vec<NodeId> {
    let doc = doc(d);
    let Ok(page) = d.session().page() else { return Vec::new() };
    doc.node(page)
        .map(|p| p.children.iter().copied().filter(|c| doc.node(*c).is_some_and(|n| matches!(n.kind, NodeKind::Frame))).collect())
        .unwrap_or_default()
}

/// Runs a command; a refusal for the document's state is no step (`Ok(false)`).
fn run(d: &mut DesignDocument, command: &str, params: Value) -> bool {
    match d.execute(command, params.clone()) {
        Ok(_) => true,
        Err(e) => {
            if std::env::var_os("DESIGN_DEBUG").is_some() {
                eprintln!("DEBUG {command} {params}: {e}");
            }
            false
        }
    }
}

fn png(seed: u32) -> Vec<u8> {
    let rgb = [(seed & 0xff) as u8, ((seed >> 8) & 0xff) as u8, ((seed >> 16) & 0xff) as u8 ^ 0x5a];
    let mut out = Vec::new();
    image::RgbaImage::from_pixel(3, 3, image::Rgba([rgb[0], rgb[1], rgb[2], 255]))
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .unwrap();
    out
}

fn color(rng: &mut Rng) -> String {
    format!("#{:06x}", rng.next_u64() & 0xff_ffff)
}

impl KitAdapter for DesignKit {
    type Doc = DesignDocument;

    fn name(&self) -> &str {
        "kit.design"
    }

    fn schema(&self) -> &str {
        kition_design_kdm::SCHEMA_JSON
    }

    fn create(&self) -> Result<DesignDocument> {
        let mut d = DesignDocument::create(NAME, 800, 600).map_err(err)?;
        let frame = layers(&d).first().copied().ok_or("no frame")?;
        let add = |d: &mut DesignDocument, command: &str, params: Value, name: &str| -> Result<()> {
            let v = d.execute(command, params).map_err(err)?;
            let id = v["id"].as_u64().or_else(|| d.session().selection().first().map(|n| n.0)).ok_or("no id")?;
            d.execute("object.rename", json!({ "ids": [id], "name": name })).map_err(err)?;
            Ok(())
        };
        add(&mut d, "shape.rectangle", json!({ "x": 20, "y": 20, "width": 100, "height": 60, "parent": frame.0 }), "Probe")?;
        add(&mut d, "shape.ellipse", json!({ "x": 150, "y": 20, "width": 60, "height": 60, "parent": frame.0 }), "Circle")?;
        add(&mut d, "frame.create", json!({ "x": 900, "y": 0, "width": 300, "height": 200 }), "Second")?;
        add(&mut d, "text.create", json!({ "text": "Hello", "x": 20, "y": 120, "parent": frame.0 }), "Title")?;
        lend_libraries(&mut d)?;
        Ok(d)
    }

    fn join(&self, snapshot: &[u8], blobs: &Blobs) -> Result<DesignDocument> {
        let mut d = DesignDocument::open_with_replica(NAME, &[], snapshot, &encode_blobs(&to_memory(blobs)))?;
        lend_libraries(&mut d)?;
        Ok(d)
    }

    fn version(&self, doc: &mut DesignDocument) -> Result<Vec<u8>> {
        doc.version()
    }

    fn export_changes(&self, doc: &mut DesignDocument, since: &[u8]) -> Result<Vec<u8>> {
        doc.export_changes(since)
    }

    fn import_changes(&self, doc: &mut DesignDocument, bytes: &[u8]) -> Result<()> {
        doc.import_changes(bytes).map(|_| ())
    }

    fn snapshot(&self, doc: &mut DesignDocument) -> Result<Vec<u8>> {
        doc.snapshot()
    }

    fn state(&self, doc: &mut DesignDocument) -> Result<Value> {
        doc.state_json()
    }

    fn undo(&self, doc: &mut DesignDocument) -> Result<bool> {
        doc.undo().map(|(done, _)| done)
    }

    fn redo(&self, doc: &mut DesignDocument) -> Result<bool> {
        doc.redo().map(|(done, _)| done)
    }

    fn random_edit(&self, d: &mut DesignDocument, rng: &mut Rng) -> Result<Option<String>> {
        let all = layers(d);
        let pick = |rng: &mut Rng| rng.pick(&all).copied();
        let x = (rng.below(700)) as f64;
        let y = (rng.below(500)) as f64;
        let op = rng.below(20);
        let label = match op {
            0 => {
                run(d, "shape.rectangle", json!({ "x": x, "y": y, "width": 20 + rng.below(80), "height": 20 + rng.below(80) })).then_some("rectangle")
            }
            1 => run(d, "shape.ellipse", json!({ "x": x, "y": y, "width": 30, "height": 30 })).then_some("ellipse"),
            2 => pick(rng)
                .and_then(|id| run(d, "transform.move", json!({ "ids": [id.0], "dx": rng.below(40) as f64 - 20.0, "dy": 5 })).then_some("move")),
            3 => pick(rng).and_then(|id| run(d, "transform.resize", json!({ "ids": [id.0], "width": 10 + rng.below(200) })).then_some("resize")),
            4 => {
                let c = color(rng);
                pick(rng).and_then(|id| run(d, "node.set", json!({ "id": id.0, "props": { "fill": c } })).then_some("fill"))
            }
            5 => pick(rng)
                .and_then(|id| run(d, "object.rename", json!({ "ids": [id.0], "name": format!("Layer {}", rng.below(1000)) })).then_some("rename")),
            6 => pick(rng).and_then(|id| run(d, "object.toggleVisible", json!({ "ids": [id.0] })).then_some("visible")),
            7 => pick(rng)
                .and_then(|id| run(d, "node.set", json!({ "id": id.0, "props": { "opacity": (rng.below(10) as f64) / 10.0 } })).then_some("opacity")),
            8 => {
                let (a, b) = (pick(rng), pick(rng));
                match (a, b) {
                    (Some(a), Some(b)) if a != b => run(d, "object.group", json!({ "ids": [a.0, b.0] })).then_some("group"),
                    _ => None,
                }
            }
            9 => pick(rng).and_then(|id| run(d, "object.ungroup", json!({ "ids": [id.0] })).then_some("ungroup")),
            10 => frames(d).first().copied().and_then(|f| run(d, "layout.add", json!({ "ids": [f.0], "direction": "vertical" })).then_some("layout")),
            11 => pick(rng).and_then(|id| run(d, "edit.delete", json!({ "ids": [id.0] })).then_some("delete")),
            12 => pick(rng).and_then(|id| run(d, "edit.duplicate", json!({ "ids": [id.0] })).then_some("duplicate")),
            13 => run(d, "text.create", json!({ "text": format!("Note {}", rng.below(100)), "x": x, "y": y })).then_some("text"),
            14 => {
                let doc = doc(d);
                let texts: Vec<NodeId> =
                    all.iter().copied().filter(|id| doc.node(*id).is_some_and(|n| matches!(n.kind, NodeKind::Text(_)))).collect();
                rng.pick(&texts)
                    .copied()
                    .and_then(|id| run(d, "text.set", json!({ "id": id.0, "characters": format!("Text {}", rng.below(100)) })).then_some("text.set"))
            }
            15 => {
                let c = color(rng);
                run(
                    d,
                    "style.create",
                    json!({ "type": "paint", "name": format!("Colour {}", rng.below(100)), "value": [{ "type": "solid", "color": c }] }),
                )
                .then_some("style")
            }
            16 => {
                let (Some(id), Some(f)) = (pick(rng), rng.pick(&frames(d)).copied()) else { return Ok(None) };
                run(d, "node.reparent", json!({ "ids": [id.0], "parent": f.0 })).then_some("reparent")
            }
            17 => {
                // Each synthetic library's one component is named as the library.
                let links: Vec<(String, String)> = doc(d).libraries.enabled.iter().map(|l| (l.id.clone(), l.name.clone())).collect();
                rng.pick(&links).cloned().and_then(|(id, name)| {
                    run(d, "library.place", json!({ "library": id, "component": name, "x": x, "y": y })).then_some("library.place")
                })
            }
            18 => {
                let enabled: Vec<String> = doc(d).libraries.enabled.iter().map(|l| l.path.clone()).collect();
                let fresh: Vec<&str> = LIBRARIES.iter().copied().filter(|p| !enabled.iter().any(|e| e == p)).collect();
                rng.pick(&fresh).copied().and_then(|path| run(d, "library.add", json!({ "path": path })).then_some("library.add"))
            }
            19 => {
                let links: Vec<String> = doc(d).libraries.enabled.iter().map(|l| l.id.clone()).collect();
                rng.pick(&links).cloned().and_then(|id| run(d, "library.remove", json!({ "id": id })).then_some("library.remove"))
            }
            _ => pick(rng).and_then(|id| run(d, "arrange.backward", json!({ "ids": [id.0] })).then_some("backward")),
        };
        // A step that did not apply (nothing to act on) adds a layer instead, so every step edits.
        Ok(match label {
            Some(label) => Some(label.to_owned()),
            None => run(d, "shape.rectangle", json!({ "x": x, "y": y, "width": 24, "height": 24 })).then(|| "rectangle".to_owned()),
        })
    }

    fn script(&self, d: &mut DesignDocument, step: &Script) -> Result<bool> {
        Ok(match step {
            Script::SetField { slot: 0, value } => match probe(d) {
                Some(id) => run(d, "node.set", json!({ "id": id.0, "props": { "opacity": f64::from(value % 100) / 100.0 } })),
                None => false,
            },
            Script::SetField { slot: 1, value } => match probe(d) {
                Some(id) => run(d, "object.rename", json!({ "ids": [id.0], "name": format!("Probe {value}") })),
                None => false,
            },
            Script::SetField { .. } => false,
            Script::Create => run(d, "shape.rectangle", json!({ "x": 300, "y": 300, "width": 40, "height": 40 })),
            Script::Delete => probe(d).is_some_and(|id| run(d, "edit.delete", json!({ "ids": [id.0] }))),
            Script::Duplicate => probe(d).is_some_and(|id| run(d, "edit.duplicate", json!({ "ids": [id.0] }))),
            Script::Reorder { which } => {
                let doc = doc(d);
                let Some(frame) = frames(d).first().copied() else { return Ok(false) };
                let kids = doc.node(frame).map(|n| n.children.clone()).unwrap_or_default();
                match (which, kids.first(), kids.last()) {
                    (0, Some(first), _) => run(d, "node.reorder", json!({ "id": first.0, "index": kids.len() - 1 })),
                    (_, _, Some(last)) => run(d, "node.reorder", json!({ "id": last.0, "index": 0 })),
                    _ => false,
                }
            }
            Script::Reparent { which } => {
                let f = frames(d);
                let (Some(a), Some(b)) = (f.first().copied(), f.get(1).copied()) else { return Ok(false) };
                let (child, parent) = if *which == 0 { (a, b) } else { (b, a) };
                run(d, "node.reparent", json!({ "ids": [child.0], "parent": parent.0 }))
            }
            Script::InsertText { at, text } => edit_title(d, |chars| {
                let at = (*at).min(chars.len());
                chars.splice(at..at, text.chars());
                true
            }),
            Script::DeleteText { at, len } => edit_title(d, |chars| {
                let end = at.saturating_add(*len).min(chars.len());
                if *at >= end {
                    return false;
                }
                chars.drain(*at..end);
                true
            }),
            Script::SplitParagraph { at } => edit_title(d, |chars| {
                let at = (*at).min(chars.len());
                chars.insert(at, '\n');
                true
            }),
            Script::MergeParagraph { at } => edit_title(d, |chars| {
                if chars.get(*at) != Some(&'\n') {
                    return false;
                }
                chars.remove(*at);
                true
            }),
            Script::InsertBlob { seed } => {
                let data = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, png(*seed));
                run(d, "file.placeImage", json!({ "data": data, "x": 400, "y": 400 }))
            }
            _ => false,
        })
    }

    /// The probe text layer's characters (paragraphs joined by `\n`): `Node.text` is a KDM `text`.
    fn text(&self, d: &mut DesignDocument) -> Result<Option<String>> {
        Ok(title(d).map(|(_, characters)| characters))
    }

    fn check_view(&self, d: &mut DesignDocument) -> Result<()> {
        let ds = d.session().ds().map_err(err)?;
        let kdm = ds.kdm().ok_or("no replica")?;
        let (read, _) = kition_design_kdm::read_document(kdm.replica(), kdm.blobs()).map_err(err)?;
        if kition_design_kdm::same_content(&read, &ds.doc) { Ok(()) } else { Err("the view differs from its replica".into()) }
    }

    fn blobs(&self, d: &mut DesignDocument) -> Result<Blobs> {
        let bytes = d.blobs()?;
        Ok(decode_blobs(&bytes)?.0.into_iter().map(|(hash, data)| (hash, data.to_vec())).collect())
    }

    fn add_blobs(&self, d: &mut DesignDocument, blobs: &Blobs) -> Result<()> {
        d.add_blobs(&encode_blobs(&to_memory(blobs)))
    }

    fn export_file(&self, d: &mut DesignDocument) -> Result<Option<Vec<u8>>> {
        d.prepare_save()?;
        d.seal_and_save(None).map(Some)
    }

    /// The largest example design, as a person opens it.
    fn large(&self) -> Result<Option<DesignDocument>> {
        let bytes = include_bytes!("../../../../examples/ridgeline-studio-dashboard.kdesign");
        DesignDocument::open(NAME, bytes).map(Some)
    }

    /// One edit and the repaint of the view it causes (a 1440 × 900 window at 100%).
    fn keystroke(&self, d: &mut DesignDocument, rng: &mut Rng) -> Result<()> {
        self.random_edit(d, rng)?;
        let (w, h) = (d.width().min(1440), d.height().min(900));
        d.render(0.0, 0.0, 1.0, w, h).map(|_| ())
    }
}
