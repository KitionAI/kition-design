//! Writes the KHP conformance fixtures (`conformance/`) and the check catalogue
//! (`agent/checks.json`, from [`kition_design_kit::check::CATALOGUE`]): `basic.kdesign` (no
//! diagnostics), `edit.json` (steps that change it), and one `check-<code>.kdesign` per catalogue code,
//! on which the check reports that code. Each fixture is made with the engine's own commands from a
//! new design; the broken references a file can hold but the commands never make (a main component,
//! an image, a variable or a style the file lost) are made by editing the saved file, as a file
//! damaged elsewhere would be.
//!
//! `cargo run -p kition-design-kit --example conformance -- <repository> [fixture names]` (with names,
//! only those are written).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use base64::Engine as _;
use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;
use serde_json::{Value, json};

type Build = fn(&mut DesignDocument) -> Result<(), String>;
type Damage = fn(&mut Value);

fn run(doc: &mut DesignDocument, command: &str, params: Value) -> Result<Value, String> {
    doc.execute(command, params).map_err(|error| format!("{command}: {error}"))
}

/// The id a command answered (`id` at the top or in `result`).
fn id_of(answer: &Value) -> Result<u64, String> {
    answer["id"].as_u64().or_else(|| answer["result"]["id"].as_u64()).ok_or_else(|| format!("no id in {answer}"))
}

/// The created design's frame (the first layer of its page).
fn frame(doc: &DesignDocument) -> u64 {
    let ds = doc.session().ds().unwrap();
    let page = ds.doc.node(ds.page).unwrap();
    page.children[0].0
}

fn rectangle(doc: &mut DesignDocument, x: f64, y: f64, w: f64, h: f64, color: &str) -> Result<u64, String> {
    let parent = frame(doc);
    id_of(&run(doc, "shape.rectangle", json!({ "x": x, "y": y, "width": w, "height": h, "parent": parent, "props": { "fills": [color] } }))?)
}

fn text(doc: &mut DesignDocument, characters: &str, x: f64, y: f64, props: Value) -> Result<u64, String> {
    let parent = frame(doc);
    id_of(&run(doc, "text.create", json!({ "text": characters, "x": x, "y": y, "parent": parent, "props": props }))?)
}

fn set(doc: &mut DesignDocument, id: u64, props: Value) -> Result<(), String> {
    run(doc, "node.set", json!({ "id": id, "props": props })).map(|_| ())
}

/// A colour collection with one variable, `Brand` = `#5645D4`; answers the variable's id.
fn brand(doc: &mut DesignDocument) -> Result<String, String> {
    let collection = run(doc, "variable.createCollection", json!({ "name": "Colors" }))?;
    let collection = collection["id"].as_str().or_else(|| collection["result"]["id"].as_str()).ok_or("no collection id")?.to_owned();
    let variable = run(doc, "variable.create", json!({ "collection": collection, "name": "Brand", "type": "color", "value": "#5645D4" }))?;
    variable["id"].as_str().or_else(|| variable["result"]["id"].as_str()).map(str::to_owned).ok_or_else(|| format!("no variable id in {variable}"))
}

const LONG: &str = "Kition Design lays this sentence out in a narrow box, so it needs several lines to fit.";

fn basic(doc: &mut DesignDocument) -> Result<(), String> {
    rectangle(doc, 40.0, 40.0, 160.0, 100.0, "#5645D4")?;
    let parent = frame(doc);
    run(doc, "shape.ellipse", json!({ "x": 240, "y": 60, "width": 120, "height": 120, "parent": parent, "props": { "fills": ["#F2A541"] } }))?;
    rectangle(doc, 40.0, 180.0, 320.0, 80.0, "#1F2937")?;
    Ok(())
}

fn missing_component(doc: &mut DesignDocument) -> Result<(), String> {
    let main = rectangle(doc, 20.0, 20.0, 80.0, 40.0, "#5645D4")?;
    let component = id_of(&run(doc, "component.create", json!({ "ids": [main] }))?)?;
    run(doc, "instance.create", json!({ "component": component, "x": 200, "y": 100 }))?;
    Ok(())
}

/// The instance's main component is gone from the file: instances name a layer it does not have.
fn lose_component(json: &mut Value) {
    rename_ids(json, "component");
}

/// Every value of `key` (a layer id) in the file names a layer it does not have.
fn rename_ids(value: &mut Value, key: &str) {
    match value {
        Value::Object(map) => {
            for (name, inner) in map.iter_mut() {
                if name == key && inner.is_u64() {
                    *inner = json!(999_999);
                } else {
                    rename_ids(inner, key);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|item| rename_ids(item, key)),
        _ => {}
    }
}

fn missing_image(doc: &mut DesignDocument) -> Result<(), String> {
    let mut png = Vec::new();
    let picture = image::RgbaImage::from_fn(8, 8, |x, y| image::Rgba([(x * 32) as u8, (y * 32) as u8, 128, 255]));
    picture.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).map_err(|e| e.to_string())?;
    let parent = frame(doc);
    let data = base64::engine::general_purpose::STANDARD.encode(png);
    run(doc, "image.place", json!({ "data": data, "x": 40, "y": 40, "width": 120, "height": 120, "parent": parent }))?;
    Ok(())
}

fn broken_interaction(doc: &mut DesignDocument) -> Result<(), String> {
    let button = rectangle(doc, 40.0, 40.0, 120.0, 40.0, "#5645D4")?;
    let second = id_of(&run(doc, "frame.create", json!({ "x": 600, "y": 0, "width": 400, "height": 300 }))?)?;
    run(doc, "proto.interaction.add", json!({ "id": button, "dest": second }))?;
    Ok(())
}

/// Every prototype destination names a layer the file no longer has.
fn lose_destinations(json: &mut Value) {
    rename_ids(json, "dest");
}

/// Every flow starts at a layer the file no longer has.
fn lose_flow_starts(json: &mut Value) {
    rename_ids(json, "start");
}

fn broken_flow(doc: &mut DesignDocument) -> Result<(), String> {
    let start = frame(doc);
    run(doc, "proto.flow.add", json!({ "start": start, "name": "Onboarding" }))?;
    Ok(())
}

fn text_overflow(doc: &mut DesignDocument) -> Result<(), String> {
    let id = text(doc, LONG, 40.0, 40.0, json!({ "fontSize": 16, "fills": ["#111111"] }))?;
    set(doc, id, json!({ "textAutoResize": "none", "width": 160, "height": 20 }))
}

fn low_contrast(doc: &mut DesignDocument) -> Result<(), String> {
    text(doc, "Pale caption", 40.0, 40.0, json!({ "fontSize": 14, "fills": ["#D9D9D9"] })).map(|_| ())
}

fn missing_variable(doc: &mut DesignDocument) -> Result<(), String> {
    let variable = brand(doc)?;
    let id = rectangle(doc, 40.0, 40.0, 120.0, 80.0, "#5645D4")?;
    run(doc, "variable.bind", json!({ "ids": [id], "field": "fills/0/color", "variable": variable }))?;
    Ok(())
}

/// The file's variables are gone; layers still name them.
fn lose_variables(json: &mut Value) {
    json["variables"] = json!({ "collections": [], "variables": [] });
}

fn missing_style(doc: &mut DesignDocument) -> Result<(), String> {
    let id = rectangle(doc, 40.0, 40.0, 120.0, 80.0, "#5645D4")?;
    let style = run(doc, "style.create", json!({ "type": "paint", "name": "Brand/Primary", "from": id }))?;
    let style = style["id"].as_str().or_else(|| style["result"]["id"].as_str()).ok_or_else(|| format!("no style id in {style}"))?.to_owned();
    run(doc, "style.apply", json!({ "ids": [id], "style": style, "target": "fill" }))?;
    Ok(())
}

/// The file's styles are gone; layers still name them.
fn lose_styles(json: &mut Value) {
    json["styles"] = json!([]);
}

fn outside_clip(doc: &mut DesignDocument) -> Result<(), String> {
    let parent = frame(doc);
    set(doc, parent, json!({ "clip": true }))?;
    let id = rectangle(doc, 40.0, 40.0, 80.0, 80.0, "#5645D4")?;
    set(doc, id, json!({ "x": 900, "y": 40 }))
}

fn empty_component(doc: &mut DesignDocument) -> Result<(), String> {
    let id = id_of(&run(doc, "frame.create", json!({ "x": 600, "y": 0, "width": 100, "height": 100, "props": { "fills": [] } }))?)?;
    run(doc, "component.create", json!({ "ids": [id] }))?;
    Ok(())
}

fn font_substituted(doc: &mut DesignDocument) -> Result<(), String> {
    text(doc, "Set in a font the kit lacks", 40.0, 40.0, json!({ "fontFamily": "Kition Absent Sans", "fontSize": 16, "fills": ["#111111"] }))
        .map(|_| ())
}

fn text_ellipsis(doc: &mut DesignDocument) -> Result<(), String> {
    let id = text(doc, LONG, 40.0, 40.0, json!({ "fontSize": 16, "fills": ["#111111"] }))?;
    set(doc, id, json!({ "textAutoResize": "truncate", "width": 160, "height": 20 }))
}

fn unbound_color(doc: &mut DesignDocument) -> Result<(), String> {
    brand(doc)?;
    rectangle(doc, 40.0, 40.0, 120.0, 80.0, "#5645D4").map(|_| ())
}

fn unused_style(doc: &mut DesignDocument) -> Result<(), String> {
    rectangle(doc, 40.0, 40.0, 120.0, 80.0, "#5645D4")?;
    run(doc, "style.create", json!({ "type": "paint", "name": "Brand/Unused", "value": [{ "type": "solid", "color": "#0E9F6E" }] }))?;
    Ok(())
}

fn off_pixel(doc: &mut DesignDocument) -> Result<(), String> {
    rectangle(doc, 40.5, 40.25, 120.0, 80.5, "#5645D4").map(|_| ())
}

fn absolute_in_auto_layout(doc: &mut DesignDocument) -> Result<(), String> {
    rectangle(doc, 40.0, 40.0, 80.0, 40.0, "#5645D4")?;
    let b = rectangle(doc, 140.0, 40.0, 80.0, 40.0, "#F2A541")?;
    let parent = frame(doc);
    run(doc, "layout.add", json!({ "ids": [parent] }))?;
    set(doc, b, json!({ "absolute": true }))
}

fn empty_text(doc: &mut DesignDocument) -> Result<(), String> {
    text(doc, " ", 40.0, 40.0, json!({})).map(|_| ())
}

fn empty_frame(doc: &mut DesignDocument) -> Result<(), String> {
    run(doc, "frame.create", json!({ "x": 600, "y": 0, "width": 100, "height": 100, "props": { "fills": [] } })).map(|_| ())
}

fn truncated(doc: &mut DesignDocument) -> Result<(), String> {
    for index in 0..=kition_design_kit::check::PER_CODE {
        let x = 600.0 + (index % 10) as f64 * 60.0;
        let y = (index / 10) as f64 * 60.0;
        run(doc, "frame.create", json!({ "x": x, "y": y, "width": 40, "height": 40, "props": { "fills": [] } }))?;
    }
    Ok(())
}

/// Rewrites a `.kdesign` file: its document through `damage` (on the document's JSON, then saved
/// as the same document, as another program's edit), members `keep` refuses dropped.
fn rewrite(bytes: &[u8], damage: Option<Damage>, keep: fn(&str) -> bool) -> Vec<u8> {
    let bytes = match damage {
        Some(damage) => {
            let mut file = kition_design_format::load_with_replica(bytes).unwrap();
            let mut json = kition_design_format::to_json(&file.doc).unwrap();
            damage(&mut json);
            let (mut doc, _) = kition_design_format::from_json(&json.to_string()).unwrap();
            doc.images = std::mem::take(&mut file.doc.images);
            doc.published = file.doc.published.take();
            file.doc = doc;
            kition_design_format::save_edited(&file).unwrap()
        }
        None => bytes.to_vec(),
    };
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
    let mut out = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for index in 0..archive.len() {
        let mut member = archive.by_index(index).unwrap();
        let name = member.name().to_owned();
        if !keep(&name) {
            continue;
        }
        let mut data = Vec::new();
        member.read_to_end(&mut data).unwrap();
        out.start_file(name, options).unwrap();
        out.write_all(&data).unwrap();
    }
    out.finish().unwrap().into_inner()
}

fn fixture(name: &str, build: Build, damage: Option<Damage>, drop_images: bool) -> Result<Vec<u8>, String> {
    let mut doc = DesignDocument::create(name, 400, 300)?;
    build(&mut doc)?;
    run(&mut doc, "select.none", json!({}))?;
    let bytes = doc.save_file(None)?;
    Ok(if damage.is_some() || drop_images {
        rewrite(&bytes, damage, if drop_images { |name: &str| !name.starts_with("images/") } else { |_: &str| true })
    } else {
        bytes
    })
}

fn main() -> Result<(), String> {
    let repo = PathBuf::from(std::env::args().nth(1).ok_or("usage: conformance <repository> [fixture names]")?);
    let only: Vec<String> = std::env::args().skip(2).collect();
    let dir = repo.join("conformance");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(repo.join("agent")).map_err(|e| e.to_string())?;
    let catalogue = serde_json::to_string_pretty(&kition_design_kit::check::catalogue_json()).map_err(|e| e.to_string())? + "\n";
    std::fs::write(repo.join("agent/checks.json"), catalogue).map_err(|e| e.to_string())?;

    let builds: Vec<(&str, Build, Option<Damage>, bool)> = vec![
        ("missing-component", missing_component, Some(lose_component), false),
        ("missing-image", missing_image, None, true),
        ("broken-interaction", broken_interaction, Some(lose_destinations), false),
        ("broken-flow", broken_flow, Some(lose_flow_starts), false),
        ("text-overflow", text_overflow, None, false),
        ("low-contrast", low_contrast, None, false),
        ("missing-variable", missing_variable, Some(lose_variables), false),
        ("missing-style", missing_style, Some(lose_styles), false),
        ("outside-clip", outside_clip, None, false),
        ("empty-component", empty_component, None, false),
        ("font-substituted", font_substituted, None, false),
        ("text-ellipsis", text_ellipsis, None, false),
        ("unbound-color", unbound_color, None, false),
        ("unused-style", unused_style, None, false),
        ("off-pixel", off_pixel, None, false),
        ("absolute-in-auto-layout", absolute_in_auto_layout, None, false),
        ("empty-text", empty_text, None, false),
        ("empty-frame", empty_frame, None, false),
        ("truncated", truncated, None, false),
    ];
    let mut files: Vec<(String, Vec<u8>)> = vec![("basic.kdesign".to_owned(), fixture("Basic.kdesign", basic, None, false)?)];
    for (code, build, damage, drop_images) in builds {
        files.push((format!("check-{code}.kdesign"), fixture(&format!("{code}.kdesign"), build, damage, drop_images)?));
    }
    files.push((
        "edit.json".to_owned(),
        (serde_json::to_string_pretty(&json!({
            "steps": [{ "command": "shape.ellipse", "params": { "x": 300, "y": 200, "width": 60, "height": 60, "props": { "fills": ["#0E9F6E"] } } }]
        }))
        .map_err(|e| e.to_string())?
            + "\n")
            .into_bytes(),
    ));
    for (name, bytes) in files {
        if only.is_empty() || only.contains(&name) {
            write(&dir, &name, &bytes)?;
        }
    }
    Ok(())
}

fn write(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(dir.join(name), bytes).map_err(|e| format!("{name}: {e}"))
}
