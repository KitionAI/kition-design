//! `Document` to replica: a full write (a new replica) and a diff (the engine's command bridge)
//! that writes only what changed. A full write is a diff from nothing.
//!
//! 1. images: one object per content hash, its bytes a blob;
//! 2. layers: every new layer gets its object, written in full; under each parent whose children
//!    changed the longest start of the new order that keeps the old order stays and the rest are
//!    moved (or created) after it in order, since a tree only ever appends; every layer whose `Arc` changed
//!    gets the registers that differ; layers `after` no longer holds are deleted;
//! 3. styles, collections and their modes, variables (their values a map by mode), comment
//!    threads and their messages: objects keyed by the engine's ids, lists in `after`'s order;
//! 4. the libraries the document uses (its library links): one object per library id, in order;
//!    the records of the copies taken from them: one `LibraryCopy` object each, in order;
//! 5. the settings map gets the keys that changed (the store root's registers among them).
//!
//! The library store's layers are the nodes of `Design.libraryStore` (its roots: the store's
//! children); the store root is no object ([`crate::store`]).

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::sync::Arc;

use kition_design_doc::{Collection, Comment, Document, LibraryLink, Message, Node, NodeId, Style, Variable};
use kition_kit::kdm::{NodeRef, ObjectId, Tx};
use kition_kit::session::{BlobSink, write_text};
use serde_json::{Map, Value, json};

use crate::fields::{self, Fields};
use crate::ids::{CollectionIds, Ids, Thread};
use crate::list::sync_list;
use crate::store::{self, COPIES, STORE_PAGE, STORE_TREE};
use crate::{KdmResult, MAX_DEPTH, kdm_err, root_id};

/// The document-level keys of a serialized `Document` that are objects of their own, the tree's,
/// or the editing person's (the current page, the id counter): never settings.
pub(crate) const MODELLED: &[&str] = &["pages", "nodes", "next_id", "styles", "variables", "comments", "current_page", "dev_snapshots"];

/// The settings map: every key of the document's serde form that is not modelled otherwise.
pub(crate) fn settings(doc: &Document) -> KdmResult<Map<String, Value>> {
    let mut shell = Document::without_pages(doc.name.clone());
    shell.kind = doc.kind;
    shell.document_colors.clone_from(&doc.document_colors);
    // The library links and the copies' records are objects of their own (`Design.libraries`,
    // `Design.libraryCopies`); the store's id stays a setting.
    shell.libraries = Arc::new(kition_design_doc::Libraries { store: doc.libraries.store, ..Default::default() });
    shell.color_profile = doc.color_profile;
    shell.image_presets.clone_from(&doc.image_presets);
    shell.annotation_categories.clone_from(&doc.annotation_categories);
    shell.timer.clone_from(&doc.timer);
    shell.vote.clone_from(&doc.vote);
    shell.votes.clone_from(&doc.votes);
    let mut map = match serde_json::to_value(&shell).map_err(kdm_err)? {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    for key in MODELLED {
        map.remove(*key);
    }
    if let Some(n) = doc.library_store().and_then(|s| doc.node(s)) {
        map.insert(STORE_PAGE.to_owned(), store::store_page(n)?);
    }
    Ok(map)
}

/// Writes a whole document into a fresh replica's creating transaction.
pub fn write_document(tx: &mut Tx<'_>, doc: &Document, ids: &mut Ids, blobs: &mut dyn BlobSink) -> KdmResult<()> {
    tx.create_with_id("Design", &root_id(), Map::new())?;
    *ids = Ids::default();
    Writer { tx, ids, blobs, wrote: true }.write(None, doc).map(|_| ())
}

/// Writes only what differs between `before` (the document `ids` describe, as the replica holds
/// it) and `after`. Answers whether it wrote anything.
pub fn write_changes(tx: &mut Tx<'_>, before: &Document, after: &Document, ids: &mut Ids, blobs: &mut dyn BlobSink) -> KdmResult<bool> {
    Writer { tx, ids, blobs, wrote: false }.write(Some(before), after)
}

/// The roots of the layer tree in order: the pages, then any other parentless layer but the
/// library store's root, by id.
pub(crate) fn roots(doc: &Document) -> Vec<NodeId> {
    let store = doc.library_store();
    let mut out: Vec<NodeId> = doc.pages.iter().copied().filter(|p| doc.node(*p).is_some_and(|n| n.parent.is_none())).collect();
    let mut seen: BTreeSet<NodeId> = out.iter().copied().collect();
    for n in doc.nodes() {
        if n.parent.is_none() && Some(n.id) != store && seen.insert(n.id) {
            out.push(n.id);
        }
    }
    out
}

/// The library store's layers (its root left out).
fn store_layers(doc: &Document) -> BTreeSet<NodeId> {
    doc.library_store().map(|s| doc.subtree(s).into_iter().filter(|id| *id != s).collect()).unwrap_or_default()
}

/// The children of the library store's root (`Design.libraryStore`'s roots).
fn store_children(doc: &Document) -> &[NodeId] {
    doc.library_store().and_then(|s| doc.node(s)).map_or(&[][..], |n| &n.children[..])
}

fn mime_of(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"\x89PNG") {
        "image/png"
    } else if bytes.starts_with(&[0xFF, 0xD8]) {
        "image/jpeg"
    } else if bytes.starts_with(b"GIF8") {
        "image/gif"
    } else if bytes.len() > 12 && bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        "image/webp"
    } else if bytes.starts_with(b"<svg") || bytes.starts_with(b"<?xml") {
        "image/svg+xml"
    } else {
        "application/octet-stream"
    }
}

fn snapshot_value(doc: &Document, id: NodeId) -> KdmResult<Option<Value>> {
    doc.dev_snapshots.get(&id).map(|s| serde_json::to_value(s).map_err(kdm_err)).transpose()
}

fn fields_map(fields: Fields) -> Map<String, Value> {
    fields.into_iter().filter(|(_, v)| !v.is_null()).map(|(k, v)| (k.to_owned(), v)).collect()
}

struct Writer<'w, 'tx> {
    tx: &'w mut Tx<'tx>,
    ids: &'w mut Ids,
    blobs: &'w mut dyn BlobSink,
    wrote: bool,
}

impl Writer<'_, '_> {
    fn write(mut self, before: Option<&Document>, after: &Document) -> KdmResult<bool> {
        let root = root_id();
        self.images(before, after)?;
        self.nodes(before, after)?;
        if before.is_none_or(|b| b.styles != after.styles) {
            self.styles(before, after)?;
        }
        if before.is_none_or(|b| b.variables != after.variables) {
            self.collections(before, after)?;
            self.variables(before, after)?;
        }
        if before.is_none_or(|b| b.comments != after.comments) {
            self.comments(before, after)?;
        }
        if before.is_none_or(|b| b.libraries.enabled != after.libraries.enabled) {
            self.libraries(before, after)?;
        }
        if before.is_none_or(|b| !same_records(&b.libraries, &after.libraries)) {
            self.copies(before, after)?;
        }
        let old = before.map(settings).transpose()?.unwrap_or_default();
        let new = settings(after)?;
        for (key, value) in &new {
            if old.get(key) != Some(value) {
                self.tx.map_set(&root, "settings", key, value.clone())?;
                self.wrote = true;
            }
        }
        for key in old.keys().filter(|k| !new.contains_key(*k)) {
            self.tx.map_delete(&root, "settings", key)?;
            self.wrote = true;
        }
        Ok(self.wrote)
    }

    fn set_fields(&mut self, object: &ObjectId, old: Option<Fields>, new: Fields) -> KdmResult<()> {
        let old: HashMap<&str, Value> = old.unwrap_or_default().into_iter().collect();
        for (field, value) in new {
            if old.get(field).unwrap_or(&Value::Null) != &value {
                self.tx.set(object, field, value)?;
                self.wrote = true;
            }
        }
        Ok(())
    }

    fn images(&mut self, before: Option<&Document>, after: &Document) -> KdmResult<()> {
        if before.is_some_and(|b| b.images.len() == after.images.len() && b.images.keys().eq(after.images.keys())) {
            return Ok(());
        }
        let root = root_id();
        let mut order = Vec::with_capacity(after.images.len());
        for (hash, img) in &after.images {
            let object = match self.ids.images.get(hash) {
                Some(o) => o.clone(),
                None => {
                    let blob = self.blobs.put(&img.bytes, mime_of(&img.bytes));
                    let blob = serde_json::to_value(&blob).map_err(kdm_err)?;
                    let mut fields = Map::new();
                    fields.insert("key".into(), json!(hash));
                    fields.insert("blob".into(), blob);
                    fields.insert("size".into(), json!({ "width": img.width, "height": img.height }));
                    let o = self.tx.create("Image", fields)?;
                    self.ids.images.insert(hash.clone(), o.clone());
                    self.wrote = true;
                    o
                }
            };
            order.push(object);
        }
        let gone: Vec<String> = self.ids.images.keys().filter(|h| !after.images.contains_key(*h)).cloned().collect();
        self.wrote |= sync_list(self.tx, &root, "images", &order)?;
        for hash in gone {
            if let Some(o) = self.ids.images.remove(&hash)
                && self.tx.exists(&o)
            {
                self.tx.delete(&o)?;
                self.wrote = true;
            }
        }
        Ok(())
    }

    fn node_object(&mut self, after: &Document, n: &Node) -> KdmResult<ObjectId> {
        let mut fields = fields_map(fields::node_fields(n, snapshot_value(after, n.id)?.as_ref())?);
        fields.insert("key".into(), json!(n.id.0));
        self.wrote = true;
        self.tx.create("Node", fields)
    }

    fn nodes(&mut self, before: Option<&Document>, after: &Document) -> KdmResult<()> {
        // The store's root is no object: its registers are a setting.
        let store = after.library_store();
        let store_before = before.and_then(Document::library_store);
        // Layers whose `Arc` changed (or are new), and those `after` no longer holds.
        let mut changed: Vec<NodeId> = after
            .ids()
            .filter(|id| Some(*id) != store)
            .filter(|id| match (before.and_then(|b| b.node_arc(*id)), after.node_arc(*id)) {
                (Some(a), Some(b)) => !Arc::ptr_eq(a, b) || !self.ids.nodes.contains_key(id),
                _ => true,
            })
            .collect();
        let mut gone: Vec<NodeId> = self.ids.nodes.keys().copied().filter(|id| !after.contains(*id) || Some(*id) == store).collect();
        // Tree removal requires children to have left before their parent.
        gone.sort_by_key(|id| (std::cmp::Reverse(before.map_or(0, |doc| depth(doc, *id))), *id));
        let roots_before = before.map(roots).unwrap_or_default();
        let roots_after = roots(after);
        let kids_before = before.filter(|_| store_before == store).map_or(&[][..], store_children);
        let kids_after = store_children(after);
        if changed.is_empty()
            && gone.is_empty()
            && roots_before == roots_after
            && kids_before == kids_after
            && store_before == store
            && !self.snapshots_changed(before, after)
        {
            return Ok(());
        }
        // A layer that moved between the store and the pages (one tree cannot move a node into
        // another) is made again in its new tree, with everything under it.
        let in_store = store_layers(after);
        let mut crossed: Vec<NodeId> =
            self.ids.nodes.keys().copied().filter(|id| after.contains(*id) && in_store.contains(id) != self.ids.store.contains(id)).collect();
        crossed.sort_by_key(|id| (std::cmp::Reverse(before.map_or(0, |doc| depth(doc, *id))), *id));
        for id in crossed {
            if let Some(object) = self.ids.node_object(id).cloned()
                && self.tx.exists(&object)
            {
                self.tx.delete(&object)?;
                self.wrote = true;
            }
            self.ids.remove_node(id);
            if !changed.contains(&id) {
                changed.push(id);
            }
        }
        // Parents first, so a new layer's parent has its tree node.
        let mut parents: Vec<(usize, NodeId)> = changed.iter().map(|id| (depth(after, *id), *id)).collect();
        parents.sort();
        let mut created: BTreeSet<NodeId> = BTreeSet::new();
        self.reconcile(before, after, None, &roots_before, &roots_after, &mut created)?;
        if let Some(s) = store {
            self.reconcile(before, after, Some(s), kids_before, kids_after, &mut created)?;
        }
        for (d, parent) in &parents {
            if *d > MAX_DEPTH {
                return Err(kdm_err(format!("layer {parent} nests deeper than {MAX_DEPTH}")));
            }
            let Some(n) = after.node(*parent) else { continue };
            if !self.ids.nodes.contains_key(parent) {
                // A layer under a parent that is not in the tree (a broken document): left out.
                continue;
            }
            let old = before.and_then(|b| b.node(*parent)).map_or(&[][..], |b| &b.children[..]);
            if old != n.children.as_slice() || created.contains(parent) {
                self.reconcile(before, after, Some(*parent), old, &n.children, &mut created)?;
            }
        }
        // Registers of the layers that existed and changed.
        for id in &changed {
            if created.contains(id) {
                continue;
            }
            let (Some(object), Some(n)) = (self.ids.node_object(*id).cloned(), after.node(*id)) else { continue };
            let old = match before.and_then(|b| b.node(*id)) {
                Some(b) => Some(fields::node_fields(b, before.and_then(|b| snapshot_value(b, *id).transpose()).transpose()?.as_ref())?),
                None => None,
            };
            let new = fields::node_fields(n, snapshot_value(after, *id)?.as_ref())?;
            self.set_fields(&object, old, new)?;
        }
        // Ready-for-dev snapshots taken or dropped while the layer stayed the same.
        if let Some(b) = before {
            let keys: BTreeSet<NodeId> = b.dev_snapshots.keys().chain(after.dev_snapshots.keys()).copied().collect();
            for id in keys {
                let same = match (b.dev_snapshots.get(&id), after.dev_snapshots.get(&id)) {
                    (Some(x), Some(y)) => Arc::ptr_eq(x, y) || x == y,
                    (None, None) => true,
                    _ => false,
                };
                if same || created.contains(&id) || changed.contains(&id) {
                    continue;
                }
                let (Some(object), Some(n)) = (self.ids.node_object(id).cloned(), after.node(id)) else { continue };
                let old = b.node(id).map(|old| fields::node_fields(old, snapshot_value(b, id).ok().flatten().as_ref())).transpose()?;
                let new = fields::node_fields(n, snapshot_value(after, id)?.as_ref())?;
                self.set_fields(&object, old, new)?;
            }
        }
        for id in gone {
            if let Some(object) = self.ids.node_object(id).cloned()
                && self.tx.exists(&object)
            {
                self.tx.delete(&object)?;
                self.wrote = true;
            }
            self.ids.remove_node(id);
        }
        Ok(())
    }

    fn snapshots_changed(&self, before: Option<&Document>, after: &Document) -> bool {
        before.is_some_and(|b| b.dev_snapshots != after.dev_snapshots)
    }

    /// Makes `parent`'s tree children `desired`. A tree move puts a node last among its new
    /// siblings, so the children that stay are exactly the longest start of `desired` that is a
    /// subsequence of the children already there (in their order); every other child is moved
    /// (or created) after them, in order. Bringing the bottom layer to the front moves that layer
    /// alone, and a concurrent reorder of other layers keeps its moves.
    fn reconcile(
        &mut self,
        before: Option<&Document>,
        after: &Document,
        parent: Option<NodeId>,
        old: &[NodeId],
        desired: &[NodeId],
        created: &mut BTreeSet<NodeId>,
    ) -> KdmResult<()> {
        let root = root_id();
        let staying: Vec<NodeId> = old
            .iter()
            .copied()
            .filter(|c| {
                self.ids.nodes.contains_key(c) && after.node(*c).is_some_and(|n| n.parent == parent) && before.is_some_and(|b| b.contains(*c))
            })
            .collect();
        let keep = kept_start(&staying, desired);
        if keep == desired.len() && keep == staying.len() && !parent.is_some_and(|p| created.contains(&p)) {
            return Ok(());
        }
        // The store's children are the roots of its own tree.
        let store = after.library_store();
        let (field, parent_ref): (&str, Option<NodeRef>) = match parent {
            None => ("nodes", None),
            Some(p) if Some(p) == store => (STORE_TREE, None),
            Some(p) => (
                if self.ids.store.contains(&p) { STORE_TREE } else { "nodes" },
                Some(self.ids.nodes.get(&p).map(|(_, r)| r.clone()).ok_or_else(|| kdm_err(format!("layer {p} has no tree node")))?),
            ),
        };
        let mut seen = BTreeSet::new();
        for child in desired.iter().skip(keep) {
            if !seen.insert(*child) {
                continue;
            }
            let Some(n) = after.node(*child) else { continue };
            match self.ids.nodes.get(child).map(|(_, r)| r.clone()) {
                Some(node) => {
                    self.tx.tree_move(&root, field, &node, parent_ref.as_ref())?;
                    self.wrote = true;
                }
                None => {
                    let object = self.node_object(after, n)?;
                    let node = self.tx.tree_create(&root, field, parent_ref.as_ref(), &object)?;
                    self.ids.insert_node(*child, object, node, field == STORE_TREE);
                    created.insert(*child);
                }
            }
        }
        Ok(())
    }

    /// The library links, by library id (the first link of an id wins).
    fn libraries(&mut self, before: Option<&Document>, after: &Document) -> KdmResult<()> {
        let root = root_id();
        let old: HashMap<&str, &LibraryLink> = before.map(|b| b.libraries.enabled.iter().map(|l| (l.id.as_str(), l)).collect()).unwrap_or_default();
        let mut order = Vec::with_capacity(after.libraries.enabled.len());
        let mut keep: BTreeSet<String> = BTreeSet::new();
        for link in &after.libraries.enabled {
            if !keep.insert(link.id.clone()) {
                continue;
            }
            let object = match self.ids.libraries.get(&link.id).cloned() {
                Some(o) => {
                    let prev = old.get(link.id.as_str()).map(|o| fields::library_fields(o));
                    self.set_fields(&o, prev, fields::library_fields(link))?;
                    o
                }
                None => {
                    let mut fields = fields_map(fields::library_fields(link));
                    fields.insert("key".into(), json!(link.id));
                    let o = self.tx.create("Library", fields)?;
                    self.ids.libraries.insert(link.id.clone(), o.clone());
                    self.wrote = true;
                    o
                }
            };
            order.push(object);
        }
        self.wrote |= sync_list(self.tx, &root, "libraries", &order)?;
        let gone: Vec<String> = self.ids.libraries.keys().filter(|k| !keep.contains(*k)).cloned().collect();
        for key in gone {
            if let Some(o) = self.ids.libraries.remove(&key)
                && self.tx.exists(&o)
            {
                self.tx.delete(&o)?;
                self.wrote = true;
            }
        }
        Ok(())
    }

    /// The records of the copies taken from libraries, by kind and local id (the first wins).
    fn copies(&mut self, before: Option<&Document>, after: &Document) -> KdmResult<()> {
        let root = root_id();
        let old: HashMap<(&str, String), Value> =
            before.map(|b| store::rows(&b.libraries)).transpose()?.unwrap_or_default().into_iter().map(|r| ((r.kind, r.local), r.record)).collect();
        let rows = store::rows(&after.libraries)?;
        let mut order = Vec::with_capacity(rows.len());
        let mut keep: BTreeSet<(String, String)> = BTreeSet::new();
        for row in rows {
            let id = (row.kind.to_owned(), row.local.clone());
            if !keep.insert(id.clone()) {
                continue;
            }
            let object = match self.ids.copies.get(&id).cloned() {
                Some(o) => {
                    if old.get(&(row.kind, row.local)) != Some(&row.record) {
                        self.tx.set(&o, "record", row.record)?;
                        self.wrote = true;
                    }
                    o
                }
                None => {
                    let mut fields = Map::new();
                    fields.insert("kind".into(), json!(row.kind));
                    fields.insert("local".into(), json!(row.local));
                    fields.insert("record".into(), row.record);
                    let o = self.tx.create("LibraryCopy", fields)?;
                    self.ids.copies.insert(id, o.clone());
                    self.wrote = true;
                    o
                }
            };
            order.push(object);
        }
        self.wrote |= sync_list(self.tx, &root, COPIES, &order)?;
        let gone: Vec<(String, String)> = self.ids.copies.keys().filter(|k| !keep.contains(*k)).cloned().collect();
        for key in gone {
            if let Some(o) = self.ids.copies.remove(&key)
                && self.tx.exists(&o)
            {
                self.tx.delete(&o)?;
                self.wrote = true;
            }
        }
        Ok(())
    }

    fn styles(&mut self, before: Option<&Document>, after: &Document) -> KdmResult<()> {
        let root = root_id();
        let old: HashMap<&str, &Style> = before.map(|b| b.styles.iter().map(|s| (s.id.as_str(), s)).collect()).unwrap_or_default();
        let mut order = Vec::with_capacity(after.styles.len());
        let mut keep: BTreeSet<String> = BTreeSet::new();
        for s in &after.styles {
            if !keep.insert(s.id.clone()) {
                continue;
            }
            let object = match self.ids.styles.get(&s.id).cloned() {
                Some(o) => {
                    if old.get(s.id.as_str()).is_none_or(|o| *o != s) {
                        let prev = old.get(s.id.as_str()).map(|o| fields::style_fields(o)).transpose()?;
                        self.set_fields(&o, prev, fields::style_fields(s)?)?;
                    }
                    o
                }
                None => {
                    let mut fields = fields_map(fields::style_fields(s)?);
                    fields.insert("key".into(), json!(s.id));
                    let o = self.tx.create("Style", fields)?;
                    self.ids.styles.insert(s.id.clone(), o.clone());
                    self.wrote = true;
                    o
                }
            };
            order.push(object);
        }
        self.wrote |= sync_list(self.tx, &root, "styles", &order)?;
        let gone: Vec<String> = self.ids.styles.keys().filter(|k| !keep.contains(*k)).cloned().collect();
        for key in gone {
            if let Some(o) = self.ids.styles.remove(&key)
                && self.tx.exists(&o)
            {
                self.tx.delete(&o)?;
                self.wrote = true;
            }
        }
        Ok(())
    }

    fn collections(&mut self, before: Option<&Document>, after: &Document) -> KdmResult<()> {
        let root = root_id();
        let old: HashMap<&str, &Collection> =
            before.map(|b| b.variables.collections.iter().map(|c| (c.id.as_str(), c)).collect()).unwrap_or_default();
        let mut order = Vec::new();
        let mut keep = BTreeSet::new();
        for c in &after.variables.collections {
            if !keep.insert(c.id.clone()) {
                continue;
            }
            let prev = old.get(c.id.as_str()).copied();
            let mut entry = match self.ids.collections.get(&c.id).cloned() {
                Some(e) => {
                    if prev.is_none_or(|p| p != c) {
                        let fields = prev.map(fields::collection_fields).transpose()?;
                        self.set_fields(&e.object, fields, fields::collection_fields(c)?)?;
                    }
                    e
                }
                None => {
                    let mut fields = fields_map(fields::collection_fields(c)?);
                    fields.insert("key".into(), json!(c.id));
                    let object = self.tx.create("Collection", fields)?;
                    self.wrote = true;
                    CollectionIds { object, modes: HashMap::new() }
                }
            };
            if prev.is_none_or(|p| p.modes != c.modes) {
                let old_modes: HashMap<&str, &str> =
                    prev.map(|p| p.modes.iter().map(|m| (m.id.as_str(), m.name.as_str())).collect()).unwrap_or_default();
                let mut mode_order = Vec::new();
                let mut mode_keep = BTreeSet::new();
                for m in &c.modes {
                    if !mode_keep.insert(m.id.clone()) {
                        continue;
                    }
                    let o = match entry.modes.get(&m.id).cloned() {
                        Some(o) => {
                            if old_modes.get(m.id.as_str()) != Some(&m.name.as_str()) {
                                self.tx.set(&o, "name", json!(m.name))?;
                                self.wrote = true;
                            }
                            o
                        }
                        None => {
                            let mut fields = Map::new();
                            fields.insert("key".into(), json!(m.id));
                            fields.insert("name".into(), json!(m.name));
                            let o = self.tx.create("Mode", fields)?;
                            entry.modes.insert(m.id.clone(), o.clone());
                            self.wrote = true;
                            o
                        }
                    };
                    mode_order.push(o);
                }
                self.wrote |= sync_list(self.tx, &entry.object, "modes", &mode_order)?;
                let gone: Vec<String> = entry.modes.keys().filter(|k| !mode_keep.contains(*k)).cloned().collect();
                for key in gone {
                    if let Some(o) = entry.modes.remove(&key)
                        && self.tx.exists(&o)
                    {
                        self.tx.delete(&o)?;
                        self.wrote = true;
                    }
                }
            }
            order.push(entry.object.clone());
            self.ids.collections.insert(c.id.clone(), entry);
        }
        self.wrote |= sync_list(self.tx, &root, "collections", &order)?;
        let gone: Vec<String> = self.ids.collections.keys().filter(|k| !keep.contains(*k)).cloned().collect();
        for key in gone {
            if let Some(e) = self.ids.collections.remove(&key) {
                for o in e.modes.values().chain(std::iter::once(&e.object)) {
                    if self.tx.exists(o) {
                        self.tx.delete(o)?;
                        self.wrote = true;
                    }
                }
            }
        }
        Ok(())
    }

    fn variables(&mut self, before: Option<&Document>, after: &Document) -> KdmResult<()> {
        let root = root_id();
        let old: HashMap<&str, &Variable> = before.map(|b| b.variables.variables.iter().map(|v| (v.id.as_str(), v)).collect()).unwrap_or_default();
        let mut order = Vec::new();
        let mut keep = BTreeSet::new();
        for v in &after.variables.variables {
            if !keep.insert(v.id.clone()) {
                continue;
            }
            let prev = old.get(v.id.as_str()).copied();
            let (object, old_values) = match self.ids.variables.get(&v.id).cloned() {
                Some(o) => {
                    if prev.is_none_or(|p| p == v) {
                        order.push(o);
                        continue;
                    }
                    let fields = prev.map(fields::variable_fields).transpose()?;
                    self.set_fields(&o, fields, fields::variable_fields(v)?)?;
                    (o, prev.map(fields::variable_values).transpose()?.unwrap_or_default())
                }
                None => {
                    let mut fields = fields_map(fields::variable_fields(v)?);
                    fields.insert("key".into(), json!(v.id));
                    let o = self.tx.create("Variable", fields)?;
                    self.ids.variables.insert(v.id.clone(), o.clone());
                    self.wrote = true;
                    (o, BTreeMap::new())
                }
            };
            let new_values = fields::variable_values(v)?;
            for (mode, value) in &new_values {
                if old_values.get(mode) != Some(value) {
                    self.tx.map_set(&object, "values", mode, value.clone())?;
                    self.wrote = true;
                }
            }
            for mode in old_values.keys().filter(|m| !new_values.contains_key(*m)) {
                self.tx.map_delete(&object, "values", mode)?;
                self.wrote = true;
            }
            order.push(object);
        }
        self.wrote |= sync_list(self.tx, &root, "variables", &order)?;
        let gone: Vec<String> = self.ids.variables.keys().filter(|k| !keep.contains(*k)).cloned().collect();
        for key in gone {
            if let Some(o) = self.ids.variables.remove(&key)
                && self.tx.exists(&o)
            {
                self.tx.delete(&o)?;
                self.wrote = true;
            }
        }
        Ok(())
    }

    /// The object a thread is anchored to: its anchor layer's, else its page's, else the root.
    fn anchor_object(&self, c: &Comment) -> String {
        c.anchor
            .and_then(|a| self.ids.node_object(a))
            .or_else(|| self.ids.node_object(c.page))
            .map_or_else(|| root_id().as_str().to_owned(), |o| o.as_str().to_owned())
    }

    fn message(&mut self, m: &Message) -> KdmResult<ObjectId> {
        let o = self.tx.create("Comment", fields_map(fields::message_fields(m)?))?;
        if !m.text.is_empty() {
            self.tx.text_insert(&o, "body", 0, &m.text)?;
        }
        self.wrote = true;
        Ok(o)
    }

    fn comments(&mut self, before: Option<&Document>, after: &Document) -> KdmResult<()> {
        let root = root_id();
        let old: HashMap<u64, &Comment> = before.map(|b| b.comments.iter().map(|c| (c.id, c)).collect()).unwrap_or_default();
        let mut order = Vec::new();
        let mut keep = BTreeSet::new();
        for c in &after.comments {
            if !keep.insert(c.id) {
                continue;
            }
            let prev = old.get(&c.id).copied();
            let anchor = fields::thread_anchor(c, &self.anchor_object(c));
            let mut thread = match self.ids.threads.get(&c.id).cloned() {
                Some(t) => {
                    if prev.is_none_or(|p| p == c) {
                        order.push(t.object);
                        continue;
                    }
                    if prev.map(|p| fields::thread_anchor(p, &self.anchor_object(p))) != Some(anchor.clone()) {
                        self.tx.set(&t.object, "anchor", anchor)?;
                        self.wrote = true;
                    }
                    if prev.is_none_or(|p| p.resolved != c.resolved) {
                        self.tx.set(&t.object, "resolved", json!(c.resolved))?;
                        self.wrote = true;
                    }
                    t
                }
                None => {
                    let mut fields = Map::new();
                    fields.insert("anchor".into(), anchor);
                    fields.insert("key".into(), json!(c.id));
                    fields.insert("resolved".into(), json!(c.resolved));
                    let object = self.tx.create("CommentThread", fields)?;
                    self.wrote = true;
                    Thread { object, messages: Vec::new() }
                }
            };
            // A message keeps its object by its message id (F19; among messages of one id, by
            // order), so deleting a reply deletes that reply's object and a concurrent edit of
            // another reply is kept, whoever wrote them and when.
            let old_messages: Vec<&Message> = prev.map(|p| p.messages().collect()).unwrap_or_default();
            let mut by_id: HashMap<u64, VecDeque<usize>> = HashMap::new();
            for (i, was) in old_messages.iter().enumerate().take(thread.messages.len()) {
                by_id.entry(was.id).or_default().push_back(i);
            }
            let mut used = vec![false; thread.messages.len()];
            let mut objects = Vec::new();
            for m in c.messages() {
                let matched = by_id.get_mut(&m.id).and_then(VecDeque::pop_front);
                match matched.and_then(|i| Some((i, thread.messages.get(i).cloned()?, old_messages.get(i).copied()?))) {
                    Some((i, o, was)) => {
                        if was != m {
                            self.set_fields(&o, Some(fields::message_fields(was)?), fields::message_fields(m)?)?;
                            if was.text != m.text {
                                write_text(self.tx, &o, "body", &was.text, &m.text).map_err(|e| kdm_err(e.to_string()))?;
                                self.wrote = true;
                            }
                        }
                        if let Some(u) = used.get_mut(i) {
                            *u = true;
                        }
                        objects.push(o);
                    }
                    None => objects.push(self.message(m)?),
                }
            }
            for (o, _) in thread.messages.iter().zip(&used).filter(|(_, used)| !**used) {
                if self.tx.exists(o) {
                    self.tx.delete(o)?;
                    self.wrote = true;
                }
            }
            self.wrote |= sync_list(self.tx, &thread.object, "comments", &objects)?;
            thread.messages = objects;
            order.push(thread.object.clone());
            self.ids.threads.insert(c.id, thread);
        }
        self.wrote |= sync_list(self.tx, &root, "comments", &order)?;
        let gone: Vec<u64> = self.ids.threads.keys().filter(|k| !keep.contains(*k)).copied().collect();
        for key in gone {
            if let Some(t) = self.ids.threads.remove(&key) {
                for o in t.messages.iter().chain(std::iter::once(&t.object)) {
                    if self.tx.exists(o) {
                        self.tx.delete(o)?;
                        self.wrote = true;
                    }
                }
            }
        }
        Ok(())
    }
}

/// How many of `desired`'s first entries appear in `staying` in that order (greedy matching
/// finds the longest such start), each once.
fn kept_start(staying: &[NodeId], desired: &[NodeId]) -> usize {
    let mut from = 0;
    let mut seen = BTreeSet::new();
    for (i, want) in desired.iter().enumerate() {
        match staying.get(from..).and_then(|rest| rest.iter().position(|c| c == want)) {
            Some(at) if seen.insert(*want) => from += at + 1,
            _ => return i,
        }
    }
    desired.len()
}

/// Whether two documents hold the same copy records.
fn same_records(a: &kition_design_doc::Libraries, b: &kition_design_doc::Libraries) -> bool {
    a.components == b.components && a.styles == b.styles && a.collections == b.collections && a.variables == b.variables
}

/// How many ancestors `id` has (0 for a root), at most [`MAX_DEPTH`] + 1.
fn depth(doc: &Document, id: NodeId) -> usize {
    let mut d = 0;
    let mut at = doc.node(id).and_then(|n| n.parent);
    while let Some(p) = at {
        d += 1;
        if d > MAX_DEPTH {
            break;
        }
        at = doc.node(p).and_then(|n| n.parent);
    }
    d
}
