//! The command bridge to each document's Kition KDM replica (engine contract 2; Kition's
//! `docs/features/kdm.md` section 4), the design of Kition Vector's.
//!
//! A document's replica ([`DesignSession`]) is its shared source of truth; [`DocState::doc`] is the
//! engine's own view of it. Commands, tools and keys run exactly as before on the in-memory
//! [`Document`]; at the end of every top-level call ([`Session::execute`], a committed
//! interaction, a group, and the tool events the binding forwards through
//! [`Session::sync_kdm`]) the bridge writes what the committed document changed since the
//! replica last saw it ([`Link::synced`]) as one transaction per history step pushed since,
//! found by their serials ([`HistoryEntry::serial`]). A change that pushed no step belongs to the
//! step on top when it is of the same undo scope, and is a `system` transaction otherwise.
//!
//! **Undo scopes** (KitModel 1.1 M12a, engine contract 2 E14a): every step remembers its scope
//! (the person's, or an agent's: [`Session::execute_as`] with an agent origin) and how many replica
//! steps it took ([`HistoryEntry::steps`]). Undo in a scope takes back the newest step of that
//! scope wherever it is in the history: the replica steps back that many transactions in that
//! scope and the document becomes the replica's result, which keeps the other scope's and other
//! replicas' later changes; the engine's own snapshot is the document when the step was the newest
//! of all and the replica holds exactly it (no one else's change came in).
//!
//! **No derived edit after a merge**: an import, an undo or a redo shows the replica's state as
//! it is; auto layout, text fit, group fit and instance propagation run only in local commands.
//!
//! The bridge is off until [`Session::enable_kdm`] (the bindings turn it on), so the engine's own
//! tests run on its snapshot history alone. A replica that refuses a write or an undo is left
//! ([`Session::kdm_resets`]): the document keeps working on its own history and the next edit makes
//! a fresh replica.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use kition_design_kdm::DesignSession;
use kition_design_kdm::file::{self, Entries, FileBase};
use kition_design_kdm::kition_kit::kdm::{ActorId, Origin, OriginKind, UndoScope, Version};
use kition_design_kdm::kition_kit::session::MemoryBlobs;

use super::*;

/// What a save produced, until it is marked written: what the file then holds.
pub(crate) type Pending = FileBase;

/// The origin of the local person's commands (when nothing else was set with
/// [`Session::execute_as`]).
pub fn local_person() -> Origin {
    Origin::person("person:local")
}

/// The scope an origin's undo steps join (a `system` or `import` origin's edits are the person's
/// when they push a history step at all).
pub fn scope_of(origin: &Origin) -> UndoScope {
    UndoScope::of(origin.kind).unwrap_or(UndoScope::Person)
}

fn kdm_error(error: impl std::fmt::Display) -> EngineError {
    EngineError::Other(format!("document replica: {error}"))
}

fn no_replica() -> EngineError {
    EngineError::Other("the document has no replica and none could be made".into())
}

/// The id space a session gets: from the replica's random actor id, in `1..2^21 - 1` (every id
/// stays below 2^53), avoiding the spaces `doc` already uses.
fn client_for(doc: &Document, actor: ActorId) -> u32 {
    const SPACES: u64 = (1 << 21) - 2;
    let used: HashSet<u32> = doc.ids().map(|id| kition_design_doc::ids::space_of(id.0)).collect();
    let mut client = u32::try_from(actor.get() % SPACES + 1).unwrap_or(1);
    for _ in 0..SPACES {
        if !used.contains(&client) {
            break;
        }
        client = if u64::from(client) >= SPACES { 1 } else { client + 1 };
    }
    client
}

/// One document's bridge state.
#[derive(Debug)]
pub struct Link {
    /// The replica; `None` until the bridge makes it, and again after a reset.
    pub(crate) kdm: Option<DesignSession>,
    /// The committed document as last written to (or read from) the replica.
    pub(crate) synced: Arc<Document>,
    pub(crate) resets: u64,
    /// [`History::pushed`] when the bridge last looked.
    pub(crate) mark: u64,
    /// Every history step (serial and replica steps) as the bridge last left them, to find the
    /// steps a command dropped from the history.
    pub(crate) last: Vec<(u64, u32)>,
    /// This session's id space ([`Document::set_client_id`]); 0 before the replica is made.
    pub(crate) client: u32,
    /// The replica entries the document's file holds, until the replica is made from them.
    pub(crate) file: Option<Entries>,
    /// What the file holds of the replica (the last open or completed save).
    pub(crate) base: Option<FileBase>,
    /// What the last [`DocState::replica_entries`] produced, until the save is marked written.
    pub(crate) pending: Arc<Mutex<Option<Pending>>>,
    /// The document the replica last refused to hold: no retry until it changes.
    pub(crate) refused: Option<Arc<Document>>,
    /// The session's id high-water mark: no id below it in its space is handed out again, though
    /// an undo, a redo or a merge puts back a document whose counter is older (an id handed out
    /// twice would name two objects).
    pub(crate) floor: u64,
}

impl Link {
    pub(crate) fn new(doc: &Arc<Document>) -> Self {
        Self {
            kdm: None,
            synced: doc.clone(),
            resets: 0,
            mark: 0,
            last: Vec::new(),
            client: 0,
            file: None,
            base: None,
            pending: Arc::default(),
            refused: None,
            floor: 0,
        }
    }
}

/// A clone holds no replica (a replica has one owner): it makes a fresh one.
impl Clone for Link {
    fn clone(&self) -> Self {
        Self::new(&self.synced)
    }
}

impl DocState {
    /// The document's replica, if it has one.
    pub fn kdm(&self) -> Option<&DesignSession> {
        self.link.kdm.as_ref()
    }

    /// How many times the document left its replica.
    pub fn kdm_resets(&self) -> u64 {
        self.link.resets
    }

    /// Whether the document's file held replica entries the replica is not made from yet.
    pub fn has_file_replica(&self) -> bool {
        self.link.file.is_some()
    }

    /// Whether the replica is the one the document's file holds (made from its entries, or saved
    /// into it since).
    pub fn on_file_replica(&self) -> bool {
        self.link.kdm.is_some() && self.link.base.is_some()
    }

    /// Opens the document with the replica entries its file holds (made into the replica when
    /// the bridge first looks).
    pub fn with_replica(mut self, entries: Entries) -> Self {
        self.link.file = (!entries.is_empty()).then_some(entries);
        self
    }

    /// The document as committed: the base of an interaction in progress, else the document.
    fn committed(&self) -> Arc<Document> {
        self.interaction.as_ref().map_or_else(|| self.doc.clone(), |it| it.doc.clone())
    }

    /// Gives the document this session's id space (the saved document stays saved).
    fn apply_client(&mut self) {
        let client = self.link.client;
        if client == 0 || (self.doc.client_id() == client && self.doc.next_id().0 >= self.link.floor) {
            return;
        }
        let saved = Arc::ptr_eq(&self.doc, &self.saved_doc);
        let doc = Arc::make_mut(&mut self.doc);
        doc.set_client_id(client);
        if let Some(below) = self.link.floor.checked_sub(1) {
            doc.reserve_ids(NodeId(below));
        }
        if saved {
            self.saved_doc = self.doc.clone();
        }
        if self.interaction.is_none() {
            self.link.synced = self.doc.clone();
        }
    }

    /// The replica's view as the document: with what the file keeps for itself (the published
    /// library) and the editing person's own current page from `local`.
    fn view_of(kdm: &DesignSession, local: &Document) -> Arc<Document> {
        let mut d = (**kdm.document()).clone();
        d.published = local.published.clone();
        d.current_page = local.current_page;
        Arc::new(d)
    }

    /// Remembers how far the session's id counter went ([`Link::floor`]).
    fn note_floor(&mut self) {
        if self.link.client != 0 && self.doc.client_id() == self.link.client {
            self.link.floor = self.link.floor.max(self.doc.next_id().0);
        }
    }

    /// Makes the replica if there is none: from the file's replica entries (its content; the
    /// document as read from it is recorded on top as one `import` transaction, which writes
    /// nothing when the replica holds it), else a new one holding the document. Answers whether
    /// there is one.
    pub(crate) fn ensure_kdm(&mut self) -> bool {
        if self.link.kdm.is_some() {
            return true;
        }
        let current = self.committed();
        if self.link.refused.as_ref().is_some_and(|r| Arc::ptr_eq(r, &current)) {
            return false;
        }
        let from_file = self.link.file.take().and_then(|entries| match file::open(&entries, kition_design_kdm::blobs_of(&current)) {
            Ok((mut kdm, base)) => {
                let import = Origin::new(OriginKind::Import, "file").with_label("file");
                match kdm.record(&import, &current) {
                    Ok(_) => Some((kdm, Some(base))),
                    Err(error) => {
                        log::warn!("the file's replica can't take its document: {error}");
                        None
                    }
                }
            }
            Err(error) => {
                log::warn!("the file's replica doesn't open: {error}");
                None
            }
        });
        let made = from_file.or_else(|| match DesignSession::create(&current, &Origin::system("create")) {
            Ok(kdm) => Some((kdm, None)),
            Err(error) => {
                log::warn!("the document can't be held in a replica: {error}");
                None
            }
        });
        let Some((kdm, base)) = made else {
            self.link.refused = Some(current);
            return false;
        };
        self.link.client = client_for(&current, kdm.replica().actor());
        self.link.kdm = Some(kdm);
        self.link.base = base;
        self.link.synced = current;
        self.link.refused = None;
        self.mark();
        self.apply_client();
        if let (Some(kdm), true) = (self.link.kdm.as_mut(), self.interaction.is_none()) {
            kdm.adopt_equal(&self.doc);
        }
        true
    }

    /// The replica steps of the history steps the bridge accounted that are gone from the history
    /// from the top (newer than the oldest one still there): they go to the step now on top.
    fn dropped_steps(&self) -> u32 {
        let present: HashSet<u64> = self.history.undo.iter().chain(&self.history.redo).map(|e| e.serial).collect();
        let Some(oldest) = self.link.last.iter().map(|(serial, _)| *serial).filter(|s| present.contains(s)).min() else { return 0 };
        self.link
            .last
            .iter()
            .filter(|(serial, _)| !present.contains(serial) && *serial > oldest)
            .map(|(_, steps)| *steps)
            .fold(0, u32::saturating_add)
    }

    /// Remembers the history as the bridge leaves it.
    fn mark(&mut self) {
        self.link.mark = self.history.pushed;
        self.link.last = self.history.undo.iter().chain(&self.history.redo).map(|e| (e.serial, e.steps)).collect();
    }

    /// Writes what the document committed since the replica last saw it: one transaction per
    /// history step pushed since the last look.
    pub(crate) fn sync_kdm(&mut self, origin: &Origin) {
        if !self.ensure_kdm() {
            return;
        }
        self.note_floor();
        let current = self.committed();
        let dropped = self.dropped_steps();
        let pushed = self.history.pushed_since(self.link.mark);
        if pushed.is_empty() && dropped == 0 && Arc::ptr_eq(&self.link.synced, &current) {
            self.mark();
            return;
        }
        let scope = scope_of(origin);
        if pushed.is_empty() {
            let mut steps = dropped;
            if !Arc::ptr_eq(&self.link.synced, &current) {
                // Part of the step on top when it is this scope's and no redo of it waits (a redo
                // of the scope would be lost if this became a new step of it); else no step.
                let top = self.history.undo.last().is_some_and(|e| e.scope == scope) && !self.history.redo.iter().any(|e| e.scope == scope);
                let origin = if top { origin.clone() } else { Origin::system("view") };
                steps = steps.saturating_add(self.record_kdm(&origin, &current).unwrap_or(0));
            }
            if steps > 0
                && let Some(top) = self.history.undo.last_mut()
            {
                top.steps = top.steps.saturating_add(steps);
            }
        } else {
            if dropped > 0
                && let Some(e) = pushed.start.checked_sub(1).and_then(|i| self.history.undo.get_mut(i))
            {
                e.steps = e.steps.saturating_add(dropped);
            }
            for i in pushed {
                // What changed before this step (a direct write between commands) is no part of
                // it: it goes first, to the step under it when that is this scope's.
                if let Some(before) = self.history.undo.get(i).map(|e| e.doc.clone())
                    && !Arc::ptr_eq(&self.link.synced, &before)
                {
                    let below = i.checked_sub(1).filter(|j| self.history.undo.get(*j).is_some_and(|e| e.scope == scope));
                    let origin = if below.is_some() { origin.clone() } else { Origin::system("view") };
                    let Some(steps) = self.record_kdm(&origin, &before) else { break };
                    if steps > 0
                        && let Some(e) = below.and_then(|j| self.history.undo.get_mut(j))
                    {
                        e.steps = e.steps.saturating_add(steps);
                    }
                }
                let after = self.history.undo.get(i + 1).map(|e| e.doc.clone()).unwrap_or_else(|| current.clone());
                let mut steps = 0;
                if !Arc::ptr_eq(&self.link.synced, &after) {
                    match self.record_kdm(origin, &after) {
                        Some(n) => steps = n,
                        None => break,
                    }
                }
                if let Some(e) = self.history.undo.get_mut(i) {
                    e.steps = steps;
                }
            }
        }
        self.mark();
    }

    /// Writes `after` on top of the replica and answers the undo steps that took; `None` when the
    /// replica refused and was left.
    fn record_kdm(&mut self, origin: &Origin, after: &Arc<Document>) -> Option<u32> {
        let kdm = self.link.kdm.as_mut()?;
        match kdm.record(origin, after) {
            Ok(steps) => {
                self.link.synced = after.clone();
                Some(steps)
            }
            Err(error) => {
                log::warn!("the document replica refused an edit: {error}");
                self.detach_kdm();
                None
            }
        }
    }

    /// Leaves the replica: every step's replica steps were the dropped replica's.
    pub(crate) fn detach_kdm(&mut self) {
        self.link.kdm = None;
        self.link.base = None;
        self.link.resets = self.link.resets.saturating_add(1);
        self.link.synced = self.committed();
        for e in self.history.undo.iter_mut().chain(self.history.redo.iter_mut()) {
            e.steps = 0;
        }
        self.mark();
    }

    /// After undo (`undo`) or redo of a step of `scope` that took `steps` replica steps: steps the
    /// replica too and takes its result as the document, or the step's own snapshot `reference`
    /// when the step was the newest of all (`top`) and the replica holds just that.
    pub(crate) fn step_kdm(&mut self, steps: u32, undo: bool, scope: UndoScope, reference: &Arc<Document>, top: bool) {
        if let Some(kdm) = self.link.kdm.as_mut() {
            // The replica may have fewer steps of the scope left than the history counts (its
            // undo drops a step a merge emptied): it steps as far as it can, and its result is the
            // document either way.
            // A call may also pass over steps with nothing left to change (UndoOutcome): they
            // count towards this step's.
            let mut left = steps as usize;
            while left > 0 {
                match kdm.step_in(scope, undo) {
                    Ok(outcome) if outcome.is_done() => left = left.saturating_sub(outcome.steps()),
                    Ok(_) => break,
                    Err(error) => {
                        log::warn!("the document replica refused an undo or redo: {error}");
                        self.detach_kdm();
                        return;
                    }
                }
            }
            self.doc = if top && kdm.adopt_equal(reference) { reference.clone() } else { Self::view_of(kdm, &self.doc) };
        }
        self.apply_client();
        self.link.synced = self.doc.clone();
        self.mark();
    }

    /// After an import: the document follows the replica (an interaction in progress goes on from
    /// it), and the selection lets go of what is gone.
    pub(crate) fn follow_kdm(&mut self) {
        self.note_floor();
        let Some(kdm) = self.link.kdm.as_ref() else { return };
        let view = Self::view_of(kdm, &self.doc);
        self.link.synced = view.clone();
        if let Some(it) = &mut self.interaction {
            it.doc = view.clone();
        }
        self.doc = view;
        self.apply_client();
        if let Some(it) = &mut self.interaction {
            it.doc = self.doc.clone();
        }
        if !self.doc.pages.contains(&self.page)
            && let Some(first) = self.doc.first_page()
        {
            self.page = first;
        }
        self.clean_selection();
        self.revision += 1;
    }

    /// The replica entries a save writes into the file: the replica's content (none without a
    /// replica), remembered until [`DocState::mark_saved`].
    pub fn replica_entries(&self) -> Result<Entries> {
        let Some(kdm) = self.link.kdm.as_ref() else { return Ok(Vec::new()) };
        let saved = file::save(kdm).map_err(kdm_error)?;
        if let Ok(mut pending) = self.link.pending.lock() {
            *pending = Some(saved.base);
        }
        Ok(saved.entries)
    }

    /// The last save's replica entries are now what the file holds.
    pub(crate) fn saved_replica(&mut self) {
        if let Some(base) = self.link.pending.lock().ok().and_then(|mut p| p.take()) {
            self.link.base = Some(base);
        }
    }
}

impl Session {
    /// Turns the bridge on: every open document (and every one opened later) gets a replica.
    pub fn enable_kdm(&mut self) {
        self.kdm_on = true;
        self.sync_all_kdm();
    }

    pub fn kdm_enabled(&self) -> bool {
        self.kdm_on
    }

    /// Who the running (or next) command acts for: the origin of [`Session::execute_as`], or the
    /// local person.
    pub fn origin(&self) -> Origin {
        self.origin.clone().unwrap_or_else(local_person)
    }

    /// The undo scope of [`Session::origin`].
    pub fn undo_scope(&self) -> UndoScope {
        scope_of(&self.origin())
    }

    /// Sets who the following commands act for (`None`: the local person); answers the previous
    /// setting, to put back.
    pub fn set_origin(&mut self, origin: Option<Origin>) -> Option<Origin> {
        std::mem::replace(&mut self.origin, origin)
    }

    /// [`Session::execute`] as `origin` (a person, an agent): the replica records the edit as
    /// theirs, and it undoes in their scope.
    pub fn execute_as(&mut self, id: &str, params: &Value, origin: &Origin) -> Result<Value> {
        let previous = self.set_origin(Some(origin.clone()));
        let r = self.execute(id, params);
        self.set_origin(previous);
        r
    }

    /// Writes every document's changes to its replica (what a top-level call does at its end);
    /// the binding calls it after tool events.
    pub fn sync_kdm(&mut self) {
        self.sync_all_kdm();
    }

    pub(crate) fn sync_all_kdm(&mut self) {
        if !self.kdm_on || self.kdm_depth > 0 {
            return;
        }
        let origin = self.origin();
        let live: Vec<u64> = self.docs.iter().map(|ds| ds.uid).filter(|uid| self.live_doc(*uid)).collect();
        for ds in self.docs.iter_mut().filter(|ds| !live.contains(&ds.uid)) {
            ds.sync_kdm(&origin);
        }
    }

    /// The active document's replica, if it has one (never makes one).
    pub fn kdm(&self) -> Option<&DesignSession> {
        self.ds().ok()?.kdm()
    }

    /// How many times the active document left its replica.
    pub fn kdm_resets(&self) -> u64 {
        self.ds().map_or(0, DocState::kdm_resets)
    }

    /// The active document's KDM identity, `None` before it has a replica.
    pub fn doc_id(&self) -> Option<String> {
        Some(self.kdm()?.doc_id().to_string())
    }

    fn active_kdm(&mut self) -> Result<&mut DesignSession> {
        let origin = self.origin();
        let ds = self.ds_mut()?;
        ds.sync_kdm(&origin);
        ds.link.kdm.as_mut().ok_or_else(no_replica)
    }

    pub fn version(&mut self) -> Result<Version> {
        Ok(self.active_kdm()?.version())
    }

    pub fn snapshot(&mut self) -> Result<Vec<u8>> {
        self.active_kdm()?.snapshot().map_err(kdm_error)
    }

    /// SHA-256 of the active document's replicated state: equal on replicas that converged.
    pub fn state_hash(&mut self) -> Result<String> {
        self.active_kdm()?.state_hash().map_err(kdm_error)
    }

    /// The canonical state (`Replica::state_json`).
    pub fn state_json(&mut self) -> Result<Value> {
        self.active_kdm()?.replica().state_json().map_err(kdm_error)
    }

    /// The active document's changes since `since` (`Version::empty()` for all of them).
    pub fn export_changes(&mut self, since: &Version) -> Result<Vec<u8>> {
        self.active_kdm()?.export_changes(since).map_err(kdm_error)
    }

    /// Every blob the active document's replica holds.
    pub fn blobs(&mut self) -> Result<MemoryBlobs> {
        Ok(self.active_kdm()?.blobs().clone())
    }

    /// The blobs a peer at `since` needs.
    pub fn blobs_since(&mut self, since: &Version) -> Result<MemoryBlobs> {
        Ok(self.active_kdm()?.blobs_since(since))
    }

    /// Adds a peer's blobs (before importing its changes).
    pub fn add_blobs(&mut self, blobs: &MemoryBlobs) -> Result<()> {
        if self.active_kdm()?.add_blobs(blobs).map_err(kdm_error)? {
            self.ds_mut()?.follow_kdm();
        }
        Ok(())
    }

    /// Imports a collaborator's (or stored) changes into the active document's replica. Local
    /// edits not written yet are written first; the document and its selection follow the
    /// replica; one whose view could not be loaded leaves it (a reset).
    pub fn import_changes(&mut self, bytes: &[u8]) -> Result<()> {
        let kdm = self.active_kdm()?;
        let before = kdm.version();
        let result = kdm.import_changes(bytes);
        let changed = kdm.version() != before;
        let follow = result.is_ok() || (changed && kdm.view_loaded());
        let ds = self.ds_mut()?;
        if follow {
            ds.follow_kdm();
        } else if changed {
            ds.detach_kdm();
        }
        result.map_err(kdm_error)
    }

    /// Takes a draft's changes into the active document's replica (`mergeChanges`,
    /// `Replica::merge_changes`) as one undo step of the **person** (KitModel M12a: the person
    /// accepts the draft, also one an agent made, and the replica records it in the person's
    /// scope whatever `origin` names; none for a `system` or `import` origin): local edits not
    /// written yet are written first, the document follows the replica as after an import, and
    /// the person's one Undo takes the whole draft back. Answers whether it changed the replica,
    /// the replica's version just before the merge (what `Replica::diff_since` reads the
    /// merge's summary from) and the undo scope the merge's step went to (`DraftMerge::scope`:
    /// the person's, `None` for no step). A refused merge changes nothing.
    pub fn merge_draft(&mut self, bytes: &[u8], origin: &Origin) -> Result<(bool, Version, Option<UndoScope>)> {
        const LABEL: &str = "Merge draft";
        let uid = self.ds()?.uid;
        if let Some(why) = self.live_read_only(uid) {
            return Err(EngineError::Disabled(LABEL.into(), why.into()));
        }
        let own = self.origin();
        let ds = self.ds_mut()?;
        if ds.preview.is_some() {
            return Err(EngineError::Disabled(LABEL.into(), PREVIEW_READ_ONLY.into()));
        }
        ds.sync_kdm(&own);
        let (doc, selection, page) = (ds.committed(), ds.selection.clone(), ds.page);
        let kdm = ds.link.kdm.as_mut().ok_or_else(no_replica)?;
        let before = kdm.version();
        let (scope, result) = match kdm.merge_changes(bytes, origin) {
            Ok((scope, repaired)) => (scope, repaired),
            Err(error) => (None, Err(error)),
        };
        let changed = kdm.version() != before;
        if !changed {
            result.map_err(kdm_error)?;
            return Ok((false, before, None));
        }
        if result.is_err() && !kdm.view_loaded() {
            ds.detach_kdm();
            return result.map_err(kdm_error).map(|()| (true, before, scope));
        }
        ds.follow_kdm();
        // The replica records a merge in the person's scope (M12a), whoever made the draft: the
        // engine's history takes the step where the replica put it.
        if let Some(scope) = scope {
            ds.push_undo(LABEL, doc, selection, page, scope);
            if let Some(top) = ds.history.undo.last_mut() {
                top.steps = 1;
            }
        }
        ds.mark();
        result.map_err(kdm_error).map(|()| (true, before, scope))
    }

    /// Takes in what another program saved to the active document's file while the document was
    /// clean (kition-sdk `KitDocument::reconcile_reread`); `fresh` is that file read again. Only
    /// what it holds differently is written, as one `import` transaction: that is outside both
    /// undo scopes, so the person's and agents' undo steps stay and each still undoes only its
    /// own change. Layers keep their ids, so the selection and the edit modes stay. The parts of
    /// the file outside the replica (the published library) are the file's. The document ends
    /// clean. Errs, changing nothing, without a replica, or while a gesture, a text preview or a
    /// version preview is open.
    pub fn reconcile_reread(&mut self, fresh: &DocState) -> Result<()> {
        let origin = self.origin();
        let ds = self.ds_mut()?;
        if ds.interaction.is_some() || ds.text_preview.is_some() || ds.preview.is_some() {
            return Err(EngineError::Other("the file changed while a gesture or a preview was open: open the file again".into()));
        }
        ds.sync_kdm(&origin);
        let kdm = ds.link.kdm.as_mut().ok_or_else(no_replica)?;
        kdm.record(&Origin::new(OriginKind::Import, "file reread").with_label("file"), &fresh.doc).map_err(kdm_error)?;
        // The published library is kept in the file, not in the replica; `follow_kdm` takes it
        // from the document, so the document takes the file's first.
        let same = match (&ds.doc.published, &fresh.doc.published) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        if !same {
            Arc::make_mut(&mut ds.doc).published.clone_from(&fresh.doc.published);
        }
        ds.follow_kdm();
        ds.take_file_parts(fresh);
        ds.mark_saved();
        Ok(())
    }

    /// Opens another replica's snapshot (with the blobs it references) as a new document, a
    /// replica of the same KDM document under a fresh actor, and makes it active: clean, with an
    /// empty history.
    pub fn open_snapshot(&mut self, snapshot: &[u8], blobs: MemoryBlobs) -> Result<usize> {
        let kdm = DesignSession::open(snapshot, &[], blobs).map_err(kdm_error)?;
        let mut ds = DocState::new((**kdm.document()).clone(), None);
        ds.link.client = client_for(&ds.doc, kdm.replica().actor());
        ds.link.kdm = Some(kdm);
        ds.apply_client();
        ds.link.synced = ds.doc.clone();
        ds.mark_saved();
        self.docs.push(ds);
        self.active = self.docs.len() - 1;
        Ok(self.active)
    }

    /// Puts the active document on another replica of it (a host's stored replica, or a sync
    /// room's): the replica's own identity, with the document recorded on top as one `import`
    /// transaction when the replica does not hold it. The document stays as it is and clean.
    pub fn attach_replica(&mut self, snapshot: &[u8], blobs: MemoryBlobs) -> Result<()> {
        let ds = self.ds_mut()?;
        let current = ds.committed();
        let mut all = blobs;
        all.0.extend(kition_design_kdm::blobs_of(&current).0);
        let mut kdm = DesignSession::open(snapshot, &[], all).map_err(kdm_error)?;
        if !kdm.adopt_equal(&current) {
            kdm.record(&Origin::new(OriginKind::Import, "file").with_label("file"), &current).map_err(kdm_error)?;
        }
        ds.link.client = client_for(&current, kdm.replica().actor());
        ds.link.kdm = Some(kdm);
        ds.link.file = None;
        ds.link.base = None;
        ds.link.synced = current;
        ds.mark();
        ds.apply_client();
        Ok(())
    }
}
