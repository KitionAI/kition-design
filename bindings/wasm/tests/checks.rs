//! `design_check` against its catalogue (`agent/checks.json`, KHP A38): the catalogue is the check's
//! own (`check::CATALOGUE`), errors first; every fixture in `conformance/` reports its code with a
//! target and, where the catalogue says the author can act, a fix; every code reported is
//! catalogued; `basic.kdesign` and a new design are clean; each fix is a command the kit offers that
//! clears its code; and checking never changes the design.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use std::path::Path;

use kition_design_kit::DesignDocument;
use kition_design_kit::check::CATALOGUE;
use kition_kit::KitDocument;
use kition_kit::diagnostics::{Diagnostic, Severity, check_diagnostics, report};
use serde_json::Value;

fn repo() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

fn catalogue() -> Vec<Value> {
    serde_json::from_str(&std::fs::read_to_string(repo().join("agent/checks.json")).unwrap()).unwrap()
}

fn fixture(name: &str) -> DesignDocument {
    let bytes = std::fs::read(repo().join("conformance").join(name)).unwrap_or_else(|error| panic!("{name}: {error}"));
    DesignDocument::open(name, &bytes).unwrap()
}

fn codes(found: &[Diagnostic]) -> Vec<String> {
    found.iter().map(|diagnostic| diagnostic.code.clone()).collect()
}

/// Codes that report something to know, with nothing for the author to fix.
const WITHOUT_FIX: &[&str] = &["design.font-substituted", "design.truncated"];

#[test]
fn the_catalogue_file_is_the_checks_own() {
    assert_eq!(serde_json::to_value(catalogue()).unwrap(), kition_design_kit::check::catalogue_json());
    let mut previous = Severity::Error;
    for entry in CATALOGUE {
        assert!(kition_kit::diagnostics::valid_code(entry.code) && entry.code.starts_with("design."), "{}", entry.code);
        assert!(entry.severity >= previous, "the catalogue lists errors first: {}", entry.code);
        previous = entry.severity;
        assert!(entry.summary.ends_with('.') && !entry.summary.contains('\n'), "{}: one sentence", entry.code);
    }
    let unique: std::collections::BTreeSet<&str> = CATALOGUE.iter().map(|entry| entry.code).collect();
    assert_eq!(unique.len(), CATALOGUE.len());
}

#[test]
fn every_fixture_reports_its_code() {
    let known: Vec<&str> = CATALOGUE.iter().map(|entry| entry.code).collect();
    let mut failures = Vec::new();
    for entry in catalogue() {
        let code = entry["code"].as_str().unwrap();
        let name = entry["fixture"].as_str().unwrap();
        assert_eq!(name, format!("check-{}.kdesign", code.trim_start_matches("design.")));
        let found = fixture(name).check();
        check_diagnostics(&found).unwrap();
        match found.iter().find(|diagnostic| diagnostic.code == code) {
            None => failures.push(format!("{name} does not report {code}: {:?}", codes(&found))),
            Some(diagnostic) => {
                if serde_json::to_value(diagnostic.severity).unwrap() != entry["severity"] {
                    failures.push(format!("{name}: {code} is not {}", entry["severity"]));
                }
                if code != "design.truncated" && diagnostic.target.path.is_none() && diagnostic.target.page.is_none() {
                    failures.push(format!("{name}: {code} has no target: {diagnostic:?}"));
                }
                if diagnostic.fix.is_none() != WITHOUT_FIX.contains(&code) {
                    failures.push(format!("{name}: {code} fix: {:?}", diagnostic.fix));
                }
            }
        }
        for diagnostic in &found {
            if !known.contains(&diagnostic.code.as_str()) {
                failures.push(format!("{name} reports {} outside the catalogue", diagnostic.code));
            }
        }
        assert_eq!(serde_json::to_value(&found).unwrap(), report(&found)["diagnostics"], "{name}: errors first");
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn the_basic_fixture_and_a_new_design_are_clean() {
    assert_eq!(codes(&fixture("basic.kdesign").check()), Vec::<String>::new());
    assert_eq!(codes(&DesignDocument::create("Untitled design.kdesign", 1440, 1024).unwrap().check()), Vec::<String>::new());
}

#[test]
fn each_fix_is_an_offered_command_and_clears_its_code() {
    let mut failures = Vec::new();
    for entry in CATALOGUE.iter().filter(|entry| !WITHOUT_FIX.contains(&entry.code)) {
        let name = format!("check-{}.kdesign", entry.code.trim_start_matches("design."));
        let mut doc = fixture(&name);
        let found = doc.check();
        let Some(fix) = found.iter().find(|diagnostic| diagnostic.code == entry.code).and_then(|diagnostic| diagnostic.fix.clone()) else {
            failures.push(format!("{name}: no fix for {}", entry.code));
            continue;
        };
        let command = doc.commands().into_iter().find(|command| command.id == fix.command);
        match command {
            None => failures.push(format!("{}: {} is not a command the kit offers", entry.code, fix.command)),
            Some(command) => assert!(command.schema.is_some(), "{} has a schema", command.id),
        }
        if let Err(error) = doc.execute(&fix.command, fix.params.clone().unwrap_or(Value::Null)) {
            failures.push(format!("{}: the fix {} fails: {error}", entry.code, fix.command));
            continue;
        }
        let after = doc.check();
        let left = after.iter().filter(|diagnostic| diagnostic.code == entry.code).count();
        let before = found.iter().filter(|diagnostic| diagnostic.code == entry.code).count();
        if left >= before {
            failures.push(format!("{}: still reported after its fix: {:?}", entry.code, codes(&after)));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// A check reads; it never changes the design (its content, dirty state and history).
#[test]
fn checking_changes_nothing() {
    for entry in catalogue() {
        let name = entry["fixture"].as_str().unwrap();
        let mut doc = fixture(name);
        let (revision, dirty, inspected, saved) = (doc.revision(), doc.dirty(), doc.inspect().unwrap(), doc.seal_and_save(None).unwrap());
        let _ = doc.check();
        assert_eq!((doc.revision(), doc.dirty()), (revision, dirty), "{name}");
        assert_eq!(doc.inspect().unwrap(), inspected, "{name}");
        assert_eq!(doc.seal_and_save(None).unwrap(), saved, "{name}");
    }
}

/// Fonts are asked about once per family, and a family the kit has is never reported.
#[test]
fn fonts_are_reported_per_layer_only_when_missing() {
    let doc = fixture("check-font-substituted.kdesign");
    let ds = doc.session().ds().unwrap();
    let mut asked = Vec::new();
    let found = kition_design_kit::check::check_with(&ds.doc, &mut |family| {
        asked.push(family.to_owned());
        family != "Kition Absent Sans"
    });
    assert_eq!(found.iter().filter(|d| d.code == "design.font-substituted").count(), 1, "{:?}", codes(&found));
    let unique: std::collections::BTreeSet<&String> = asked.iter().collect();
    assert_eq!(unique.len(), asked.len(), "{asked:?}");
    let all = kition_design_kit::check::check_with(&ds.doc, &mut |_| true);
    assert!(all.iter().all(|d| d.code != "design.font-substituted"));
}

/// Review #25e: a layer whose style reference is another style's *name* (not an id) follows no
/// style (styles propagate by id), so it is reported missing, and that style is not "used" by it.
#[test]
fn a_style_reference_by_name_is_missing() {
    use kition_design_doc::styles::{Style, StyleKind};
    use kition_design_doc::{Node, NodeId, NodeKind};
    let mut doc = kition_design_doc::Document::new("Styles");
    let page = doc.pages[0];
    doc.styles.push(Style {
        id: "S:brand".into(),
        name: "Brand".into(),
        description: String::new(),
        kind: StyleKind::Paint { paints: Vec::new() },
        bound: Default::default(),
    });
    let mut rect = Node::new(NodeId(0), NodeKind::Rectangle, "Button", 10.0, 10.0);
    rect.id = doc.alloc_id();
    rect.styles.fill = Some("Brand".into());
    doc.insert(rect, page, None).unwrap();
    let found = kition_design_kit::check::check(&doc);
    assert!(codes(&found).contains(&"design.missing-style".to_owned()), "{:?}", codes(&found));
    assert!(codes(&found).contains(&"design.unused-style".to_owned()), "{:?}", codes(&found));
}

/// Review #25e: the check is about linear in the layers: many empty main components and many text
/// layers side by side in one frame check quickly.
#[test]
fn checking_a_large_flat_design_is_fast() {
    use kition_design_doc::{Node, NodeKind, TextData};
    let mut doc = kition_design_doc::Document::new("Large");
    let page = doc.pages[0];
    for i in 0..8000 {
        let id = doc.alloc_id();
        let mut main = Node::new(id, NodeKind::Component { description: String::new(), props: Default::default() }, format!("Empty {i}"), 10.0, 10.0);
        main.fills.clear();
        main.transform = kition_design_geom::Affine::translate((f64::from(i) * 20.0, 0.0));
        doc.insert(main, page, None).unwrap();
    }
    let frame_id = doc.alloc_id();
    let mut frame = Node::new(frame_id, NodeKind::Frame, "Board", 170_000.0, 100.0);
    frame.transform = kition_design_geom::Affine::translate((0.0, 1000.0));
    doc.insert(frame, page, None).unwrap();
    for i in 0..8000 {
        let id = doc.alloc_id();
        let mut text = Node::new(id, NodeKind::Text(TextData::new("Label")), format!("Label {i}"), 15.0, 12.0);
        text.transform = kition_design_geom::Affine::translate((f64::from(i) * 20.0, 0.0));
        doc.insert(text, frame_id, None).unwrap();
    }
    let started = std::time::Instant::now();
    let found = kition_design_kit::check::check(&doc);
    let took = started.elapsed();
    assert!(codes(&found).contains(&"design.empty-component".to_owned()));
    assert!(took < std::time::Duration::from_millis(1500), "the check took {took:?}");
}

/// Review #25e: the backdrop under a text layer is still the nearest sibling under it that
/// overlaps it, with a sibling covering the whole frame and many small ones in between.
#[test]
fn the_backdrop_is_the_nearest_sibling_under_the_text() {
    use kition_design_color::{Color, Paint};
    use kition_design_doc::{Node, NodeKind, TextData};
    use kition_design_geom::Affine;
    let mut doc = kition_design_doc::Document::new("Backdrops");
    let page = doc.pages[0];
    let frame = doc.alloc_id();
    doc.insert(Node::new(frame, NodeKind::Frame, "Board", 4000.0, 400.0), page, None).unwrap();
    let add = |doc: &mut kition_design_doc::Document, kind: NodeKind, name: &str, (x, y, w, h): (f64, f64, f64, f64), fill: Option<Color>| {
        let id = doc.alloc_id();
        let mut n = Node::new(id, kind, name, w, h);
        n.transform = Affine::translate((x, y));
        if let Some(c) = fill {
            n.fills = vec![Paint::solid(c)];
        }
        doc.insert(n, frame, None).unwrap();
        id
    };
    add(&mut doc, NodeKind::Rectangle, "Dark", (0.0, 0.0, 4000.0, 400.0), Some(Color::from_rgb8_const(0x11, 0x11, 0x11)));
    for i in 0..200 {
        add(&mut doc, NodeKind::Rectangle, &format!("Light {i}"), (f64::from(i) * 20.0, 300.0, 18.0, 18.0), Some(Color::WHITE));
    }
    let black = Some(Color::BLACK);
    let on_dark = add(&mut doc, NodeKind::Text(TextData::new("On dark")), "On dark", (100.0, 50.0, 60.0, 14.0), black);
    let on_light = add(&mut doc, NodeKind::Text(TextData::new("x")), "On light", (401.0, 302.0, 10.0, 10.0), black);
    let found = kition_design_kit::check::check(&doc);
    let low: Vec<String> = found.iter().filter(|d| d.code == "design.low-contrast").filter_map(|d| d.target.path.clone()).collect();
    assert!(low.contains(&format!("layer:{on_dark}")), "{low:?}");
    assert!(!low.contains(&format!("layer:{on_light}")), "{low:?}");
}
