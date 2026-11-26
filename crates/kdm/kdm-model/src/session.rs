//! [`DesignSession`]: one document's replica, its view and its blobs, for the engine's command
//! bridge (`docs/features/kdm.md` section 4).
//!
//! It is `kition_kit::session::KitSession`'s flow on concrete parts, as Kition Vector's and
//! Design's sessions are: transactions with an origin, the replica's undo in scopes, the change
//! exchange, the repair after an import, an undo or a redo, and the view patched from the
//! replica. Edits are diff-bridged: the engine changes its own [`Document`] and
//! [`DesignSession::record`] writes what changed as one transaction, then adopts it as the view.

use std::collections::BTreeSet;
use std::sync::Arc;

use kition_design_doc::Document;
use kition_kit::kdm::{ActorId, DocId, KdmError, ObjectId, Origin, OriginKind, Replica, Tx, UndoOutcome, UndoScope, Version};
use kition_kit::session::{BlobSource, KitError, MemoryBlobs};

use crate::ids::Ids;
use crate::read::{Reuse, read_document, read_with};
use crate::{Result, kit_err, schema, validate, write_changes, write_document};

/// One open document as a KDM replica and the engine's view of it.
pub struct DesignSession {
    replica: Replica,
    view: Arc<Document>,
    ids: Ids,
    blobs: MemoryBlobs,
    /// The blob hashes each change started referencing, with the version right after it, for
    /// [`DesignSession::blobs_since`].
    blob_log: Vec<(Version, Vec<String>)>,
}

impl std::fmt::Debug for DesignSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DesignSession").field("doc", &self.replica.doc_id()).field("actor", &self.replica.actor()).finish_non_exhaustive()
    }
}

/// Whether a transaction of `origin` is an undo step: a person's, an agent's or an extension's,
/// never an import's or the system's.
pub fn undoable(origin: &Origin) -> bool {
    !matches!(origin.kind, OriginKind::System | OriginKind::Import)
}

/// [`Replica::transact`] with a kit error kept aside (the KDM's closure fails with a [`KdmError`]).
fn transact(replica: &mut Replica, origin: &Origin, f: impl FnOnce(&mut Tx<'_>) -> Result<bool>) -> Result<bool> {
    let mut kit_error = None;
    let result = replica.transact(origin, |tx| {
        f(tx).map_err(|error| match error {
            KitError::Kdm(error) => error,
            other => {
                let message = other.to_string();
                kit_error = Some(other);
                KdmError::Engine(message)
            }
        })
    });
    match (result, kit_error) {
        (_, Some(error)) => Err(error),
        (result, None) => result.map_err(kit_err),
    }
}

/// The repair (rules V1 to V12) in a `system` transaction labelled `repair`, written only when
/// there is something to repair.
fn repair(replica: &mut Replica) -> Result<()> {
    let everything = BTreeSet::new();
    let texts = crate::validate::store_texts(replica);
    transact(replica, &Origin::system("repair"), |tx| validate(tx, &everything, &texts)).map(|_| ())
}

/// The images of `doc` as blobs.
pub fn blobs_of(doc: &Document) -> MemoryBlobs {
    MemoryBlobs(doc.images.iter().map(|(hash, img)| (hash.clone(), Arc::from(img.bytes.as_slice()))).collect())
}

impl DesignSession {
    /// A new replica holding `document`, written in one creating transaction of `origin` (a
    /// `system` or `import` one: never an undo step). The view is `document` itself.
    pub fn create(document: &Arc<Document>, origin: &Origin) -> Result<Self> {
        let mut blobs = MemoryBlobs::default();
        let mut ids = Ids::default();
        let (mut replica, ()) =
            Replica::create_with(schema()?, DocId::new(), ActorId::random(), origin, |tx| write_document(tx, document, &mut ids, &mut blobs))
                .map_err(kit_err)?;
        replica.take_changed();
        Ok(Self { replica, view: document.clone(), ids, blobs, blob_log: Vec::new() })
    }

    /// Opens a snapshot (or a full change export) and the changes logged after it, in order, as
    /// a new replica of a fresh actor; `blobs` holds what they reference. The state is repaired
    /// once, so a file a merge left inconsistent opens consistent.
    pub fn open(snapshot: &[u8], changes: &[Vec<u8>], blobs: MemoryBlobs) -> Result<Self> {
        let mut replica = Replica::load(schema()?, snapshot, ActorId::random()).map_err(kit_err)?;
        for change in changes {
            replica.import_changes(change).map_err(kit_err)?;
        }
        repair(&mut replica)?;
        replica.take_changed();
        let (doc, ids) = read_document(&replica, &blobs).map_err(kit_err)?;
        Ok(Self { replica, view: Arc::new(doc), ids, blobs, blob_log: Vec::new() })
    }

    /// The document a saved version holds (Kit UI protocol 7m: the host's checkpoint carries its
    /// own `snapshot`), read from those bytes (a KDM snapshot or a file's content) with this
    /// session's blobs. Refuses bytes that are not this document's (another docId) or do not load.
    /// Never changes this session.
    pub fn document_at(&self, snapshot: &[u8]) -> Result<Document> {
        let mut replica = Replica::load(schema()?, snapshot, ActorId::random()).map_err(kit_err)?;
        if replica.doc_id() != self.replica.doc_id() {
            return Err(KitError::new(format!("not-this-document: the version is of document {}, not {}", replica.doc_id(), self.replica.doc_id())));
        }
        repair(&mut replica)?;
        let (doc, _) = read_document(&replica, &self.blobs).map_err(kit_err)?;
        Ok(doc)
    }

    /// The view: the document as of the last record, undo, redo or import.
    pub fn document(&self) -> &Arc<Document> {
        &self.view
    }

    pub fn ids(&self) -> &Ids {
        &self.ids
    }

    /// Writes what differs between the view and `after` as one transaction of `origin`, then makes
    /// `after` the view. Answers how many undo steps that made: 1, or 0 when nothing replicated
    /// changed or `origin` is not an undoable one. On failure nothing is written and the view is
    /// unchanged.
    pub fn record(&mut self, origin: &Origin, after: &Arc<Document>) -> Result<u32> {
        let mut ids = self.ids.clone();
        let before = self.replica.version();
        let had: BTreeSet<String> = self.blobs.0.keys().cloned().collect();
        let (view, blobs) = (&self.view, &mut self.blobs);
        let result = transact(&mut self.replica, origin, |tx| write_changes(tx, view, after, &mut ids, blobs).map_err(KitError::Kdm));
        self.replica.take_changed();
        result?;
        self.view = after.clone();
        self.ids = ids;
        let added: Vec<String> = self.blobs.0.keys().filter(|hash| !had.contains(*hash)).cloned().collect();
        self.log_added(added);
        let wrote = self.replica.version() != before;
        Ok(u32::from(wrote && undoable(origin)))
    }

    /// Makes `document` the view when it holds exactly the view's content (the engine's own undo
    /// snapshot of what the replica now holds), keeping its `Arc`s. Answers whether it did.
    pub fn adopt_equal(&mut self, document: &Arc<Document>) -> bool {
        if Arc::ptr_eq(document, &self.view) {
            return true;
        }
        if !crate::same_content(&self.view, document) {
            return false;
        }
        self.view = document.clone();
        true
    }

    /// Whether the replica holds `document`'s content (written to a scratch replica and read back,
    /// so both go through the same form).
    pub fn holds(&self, document: &Arc<Document>) -> Result<bool> {
        let scratch = Self::create(document, &Origin::system("compare"))?;
        let (theirs, _) = read_document(&scratch.replica, &scratch.blobs).map_err(kit_err)?;
        let (mine, _) = read_document(&self.replica, &self.blobs).map_err(kit_err)?;
        Ok(crate::same_content(&theirs, &mine))
    }

    /// Undoes this replica's last step of `scope` (never an import or a repair), repairs the result
    /// and patches the view. Answers whether there was one.
    pub fn undo_in(&mut self, scope: UndoScope) -> Result<bool> {
        Ok(self.step_in(scope, true)?.is_done())
    }

    pub fn redo_in(&mut self, scope: UndoScope) -> Result<bool> {
        Ok(self.step_in(scope, false)?.is_done())
    }

    /// Undoes (`undo`) or redoes this replica's last step of `scope`, repairs the result and patches
    /// the view. Answers which steps the replica took off its stack ([`UndoOutcome`]: the steps it
    /// passed over, their objects gone since, and the one it took back), so a history kept beside
    /// it stays in step.
    pub fn step_in(&mut self, scope: UndoScope, undo: bool) -> Result<UndoOutcome> {
        let can = if undo { self.replica.can_undo_in(scope) } else { self.replica.can_redo_in(scope) };
        if !can {
            return Ok(UndoOutcome::default());
        }
        let outcome = if undo { self.replica.undo_in(scope) } else { self.replica.redo_in(scope) }.map_err(kit_err)?;
        self.settle()?;
        Ok(outcome)
    }

    /// Takes a draft's changes (made on a fork of this replica) in as one transaction of `origin`
    /// (`Replica::merge_changes`): one undo step of the person (KitModel M12a, also for an agent's
    /// `origin`) unless `origin` is a `system` or `import` one. Repairs the result and patches the
    /// view as an import does; add the draft's blobs first ([`DesignSession::add_blobs`]). A
    /// refused merge changes nothing (the outer error). Answers the undo scope the merge's step was
    /// recorded in (`DraftMerge::scope`, `None` for no step) and the repair's result: a merge
    /// whose repair failed is still taken in.
    pub fn merge_changes(&mut self, bytes: &[u8], origin: &Origin) -> Result<(Option<UndoScope>, Result<()>)> {
        let merged = self.replica.merge_changes(bytes, origin).map_err(kit_err)?;
        Ok((merged.scope, self.settle()))
    }

    pub fn can_undo_in(&self, scope: UndoScope) -> bool {
        self.replica.can_undo_in(scope)
    }

    pub fn can_redo_in(&self, scope: UndoScope) -> bool {
        self.replica.can_redo_in(scope)
    }

    /// Imports remote or stored changes (their blobs added first, [`DesignSession::add_blobs`]),
    /// repairs the merged state and patches the view. Only the repair runs: no derived edit (auto
    /// layout, text fit, instance propagation) follows a merge (`docs/features/kdm.md` section 4.1).
    pub fn import_changes(&mut self, bytes: &[u8]) -> Result<()> {
        self.replica.import_changes(bytes).map_err(kit_err)?;
        self.settle()
    }

    /// Whether the replica's state reads as a document.
    pub fn view_loaded(&self) -> bool {
        read_document(&self.replica, &self.blobs).is_ok()
    }

    /// The blobs a peer at `since` needs to reach this session's state; an empty `since` answers
    /// every blob the current state references.
    pub fn blobs_since(&self, since: &Version) -> MemoryBlobs {
        if *since == Version::empty() {
            return self.referenced_blobs();
        }
        let mut out = MemoryBlobs::default();
        for (version, hashes) in &self.blob_log {
            if since.includes(version) {
                continue;
            }
            for hash in hashes {
                if let Some(bytes) = self.blobs.0.get(hash) {
                    out.0.insert(hash.clone(), bytes.clone());
                }
            }
        }
        out
    }

    /// The blobs the current state references.
    pub fn referenced_blobs(&self) -> MemoryBlobs {
        MemoryBlobs(self.view.images.keys().filter_map(|hash| self.blobs.0.get(hash).map(|b| (hash.clone(), b.clone()))).collect())
    }

    fn log_added(&mut self, added: Vec<String>) {
        if !added.is_empty() {
            self.blob_log.push((self.replica.version(), added));
        }
    }

    pub fn export_changes(&self, since: &Version) -> Result<Vec<u8>> {
        self.replica.export_changes(since).map_err(kit_err)
    }

    pub fn version(&self) -> Version {
        self.replica.version()
    }

    pub fn snapshot(&self) -> Result<Vec<u8>> {
        self.replica.snapshot().map_err(kit_err)
    }

    /// SHA-256 of the replicated state: equal on replicas that converged.
    pub fn state_hash(&self) -> Result<String> {
        self.replica.state_hash().map_err(kit_err)
    }

    pub fn doc_id(&self) -> DocId {
        self.replica.doc_id()
    }

    pub fn replica(&self) -> &Replica {
        &self.replica
    }

    /// Every blob the store holds.
    pub fn blobs(&self) -> &MemoryBlobs {
        &self.blobs
    }

    /// Adds blobs (a peer's, before importing its changes). Answers whether the view changed:
    /// blobs the state already references that were missing reload the images.
    pub fn add_blobs(&mut self, blobs: &MemoryBlobs) -> Result<bool> {
        let mut added = Vec::new();
        for (hash, bytes) in &blobs.0 {
            if !self.blobs.0.contains_key(hash) {
                self.blobs.0.insert(hash.clone(), bytes.clone());
                added.push(hash.clone());
            }
        }
        let late = self.missing_images(&added);
        self.log_added(added);
        if !late {
            return Ok(false);
        }
        let changed: BTreeSet<ObjectId> = self.replica.objects_of_type("Image").into_iter().collect();
        self.refresh(&changed)?;
        Ok(true)
    }

    /// Whether an image the replica lists is missing from the view and now has its bytes.
    fn missing_images(&self, added: &[String]) -> bool {
        self.replica.list(&crate::root_id(), "images").unwrap_or_default().iter().any(|o| {
            self.replica
                .get(o, "key")
                .ok()
                .and_then(|k| k.as_str().map(str::to_owned))
                .is_some_and(|k| !self.view.images.contains_key(&k) && added.contains(&k))
        })
    }

    /// Repairs what an import, undo or redo left, then patches the view with both.
    fn settle(&mut self) -> Result<()> {
        let mut changed = self.replica.take_changed();
        if changed.is_empty() {
            return Ok(());
        }
        let repaired = repair(&mut self.replica);
        changed.extend(self.replica.take_changed());
        self.refresh(&changed)?;
        repaired
    }

    fn refresh(&mut self, changed: &BTreeSet<ObjectId>) -> Result<()> {
        let blobs: &dyn BlobSource = &self.blobs;
        let (doc, ids) = read_with(&self.replica, blobs, Some(Reuse { view: &self.view, ids: &self.ids, changed })).map_err(kit_err)?;
        self.view = Arc::new(doc);
        self.ids = ids;
        Ok(())
    }
}
