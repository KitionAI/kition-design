//! Explicit call context (kition-sdk `KitDocument::editor_state`, `set_context`; Kit Process
//! Protocol section 7a; task card C4/C5/C6-design): the editor state a command may read besides its
//! parameters, taken and put back without an edit, set for one call from what the call says it
//! acts on, and kept out of an agent's commands that name what they act on.
//!
//! A call's **part** is a page: `{"pageId": id}` (the id inspect lists for it, which stays the same
//! when pages are added, moved or renamed) or `{"page": n}` (its place, counted from 1, as
//! diagnostics give it). `slide` and `slideId` name no part of a design.
//!
//! A call's **`on`** is layers by their numeric ids (`12` or `"12"`), as inspect lists them. They
//! become the selection, which lives on one page: on the call's part when it has one, else on the
//! page the layers are on, which their ids name (no part is needed). Layers of two pages in one
//! step are refused: a command that reads the selection acts on the current page's. A command that
//! takes `ids` acts across pages, and `on` fills that parameter instead (`acts::targets`).
//!
//! Everything is checked before anything changes, and nothing of it is an edit: the revision, the
//! dirty state and the undo history stay as they are. A design has no timeline: `time` is not
//! available.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use kition_design_doc::{Document, NodeId};
use kition_design_engine::context::EditorState;
use kition_kit::CallContext;
use serde_json::{Value, json};

use crate::DesignDocument;

/// How many snapshots [`DesignDocument::editor_state`](kition_kit::KitDocument::editor_state) keeps
/// for [`DesignDocument::restore_editor_state`](kition_kit::KitDocument::restore_editor_state): a
/// host restores the one it took after a call, and an editor running an agent's steps holds two
/// or three at a time.
const KEPT: usize = 64;

/// The copies made so far ([`Kept::copy`]).
static COPIES: AtomicU64 = AtomicU64::new(1);

/// A snapshot answered: its fingerprint, the copy of the design it was taken on, and the state.
struct Snapshot {
    number: String,
    copy: u64,
    state: EditorState,
}

/// The snapshots answered so far (the newest last), shared by the copies of one design: the
/// design read again from its file after another program saved it, and the isolated copy a
/// guard plans a run on, restore a state taken on the design they replace or copy
/// ([`DesignDocument::share_kept`]).
pub(crate) struct Kept {
    /// This copy of the design, to tell its own snapshots from another copy's.
    copy: u64,
    shared: Arc<Mutex<VecDeque<Snapshot>>>,
}

impl Default for Kept {
    fn default() -> Self {
        Self { copy: COPIES.fetch_add(1, Ordering::Relaxed), shared: Arc::default() }
    }
}

impl Kept {
    fn snapshots(&self) -> MutexGuard<'_, VecDeque<Snapshot>> {
        self.shared.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn push(snapshots: &mut VecDeque<Snapshot>, snapshot: Snapshot) {
        snapshots.retain(|kept| kept.number != snapshot.number || kept.copy != snapshot.copy);
        snapshots.push_back(snapshot);
        while snapshots.len() > KEPT {
            snapshots.pop_front();
        }
    }

    /// Joins `other`'s snapshots: this copy's own move there, and both copies then keep them.
    fn join(&mut self, other: &Self) {
        if Arc::ptr_eq(&self.shared, &other.shared) {
            return;
        }
        let own = std::mem::take(&mut *self.snapshots());
        {
            let mut theirs = other.snapshots();
            for snapshot in own {
                Self::push(&mut theirs, snapshot);
            }
        }
        self.shared = other.shared.clone();
    }

    /// The state `number` names, this copy's own first, and whether another copy took it.
    fn find(&self, number: &str) -> Option<(EditorState, bool)> {
        let snapshots = self.snapshots();
        let mut found = snapshots.iter().rev().filter(|kept| kept.number == number);
        let first = found.next()?;
        let own = if first.copy == self.copy { Some(first) } else { found.find(|kept| kept.copy == self.copy) };
        Some(own.map_or_else(|| (first.state.clone(), true), |own| (own.state.clone(), false)))
    }
}

/// A fingerprint of `state`: the same for the same state, so taking the editor state twice
/// answers the same value (`testing::check_context` compares them).
fn fingerprint(state: &EditorState) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    format!("{state:?}").hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Commands whose work is moving the editor (the selection, the current page, the view, the tool
/// and the edit modes): they keep their effect for an agent too.
pub(crate) fn moves_the_editor(command: &str) -> bool {
    ["select.", "view.", "tool.", "text.edit.", "vector.edit."].iter().any(|prefix| command.starts_with(prefix))
        || matches!(command, "page.select" | "page.go")
}

/// The parameters that name what a command acts on: layers, a parent, a page, a frame.
const NAMING: [&str; 8] = ["id", "ids", "parent", "page", "node", "target", "component", "frame"];

/// Whether an agent's command names what it acts on in its parameters.
pub(crate) fn names_target(params: &Value) -> bool {
    NAMING.iter().any(|key| params.get(*key).is_some_and(|value| !value.is_null()))
}

/// Where the inspect tool lists what a call may name.
const LISTED: &str = "pages' ids are in design_inspect's `pages`, layers' ids in a page's `layers` (design_inspect with `target: {\"pageId\": id}`)";

fn page_name(doc: &Document, page: NodeId) -> String {
    doc.node(page).map_or_else(|| page.to_string(), |node| format!("`{}` ({page})", node.name))
}

/// The page a call's part names: `{"pageId": id}` or `{"page": n}` (counted from 1).
pub(crate) fn part_page(doc: &Document, part: &Value) -> Result<NodeId, String> {
    let Some(fields) = part.as_object() else { return Err(format!("the part {part} is not `{{\"pageId\": id}}` or `{{\"page\": n}}`: {LISTED}")) };
    if fields.len() != 1 {
        return Err(format!("the part {part} names {} members: a part is one page, `{{\"pageId\": id}}` or `{{\"page\": n}}`", fields.len()));
    }
    let (key, value) = fields.iter().next().ok_or_else(|| format!("the part {part} names no page"))?;
    match key.as_str() {
        "pageId" => {
            let id = value
                .as_u64()
                .or_else(|| value.as_str().and_then(|text| text.trim().parse().ok()))
                .map(NodeId)
                .ok_or_else(|| format!("`pageId` is {value}: it is a page's id, a whole number: {LISTED}"))?;
            if doc.pages.contains(&id) {
                Ok(id)
            } else if doc.contains(id) {
                Err(format!("{id} is a layer, not a page: a part is a page; {LISTED}"))
            } else {
                Err(format!("the design has no page {id}: {LISTED}"))
            }
        }
        "page" => {
            let count = doc.pages.len();
            value
                .as_u64()
                .and_then(|n| n.checked_sub(1))
                .and_then(|index| usize::try_from(index).ok())
                .and_then(|index| doc.pages.get(index).copied())
                .ok_or_else(|| {
                    format!("`page` is {value}: pages are counted from 1 and the design has {count} page(s); or name it by id, `{{\"pageId\": id}}`")
                })
        }
        other => Err(format!("the part has `{other}`, which names no part of a design: a part is a page, `{{\"pageId\": id}}` or `{{\"page\": n}}`")),
    }
}

/// A layer an `on` item names.
fn on_item(doc: &Document, item: &Value) -> Result<NodeId, String> {
    let id = item
        .as_u64()
        .or_else(|| item.as_str().and_then(|text| text.trim().parse().ok()))
        .map(NodeId)
        .ok_or_else(|| format!("`on` holds {item}, which is not a layer id: a layer is named by its numeric id (`12`); {LISTED}"))?;
    if doc.pages.contains(&id) {
        return Err(format!(
            "`on` holds {id}, a page: a page is the call's part (the run's `target` or the step's `at.part`, `{{\"pageId\": {id}}}`), and `on` names layers on it"
        ));
    }
    if !doc.contains(id) {
        return Err(format!("the design has no layer {id}: {LISTED}"));
    }
    Ok(id)
}

/// The page layer `id` is on, if it is on one (library copies are on none).
fn page_of(doc: &Document, id: NodeId) -> Option<NodeId> {
    doc.page_of(id).filter(|page| doc.pages.contains(page))
}

impl DesignDocument {
    /// Whether the command running now runs for an agent: the host's dispatch says so (headless),
    /// or the document runs it as an agent's ([`DesignDocument::execute_as`]).
    pub(crate) fn is_agent(&self) -> bool {
        kition_kit::dispatch::role() == Some(kition_kit::dispatch::Role::Agent) || self.session.origin().kind == kition_kit::kdm::OriginKind::Agent
    }

    /// The editor's state, kept for [`DesignDocument::put_back`], as `{snapshot, page, selection}`.
    pub(crate) fn take_editor_state(&self) -> Value {
        let Some(state) = self.session.editor_state() else { return Value::Null };
        let (page, selection) = (state.page, state.selection.clone());
        let number = fingerprint(&state);
        Kept::push(&mut self.kept.snapshots(), Snapshot { number: number.clone(), copy: self.kept.copy, state });
        json!({ "snapshot": number, "page": page, "selection": selection })
    }

    /// Puts back what [`DesignDocument::take_editor_state`] answered; no edit. A state this copy
    /// of the design took drops what a run deleted since. A state another copy took (the design
    /// before its file was read again, or the design a planning copy was made of) is put back
    /// only when every active page, target and mode resolves here; else it is refused with the
    /// reason (`deleted active target 12`), and nothing changes.
    pub(crate) fn put_back(&mut self, saved: &Value) -> Result<(), String> {
        if saved.is_null() {
            return Ok(());
        }
        let number = saved.get("snapshot").and_then(Value::as_str).ok_or_else(|| format!("{saved} is not an editor state this kit answered"))?;
        let (state, other_copy) = self
            .kept
            .find(number)
            .ok_or_else(|| format!("editor state {number} is no longer kept (the kit keeps the last {KEPT}); take it again"))?;
        if other_copy {
            state.resolves_on(&self.ds()?.doc)?;
        }
        self.session.restore_editor_state(&state).map_err(crate::engine_error)
    }

    /// Shares `other`'s kept editor states: this document is `other`'s file read again, or a copy
    /// of it, and restores the states taken on it ([`DesignDocument::put_back`]).
    pub(crate) fn share_kept(&mut self, other: &Self) {
        self.kept.join(&other.kept);
    }

    /// Whether the editor's current state resolves on `fresh`, the design's file read again
    /// ([`EditorState::resolves_on`]).
    pub(crate) fn state_resolves_on(&self, fresh: &Self) -> Result<(), String> {
        match self.session.editor_state() {
            Some(state) => state.resolves_on(&fresh.ds()?.doc),
            None => Ok(()),
        }
    }

    /// Sets the page a call acts on (`part`) and the layers it acts on there (`on`), as the module
    /// docs say; refuses what the design lacks and `time`. No edit.
    pub(crate) fn set_call_context(&mut self, context: &CallContext) -> Result<(), String> {
        if context.time.is_some() {
            return Err("a design has no timeline: setting the current time (`at.time`) is not available in this kit".into());
        }
        let doc: Arc<Document> = self.ds()?.doc.clone();
        let part = context.part.as_ref().map(|part| part_page(&doc, part)).transpose()?;
        let ids = context.on.iter().map(|item| on_item(&doc, item)).collect::<Result<Vec<_>, _>>()?;
        let mut named: Option<(NodeId, NodeId)> = None;
        for id in &ids {
            let page = page_of(&doc, *id).ok_or_else(|| format!("layer {id} is on no page (a library copy): `on` names layers on a page"))?;
            if let Some(part) = part
                && part != page
            {
                return Err(format!(
                    "layer {id} is on page {}, and the call's part is page {}: give it with that page, or leave the part out (a layer's id names its page)",
                    page_name(&doc, page),
                    page_name(&doc, part)
                ));
            }
            match named {
                Some((first, other)) if other != page => {
                    return Err(format!(
                        "`on` names layers of two pages ({first} on page {}, {id} on page {}): the selection is on one page, so give them in one step per page, or pass them as the command's `ids` where it takes them",
                        page_name(&doc, other),
                        page_name(&doc, page)
                    ));
                }
                Some(_) => {}
                None => named = Some((*id, page)),
            }
        }
        let page = part.or(named.map(|(_, page)| page));
        let mut selection = Vec::new();
        for id in ids {
            if !selection.contains(&id) {
                selection.push(id);
            }
        }
        let selection = (!context.on.is_empty()).then_some(selection);
        self.session.set_call_context(page, selection).map_err(crate::engine_error)
    }

    /// The page an agent's command names: its `page` (a page id), else the page of the first
    /// layer its `ids`, `id`, `parent`, `node`, `target`, `component` or `frame` names.
    fn named_page(&self, params: &Value) -> Option<NodeId> {
        let doc = &self.ds().ok()?.doc;
        let as_id = |value: &Value| value.as_u64().or_else(|| value.as_str().and_then(|text| text.trim().parse().ok())).map(NodeId);
        if let Some(page) = params.get("page").and_then(as_id).filter(|page| doc.pages.contains(page)) {
            return Some(page);
        }
        NAMING.iter().filter(|key| **key != "page").find_map(|key| {
            let value = params.get(*key)?;
            let first = match value {
                Value::Array(items) => items.iter().find_map(as_id),
                other => as_id(other),
            }?;
            if doc.pages.contains(&first) { Some(first) } else { page_of(doc, first) }
        })
    }

    /// Runs an agent's command that names what it acts on (`run`) on the page it names, and puts
    /// the current page, the selection, the view, the modes and the tool back afterwards, also
    /// when it fails. Layers it made on the current page become the selection, as the command
    /// itself selects them, so the run's next step can act on them.
    pub(crate) fn bracketed(&mut self, params: &Value, run: impl FnOnce(&mut Self) -> Result<Value, String>) -> Result<Value, String> {
        let kept = self.session.editor_state();
        let before = self.ds().ok().map(|ds| ds.doc.clone());
        if let Some(page) = self.named_page(params)
            && self.ds().is_ok_and(|ds| ds.page != page)
        {
            self.session.set_call_context(Some(page), None).map_err(crate::engine_error)?;
        }
        let result = run(self);
        if let Some(kept) = kept {
            // What the call copied or cut stays on the clipboard for the run's next step: it is
            // what the call made, not where the editor was (C8-design-read: an agent's cut by id lost
            // the layer).
            let clipboard = self.session.clipboard.clone();
            // Fails only when the command closed the document, which leaves nothing to put back.
            let _ = self.session.restore_editor_state(&kept);
            self.session.clipboard = clipboard;
        }
        if let (Ok(answer), Some(before)) = (&result, before)
            && let Ok(ds) = self.ds()
        {
            let current = ds.page;
            let answered = answer.get("ids").and_then(Value::as_array).cloned().unwrap_or_default().into_iter().chain(answer.get("id").cloned());
            let made: Vec<NodeId> = answered
                .filter_map(|value| value.as_u64().map(NodeId))
                .filter(|id| !before.contains(*id) && ds.doc.contains(*id) && page_of(&ds.doc, *id) == Some(current) && !ds.doc.pages.contains(id))
                .collect();
            if !made.is_empty() {
                let _ = self.session.set_call_context(None, Some(made));
            }
        }
        result
    }
}
