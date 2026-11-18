//! The prototype player (`player`, `playerRender`): it plays on its own copy of the design, follows
//! interactions (click, hover, press, keys, delays, drag), overlays, scrolling, variables and
//! conditionals, describes each transition and draws the screen, never changing the document.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;
use serde_json::{Value, json};

/// A design with frame A (400 × 300 at the canvas origin) holding a red button (20, 20, 120 × 80),
/// and frame B (400 × 300) to its right with a blue panel.
fn design() -> (DesignDocument, Value, Value, Value) {
    let mut doc = DesignDocument::create("Proto.kdesign", 400, 300).unwrap();
    let a = doc.query("layers", &json!({})).unwrap()["rows"][0]["id"].clone();
    let button = doc.execute("shape.rectangle", json!({ "x": 20, "y": 20, "width": 120, "height": 80, "parent": a })).unwrap()["id"].clone();
    doc.execute("fill.set", json!({ "ids": [button], "color": "#ff0000" })).unwrap();
    let b = doc.execute("frame.create", json!({ "x": 600, "y": 0, "width": 400, "height": 300 })).unwrap()["id"].clone();
    let panel = doc.execute("shape.rectangle", json!({ "x": 0, "y": 0, "width": 400, "height": 300, "parent": b })).unwrap()["id"].clone();
    doc.execute("fill.set", json!({ "ids": [panel], "color": "#0000ff" })).unwrap();
    doc.execute("select.none", json!({})).unwrap();
    doc.mark_saved();
    (doc, a, button, b)
}

fn start(doc: &mut DesignDocument, frame: &Value) -> Value {
    doc.player("start", &json!({ "start": frame, "time": 0 })).unwrap()
}

fn event(doc: &mut DesignDocument, e: Value) -> Value {
    doc.player("event", &e).unwrap()
}

/// `playerRender` as RGBA: the picture's width and height (its 8-byte header) and its pixels.
fn rgba(doc: &mut DesignDocument, params: Value) -> (usize, usize, Vec<u8>) {
    let mut p = params;
    p["format"] = json!("rgba");
    let bytes = doc.player_render(&p).unwrap();
    let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
    let (w, h) = (word(0), word(4));
    assert_eq!(bytes.len(), 8 + w * h * 4, "the header gives the picture's size");
    (w, h, bytes[8..].to_vec())
}

fn rgba_at(doc: &mut DesignDocument, params: Value, x: usize, y: usize) -> [u8; 4] {
    let (w, _, pixels) = rgba(doc, params);
    let i = (y * w + x) * 4;
    [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
}

/// Review #18: an RGBA picture says its own size (the engine's rounding, not the editor's guess):
/// at a scale where `screen × scale` lands a hair past a whole number, it is that whole number.
#[test]
fn an_rgba_picture_carries_its_size() {
    let (mut doc, a, _, _) = design();
    start(&mut doc, &a);
    let scale = 101.000_000_1 / 400.0;
    let (w, h, _) = rgba(&mut doc, json!({ "scale": scale }));
    assert_eq!((w, h), (101, 76), "400 x 300 at {scale}");
    assert_eq!(rgba(&mut doc, json!({ "scale": 2 })).0, 800);
}

#[test]
fn a_click_navigates_back_returns_and_the_document_never_changes() {
    let (mut doc, a, button, b) = design();
    doc.execute(
        "proto.interaction.add",
        json!({ "id": button, "dest": b, "transition": { "type": "push", "direction": "left" }, "durationMs": 400, "easing": "linear" }),
    )
    .unwrap();
    doc.mark_saved();
    let revision = doc.revision();
    let answer = start(&mut doc, &a);
    let state = &answer["state"];
    assert_eq!(state["current"], a);
    assert_eq!(state["screen"], json!({ "width": 400.0, "height": 300.0 }));
    assert_eq!(state["frame"], json!({ "x": 0.0, "y": 0.0, "width": 400.0, "height": 300.0 }));
    let hot = &state["hotspots"][0];
    assert_eq!(hot["id"], button);
    assert_eq!((hot["x"].as_f64(), hot["y"].as_f64(), hot["width"].as_f64()), (Some(20.0), Some(20.0), Some(120.0)));
    assert_eq!(hot["triggers"], json!(["onClick"]));

    let hover = event(&mut doc, json!({ "type": "pointerMove", "x": 50, "y": 50 }));
    assert_eq!(hover["cursor"], "pointer");
    let clicked = event(&mut doc, json!({ "type": "click", "x": 50, "y": 50 }));
    assert_eq!(clicked["handled"], true);
    assert_eq!(clicked["changed"], true);
    assert_eq!(clicked["state"]["current"], b);
    assert_eq!(clicked["state"]["history"], json!([a]));
    let t = &clicked["transition"];
    assert_eq!((t["kind"].as_str(), &t["from"], &t["to"]), (Some("navigate"), &a, &b));
    assert_eq!(t["transition"], json!({ "type": "push", "direction": "left" }));
    assert_eq!(t["durationMs"], 400.0);
    assert_eq!(t["easing"], "linear");

    let back = doc.player("back", &json!({})).unwrap();
    assert_eq!(back["state"]["current"], a);
    assert_eq!(back["transition"]["kind"], "back");
    assert_eq!(back["transition"]["transition"], json!({ "type": "push", "direction": "right" }), "back plays it the other way");

    let missed = event(&mut doc, json!({ "type": "click", "x": 300, "y": 250 }));
    assert_eq!((missed["handled"].as_bool(), missed["missed"].as_bool()), (Some(false), Some(true)));
    assert_eq!(doc.revision(), revision, "playing never moves the revision");
    assert!(!doc.dirty());
    assert_eq!(doc.player("stop", &json!({})).unwrap()["stopped"], true);
    assert!(doc.player("state", &json!({})).unwrap_err().starts_with("not-playing"));
}

#[test]
fn render_draws_the_screen_and_a_transition_at_any_point() {
    let (mut doc, a, button, b) = design();
    doc.execute(
        "proto.interaction.add",
        json!({ "id": button, "dest": b, "transition": { "type": "dissolve" }, "durationMs": 300, "easing": "linear" }),
    )
    .unwrap();
    start(&mut doc, &a);
    let png = doc.player_render(&json!({ "scale": 0.5 })).unwrap();
    let image = image::load_from_memory(&png).unwrap();
    assert_eq!((image.width(), image.height()), (200, 150));
    assert_eq!(rgba_at(&mut doc, json!({}), 50, 50), [255, 0, 0, 255], "the red button");
    assert!(doc.player_render(&json!({ "state": "from" })).is_err(), "no transition yet");
    event(&mut doc, json!({ "type": "click", "x": 50, "y": 50 }));
    assert_eq!(rgba_at(&mut doc, json!({}), 50, 50), [0, 0, 255, 255], "B's blue panel");
    assert_eq!(rgba_at(&mut doc, json!({ "state": "from" }), 50, 50), [255, 0, 0, 255], "what showed before");
    let mid = rgba_at(&mut doc, json!({ "progress": 0.5 }), 50, 50);
    assert!(mid[0] > 90 && mid[0] < 170 && mid[2] > 90 && mid[2] < 170, "half dissolved: {mid:?}");
    assert_eq!(rgba_at(&mut doc, json!({ "progress": 0.0 }), 50, 50), [255, 0, 0, 255]);
    assert!(doc.player_render(&json!({ "scale": 100 })).is_ok(), "scale is clamped to 8");
    assert!(doc.player_render(&json!({ "format": "gif" })).unwrap_err().starts_with("invalid-params"));
}

#[test]
fn after_delay_fires_on_the_editors_clock() {
    let (mut doc, a, _, b) = design();
    doc.execute(
        "proto.interaction.add",
        json!({ "id": a, "trigger": { "type": "afterDelay", "ms": 500 }, "action": { "type": "navigate", "dest": b } }),
    )
    .unwrap();
    let started = start(&mut doc, &a);
    assert_eq!(started["state"]["nextDue"], 0.5);
    let early = doc.player("tick", &json!({ "time": 0.2 })).unwrap();
    assert_eq!(early["state"]["current"], a);
    assert!((early["state"]["nextDue"].as_f64().unwrap() - 0.3).abs() < 1e-9);
    let due = doc.player("tick", &json!({ "time": 0.6 })).unwrap();
    assert_eq!(due["state"]["current"], b);
    assert_eq!(due["transition"]["kind"], "navigate");
}

#[test]
fn overlays_open_centred_and_close_on_a_click_outside() {
    let (mut doc, a, button, _) = design();
    let menu = doc.execute("frame.create", json!({ "x": 0, "y": 600, "width": 100, "height": 50 })).unwrap()["id"].clone();
    doc.execute(
        "proto.interaction.add",
        json!({ "id": button, "action": { "type": "openOverlay", "dest": menu, "background": "#00000080" }, "transition": { "type": "moveIn", "direction": "top" } }),
    )
    .unwrap();
    start(&mut doc, &a);
    let opened = event(&mut doc, json!({ "type": "click", "x": 50, "y": 50 }));
    let o = &opened["state"]["overlays"][0];
    assert_eq!(o["id"], menu);
    assert_eq!((o["x"].as_f64(), o["y"].as_f64(), o["width"].as_f64()), (Some(150.0), Some(125.0), Some(100.0)));
    assert_eq!(o["closeOnOutside"], true);
    assert_eq!(opened["transition"]["kind"], "overlayOpen");
    assert_eq!(opened["transition"]["overlay"], menu);
    assert_eq!(opened["state"]["canBack"], true);
    // The background darkens the screen; mid-transition the overlay is still moving in.
    let dark = rgba_at(&mut doc, json!({}), 300, 20);
    assert!(dark[0] < 160, "the overlay's background is over the screen: {dark:?}");
    assert!(doc.player_render(&json!({ "progress": 0.3 })).is_ok());
    let closed = event(&mut doc, json!({ "type": "click", "x": 10, "y": 290 }));
    assert_eq!(closed["state"]["overlays"], json!([]));
    assert_eq!(closed["transition"]["kind"], "overlayClose");
    assert!(doc.player_render(&json!({ "progress": 0.5 })).is_ok(), "closing animates too");
}

#[test]
fn set_variable_and_conditionals_run_on_the_players_copy() {
    let (mut doc, a, button, b) = design();
    doc.execute("variable.createCollection", json!({ "name": "State" })).unwrap();
    doc.execute("variable.create", json!({ "collection": "State", "name": "count", "type": "number", "value": 0 })).unwrap();
    doc.execute(
        "proto.interaction.add",
        json!({ "id": button, "actions": [
            { "type": "setVariable", "variable": "count", "expression": "count + 1" },
            { "type": "conditional", "blocks": [{ "expression": "count >= 2", "actions": [{ "type": "navigate", "dest": b }] }] },
        ] }),
    )
    .unwrap();
    let variables = doc.query("variables", &json!({})).unwrap();
    start(&mut doc, &a);
    let first = event(&mut doc, json!({ "type": "click", "x": 50, "y": 50 }));
    assert_eq!(first["state"]["current"], a, "count is 1");
    let second = event(&mut doc, json!({ "type": "click", "x": 50, "y": 50 }));
    assert_eq!(second["state"]["current"], b, "count is 2");
    assert_eq!(doc.query("variables", &json!({})).unwrap(), variables, "the file's variable is untouched");
    let again = doc.player("restart", &json!({})).unwrap();
    assert_eq!(again["state"]["current"], a);
    assert_eq!(event(&mut doc, json!({ "type": "click", "x": 50, "y": 50 }))["state"]["current"], a, "restart starts the variables over");
}

#[test]
fn hover_reverts_keys_fire_and_r_restarts() {
    let (mut doc, a, button, b) = design();
    doc.execute("proto.interaction.add", json!({ "id": button, "trigger": { "type": "onHover" }, "action": { "type": "navigate", "dest": b } }))
        .unwrap();
    start(&mut doc, &a);
    let over = event(&mut doc, json!({ "type": "pointerMove", "x": 60, "y": 60 }));
    assert_eq!(over["state"]["current"], b);
    let out = event(&mut doc, json!({ "type": "pointerMove", "x": 390, "y": 290 }));
    assert_eq!(out["state"]["current"], a, "leaving the hotspot reverts");
    assert_eq!(out["transition"]["kind"], "revert");

    let (mut doc, a, _, b) = design();
    doc.execute(
        "proto.interaction.add",
        json!({ "id": a, "trigger": { "type": "keyPress", "key": "Shift+K" }, "action": { "type": "navigate", "dest": b } }),
    )
    .unwrap();
    start(&mut doc, &a);
    assert_eq!(event(&mut doc, json!({ "type": "key", "key": "k" }))["handled"], false, "without Shift");
    let keyed = event(&mut doc, json!({ "type": "key", "key": "K", "shift": true }));
    assert_eq!((keyed["handled"].as_bool(), &keyed["state"]["current"]), (Some(true), &b));
    let restarted = event(&mut doc, json!({ "type": "key", "key": "r" }));
    assert_eq!(restarted["state"]["current"], a);
    assert_eq!(restarted["transition"]["kind"], "restart");
    let stepped = event(&mut doc, json!({ "type": "key", "key": "ArrowRight" }));
    assert_eq!(stepped["state"]["current"], b, "the arrows step through the frames");
}

#[test]
fn an_overflowing_frame_scrolls_with_the_wheel_and_scroll_to() {
    let (mut doc, a, _, _) = design();
    let low = doc.execute("shape.rectangle", json!({ "x": 0, "y": 700, "width": 100, "height": 100, "parent": a })).unwrap()["id"].clone();
    doc.execute("node.set", json!({ "ids": [a], "props": { "overflow": "vertical", "clip": true } })).unwrap();
    let jump = doc.execute("shape.rectangle", json!({ "x": 300, "y": 20, "width": 50, "height": 50, "parent": a })).unwrap()["id"].clone();
    doc.execute(
        "proto.interaction.add",
        json!({ "id": jump, "action": { "type": "scrollTo", "dest": low }, "transition": { "type": "smartAnimate" } }),
    )
    .unwrap();
    start(&mut doc, &a);
    let state = doc.player("state", &json!({})).unwrap()["state"].clone();
    assert_eq!(state["scrollRange"], json!([0.0, 500.0]));
    let wheeled = event(&mut doc, json!({ "type": "wheel", "x": 200, "y": 150, "dx": 0, "dy": 120 }));
    assert_eq!(wheeled["state"]["scroll"], json!([0.0, 120.0]));
    let top = doc.player("scrollTo", &json!({ "x": 0, "y": 0 })).unwrap();
    assert_eq!(top["state"]["scroll"], json!([0.0, 0.0]));
    let scrolled = event(&mut doc, json!({ "type": "click", "x": 320, "y": 40 }));
    assert_eq!(scrolled["state"]["scroll"], json!([0.0, 500.0]), "as far as it scrolls");
    assert_eq!(scrolled["transition"]["kind"], "scroll");
    assert!(doc.player_render(&json!({ "progress": 0.5 })).is_ok());
    // A drag scrolls too.
    doc.player("scrollTo", &json!({ "x": 0, "y": 0 })).unwrap();
    event(&mut doc, json!({ "type": "pointerDown", "x": 200, "y": 200 }));
    let dragged = event(&mut doc, json!({ "type": "pointerMove", "x": 200, "y": 100 }));
    assert_eq!(dragged["state"]["scroll"], json!([0.0, 100.0]));
    assert_eq!(dragged["cursor"], "grabbing");
    event(&mut doc, json!({ "type": "pointerUp", "x": 200, "y": 100 }));
}

#[test]
fn drags_fire_on_drag_with_a_follow_vector() {
    let (mut doc, a, button, b) = design();
    doc.execute(
        "proto.interaction.add",
        json!({ "id": button, "trigger": { "type": "onDrag" }, "action": { "type": "navigate", "dest": b }, "transition": { "type": "push", "direction": "left" } }),
    )
    .unwrap();
    start(&mut doc, &a);
    event(&mut doc, json!({ "type": "pointerDown", "x": 100, "y": 50 }));
    let right = event(&mut doc, json!({ "type": "pointerMove", "x": 130, "y": 50 }));
    assert_eq!(right["state"]["current"], a, "a push left only starts on a drag to the left");
    event(&mut doc, json!({ "type": "pointerUp", "x": 130, "y": 50 }));
    event(&mut doc, json!({ "type": "pointerDown", "x": 100, "y": 50 }));
    let left = event(&mut doc, json!({ "type": "pointerMove", "x": 70, "y": 52 }));
    assert_eq!(left["state"]["current"], b);
    assert_eq!(left["transition"]["follow"], json!({ "x": -1.0, "y": 0.0, "length": 400.0 }));
}

#[test]
fn the_player_follows_edits_and_devices_and_checks_its_params() {
    let (mut doc, a, button, b) = design();
    doc.execute("proto.interaction.add", json!({ "id": button, "dest": b })).unwrap();
    doc.execute("proto.flow.add", json!({ "start": a, "name": "Onboarding" })).unwrap();
    let started = doc.player("start", &json!({})).unwrap();
    assert_eq!(started["state"]["current"], a, "no start: the page's first flow");
    assert_eq!(started["state"]["flow"]["name"], "Onboarding");
    doc.execute("node.set", json!({ "ids": [a], "props": { "name": "Home" } })).unwrap();
    assert_eq!(doc.player("state", &json!({})).unwrap()["state"]["name"], "Home", "edits show live");
    doc.execute("proto.settings", json!({ "device": "Custom size", "width": 500, "height": 400 })).unwrap();
    let state = doc.player("state", &json!({})).unwrap()["state"].clone();
    assert_eq!(state["screen"], json!({ "width": 500.0, "height": 400.0 }));
    assert_eq!(state["frame"]["x"], 50.0, "a smaller frame sits centred on the device");
    assert_eq!(state["device"]["name"], "Custom size");
    let clicked = event(&mut doc, json!({ "type": "click", "x": 100, "y": 100 }));
    assert_eq!(clicked["state"]["current"], b, "pointer positions are screen units");
    for (command, params) in [
        ("event", json!({})),
        ("event", json!({ "type": "fling" })),
        ("event", json!({ "type": "click" })),
        ("resize", json!({ "width": 0 })),
        ("dance", json!({})),
    ] {
        assert!(doc.player(command, &params).unwrap_err().starts_with("invalid-params"), "{command} {params}");
    }
    assert!(doc.player("start", &json!({ "start": 999_999 })).is_err());
    assert!(doc.player("state", &json!([1])).is_err());
}

#[test]
fn smart_animate_tweens_matching_layers() {
    let mut doc = DesignDocument::create("Smart.kdesign", 400, 300).unwrap();
    let a = doc.query("layers", &json!({})).unwrap()["rows"][0]["id"].clone();
    let b = doc.execute("frame.create", json!({ "x": 600, "y": 0, "width": 400, "height": 300 })).unwrap()["id"].clone();
    for (frame, x) in [(&a, 0), (&b, 300)] {
        let r = doc.execute("shape.rectangle", json!({ "x": x, "y": 100, "width": 100, "height": 100, "parent": frame })).unwrap()["id"].clone();
        doc.execute("fill.set", json!({ "ids": [r], "color": "#00ff00" })).unwrap();
        doc.execute("node.set", json!({ "ids": [r], "props": { "name": "Box" } })).unwrap();
    }
    doc.execute(
        "proto.interaction.add",
        json!({ "id": a, "trigger": { "type": "keyPress", "key": "Space" }, "action": { "type": "navigate", "dest": b }, "transition": { "type": "smartAnimate" }, "easing": "linear" }),
    )
    .unwrap();
    start(&mut doc, &a);
    let t = event(&mut doc, json!({ "type": "key", "key": " " }));
    assert_eq!(t["transition"]["smartAnimate"], true);
    let green = |p: [u8; 4]| p[1] > 200 && p[0] < 60;
    assert!(green(rgba_at(&mut doc, json!({ "progress": 0.5 }), 200, 150)), "half way the box is in the middle");
    assert!(!green(rgba_at(&mut doc, json!({ "progress": 0.5 }), 20, 150)), "and left where it was");
    assert!(green(rgba_at(&mut doc, json!({}), 350, 150)), "at the end it is where B has it");
}

#[test]
fn junk_events_never_panic() {
    let (mut doc, a, button, b) = design();
    doc.execute("proto.interaction.add", json!({ "id": button, "dest": b })).unwrap();
    start(&mut doc, &a);
    let junk = [json!(null), json!(1), json!("x"), json!(-1e308), json!(f64::MAX), json!([]), json!({})];
    for kind in ["pointerMove", "pointerDown", "pointerUp", "pointerLeave", "pointerCancel", "click", "wheel", "key"] {
        for v in &junk {
            let _ = doc.player("event", &json!({ "type": kind, "x": v, "y": v, "dx": v, "dy": v, "key": v, "time": v }));
        }
    }
    for command in ["state", "tick", "restart", "back", "closeOverlay", "scrollTo", "resize"] {
        for v in &junk {
            let _ = doc.player(command, &json!({ "x": v, "y": v, "width": v, "height": v, "start": v, "time": v }));
            let _ = doc.player_render(&json!({ "scale": v, "progress": v, "state": v, "format": v }));
        }
    }
    assert!(doc.player("state", &json!({})).is_ok());
}

#[test]
fn inspect_reads_the_layer_under_the_pointer_from_the_players_copy() {
    let (mut doc, a, button, _) = design();
    doc.execute("variable.createCollection", json!({ "name": "Size" })).unwrap();
    doc.execute("variable.create", json!({ "collection": "Size", "name": "w", "type": "number", "value": 120 })).unwrap();
    doc.execute("variable.bind", json!({ "ids": [button], "field": "width", "variable": "w" })).unwrap();
    doc.execute("proto.interaction.add", json!({ "id": button, "actions": [{ "type": "setVariable", "variable": "w", "expression": "200" }] }))
        .unwrap();
    let width = |sections: &Value| -> String {
        let rows = sections.as_array().unwrap().iter().flat_map(|s| s["rows"].as_array().unwrap().clone());
        rows.into_iter().find(|r| r["label"] == "Width").map(|r| r["value"].as_str().unwrap().to_owned()).unwrap_or_default()
    };
    start(&mut doc, &a);
    let hit = doc.player("inspect", &json!({ "x": 50, "y": 50 })).unwrap();
    assert_eq!(hit["id"], button, "{hit}");
    assert_eq!(hit["bounds"]["x"], 20.0);
    assert_eq!(width(&hit["sections"]), "120");
    let frame = doc.player("inspect", &json!({ "x": 300, "y": 250 })).unwrap();
    assert_eq!(frame["id"], a, "outside the button: the frame");
    // The prototype sets the width on its copy: Inspect reads the copy, the file keeps 120.
    event(&mut doc, json!({ "type": "click", "x": 50, "y": 50 }));
    let after = doc.player("inspect", &json!({ "id": button })).unwrap();
    assert_eq!(width(&after["sections"]), "200", "{after}");
    assert_eq!(after["bounds"]["width"], 200.0);
    let file = doc.query("devInspect", &json!({ "id": button })).unwrap();
    assert_eq!(width(&file["sections"]), "120");
    // Off the screen nothing is hit and an id elsewhere is not on it.
    assert_eq!(doc.player("inspect", &json!({ "x": -5, "y": -5 })).unwrap()["id"], Value::Null);
    assert!(doc.player("inspect", &json!({ "id": button, "colorFormat": "cmyk" })).is_err());
}

/// Review #23: when an edit deletes the start, the design's default start becomes the start;
/// when it deletes both the frame playing and the start, the player starts again from the default
/// start, with nothing of the old screen left; with no frame left at all it stops.
#[test]
fn the_player_recovers_when_its_frames_are_deleted() {
    let (mut doc, a, button, b) = design();
    let c = doc.execute("frame.create", json!({ "x": 1200, "y": 0, "width": 400, "height": 300 })).unwrap()["id"].clone();
    doc.execute("proto.interaction.add", json!({ "id": button, "dest": b })).unwrap();
    start(&mut doc, &a);
    assert_eq!(event(&mut doc, json!({ "type": "click", "x": 50, "y": 50 }))["state"]["current"], b);
    doc.execute("edit.delete", json!({ "ids": [a] })).unwrap();
    let state = doc.player("state", &json!({})).unwrap()["state"].clone();
    assert_eq!(state["current"], b, "still on the frame playing");
    assert_eq!(state["start"], b, "the design's default start: its first frame");
    let restarted = doc.player("restart", &json!({})).unwrap()["state"].clone();
    assert_eq!(restarted["current"], b);
    doc.execute("edit.delete", json!({ "ids": [b] })).unwrap();
    let state = doc.player("state", &json!({})).unwrap()["state"].clone();
    assert_eq!(state["current"], c, "the default start: the frame left");
    assert_eq!(state["start"], c);
    assert_eq!(state["history"], json!([]));
    assert_eq!(state["overlays"], json!([]));
    assert!(doc.player_render(&json!({})).is_ok(), "it draws again");
    doc.execute("edit.delete", json!({ "ids": [c] })).unwrap();
    assert!(doc.player("state", &json!({})).unwrap_err().starts_with("not-playing"));
    assert!(doc.player_render(&json!({})).unwrap_err().starts_with("not-playing"));
}

/// Review #22: a picture inside a transition is drawn within a quarter of the player's pixel
/// budget (a transition holds several screens at once); its last frame is drawn at full size.
#[test]
fn transition_pictures_keep_within_their_budget() {
    let mut doc = DesignDocument::create("Proto.kdesign", 4000, 4000).unwrap();
    let a = doc.query("layers", &json!({})).unwrap()["rows"][0]["id"].clone();
    let button = doc.execute("shape.rectangle", json!({ "x": 0, "y": 0, "width": 400, "height": 400, "parent": a })).unwrap()["id"].clone();
    let b = doc.execute("frame.create", json!({ "x": 5000, "y": 0, "width": 4000, "height": 4000 })).unwrap()["id"].clone();
    doc.execute(
        "proto.interaction.add",
        json!({ "id": button, "dest": b, "transition": { "type": "push", "direction": "left" }, "durationMs": 400, "easing": "linear" }),
    )
    .unwrap();
    start(&mut doc, &a);
    event(&mut doc, json!({ "type": "click", "x": 50, "y": 50 }));
    let (w, h, _) = rgba(&mut doc, json!({ "scale": 1.4, "progress": 0.5 }));
    assert!(w * h <= 8 * 1024 * 1024, "{w} x {h} inside the transition");
    assert!(w * h > 7 * 1024 * 1024, "{w} x {h}: lowered only as far as it has to be");
    assert_eq!(w, h);
    let (w, h, _) = rgba(&mut doc, json!({ "scale": 1.4, "progress": 1 }));
    assert_eq!((w, h), (5600, 5600), "the last frame is full size");
}

/// Limit closed: a press the browser takes away (`pointerCancel`) ends the press and the drag
/// without a click, a mouse up or a missed-click hint, and reverts a While pressing state.
#[test]
fn a_cancelled_press_fires_nothing_and_reverts_while_pressing() {
    let (mut doc, a, button, b) = design();
    doc.execute("proto.interaction.add", json!({ "id": button, "dest": b })).unwrap();
    start(&mut doc, &a);
    let idle = event(&mut doc, json!({ "type": "pointerCancel" }));
    assert_eq!((idle["handled"].as_bool(), idle["missed"].as_bool()), (Some(false), Some(false)), "nothing pressed");
    event(&mut doc, json!({ "type": "pointerDown", "x": 60, "y": 60 }));
    assert_eq!(doc.player("state", &json!({})).unwrap()["state"]["pressed"], true);
    let cancelled = event(&mut doc, json!({ "type": "pointerCancel" }));
    assert_eq!(cancelled["handled"], true, "the press ended");
    assert_eq!(cancelled["missed"], false, "no hotspot hints");
    assert_eq!((&cancelled["state"]["pressed"], &cancelled["state"]["current"]), (&json!(false), &a));
    let up = event(&mut doc, json!({ "type": "pointerUp", "x": 60, "y": 60 }));
    assert_eq!((&up["state"]["current"], up["missed"].as_bool()), (&a, Some(false)), "the late release clicks nothing");

    // While pressing: the cancel reverts it.
    let (mut doc, a, button, b) = design();
    doc.execute("proto.interaction.add", json!({ "id": button, "trigger": { "type": "onPress" }, "action": { "type": "navigate", "dest": b } }))
        .unwrap();
    start(&mut doc, &a);
    assert_eq!(event(&mut doc, json!({ "type": "pointerDown", "x": 60, "y": 60 }))["state"]["current"], b);
    let cancelled = event(&mut doc, json!({ "type": "pointerCancel" }));
    assert_eq!(cancelled["state"]["current"], a, "the press state reverts");
    assert_eq!(cancelled["transition"]["kind"], "revert");

    // Mouse up: not fired by a cancel.
    let (mut doc, a, button, b) = design();
    doc.execute("proto.interaction.add", json!({ "id": button, "trigger": { "type": "mouseUp" }, "action": { "type": "navigate", "dest": b } }))
        .unwrap();
    start(&mut doc, &a);
    event(&mut doc, json!({ "type": "pointerDown", "x": 60, "y": 60 }));
    assert_eq!(event(&mut doc, json!({ "type": "pointerCancel" }))["state"]["current"], a);
    assert_eq!(event(&mut doc, json!({ "type": "pointerUp", "x": 60, "y": 60 }))["state"]["current"], a);

    // A drag that scrolls ends: the cursor lets go and the scroll stays where the drag left it.
    let (mut doc, a, _, _) = design();
    doc.execute("shape.rectangle", json!({ "x": 0, "y": 700, "width": 100, "height": 100, "parent": a })).unwrap();
    doc.execute("node.set", json!({ "ids": [a], "props": { "overflow": "vertical", "clip": true } })).unwrap();
    start(&mut doc, &a);
    event(&mut doc, json!({ "type": "pointerDown", "x": 200, "y": 200 }));
    assert_eq!(event(&mut doc, json!({ "type": "pointerMove", "x": 200, "y": 100 }))["cursor"], "grabbing");
    let cancelled = event(&mut doc, json!({ "type": "pointerCancel" }));
    assert_ne!(cancelled["cursor"], "grabbing");
    assert_eq!(cancelled["state"]["scroll"], json!([0.0, 100.0]));
    let moved = event(&mut doc, json!({ "type": "pointerMove", "x": 200, "y": 0 }));
    assert_eq!(moved["state"]["scroll"], json!([0.0, 100.0]), "moving after the cancel drags nothing");
}
