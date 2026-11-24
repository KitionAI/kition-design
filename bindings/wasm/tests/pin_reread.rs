//! A clean design keeps its editor state and both undo histories when another program saves its
//! file (kition-sdk 2ce294a1, "Session Continuity On Reread"), and a planning copy restores the
//! editor state it was made with.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;
use kition_kit::kdm::Origin;
use serde_json::{Value, json};

const FILE: &str = "Reread.kdesign";

fn rectangle(doc: &mut DesignDocument, name: &str, x: f64) -> Value {
    let frame = doc.inspect().unwrap()["tree"][0]["id"].clone();
    doc.execute("node.create", json!({"type":"rectangle","parent":frame,"name":name,"x":x,"y":20,"width":40,"height":40})).unwrap()["id"].clone()
}

/// `live`'s file as another program saved it after `edit`, read again.
fn saved_elsewhere(live: &mut DesignDocument, edit: impl FnOnce(&mut DesignDocument)) -> DesignDocument {
    let mut writer = DesignDocument::open(FILE, &live.seal_and_save(None).unwrap()).unwrap();
    edit(&mut writer);
    DesignDocument::open(FILE, &writer.seal_and_save(None).unwrap()).unwrap()
}

fn x_of(doc: &DesignDocument, id: &Value) -> f64 {
    doc.session().doc().unwrap().node(kition_design_doc::NodeId(id.as_u64().unwrap())).unwrap().transform.translation().x
}

#[test]
fn the_file_read_again_restores_the_previous_selection_and_page() {
    let mut live = DesignDocument::create(FILE, 800, 600).unwrap();
    let a = rectangle(&mut live, "A", 20.0);
    live.execute("select.set", json!({"ids":[a]})).unwrap();
    let saved = live.editor_state();
    let mut fresh = saved_elsewhere(&mut live, |_| {});
    fresh.carry_replica_objects(&live).unwrap();
    fresh.restore_editor_state(&saved).unwrap();
    assert_eq!(fresh.inspect().unwrap()["selection"], json!([a]));
    assert_eq!(fresh.inspect().unwrap()["page"], live.inspect().unwrap()["page"]);
}

#[test]
fn the_file_read_again_refuses_a_deleted_active_target_and_changes_nothing() {
    let mut live = DesignDocument::create(FILE, 800, 600).unwrap();
    let a = rectangle(&mut live, "A", 20.0);
    live.execute("select.set", json!({"ids":[a]})).unwrap();
    let saved = live.editor_state();
    let mut fresh = saved_elsewhere(&mut live, |writer| {
        writer.execute("edit.delete", json!({"ids":[a]})).unwrap();
    });
    fresh.carry_replica_objects(&live).unwrap();
    let before = fresh.inspect().unwrap();
    let error = fresh.restore_editor_state(&saved).unwrap_err();
    assert!(error.contains("deleted active target"), "{error}");
    assert_eq!(fresh.inspect().unwrap()["selection"], before["selection"]);
    // The live design keeps the state, and reconciling with that file is refused untouched.
    let kept = live.seal_and_save(None).unwrap();
    let error = live.reconcile_reread(&fresh).unwrap_err();
    assert!(error.contains("deleted active target"), "{error}");
    assert_eq!(live.seal_and_save(None).unwrap(), kept);
    assert_eq!(live.inspect().unwrap()["selection"], json!([a]));
}

#[test]
fn an_outside_save_keeps_the_agents_undo_and_the_colleagues_edit() {
    let mut live = DesignDocument::create(FILE, 800, 600).unwrap();
    let a = rectangle(&mut live, "A", 20.0);
    live.execute_as("transform.move", json!({"ids":[a],"dx":30,"dy":0}), &Origin::agent("agent:me")).unwrap();
    live.mark_saved();
    assert_eq!(x_of(&live, &a), 50.0);
    let mut b = Value::Null;
    let fresh = saved_elsewhere(&mut live, |writer| b = rectangle(writer, "B", 200.0));
    assert!(live.reconcile_reread(&fresh).unwrap());
    assert!(!live.dirty(), "a reconciled design is the file as saved");
    assert_eq!(x_of(&live, &b), 200.0, "the colleague's layer is in");
    assert_eq!(live.inspect().unwrap()["canUndo"], true, "the undo history stays");
    let undone = live.execute_as("edit.undo", json!({}), &Origin::agent("agent:me")).unwrap();
    assert_eq!(undone["done"], true, "{undone}");
    assert_eq!(x_of(&live, &a), 20.0, "the agent's own move is undone");
    assert_eq!(x_of(&live, &b), 200.0, "the colleague's layer stays");
}

#[test]
fn a_planning_copy_restores_the_editor_state_it_was_made_with() {
    let mut live = DesignDocument::create(FILE, 800, 600).unwrap();
    let a = rectangle(&mut live, "A", 20.0);
    live.execute("select.set", json!({"ids":[a]})).unwrap();
    live.version().unwrap();
    let saved = live.editor_state();
    let mut copy = live.fork_for_premises().unwrap();
    copy.restore_editor_state(&saved).unwrap();
    assert_eq!(copy.inspect().unwrap()["selection"], json!([a]));
    assert_eq!(copy.doc_id(), live.doc_id(), "the copy keeps the replica's identity");
    copy.execute("edit.delete", json!({"ids":[a]})).unwrap();
    assert_eq!(x_of(&live, &a), 20.0, "the copy is isolated");
}
