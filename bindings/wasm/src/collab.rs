//! Kition's collaboration members (engine contract 2; `docs/features/kdm.md`): the design as a KDM
//! replica.
//!
//! The engine keeps its own `Session` (the command bridge to each document's replica,
//! `kition_design_engine::kdm`), not a `kition_kit::session::KitSession`, so
//! `kition_kit::export_kit_session!` does not apply; these members are added by hand with the JS
//! names and shapes of kition-sdk's `session::web`, as Kition Vector and Design do, so Kition's
//! host code is the same for every kit. The wasm bindings are in [`web`].
//!
//! A `.kdesign` file carries its replica's content (`kdm/` entries: `content.json`, the state alone,
//! no history); the history is the replica the host stores (`snapshot`), never the file. The members below still answer `replica/*` so a host can relay changes
//! between people editing the same design. Images are blobs, encoded for transfer as
//! `(64-char ASCII-hex sha256, u64 LE length, bytes) * N`, as every kit does.
//!
//! `contractVersion()` stays 1, as kition-sdk's export macro answers it: Kition and
//! `@kition/sdk/editor` refuse a newer number, and recognise contract 2 by the manifest and these
//! members (platform `kits/engine.md`).

use std::collections::HashMap;
use std::sync::Arc;

use kition_design_engine::Session;
use kition_design_kdm::kition_kit::kdm::{Origin, UndoScope, Version};
use kition_design_kdm::kition_kit::session::{Damage, MemoryBlobs};
use kition_kit::KitDocument;
use serde_json::{Value, json};

use crate::{DesignDocument, engine_error};

fn decode_version(bytes: &[u8]) -> Result<Version, String> {
    if bytes.is_empty() { Ok(Version::empty()) } else { Version::decode(bytes).map_err(engine_error) }
}

impl DesignDocument {
    /// Opens a file with the replica a host stored for it (the Kit UI protocol's `replica`
    /// capability: the document's history). A `.kdesign` of the same document is taken into the
    /// stored replica (`kition_kit::session::take_file`): nothing when the replica holds the file's
    /// state; otherwise its state, as one change keyed by object id, so the stored line of changes
    /// continues. A `.kdesign` of another document opens with its
    /// own content. Any other file opens on the stored replica, with the file recorded on top as
    /// one `import` transaction when the replica does not hold it; a stored replica that does not
    /// open is ignored and the file opens as [`KitDocument::open`] opens it; a file that does not
    /// open gives way to the stored replica. Empty `bytes` mean there is no file yet, only the
    /// replica (a sync room's snapshot): the design is the replica's alone, and a replica that
    /// does not open is an error.
    pub fn open_with_replica(name: &str, bytes: &[u8], snapshot: &[u8], blobs: &[u8]) -> Result<Self, String> {
        if bytes.is_empty() {
            let mut session = Session::empty();
            session.open_snapshot(snapshot, decode_blobs(blobs)?).map_err(engine_error)?;
            return Self::from_session(session);
        }
        if snapshot.is_empty() {
            return Self::open(name, bytes);
        }
        let mut session = match crate::open_session(name, bytes) {
            Ok(session) => session,
            // A file that does not open (its content is damaged) gives way to the stored replica.
            Err(error) => {
                let mut session = Session::empty();
                return match decode_blobs(blobs).and_then(|blobs| session.open_snapshot(snapshot, blobs).map_err(engine_error)) {
                    Ok(_) => Self::from_session(session),
                    Err(_) => Err(error),
                };
            }
        };
        if session.ds().is_ok_and(kition_design_engine::DocState::has_file_replica) {
            let doc = Self::from_session(session)?;
            if doc.session.ds().is_ok_and(kition_design_engine::DocState::on_file_replica) {
                let stored = kition_design_kdm::schema().ok().and_then(|schema| {
                    kition_design_kdm::kition_kit::kdm::Replica::load(schema, snapshot, kition_design_kdm::kition_kit::kdm::ActorId::random()).ok()
                });
                let file = doc.session.kdm().map(kition_design_kdm::DesignSession::replica);
                let (Some(mut stored), Some(file)) = (stored, file) else { return Ok(doc) };
                if stored.doc_id() != file.doc_id() {
                    // Another document's file: its own content.
                    return Ok(doc);
                }
                kition_design_kdm::kition_kit::session::take_file(&mut stored, file).map_err(|error| format!("damaged: {error}"))?;
                let merged = stored.snapshot().map_err(engine_error)?;
                let mut session = crate::open_session(name, bytes)?;
                session.attach_replica(&merged, decode_blobs(blobs)?).map_err(engine_error)?;
                return Self::from_session(session);
            }
            // The file's replica entries do not open: the stored replica is the design's.
            session = crate::open_session(name, bytes)?;
        }
        let attached = (|| -> Result<Self, String> {
            session.attach_replica(snapshot, decode_blobs(blobs)?).map_err(engine_error)?;
            Self::from_session(session)
        })();
        attached.or_else(|_| Self::open(name, bytes))
    }

    /// Whether the design has its replica (it always does once open, unless the replica refused
    /// it: [`DesignDocument::kdm_resets`]).
    pub fn has_replica(&self) -> bool {
        self.session.kdm().is_some()
    }

    /// The replica's own document id (stable across saves, the same on every replica of the
    /// design), for the Kit UI protocol's `replica/info`.
    pub fn doc_id(&self) -> Option<String> {
        self.session.doc_id()
    }

    pub fn version(&mut self) -> Result<Vec<u8>, String> {
        Ok(self.session.version().map_err(engine_error)?.encode())
    }

    /// The changes since `since` (empty bytes: every change).
    pub fn export_changes(&mut self, since: &[u8]) -> Result<Vec<u8>, String> {
        let since = decode_version(since)?;
        self.session.export_changes(&since).map_err(engine_error)
    }

    /// Imports a peer's (or stored) changes; add their blobs first with
    /// [`DesignDocument::add_blobs`]. Only the replica's repair follows: no auto layout, text fit or
    /// instance propagation runs because of someone else's edit (`docs/features/kdm.md` section 4.1).
    /// An interaction the editor holds open stays open across it, one step of the person's
    /// ([`DesignDocument::aside`]).
    pub fn import_changes(&mut self, bytes: &[u8]) -> Result<Damage, String> {
        self.aside(|doc| doc.session.import_changes(bytes).map_err(engine_error))?;
        Ok(Damage::Full)
    }

    pub fn snapshot(&mut self) -> Result<Vec<u8>, String> {
        self.session.snapshot().map_err(engine_error)
    }

    /// Undoes the person's last step. `done` is `false`, not an error, when there is nothing to
    /// undo.
    pub fn undo(&mut self) -> Result<(bool, Damage), String> {
        self.step(true)
    }

    pub fn redo(&mut self) -> Result<(bool, Damage), String> {
        self.step(false)
    }

    fn step(&mut self, undo: bool) -> Result<(bool, Damage), String> {
        let done = if undo { self.can_undo() } else { self.can_redo() };
        if done {
            self.execute(if undo { "edit.undo" } else { "edit.redo" }, json!({ "scope": "person" }))?;
        }
        Ok((done, if done { Damage::Full } else { Damage::None }))
    }

    /// Whether the person has a step to undo (the agent's steps are not theirs to undo here).
    pub fn can_undo(&self) -> bool {
        self.session.undo_depth_in(UndoScope::Person).0 > 0
    }

    pub fn can_redo(&self) -> bool {
        self.session.undo_depth_in(UndoScope::Person).1 > 0
    }

    /// Runs a command as `origin` (a person, an agent): the replica records it as theirs, and it
    /// undoes in their scope (an agent's `edit.undo` takes back only the agent's steps). Contract
    /// 1's [`KitDocument::execute`] is this with the local person.
    ///
    /// Someone else's command (any origin but the local person) while the editor holds an
    /// interaction open runs beside it as its own step ([`DesignDocument::aside`]): it never ends the
    /// person's interaction nor joins it.
    pub fn execute_as(&mut self, command: &str, params: Value, origin: &Origin) -> Result<Value, String> {
        let run = |doc: &mut Self| {
            let previous = doc.session.set_origin(Some(origin.clone()));
            let result = KitDocument::execute(doc, command, params);
            doc.session.set_origin(previous);
            result
        };
        if *origin == kition_design_engine::kdm::local_person() { run(self) } else { self.aside(run) }
    }

    /// Restores a saved version (`snapshot`: a KDM snapshot or this document's content, as a
    /// host's checkpoint keeps it; the Kit UI protocol's `replica/restore`, as `kition-sdk`'s
    /// `export_kit_session!` answers `restoreState`) through the kit's own `version.restore`, so a
    /// restore behaves one way however it is started: the design takes the content the version
    /// holds, and the file name, what the file published and the comments stay. One undo step of
    /// `origin`'s scope; nothing changes when the design already holds that. Add the snapshot's
    /// blobs first. Another document's snapshot is refused.
    pub fn restore_state(&mut self, snapshot: &[u8], origin: &Origin) -> Result<Damage, String> {
        let params = json!({ "snapshot": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, snapshot) });
        let before = self.session.version().map_err(engine_error)?;
        self.aside(|doc| {
            let previous = doc.session.set_origin(Some(origin.clone()));
            let result = doc.run("version.restore", &params);
            doc.session.set_origin(previous);
            result
        })?;
        let after = self.session.version().map_err(engine_error)?;
        Ok(if after == before { Damage::None } else { Damage::Full })
    }

    /// Takes a draft's changes (from a replica forked from this one) in as one undo step of the
    /// person (`mergeChanges`, as `export_kit_session!` answers it; M12a: the person accepts the
    /// draft, also one an agent made, `origin`): the engine's design follows, and the person's one
    /// Undo takes the whole draft back. Add the draft's blobs first. Answers `{"damage",
    /// "summary", "scope"}`, `scope` the undo scope the merge's step went to (`"person"`, `null`
    /// for no step), `summary` what the merge and the repair after it changed (`StateDiff`, read
    /// only at the objects they wrote, `Replica::diff_since`). A refused merge changes nothing and
    /// fails with the KDM's message.
    pub fn merge_changes(&mut self, bytes: &[u8], origin: &Origin) -> Result<Value, String> {
        let (wrote, since, scope) = self.aside(|doc| doc.session.merge_draft(bytes, origin).map_err(engine_error))?;
        let kdm = self.session.kdm().ok_or("the design left its replica")?;
        let summary = kdm.replica().diff_since(&since).map_err(engine_error)?;
        let damage = if wrote { Damage::Full } else { Damage::None };
        Ok(json!({ "damage": damage, "summary": summary.to_json(), "scope": scope.map(UndoScope::as_str) }))
    }

    /// What merging a draft's changes would change in the design now, as `StateDiff` JSON, read
    /// only at the objects the merge would write (`Replica::compare_changes`); nothing changes.
    /// Refuses what [`DesignDocument::merge_changes`] refuses.
    pub fn compare_changes(&self, bytes: &[u8]) -> Result<Value, String> {
        let kdm = self.session.kdm().ok_or("the design has no replica, so no draft of it")?;
        Ok(kdm.replica().compare_changes(bytes).map_err(engine_error)?.to_json())
    }

    /// What changed since the version `since` (empty bytes: since the replica began), a receipt as
    /// `export_kit_session!`'s `changesSince` answers it: `{"changes": StateDiff}`, or
    /// `{"changes": null, "unavailable": "<why>"}` when the replica's log does not hold `since`
    /// (or there is no replica), never an empty change set in its place.
    pub fn changes_since(&self, since: &[u8]) -> Value {
        let Some(kdm) = self.session.kdm() else { return json!({ "changes": null, "unavailable": "the design has no replica" }) };
        let replica = kdm.replica();
        let since = if since.is_empty() { Ok(Version::empty()) } else { Version::decode(since) };
        match since.and_then(|since| replica.changes_since(&since)) {
            Ok(changes) => json!({ "changes": changes.to_recorded_json(replica.schema()) }),
            Err(error) => json!({ "changes": null, "unavailable": error.to_string() }),
        }
    }

    /// An agent's reply (JSON text) with every object id it holds as a value or a key named as
    /// the tool interface names it (its tool id, KitObjectModel OM9); unchanged without a replica,
    /// when the schema declares no tool ids or when it names no object. Only the objects the
    /// reply names are read (`ToolIds::show_reply`), never the whole document's tool ids: the
    /// editor sends every `ai/inspect`, `document/check` and `ai/run` reply through it.
    pub fn model_reply(&self, value: &str) -> Result<String, String> {
        let value: Value = serde_json::from_str(value).map_err(|error| format!("the reply is not JSON: {error}"))?;
        let Some(kdm) = self.session.kdm() else { return Ok(value.to_string()) };
        Ok(kition_kit::kdm::tool_ids::ToolIds::show_reply(kdm.replica(), &value).to_string())
    }

    /// `model/describe` (`KitObjectModel 1.0`) as JSON, through the change exchange `kition-design`
    /// serves it with headless. A failure's message is the error's JSON (`{"code", "index",
    /// "message"}`).
    pub fn model_describe(&mut self) -> Result<Value, String> {
        kition_kit::session::model::Exchange::new().describe(self).map_err(|failure| failure.to_json().to_string())
    }

    /// `model/batch`: the request and the answer as JSON, one undo step; a failure's message is
    /// the error's JSON (`{"code", "index", "message"}`).
    pub fn model_batch(&mut self, request: &str) -> Result<Value, String> {
        let request: Value = serde_json::from_str(request)
            .map_err(|failure| json!({ "code": "invalid-op", "index": null, "message": format!("the batch is not JSON: {failure}") }).to_string())?;
        kition_kit::session::model::Exchange::new().batch(self, &request, false).map_err(|failure| failure.to_json().to_string())
    }

    /// Every blob the replica holds, as one buffer a peer feeds into [`DesignDocument::add_blobs`].
    pub fn blobs(&mut self) -> Result<Vec<u8>, String> {
        Ok(encode_blobs(&self.session.blobs().map_err(engine_error)?))
    }

    /// Adds a peer's blobs (from their `blobs()` or `blobsSince()`), before importing its changes.
    pub fn add_blobs(&mut self, bytes: &[u8]) -> Result<(), String> {
        let blobs = decode_blobs(bytes)?;
        self.session.add_blobs(&blobs).map_err(engine_error)
    }

    /// The blobs newly needed since `since` (empty bytes: every blob the state references).
    pub fn blobs_since(&mut self, since: &[u8]) -> Result<Vec<u8>, String> {
        let since = decode_version(since)?;
        Ok(encode_blobs(&self.session.blobs_since(&since).map_err(engine_error)?))
    }

    /// How many times the design left its replica because it refused a write or an undo; the
    /// next edit makes a fresh replica, which collaborators must resync. Hosts watch this.
    pub fn kdm_resets(&self) -> u32 {
        u32::try_from(self.session.kdm_resets()).unwrap_or(u32::MAX)
    }

    /// The schema version the replica reads (`kit.json` `model.schemaVersion`).
    pub fn schema_version(&self) -> u32 {
        self.schema.schema_version
    }

    /// The newest schema version any of the replica's changes was written under (rule M5a).
    pub fn author_schema_version(&self) -> Option<u32> {
        self.session.kdm().and_then(|kdm| kdm.replica().author_schema_version())
    }

    /// The canonical state (`Replica::state_json`), for tests and the convergence suite.
    pub fn state_json(&mut self) -> Result<Value, String> {
        self.session.state_json().map_err(engine_error)
    }
}

/// Encodes blobs for one call's worth of transfer: `(64-byte ASCII-hex hash, u64 LE length,
/// bytes) * N`, in hash order.
pub fn encode_blobs(blobs: &MemoryBlobs) -> Vec<u8> {
    let mut sorted: Vec<(&String, &Arc<[u8]>)> = blobs.0.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(b.0));
    let mut out = Vec::new();
    for (hash, data) in sorted {
        out.extend_from_slice(hash.as_bytes());
        out.extend_from_slice(&(data.len() as u64).to_le_bytes());
        out.extend_from_slice(data);
    }
    out
}

/// Reverses [`encode_blobs`]; truncated or malformed input is an error, never a panic.
pub fn decode_blobs(bytes: &[u8]) -> Result<MemoryBlobs, String> {
    let mut map = HashMap::new();
    let mut i = 0usize;
    while i < bytes.len() {
        let hash_end = i.checked_add(64).ok_or("blob bundle: overflow")?;
        let hash = bytes.get(i..hash_end).ok_or("truncated blob bundle: hash")?;
        let hash = std::str::from_utf8(hash).map_err(|_| "blob bundle: hash is not ASCII".to_owned())?.to_owned();
        let len_end = hash_end.checked_add(8).ok_or("blob bundle: overflow")?;
        let len_bytes: [u8; 8] =
            bytes.get(hash_end..len_end).ok_or("truncated blob bundle: length")?.try_into().map_err(|_| "blob bundle: length".to_owned())?;
        let len = usize::try_from(u64::from_le_bytes(len_bytes)).map_err(|_| "blob bundle: length too large".to_owned())?;
        let data_end = len_end.checked_add(len).ok_or("blob bundle: overflow")?;
        let data = bytes.get(len_end..data_end).ok_or("truncated blob bundle: data")?;
        map.insert(hash, Arc::from(data));
        i = data_end;
    }
    Ok(MemoryBlobs(map))
}

/// The object model (`KitObjectModel 1.0`, namespace `Design`) over the document's change exchange:
/// `kition-design` serves `model.describe` and `model.batch` headless through
/// `kition_kit::session::model::Exchange`.
impl kition_kit::session::model::ReplicaPort for DesignDocument {
    fn model_schema(&self) -> Arc<kition_kit::kdm::Schema> {
        self.schema.clone()
    }

    fn model_export(&mut self, since: &[u8]) -> Result<Vec<u8>, String> {
        self.export_changes(since)
    }

    fn model_import(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.import_changes(bytes).map(|_| ())
    }

    fn model_command(&mut self, command: &str, params: Value, origin: &Origin) -> Result<Value, String> {
        self.execute_as(command, params, origin)
    }

    fn model_commands(&self) -> Vec<kition_kit::KitCommand> {
        self.commands()
    }

    /// A shared design with no file yet (a server's headless kit opening a sync room): the
    /// replica alone, failing rather than falling back to an empty design.
    fn model_open_replica(name: &str, snapshot: &[u8], blobs: &[u8]) -> Result<Self, String> {
        Self::open_with_replica(name, &[], snapshot, blobs)
    }
}

/// The wasm-bindgen members of the JavaScript class (`DesignDocument`).
#[cfg(target_arch = "wasm32")]
pub mod web {
    use kition_kit::wasm_bindgen::prelude::*;
    use serde_json::json;

    use crate::__kition_kit_export::WebDocument;

    fn error(message: String) -> JsError {
        JsError::new(&message)
    }

    /// An origin in the KDM's commit form, the JSON array `[kind, principal, label]`.
    fn origin_of(origin: &str) -> Result<kition_kit::kdm::Origin, JsError> {
        kition_kit::kdm::Origin::decode(origin).ok_or_else(|| error(format!("origin is not [kind, principal, label]: {origin}")))
    }

    #[wasm_bindgen(js_class = "DesignDocument", wasm_bindgen = kition_kit::wasm_bindgen)]
    impl WebDocument {
        pub fn version(&mut self) -> Result<Vec<u8>, JsError> {
            self.0.version().map_err(error)
        }

        #[wasm_bindgen(js_name = exportChanges)]
        pub fn export_changes(&mut self, since: &[u8]) -> Result<Vec<u8>, JsError> {
            self.0.export_changes(since).map_err(error)
        }

        #[wasm_bindgen(js_name = importChanges)]
        pub fn import_changes(&mut self, bytes: &[u8]) -> Result<String, JsError> {
            self.0.import_changes(bytes).map(|damage| serde_json::to_string(&damage).unwrap_or_else(|_| "null".to_owned())).map_err(error)
        }

        pub fn snapshot(&mut self) -> Result<Vec<u8>, JsError> {
            self.0.snapshot().map_err(error)
        }

        pub fn undo(&mut self) -> Result<String, JsError> {
            self.0.undo().map(|(done, damage)| json!({ "done": done, "damage": damage }).to_string()).map_err(error)
        }

        pub fn redo(&mut self) -> Result<String, JsError> {
            self.0.redo().map(|(done, damage)| json!({ "done": done, "damage": damage }).to_string()).map_err(error)
        }

        #[wasm_bindgen(js_name = canUndo)]
        pub fn can_undo(&self) -> bool {
            self.0.can_undo()
        }

        #[wasm_bindgen(js_name = canRedo)]
        pub fn can_redo(&self) -> bool {
            self.0.can_redo()
        }

        /// `origin` is the KDM's commit form, the JSON array `[kind, principal, label]`.
        #[wasm_bindgen(js_name = executeAs)]
        pub fn execute_as(&mut self, command: &str, params: &str, origin: &str) -> Result<String, JsError> {
            let params = kition_kit::parse_params(params).map_err(error)?;
            let origin = kition_kit::kdm::Origin::decode(origin).ok_or_else(|| error(format!("origin is not [kind, principal, label]: {origin}")))?;
            self.0.execute_as(command, params, &origin).map(|value| value.to_string()).map_err(error)
        }

        /// `origin` is the KDM's commit form, the JSON array `[kind, principal, label]`; answers the
        /// damage JSON (Kit UI protocol `replica/restore`).
        #[wasm_bindgen(js_name = restoreState)]
        pub fn restore_state(&mut self, snapshot: &[u8], origin: &str) -> Result<String, JsError> {
            let origin = origin_of(origin)?;
            self.0.restore_state(snapshot, &origin).map(|damage| serde_json::to_string(&damage).unwrap_or_else(|_| "null".to_owned())).map_err(error)
        }

        /// A draft's changes taken in as one undo step, recorded with `origin` (`[kind, principal,
        /// label]`): `{"damage", "summary"}`. A refusal fails with the KDM's message.
        #[wasm_bindgen(js_name = mergeChanges)]
        pub fn merge_changes(&mut self, changes: &[u8], origin: &str) -> Result<String, JsError> {
            let origin = origin_of(origin)?;
            self.0.merge_changes(changes, &origin).map(|answer| answer.to_string()).map_err(error)
        }

        /// What merging a draft's changes would change now, as `StateDiff` JSON; changes nothing.
        #[wasm_bindgen(js_name = compareChanges)]
        pub fn compare_changes(&self, changes: &[u8]) -> Result<String, JsError> {
            self.0.compare_changes(changes).map(|answer| answer.to_string()).map_err(error)
        }

        /// What changed since the version `since` (a receipt): `{"changes": StateDiff}`, or
        /// `{"changes": null, "unavailable"}` when the replica's log does not hold it.
        #[wasm_bindgen(js_name = changesSince)]
        pub fn changes_since(&self, since: &[u8]) -> String {
            self.0.changes_since(since).to_string()
        }

        /// An agent's reply (JSON text) with its object ids named as the tool interface names them.
        #[wasm_bindgen(js_name = modelReply)]
        pub fn model_reply(&self, value: &str) -> Result<String, JsError> {
            self.0.model_reply(value).map_err(error)
        }

        /// `model/describe` (the object model) as JSON text.
        #[wasm_bindgen(js_name = modelDescribe)]
        pub fn model_describe(&mut self) -> Result<String, JsError> {
            self.0.model_describe().map(|answer| answer.to_string()).map_err(error)
        }

        /// `model/batch`: the request and the answer as JSON text, one undo step.
        #[wasm_bindgen(js_name = modelBatch)]
        pub fn model_batch(&mut self, request: &str) -> Result<String, JsError> {
            self.0.model_batch(request).map(|answer| answer.to_string()).map_err(error)
        }

        pub fn blobs(&mut self) -> Result<Vec<u8>, JsError> {
            self.0.blobs().map_err(error)
        }

        #[wasm_bindgen(js_name = addBlobs)]
        pub fn add_blobs(&mut self, bytes: &[u8]) -> Result<(), JsError> {
            self.0.add_blobs(bytes).map_err(error)
        }

        #[wasm_bindgen(js_name = blobsSince)]
        pub fn blobs_since(&mut self, since: &[u8]) -> Result<Vec<u8>, JsError> {
            self.0.blobs_since(since).map_err(error)
        }

        #[wasm_bindgen(js_name = kdmResets)]
        pub fn kdm_resets(&self) -> u32 {
            self.0.kdm_resets()
        }

        #[wasm_bindgen(js_name = schemaVersion)]
        pub fn schema_version(&self) -> u32 {
            self.0.schema_version()
        }

        #[wasm_bindgen(js_name = authorSchemaVersion)]
        pub fn author_schema_version(&self) -> Option<u32> {
            self.0.author_schema_version()
        }

        /// The canonical state as JSON text (the convergence suite's WebAssembly parity check).
        #[wasm_bindgen(js_name = stateJson)]
        pub fn state_json(&mut self) -> Result<String, JsError> {
            self.0.state_json().map(|state| state.to_string()).map_err(error)
        }

        #[wasm_bindgen(js_name = openWithReplica)]
        pub fn open_with_replica(name: &str, bytes: &[u8], snapshot: &[u8], blobs: &[u8]) -> Result<WebDocument, JsError> {
            crate::DesignDocument::open_with_replica(name, bytes, snapshot, blobs).map(Self).map_err(error)
        }

        #[wasm_bindgen(getter, js_name = hasReplica)]
        pub fn has_replica(&self) -> bool {
            self.0.has_replica()
        }

        #[wasm_bindgen(getter, js_name = docId)]
        pub fn doc_id(&self) -> Option<String> {
            self.0.doc_id()
        }
    }
}
