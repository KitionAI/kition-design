//! Session data read by a call is declared and restored with its call context.
#![allow(clippy::unwrap_used, clippy::panic)]

use kition_design_kit::DesignDocument;
use kition_kit::{EditorState, KitDocument};
use serde_json::json;

#[test]
fn restoring_the_editor_state_restores_the_clipboard() {
    let mut doc = DesignDocument::create("Context.kdesign", 320, 240).unwrap();
    let frame = doc.inspect().unwrap()["tree"][0]["id"].clone();
    doc.execute("select.set", json!({"ids":[frame]})).unwrap();
    doc.execute("edit.copy", json!({})).unwrap();
    let saved = doc.editor_state();
    let before = format!("{:?}", doc.session().clipboard);
    doc.execute("node.set", json!({"id":frame,"props":{"name":"Other clipboard"}})).unwrap();
    doc.execute("edit.copy", json!({})).unwrap();
    assert_ne!(format!("{:?}", doc.session().clipboard), before);
    doc.restore_editor_state(&saved).unwrap();
    assert_eq!(format!("{:?}", doc.session().clipboard), before);
}

#[test]
fn rotation_origin_and_clipboard_readers_declare_mode() {
    let doc = DesignDocument::create("Context.kdesign", 320, 240).unwrap();
    let commands = doc.commands();
    for id in ["node.set", "transform.rotate", "edit.paste", "fill.paste", "effect.paste"] {
        let command = commands.iter().find(|command| command.id == id).unwrap();
        // node.set rotates its own transform directly; only transform.rotate uses the saved
        // rotation origin. The original source scan reached another helper with the same name.
        assert_eq!(command.reads.contains(&EditorState::Mode), id != "node.set", "{id}: {:?}", command.reads);
    }
}
