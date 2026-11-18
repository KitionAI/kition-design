//! Kition's collaboration members (engine contract 2; `docs/features/kdm.md`): the design as a KDM
//! replica.
//!
//! The engine keeps its own `Session` (the command bridge to each document's replica,
//! `kition_design_engine::kdm`), not a `kition_kit::session::KitSession`, so
//! `kition_kit::export_kit_session!` does not apply; these members are added by hand with the JS
//! names and shapes of kition-sdk's `session::web`, as Kition Vector and Design do, so Kition's
//! host code is the same for every kit. The wasm bindings are in [`web`].
//!
//! A `.kdesign` file carries its replica (`kdm/` entries): opening one opens its replica and saving
//! one appends to it. The members below still answer `replica/*` so a host can relay changes
//! between people editing the same design. Images are blobs, encoded for transfer as
//! `(64-char ASCII-hex sha256, u64 LE length, bytes) * N`, as every kit does.
//!
//! `contractVersion()` stays 1, as kition-sdk's export macro answers it: Kition and
//! `@kition/kit-editor` refuse a newer number, and recognise contract 2 by the manifest and these
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
    /// capability). A `.kdesign` that holds its own replica opens with that one (when its replica
    /// entries open; a file merely naming them is not one): the file is what the person saved. Any other file opens on the stored replica, with the file recorded on top
    /// as one `import` transaction when the replica does not hold it; a stored replica that does
    /// not open is ignored and the file opens as [`KitDocument::open`] opens it. Empty `bytes`
    /// mean there is no file yet, only the replica (a sync room's snapshot): the design is the
    /// replica's alone, and a replica that does not open is an error.
    pub fn open_with_replica(name: &str, bytes: &[u8], snapshot: &[u8], blobs: &[u8]) -> Result<Self, String> {
        if bytes.is_empty() {
            let mut session = Session::empty();
            session.open_snapshot(snapshot, decode_blobs(blobs)?).map_err(engine_error)?;
            return Self::from_session(session);
        }
        if snapshot.is_empty() {
            return Self::open(name, bytes);
        }
        let mut session = crate::open_session(name, bytes)?;
        if session.ds().is_ok_and(kition_design_engine::DocState::has_file_replica) {
            let doc = Self::from_session(session)?;
            if doc.session.ds().is_ok_and(kition_design_engine::DocState::on_file_replica) {
                return Ok(doc);
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
