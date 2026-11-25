//! The panel lists and canvas reads of `bindings/wasm/src/lists.rs`: layers drag info, assets,
//! drop previews, libraries, the variable picker, the context menu, the canvas hit test, rename
//! previews, recent actions, grid geometry, spacing, Dev Mode reads, and the prototype's
//! destinations and expressions.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;
use serde_json::{Value, json};

/// A 400 × 300 design with a red 120 × 80 rectangle at (20, 20) in its frame.
fn design() -> (DesignDocument, Value, Value) {
    let mut doc = DesignDocument::create("Design.kdesign", 400, 300).unwrap();
    let frame = doc.query("layers", &json!({})).unwrap()["rows"][0]["id"].clone();
    let rect = doc.execute("shape.rectangle", json!({ "x": 20, "y": 20, "width": 120, "height": 80, "parent": frame })).unwrap()["id"].clone();
    doc.execute("fill.set", json!({ "ids": [rect], "color": "#ff0000" })).unwrap();
    doc.execute("edit.deselectAll", json!({})).unwrap();
    doc.mark_saved();
    (doc, frame, rect)
}

/// The document pixel of canvas point (`x`, `y`).
fn to_doc(doc: &DesignDocument, x: f64, y: f64) -> (f64, f64) {
    let area = &doc.inspect().unwrap()["area"];
    (x - area["x"].as_f64().unwrap(), y - area["y"].as_f64().unwrap())
}

fn select(doc: &mut DesignDocument, ids: &[&Value]) {
    doc.execute("select.set", json!({ "ids": ids })).unwrap();
}

fn is_png(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x89, b'P', b'N', b'G'])
}

#[test]
fn drop_info_lists_parents_indexes_and_the_zones_a_drag_takes() {
    let (mut doc, frame, rect) = design();
    let other = doc.execute("shape.ellipse", json!({ "x": 200, "y": 20, "width": 50, "height": 50, "parent": frame })).unwrap()["id"].clone();
    let info = doc.query("layers.dropInfo", &json!({ "moving": [rect] })).unwrap();
    let rows = info["rows"].as_array().unwrap();
    let row = |id: &Value| rows.iter().find(|r| r["id"] == *id).unwrap().clone();
    let f = row(&frame);
    assert_eq!(f["container"], true, "a frame takes drops into it");
    assert_eq!(f["index"], 0);
    let e = row(&other);
    assert_eq!(e["parent"], frame);
    assert_eq!(e["index"], 1, "bottom first: the ellipse was added after the rectangle");
    assert_eq!(e["container"], false);
    assert_eq!(e["structureLocked"], false);
    // Dropping the rectangle above the ellipse (higher z) puts it last: the rectangle below the
    // gap leaves it, so the final index is 1.
    assert_eq!(e["drop"]["above"], json!({ "parent": frame, "index": 1 }));
    assert_eq!(e["drop"]["below"], json!({ "parent": frame, "index": 0 }));
    assert_eq!(e["drop"]["into"], Value::Null);
    assert_eq!(row(&rect)["blocked"], true, "the dragged layer takes no drop");
    let above = &e["drop"]["above"];
    doc.execute("node.reparent", json!({ "ids": [rect], "parent": above["parent"], "index": above["index"] })).unwrap();
    let kids = doc.query("node", &json!({ "id": frame, "depth": 1 })).unwrap()["children"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].clone())
        .collect::<Vec<_>>();
    assert_eq!(kids, vec![other.clone(), rect.clone()], "the answer is what node.reparent takes");
    // Inside an instance the structure is locked.
    let comp = doc.execute("component.add", json!({ "ids": [frame] })).unwrap();
    let comp = comp.get("id").cloned().unwrap_or(frame.clone());
    let inst = doc.execute("instance.add", json!({ "component": comp, "x": 600, "y": 0 })).unwrap()["id"].clone();
    let inner = doc.query("node", &json!({ "id": inst, "depth": 1 })).unwrap()["children"][0]["id"].clone();
    let info = doc.query("layers.dropInfo", &json!({ "ids": [inst, inner] })).unwrap();
    assert_eq!(info["rows"][0]["container"], false, "an instance takes no drop into it");
    assert_eq!(info["rows"][0]["childrenLocked"], true);
    assert_eq!(info["rows"][1]["structureLocked"], true, "layers in an instance cannot be moved around");
}

#[test]
fn assets_list_tiles_folders_and_styles_and_drop_preview_finds_the_frame() {
    let (mut doc, frame, rect) = design();
    let button = doc.execute("frame.add", json!({ "x": 600, "y": 0, "width": 100, "height": 40, "name": "Buttons/Primary" })).unwrap()["id"].clone();
    doc.execute("component.add", json!({ "ids": [button] })).unwrap();
    let hidden = doc.execute("frame.add", json!({ "x": 800, "y": 0, "width": 40, "height": 40, "name": "_Private" })).unwrap()["id"].clone();
    doc.execute("component.add", json!({ "ids": [hidden] })).unwrap();
    select(&mut doc, &[&rect]);
    doc.execute("style.add", json!({ "type": "paint", "name": "Brand/Red" })).unwrap();
    let list = doc.query("assets.list", &json!({})).unwrap();
    assert_eq!(list["count"], 2);
    let tiles: Vec<Value> = list["folders"].as_array().unwrap().iter().flat_map(|f| f["tiles"].as_array().unwrap().clone()).collect();
    let primary = tiles.iter().find(|t| t["name"] == "Buttons/Primary").unwrap();
    assert_eq!(primary["label"], "Primary");
    assert_eq!(primary["set"], false);
    assert_eq!(primary["hidden"], false);
    assert!(list["folders"].as_array().unwrap().iter().any(|f| f["name"].as_str().unwrap().ends_with("Buttons")), "{list}");
    assert_eq!(tiles.iter().find(|t| t["name"] == "_Private").unwrap()["hidden"], true);
    let style = &list["styles"][0];
    assert_eq!((style["type"].as_str(), style["folder"].as_str(), style["label"].as_str()), (Some("paint"), Some("Brand"), Some("Red")));
    let found = doc.query("assets.list", &json!({ "query": "PRIM" })).unwrap();
    assert_eq!(found["count"], 1, "the search matches any case");
    assert_eq!(found["styles"], json!([]));
    // A drop over the frame lands in it; over empty canvas, on the page.
    let (x, y) = to_doc(&doc, 300.0, 250.0);
    let p = doc.query("drop.preview", &json!({ "x": x, "y": y, "id": primary["id"] })).unwrap();
    assert_eq!(p["parent"], frame);
    assert_eq!(p["dropTarget"], frame);
    assert_eq!(p["swap"], Value::Null);
    let (x, y) = to_doc(&doc, -500.0, -500.0);
    let p = doc.query("drop.preview", &json!({ "x": x, "y": y })).unwrap();
    assert_eq!(p["dropTarget"], Value::Null, "{p}");
    assert!(doc.query("drop.preview", &json!({ "x": "a" })).unwrap_err().starts_with("invalid-params"));
    // In auto layout: a slot and its insertion line.
    doc.execute("layout.add", json!({ "ids": [frame], "direction": "horizontal" })).unwrap();
    let (x, y) = to_doc(&doc, 5.0, 30.0);
    let p = doc.query("drop.preview", &json!({ "x": x, "y": y, "id": primary["id"] })).unwrap();
    assert_eq!(p["parent"], frame);
    assert_eq!(p["index"], 0, "{p}");
    assert_eq!(p["line"].as_array().map(Vec::len), Some(4));
}

#[test]
fn libraries_answer_without_reading_files() {
    let (doc, ..) = design();
    let list = doc.query("library.list", &json!({})).unwrap();
    assert_eq!(list["libraries"], json!([]), "{list}");
    assert!(doc.query("library.updates", &json!({})).is_ok());
}

#[test]
fn the_variable_picker_filters_by_type_scope_query_and_alias_loops() {
    let (mut doc, _, rect) = design();
    doc.execute("variable.addCollection", json!({ "name": "Tokens", "modes": ["Light", "Dark"] })).unwrap();
    doc.execute("variable.add", json!({ "collection": "Tokens", "name": "brand", "type": "color", "value": "#5645d4" })).unwrap();
    doc.execute("variable.add", json!({ "collection": "Tokens", "name": "frame only", "type": "color", "value": "#000000" })).unwrap();
    doc.execute("variable.update", json!({ "variable": "frame only", "scopes": ["FRAME_FILL"] })).unwrap();
    doc.execute("variable.add", json!({ "collection": "Tokens", "name": "space", "type": "number", "value": 8 })).unwrap();
    let names = |v: &Value| -> Vec<String> {
        v["groups"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|g| g["variables"].as_array().unwrap().iter().map(|x| x["name"].as_str().unwrap().to_owned()))
            .collect()
    };
    let fill = doc.query("variables.pickFor", &json!({ "ids": [rect], "field": "fills/0/color", "kind": "color" })).unwrap();
    assert_eq!(names(&fill), vec!["brand"], "a rectangle's fill leaves out frame-only colours and numbers: {fill}");
    assert_eq!(fill["groups"][0]["name"], "Tokens");
    assert_eq!(fill["groups"][0]["variables"][0]["swatch"], "#5645D4");
    let any = doc.query("variables.pickFor", &json!({ "target": "proto" })).unwrap();
    assert_eq!(names(&any).len(), 3);
    let space = any["groups"][0]["variables"].as_array().unwrap().iter().find(|v| v["name"] == "space").unwrap().clone();
    assert_eq!(space["display"], "8");
    assert_eq!(names(&doc.query("variables.pickFor", &json!({ "target": "proto", "query": "SPA" })).unwrap()), vec!["space"]);
    // An alias never points back at the variable itself.
    let alias = doc.query("variables.pickFor", &json!({ "target": "alias", "variable": "brand" })).unwrap();
    assert_eq!(names(&alias), vec!["frame only"], "{alias}");
    assert!(doc.query("variables.pickFor", &json!({ "kind": "vector" })).unwrap_err().starts_with("invalid-params"));
}

#[test]
fn the_context_menu_enables_every_offered_command_as_upstream() {
    let (mut doc, frame, rect) = design();
    let revision = doc.revision();
    let menu = doc.query("menu.context", &json!({ "ids": [rect] })).unwrap();
    let items = menu["items"].as_array().unwrap();
    let item = |id: &str| items.iter().find(|i| i["command"] == id).unwrap_or_else(|| panic!("no {id} in {menu}")).clone();
    assert_eq!(item("edit.copy")["enabled"], true);
    assert_eq!(item("edit.copy")["shortcut"], "Cmd+C");
    // Upstream's `available`: a command that has nothing to act on stays enabled.
    assert_eq!(item("edit.paste")["enabled"], true, "nothing copied yet, still enabled");
    assert_eq!(item("arrange.ungroup")["enabled"], true, "a rectangle: Ungroup does nothing");
    assert_eq!(item("component.detach")["enabled"], true, "not an instance: Detach does nothing");
    assert_eq!(item("vector.outlineStroke")["enabled"], true);
    assert_eq!(item("edit.delete")["enabled"], true);
    assert!(item("copyAs")["submenu"].as_array().unwrap().iter().any(|s| s["command"] == "edit.copyAsSvg"));
    assert!(items.iter().any(|i| i["separator"] == true));
    assert_eq!(doc.revision(), revision, "building the menu leaves the document alone");
    assert_eq!(doc.query("layers", &json!({})).unwrap()["rows"][0]["id"], frame, "nothing deleted");
    // Nothing selected: the canvas menu with its View toggles.
    doc.execute("edit.deselectAll", json!({})).unwrap();
    let (x, y) = to_doc(&doc, 50.0, 50.0);
    let menu = doc.query("menu.context", &json!({ "ids": [], "at": { "x": x, "y": y } })).unwrap();
    let items = menu["items"].as_array().unwrap();
    let rulers = items.iter().find(|i| i["command"] == "view.rulers").unwrap();
    assert_eq!(rulers["checked"], false);
    assert_eq!(items.iter().find(|i| i["command"] == "view.zoomFit").unwrap()["ui"], true);
    assert!(items.iter().find(|i| i["command"] == "edit.paste").unwrap()["params"]["x"].is_number());
    assert!(menu["selectLayer"].as_array().unwrap().iter().any(|l| l["id"] == rect), "{menu}");
}

#[test]
fn hit_test_resolves_a_click_like_the_canvas() {
    let (mut doc, frame, rect) = design();
    let group = doc.execute("arrange.group", json!({ "ids": [rect] })).unwrap();
    let group = group.get("id").cloned().unwrap_or_else(|| doc.query("node", &json!({ "id": rect })).unwrap()["id"].clone());
    let (x, y) = to_doc(&doc, 50.0, 50.0);
    let hit = doc.query("hitTest", &json!({ "x": x, "y": y })).unwrap();
    assert_eq!(hit["leaf"], rect);
    assert_eq!(hit["frame"], frame);
    assert_ne!(hit["target"], rect, "a plain click selects the group, not what is in it: {hit}");
    let _ = group;
    let deep = doc.query("hitTest", &json!({ "x": x, "y": y, "deep": true })).unwrap();
    assert_eq!(deep["target"], rect, "Cmd-click selects the leaf");
    let (x, y) = to_doc(&doc, -300.0, -300.0);
    assert_eq!(doc.query("hitTest", &json!({ "x": x, "y": y })).unwrap()["leaf"], Value::Null);
}

#[test]
fn rename_preview_shows_names_without_renaming() {
    let (mut doc, frame, rect) = design();
    let other = doc.execute("shape.ellipse", json!({ "x": 200, "y": 20, "width": 50, "height": 50, "parent": frame })).unwrap()["id"].clone();
    let revision = doc.revision();
    let p = doc.query("rename.preview", &json!({ "ids": [rect, other], "name": "Item $n" })).unwrap();
    let rows = p["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["after"], "Item 1");
    assert_eq!(rows[1]["after"], "Item 2");
    assert_eq!(rows[0]["changed"], true);
    assert_eq!(doc.revision(), revision);
    let bad = doc.query("rename.preview", &json!({ "ids": [rect], "find": "(", "regex": true })).unwrap();
    assert!(bad["error"].as_str().unwrap().contains("regular expression"));
}

#[test]
fn recent_actions_are_newest_first_and_capped() {
    let (mut doc, ..) = design();
    assert_eq!(doc.query("prefs.recentActions", &json!({})).unwrap()["actions"], json!([]));
    doc.record_action("edit.copy").unwrap();
    doc.record_action("arrange.group").unwrap();
    doc.record_action("edit.copy").unwrap();
    let r = doc.query("prefs.recentActions", &json!({})).unwrap();
    assert_eq!(r["actions"], json!(["edit.copy", "arrange.group"]));
    let max = r["max"].as_u64().unwrap() as usize;
    for k in 0..max + 5 {
        doc.record_action(&format!("a{k}")).unwrap();
    }
    assert_eq!(doc.query("prefs.recentActions", &json!({})).unwrap()["actions"].as_array().unwrap().len(), max);
    assert!(doc.record_action("  ").is_err());
}

#[test]
fn grid_geometry_and_spacing_info() {
    let mut doc = DesignDocument::create("Grid.kdesign", 400, 300).unwrap();
    let grid = doc
        .execute(
            "frame.add",
            json!({ "x": 0, "y": 400, "width": 320, "height": 200, "props": { "layout": "grid", "gridColumns": 3, "columnGap": 10, "rowGap": 10 } }),
        )
        .unwrap()["id"]
        .clone();
    for _ in 0..3 {
        doc.execute("shape.rectangle", json!({ "x": 0, "y": 0, "width": 50, "height": 50, "parent": grid })).unwrap();
    }
    let g = doc.query("layout.gridGeometry", &json!({ "id": grid })).unwrap();
    assert_eq!(g["columns"].as_array().unwrap().len(), 3, "{g}");
    assert_eq!(g["cells"].as_array().unwrap().len(), 3);
    assert_eq!(g["unit"].as_array().unwrap().len(), 6);
    let frame = doc.query("layers", &json!({})).unwrap()["rows"][1]["id"].clone();
    assert_eq!(doc.query("layout.gridGeometry", &json!({ "id": frame })).unwrap(), Value::Null, "not a grid");
    // Three layers in a row, 20 apart.
    let a = doc.execute("shape.rectangle", json!({ "x": 1000, "y": 0, "width": 40, "height": 40 })).unwrap()["id"].clone();
    let b = doc.execute("shape.rectangle", json!({ "x": 1060, "y": 0, "width": 40, "height": 40 })).unwrap()["id"].clone();
    let c = doc.execute("shape.rectangle", json!({ "x": 1120, "y": 0, "width": 40, "height": 40 })).unwrap()["id"].clone();
    let s = doc.query("arrange.spacingInfo", &json!({ "ids": [a, b, c] })).unwrap();
    assert_eq!(s["horizontal"]["value"], 20.0, "{s}");
    assert_eq!(s["horizontal"]["tracks"].as_array().unwrap().len(), 3);
    assert_eq!(s["horizontal"]["gaps"], json!([20.0, 20.0]));
    assert_eq!(s["vertical"], Value::Null);
}

#[test]
fn dev_focus_compare_and_the_playground_picture() {
    let (mut doc, frame, rect) = design();
    assert_eq!(doc.query("dev.focusState", &json!({})).unwrap(), Value::Null);
    doc.execute("node.set", json!({ "ids": [frame], "props": { "devStatus": "readyForDev" } })).unwrap();
    doc.execute("dev.focus", json!({ "id": frame })).unwrap();
    let f = doc.query("dev.focusState", &json!({})).unwrap();
    assert_eq!(f["id"], frame);
    assert_eq!(f["status"], "readyForDev");
    assert_eq!((f["index"].as_u64(), f["count"].as_u64()), (Some(0), Some(1)));
    assert_eq!(f["canStep"], false, "the only ready frame is the one shown");
    // A change after it was marked shows in the compare view.
    doc.execute("node.set", json!({ "ids": [rect], "props": { "width": 200 } })).unwrap();
    let c = doc.query("dev.compare", &json!({ "id": frame })).unwrap();
    assert_eq!(c["changed"], true, "{c}");
    assert!(c["modified"].as_u64().unwrap() >= 1);
    let after = &c["after"];
    assert!(after["width"].as_f64().unwrap() > 0.0);
    assert!(after["boxes"].as_array().unwrap().iter().any(|b| b["id"] == rect), "{c}");
    assert!(c["before"].is_object(), "the snapshot from when it was marked: {c}");
    let png = doc.dev_compare_render(&json!({ "id": frame, "side": "after" })).unwrap();
    assert!(is_png(&png));
    assert!(is_png(&doc.dev_compare_render(&json!({ "id": frame, "side": "before" })).unwrap()));
    assert!(doc.dev_compare_render(&json!({ "id": frame, "side": "middle" })).unwrap_err().starts_with("invalid-params"));
    // Against a saved version (the host's checkpoint carries its snapshot; the answer names its
    // replica version).
    let vid = json!(base64::Engine::encode(&base64::engine::general_purpose::STANDARD, doc.version().unwrap()));
    let snapshot = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, doc.snapshot().unwrap());
    let c = doc.query("dev.compare", &json!({ "id": frame, "snapshot": snapshot })).unwrap();
    assert_eq!(c["version"], vid);
    assert!(c.get("versions").is_none(), "the editor lists the host's checkpoints itself");
    assert!(doc.query("dev.compare", &json!({ "id": frame, "snapshot": "nope" })).is_err());
    // The playground's instance as a picture; the document stays as it was.
    let revision = doc.revision();
    let button =
        doc.execute("frame.add", json!({ "x": 600, "y": 0, "width": 100, "height": 40, "props": { "fill": "#5645d4" } })).unwrap()["id"].clone();
    doc.execute("component.add", json!({ "ids": [button] })).unwrap();
    let revision_after = doc.revision();
    assert!(revision_after > revision);
    let png = doc.playground_preview(&json!({ "id": button })).unwrap();
    assert!(is_png(&png));
    let img = image::load_from_memory(&png).unwrap();
    assert_eq!((img.width(), img.height()), (200, 80), "scale 2 by default");
    assert_eq!(doc.revision(), revision_after);
    assert!(doc.playground_preview(&json!({ "id": rect })).is_err(), "a rectangle has no playground");
    // Exiting the focus view ends the bar.
    doc.execute("dev.unfocus", json!({})).unwrap();
    assert_eq!(doc.query("dev.focusState", &json!({})).unwrap(), Value::Null);
}

#[test]
fn code_links_callouts_and_the_version_preview_state() {
    let (mut doc, frame, rect) = design();
    let comp = doc.execute("component.add", json!({ "ids": [frame] })).unwrap();
    let comp = comp.get("id").cloned().unwrap_or(frame.clone());
    assert_eq!(doc.query("codeLinks", &json!({ "id": comp })).unwrap()["links"], json!([]));
    doc.execute("codeConnect.set", json!({ "id": comp, "language": "react", "importPath": "./Card", "name": "Card" })).unwrap();
    let links = doc.query("codeLinks", &json!({ "id": comp })).unwrap();
    assert_eq!(links["owner"], comp);
    assert_eq!(links["links"].as_array().unwrap().len(), 1, "{links}");
    doc.execute("annotation.add", json!({ "id": rect, "label": "Primary action" })).unwrap();
    let c = doc.query("annotation.callouts", &json!({})).unwrap();
    let first = &c["callouts"][0];
    assert_eq!(first["id"], rect);
    assert_eq!(first["label"], "Primary action");
    assert_eq!(first["bounds"]["width"], 120.0);
    assert_eq!(first["visible"], true);
    assert_eq!(doc.query("version.previewState", &json!({})).unwrap(), Value::Null);
    let version = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, doc.version().unwrap());
    let snapshot = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, doc.snapshot().unwrap());
    doc.execute("version.preview", json!({ "snapshot": snapshot, "id": "checkpoint-1", "name": "First", "time": 1 })).unwrap();
    let p = doc.query("version.previewState", &json!({})).unwrap();
    assert_eq!(p["id"], "checkpoint-1", "{p}");
    assert_eq!(p["name"], "First");
    assert_eq!(p["version"], version);
}

#[test]
fn prototype_destinations_and_expressions() {
    let (mut doc, frame, rect) = design();
    let second = doc.execute("frame.add", json!({ "x": 600, "y": 0, "width": 200, "height": 200, "name": "Second" })).unwrap()["id"].clone();
    let nav = doc.query("proto.destinations", &json!({ "id": rect })).unwrap();
    let ids: Vec<Value> = nav["destinations"].as_array().unwrap().iter().map(|d| d["id"].clone()).collect();
    assert!(ids.contains(&frame) && ids.contains(&second), "{nav}");
    let scroll = doc.query("proto.destinations", &json!({ "id": rect, "kind": "scrollTo" })).unwrap();
    assert_eq!(scroll["destinations"], json!([]), "the rectangle is the frame's only layer");
    // Variants for Change to.
    let a = doc.execute("frame.add", json!({ "x": 0, "y": 600, "width": 50, "height": 50, "name": "State=On" })).unwrap()["id"].clone();
    let b = doc.execute("frame.add", json!({ "x": 100, "y": 600, "width": 50, "height": 50, "name": "State=Off" })).unwrap()["id"].clone();
    let ca = doc.execute("component.add", json!({ "ids": [a] })).unwrap();
    let ca = ca.get("id").cloned().unwrap_or(a.clone());
    let cb = doc.execute("component.add", json!({ "ids": [b] })).unwrap();
    let cb = cb.get("id").cloned().unwrap_or(b.clone());
    doc.execute("component.combineVariants", json!({ "ids": [ca, cb] })).unwrap();
    let change = doc.query("proto.destinations", &json!({ "id": ca, "kind": "changeTo" })).unwrap();
    assert_eq!(change["destinations"].as_array().unwrap().len(), 2, "{change}");
    assert!(doc.query("proto.destinations", &json!({ "kind": "teleport" })).unwrap_err().starts_with("invalid-params"));
    // Expressions.
    doc.execute("variable.addCollection", json!({ "name": "State" })).unwrap();
    doc.execute("variable.add", json!({ "collection": "State", "name": "count", "type": "number", "value": 0 })).unwrap();
    let ok = doc.query("proto.parseExpression", &json!({ "text": "count + 1", "expect": "value", "variable": "count" })).unwrap();
    assert_eq!(ok["ok"], true, "{ok}");
    assert_eq!(ok["type"], "number");
    assert!(ok["display"].as_str().unwrap().contains("count"));
    assert!(ok["expr"].is_object() || ok["expr"].is_array());
    let cond = doc.query("proto.parseExpression", &json!({ "text": "count + 1", "expect": "condition" })).unwrap();
    assert_eq!(cond["ok"], false);
    assert!(cond["error"].as_str().unwrap().contains("true or false"));
    let broken = doc.query("proto.parseExpression", &json!({ "text": "count +" })).unwrap();
    assert_eq!(broken["ok"], false);
    assert!(broken["at"].is_number());
    assert_eq!(broken["display"], Value::Null);
}

#[test]
fn junk_params_answer_errors_not_panics() {
    let (doc, ..) = design();
    for name in [
        "layers.dropInfo",
        "assets.list",
        "drop.preview",
        "variables.pickFor",
        "menu.context",
        "hitTest",
        "rename.preview",
        "layout.gridGeometry",
        "arrange.spacingInfo",
        "dev.compare",
        "codeLinks",
        "annotation.callouts",
        "proto.destinations",
        "proto.parseExpression",
    ] {
        for junk in [
            json!({}),
            json!({ "id": "x", "ids": "y", "x": "z", "at": 3, "kind": 5, "text": 7, "page": "p", "moving": {} }),
            json!({ "id": 999_999, "ids": [999_999], "x": 1e308, "y": -1e308 }),
        ] {
            let _ = doc.query(name, &junk);
        }
    }
    let _ = doc.dev_compare_render(&json!({ "id": 999_999, "side": "after" }));
    let _ = doc.playground_preview(&json!({ "properties": 3 }));
}

/// Review #14: dragging many siblings costs about linear time per row, not cubic: 2048 siblings
/// with every other one dragged answer quickly, each gap discounting the dragged layers below it.
#[test]
fn drop_info_over_many_dragged_siblings_is_fast() {
    let mut doc = DesignDocument::create("Design.kdesign", 400, 300).unwrap();
    let steps: Vec<Value> = (0..2048)
        .map(|i| json!({ "command": "shape.rectangle", "params": { "x": 500 + (i % 64) * 12, "y": (i / 64) * 12, "width": 10, "height": 10 } }))
        .collect();
    doc.execute("command.batch", json!({ "commands": steps })).unwrap();
    let page = doc.inspect().unwrap()["page"].clone();
    let kids: Vec<Value> =
        doc.query("node", &json!({ "id": page, "depth": 1 })).unwrap()["children"].as_array().unwrap().iter().map(|c| c["id"].clone()).collect();
    assert_eq!(kids.len(), 2049);
    let moving: Vec<Value> = kids.iter().step_by(2).cloned().collect();
    let started = std::time::Instant::now();
    let info = doc.query("layers.dropInfo", &json!({ "moving": moving })).unwrap();
    let took = started.elapsed();
    let rows = info["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 2049);
    for (index, row) in rows.iter().enumerate() {
        let index_in_parent = kids.iter().position(|k| *k == row["id"]).unwrap();
        assert_eq!(row["index"], index_in_parent, "{index}");
        if index_in_parent % 2 == 0 {
            assert_eq!(row["blocked"], true);
            continue;
        }
        let below = |gap: usize| gap - gap.div_ceil(2);
        assert_eq!(row["drop"]["above"]["index"], below(index_in_parent + 1), "{row}");
        assert_eq!(row["drop"]["below"]["index"], below(index_in_parent), "{row}");
    }
    assert!(took < std::time::Duration::from_millis(100), "layers.dropInfo took {took:?}");
}

/// Review #19: Dev Mode's overlays for another page (`redlines { page }`, `annotation.callouts {
/// page }`) are in that page's document pixels, as they are once that page is the current one.
#[test]
fn another_pages_dev_overlays_use_that_pages_frame() {
    let (mut doc, _, _) = design();
    let first = doc.inspect().unwrap()["page"].clone();
    doc.execute("page.add", json!({ "name": "Far" })).unwrap();
    let far = doc.inspect().unwrap()["page"].clone();
    assert_ne!(first, far);
    let a = doc.execute("shape.rectangle", json!({ "x": 7000, "y": 9000, "width": 100, "height": 60 })).unwrap()["id"].clone();
    let b = doc.execute("shape.rectangle", json!({ "x": 7300, "y": 9000, "width": 100, "height": 60 })).unwrap()["id"].clone();
    doc.execute("annotation.add", json!({ "id": a, "label": "Far away" })).unwrap();
    doc.execute("devMode.addMeasurement", json!({ "from": a, "to": b })).unwrap();
    let on_far = (doc.query("redlines", &json!({})).unwrap(), doc.query("annotation.callouts", &json!({})).unwrap());
    assert!(!on_far.0.as_array().unwrap().is_empty() && !on_far.1["callouts"].as_array().unwrap().is_empty());
    doc.execute("page.select", json!({ "id": first })).unwrap();
    assert_ne!(doc.inspect().unwrap()["area"], json!(null));
    let from_first = (doc.query("redlines", &json!({ "page": far })).unwrap(), doc.query("annotation.callouts", &json!({ "page": far })).unwrap());
    assert_eq!(from_first.0, on_far.0, "redlines");
    assert_eq!(from_first.1, on_far.1, "callouts");
}
