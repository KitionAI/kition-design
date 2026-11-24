//! Saved versions: the host keeps them (Kit UI protocol section 7m, checkpoints), never the engine
//! or the file. A saved version is one version of the design's replica; the engine only derives
//! the design as it was there ([`content_at`]) to show it (`version.preview`), restore it
//! (`version.restore`) or compare with it (`dev.compare`).

use kition_design_kdm::kition_kit::kdm::Version as ReplicaVersion;

use super::*;

/// The design as it was at replica version `version` (base64, as the host's checkpoints name it),
/// derived from the document's replica, with the pictures it holds (a host adds the ones a
/// checkpoint needs first, `addBlobs`). Refused when the document has no replica or its history
/// does not reach that version (`history-missing`: the file was opened without its stored history).
pub fn content_at(cmd: &str, ds: &DocState, version: &str) -> Result<Arc<Document>> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(version.trim()).map_err(|_| bad(cmd, "`version` is a replica version in base64"))?;
    let version = ReplicaVersion::decode(&bytes).map_err(|e| bad(cmd, format!("`version` is not a replica version: {e}")))?;
    let kdm = ds.kdm().ok_or_else(|| EngineError::Disabled(cmd.into(), "the design has no history here (no replica)".into()))?;
    let doc = kdm
        .document_at(&version)
        .map_err(|e| EngineError::Disabled(cmd.into(), format!("history-missing: the design's history here does not reach that version ({e})")))?;
    Ok(Arc::new(doc))
}

/// `version.preview`: shows the design as it was at a saved version, read-only.
pub(super) fn preview_version(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "version.preview";
    let version = req_string(CMD, p, "version")?;
    let id = string(p, "id").unwrap_or_default();
    let name = string(p, "name").unwrap_or_default();
    let time = get(p, "time").and_then(Value::as_u64).unwrap_or(0);
    let ds = s.ds_mut()?;
    let doc = content_at(CMD, ds, &version)?;
    ds.start_preview(&id, &name, time, &version, doc);
    Ok(json!({"id": id, "name": name, "time": time, "version": version}))
}

/// `version.restore`: the design takes the content it had at a saved version, as one undoable
/// edit (a new change of the replica, so it reaches everyone editing the design). The file name,
/// what the file published and the comments stay.
pub(super) fn restore_version(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "version.restore";
    // Refused before anything changes.
    if let Some(why) = s.live_read_only(s.ds()?.uid) {
        return Err(EngineError::Disabled(CMD.into(), why.into()));
    }
    let version = req_string(CMD, p, "version")?;
    let ds = s.ds_mut()?;
    let mut doc = content_at(CMD, ds, &version)?;
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
    Ok(json!({"restored": true, "version": version}))
}

/// The design's replica version now, as the host's checkpoint of a version saved now names it
/// (base64): what a test passes to `version.preview`, `version.restore` and `dev.compare`. Turns
/// the bridge on, so the replica follows every edit after it.
#[cfg(test)]
pub(crate) fn saved_version(s: &mut Session) -> String {
    s.enable_kdm();
    let version = s.version().map(|v| v.encode()).unwrap_or_default();
    base64::engine::general_purpose::STANDARD.encode(version)
}
