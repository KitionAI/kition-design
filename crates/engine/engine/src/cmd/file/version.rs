//! Saved versions: the host keeps them (Kit UI protocol section 7m, checkpoints), never the engine
//! or the file. A saved version is a checkpoint that carries its own snapshot of the design's
//! replica; the engine reads the design it holds ([`content_at`]) to show it (`version.preview`),
//! restore it (`version.restore`) or compare with it (`dev.compare`).

use kition_design_kdm::kition_kit::kdm::{ActorId, Replica};

use super::*;

/// The bytes of `snapshot` (a saved version's KDM snapshot, base64, as the host's checkpoint
/// carries it).
fn snapshot_bytes(cmd: &str, snapshot: &str) -> Result<Vec<u8>> {
    base64::engine::general_purpose::STANDARD.decode(snapshot.trim()).map_err(|_| bad(cmd, "`snapshot` is a KDM snapshot in base64"))
}

/// The replica version `snapshot` holds (base64): what names a saved version shown or compared.
pub fn snapshot_version(cmd: &str, snapshot: &str) -> Result<String> {
    let bytes = snapshot_bytes(cmd, snapshot)?;
    let schema = kition_design_kdm::schema().map_err(|e| EngineError::Other(e.to_string()))?;
    let replica = Replica::load(schema, &bytes, ActorId::random()).map_err(|e| bad(cmd, format!("`snapshot` is not a design's snapshot: {e}")))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(replica.version().encode()))
}

/// The design saved version `snapshot` holds (a KDM snapshot or a file's content, base64, as the
/// host's checkpoint carries it), with the pictures the document holds (a host adds the ones a
/// checkpoint needs first, `addBlobs`). Refused when the document has no replica or the snapshot
/// is not this design's (`not-this-document`) or does not load.
pub fn content_at(cmd: &str, ds: &DocState, snapshot: &str) -> Result<Arc<Document>> {
    let bytes = snapshot_bytes(cmd, snapshot)?;
    let kdm = ds.kdm().ok_or_else(|| EngineError::Disabled(cmd.into(), "the design has no history here (no replica)".into()))?;
    let doc = kdm.document_at(&bytes).map_err(|e| EngineError::Disabled(cmd.into(), format!("the saved version does not open here ({e})")))?;
    Ok(Arc::new(doc))
}

/// `version.preview`: shows the design as it was at a saved version, read-only.
pub(super) fn preview_version(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "version.preview";
    let snapshot = req_string(CMD, p, "snapshot")?;
    let id = string(p, "id").unwrap_or_default();
    let name = string(p, "name").unwrap_or_default();
    let time = get(p, "time").and_then(Value::as_u64).unwrap_or(0);
    let ds = s.ds_mut()?;
    let doc = content_at(CMD, ds, &snapshot)?;
    let version = snapshot_version(CMD, &snapshot)?;
    ds.start_preview(&id, &name, time, &version, doc);
    Ok(json!({"id": id, "name": name, "time": time, "version": version}))
}

/// `version.restore`: the design takes the content a saved version holds, as one undoable
/// edit (a new change of the replica, so it reaches everyone editing the design). The file name,
/// what the file published and the comments stay. A host's restore (`restoreState`, Kit UI
/// protocol `replica/restore`) runs this command too, so a restore behaves one way however it
/// is started. Nothing changes (no undo step, `changed: false`) when the design already holds
/// what the restore would give.
pub(super) fn restore_version(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "version.restore";
    // Refused before anything changes.
    if let Some(why) = s.live_read_only(s.ds()?.uid) {
        return Err(EngineError::Disabled(CMD.into(), why.into()));
    }
    let snapshot = req_string(CMD, p, "snapshot")?;
    let version = snapshot_version(CMD, &snapshot)?;
    let ds = s.ds_mut()?;
    let mut doc = content_at(CMD, ds, &snapshot)?;
    let d = Arc::make_mut(&mut doc);
    d.name = ds.doc.name.clone();
    // What the file published belongs to the file, not to the version.
    d.published = ds.doc.published.clone();
    // Comments are a conversation about the file, kept apart from its design history: they stay,
    // and those on layers the version doesn't have stay where they were.
    d.comments = ds.doc.comments.clone();
    crate::cmd::comment::follow_anchors(d, &ds.doc);
    // The content is already consistent: nothing to re-normalise.
    d.take_dirty();
    if kition_design_kdm::same_content(d, &ds.doc) {
        return Ok(json!({"restored": true, "changed": false, "version": version}));
    }
    // Stay on the current page when the version has it.
    ds.page = Some(ds.page)
        .filter(|pg| d.pages.contains(pg))
        .or_else(|| d.current_page.filter(|pg| d.pages.contains(pg)))
        .or_else(|| d.first_page())
        .unwrap_or(ds.page);
    ds.doc = doc;
    ds.selection.clear();
    ds.text_edit = None;
    ds.vector_edit = None;
    Ok(json!({"restored": true, "changed": true, "version": version}))
}

/// The design's replica snapshot now, as the host's checkpoint of a version saved now carries it
/// (base64): what a test passes to `version.preview`, `version.restore` and `dev.compare`. Turns
/// the bridge on, so the replica follows every edit after it.
#[cfg(test)]
pub(crate) fn saved_version(s: &mut Session) -> String {
    s.enable_kdm();
    let snapshot = s.snapshot().unwrap_or_default();
    base64::engine::general_purpose::STANDARD.encode(snapshot)
}
