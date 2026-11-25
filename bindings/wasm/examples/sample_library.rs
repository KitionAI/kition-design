//! Writes the development host's sample library file and the editor tests' update of it, both
//! made with the kit's own engine from a new design (synthetic content, no third-party assets):
//!
//! - `web/public/sample/libraries/Design System.kdesign`: a design published as a library: the
//!   components "Button/Primary", "Button/Secondary" and "Badge", the paint styles
//!   "Brand/Primary" and "Brand/Success", and the variable collection "Theme" (modes Light and
//!   Dark) with the colours "Surface" and "Text";
//! - `web/tests/fixtures/Design System v2.kdesign`: the same library published again after
//!   "Button/Primary" turned red, for the editor test that accepts a library update.
//!
//! `cargo run -p kition-design-kit --example sample_library -- <repository>`
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;
use serde_json::{Value, json};

fn run(doc: &mut DesignDocument, command: &str, params: Value) -> Value {
    doc.execute(command, params.clone()).unwrap_or_else(|e| panic!("{command} {params}: {e}"))
}

fn id(answer: &Value) -> u64 {
    answer["id"].as_u64().or_else(|| answer["result"]["id"].as_u64()).unwrap_or_else(|| panic!("no id in {answer}"))
}

/// A frame named `name` at (`x`, `y`) with a fill, a corner radius and a centred label, made a
/// component; answers its id.
fn component(doc: &mut DesignDocument, name: &str, (x, y, w, h): (f64, f64, f64, f64), fill: &str, label: (&str, &str)) -> u64 {
    let frame = id(&run(doc, "frame.create", json!({ "x": x, "y": y, "width": w, "height": h, "name": name })));
    run(doc, "node.set", json!({ "id": frame, "props": { "fill": fill, "cornerRadius": 8 } }));
    let text = id(&run(
        doc,
        "text.create",
        json!({ "text": label.0, "x": x + 16.0, "y": y + h / 2.0 - 8.0, "parent": frame, "props": { "name": "Label" } }),
    ));
    run(doc, "node.set", json!({ "id": text, "props": { "fill": label.1 } }));
    id(&run(doc, "component.create", json!({ "ids": [frame] })))
}

fn main() {
    let repo = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| ".".into()));
    let mut doc = DesignDocument::create("Design System.kdesign", 1200, 800).unwrap();
    // The created design's frame is the canvas for nothing here: the components stand alone.
    let first = doc.query("layers", &json!({})).unwrap()["rows"][0]["id"].clone();
    run(&mut doc, "edit.delete", json!({ "ids": [first] }));
    let primary = component(&mut doc, "Button/Primary", (0.0, 0.0, 160.0, 44.0), "#5645d4", ("Continue", "#ffffff"));
    component(&mut doc, "Button/Secondary", (0.0, 80.0, 160.0, 44.0), "#ece9fb", ("Cancel", "#2a2380"));
    let badge = component(&mut doc, "Badge", (220.0, 0.0, 96.0, 28.0), "#00875f", ("New", "#ffffff"));
    run(&mut doc, "style.create", json!({ "type": "paint", "name": "Brand/Primary", "from": primary }));
    run(&mut doc, "style.create", json!({ "type": "paint", "name": "Brand/Success", "from": badge }));
    let theme = run(&mut doc, "variable.createCollection", json!({ "name": "Theme", "modes": ["Light", "Dark"] }))["id"].clone();
    run(
        &mut doc,
        "variable.create",
        json!({ "collection": theme, "name": "Surface", "type": "color", "values": { "Light": "#ffffff", "Dark": "#1b1e23" } }),
    );
    run(
        &mut doc,
        "variable.create",
        json!({ "collection": theme, "name": "Text", "type": "color", "values": { "Light": "#1a1a1a", "Dark": "#eceff1" } }),
    );
    run(&mut doc, "library.publish", json!({ "description": "Buttons, a badge, brand colours and a light and dark theme." }));
    let v1 = doc.save_file(None).unwrap();
    run(&mut doc, "node.set", json!({ "id": primary, "props": { "fill": "#e5484d" } }));
    run(&mut doc, "library.publish", json!({ "description": "Primary buttons are red." }));
    let v2 = doc.save_file(None).unwrap();
    for (path, bytes) in [("web/public/sample/libraries/Design System.kdesign", v1), ("web/tests/fixtures/Design System v2.kdesign", v2)] {
        let at = repo.join(path);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(&at, bytes).unwrap();
        println!("wrote {path}");
    }
}
