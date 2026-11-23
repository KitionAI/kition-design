//! Replica to `Document`: the view the engine edits.
//!
//! [`read_with`] reuses a previous view: a layer whose object did not change and whose place in
//! the tree is the same keeps its `Arc` (and the caches the renderer hangs on it); one whose
//! object did not change but moved is the same node with a new parent or children; only changed
//! objects' registers are read (a layer changed when its object or one of its paint and effect
//! objects did). Images the previous view holds keep their bytes. Who has read each comment
//! thread is not replicated: the previous view's read state is kept.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use kition_design_doc::{Comment, DevSnapshot, Document, ImageData, LibraryLink, Node, NodeId, NodeKind, Style, Variable, Variables};
use kition_kit::kdm::{KdmError, ObjectId, Replica};
use kition_kit::session::BlobSource;
use serde_json::{Map, Value, json};

use crate::fields::{self, NODE_REGISTERS};
use crate::ids::{CollectionIds, Ids, Thread};
use crate::items::{self, BOUND, Item, LISTS};
use crate::store::{self, COPIES, STORE_PAGE, STORE_TREE};
use crate::text;
use crate::write::VOTES;
use crate::{KdmResult, MAX_DEPTH, kdm_err, root_id};

/// A previous view and its ids, and which objects changed since it was read.
pub struct Reuse<'a> {
    pub view: &'a Document,
    pub ids: &'a Ids,
    pub changed: &'a BTreeSet<ObjectId>,
}

/// Reads the whole document.
pub fn read_document(replica: &Replica, blobs: &dyn BlobSource) -> KdmResult<(Document, Ids)> {
    read_with(replica, blobs, None)
}

fn registers<'f>(replica: &Replica, object: &ObjectId, names: &[&'f str]) -> KdmResult<BTreeMap<&'f str, Value>> {
    names.iter().map(|name| Ok((*name, replica.get(object, name)?))).collect()
}

/// `doc` without what the replica does not hold (the current page, the published library, who has
/// read the comments) and with its texts in the one form the replica gives them, for comparing a
/// view with the engine's document.
pub(crate) fn comparable(doc: &Document) -> Document {
    let mut d = doc.clone();
    d.current_page = None;
    d.published = None;
    for c in &mut d.comments {
        c.read_by = None;
    }
    let texts: Vec<Arc<Node>> = d
        .nodes()
        .filter_map(|n| match &n.kind {
            NodeKind::Text(t) => {
                let mut m = n.clone();
                m.kind = NodeKind::Text(text::canonical(t));
                Some(Arc::new(m))
            }
            _ => None,
        })
        .collect();
    d.put_nodes_raw(texts);
    d
}

/// A layer's paint and effect objects and items, list by list ([`LISTS`]): the live objects in
/// order, each once.
fn read_items(replica: &Replica, object: &ObjectId) -> KdmResult<[Vec<(ObjectId, Item)>; 3]> {
    let mut out: [Vec<(ObjectId, Item)>; 3] = Default::default();
    for (slot, (field, _, value_field)) in LISTS.iter().enumerate() {
        let mut seen = BTreeSet::new();
        for o in replica.list(object, field)? {
            if !replica.exists(&o) || !seen.insert(o.clone()) {
                continue;
            }
            let value = replica.get(&o, value_field)?;
            let bound: BTreeMap<String, Value> = replica.map_entries(&o, BOUND)?.into_iter().collect();
            if let Some(list) = out.get_mut(slot) {
                list.push((o, Item { value, bound }));
            }
        }
    }
    Ok(out)
}

/// A layer from its object: its registers, its paints and effects and its text. Answers the
/// layer, its ready-for-dev snapshot and its item objects (those of a list that did not read are
/// left out).
pub(crate) fn read_node(
    replica: &Replica,
    object: &ObjectId,
    key: NodeId,
    parent: Option<NodeId>,
    kids: Vec<NodeId>,
) -> KdmResult<(Node, Option<Value>, [Vec<ObjectId>; 3])> {
    let mut regs = registers(replica, object, NODE_REGISTERS)?;
    let lists = read_items(replica, object)?;
    for ((field, _, _), list) in LISTS.iter().zip(&lists) {
        if !list.is_empty() {
            regs.insert(*field, Value::Array(list.iter().map(|(_, item)| item.value.clone()).collect()));
        }
    }
    let (mut node, snapshot, dropped) = fields::node_from_lenient(key, &regs, parent, kids)?;
    let mut objects: [Vec<ObjectId>; 3] = Default::default();
    for (slot, ((field, _, _), list)) in LISTS.iter().zip(lists).enumerate() {
        if list.is_empty() || dropped.contains(field) {
            continue;
        }
        let read: Vec<Item> = list.iter().map(|(_, item)| item.clone()).collect();
        items::bind(&mut node, field, &read);
        if let Some(slot) = objects.get_mut(slot) {
            *slot = list.into_iter().map(|(o, _)| o).collect();
        }
    }
    if let NodeKind::Text(t) = &mut node.kind {
        let runs = replica.text_delta(object, "text")?;
        text::apply_runs(t, &runs);
    }
    Ok((node, snapshot, objects))
}

/// Reads the document, reusing `reuse`'s view where its objects did not change.
pub fn read_with(replica: &Replica, blobs: &dyn BlobSource, reuse: Option<Reuse<'_>>) -> KdmResult<(Document, Ids)> {
    let root = root_id();
    let mut ids = Ids::default();

    // Settings: the document's own values (the ballots gathered from their keys).
    let mut shell: Map<String, Value> = Map::new();
    let mut ballots: Map<String, Value> = Map::new();
    for (key, value) in replica.map_entries(&root, "settings")? {
        match key.strip_prefix(VOTES) {
            Some(voter) => {
                ballots.insert(voter.to_owned(), value);
            }
            None => {
                shell.insert(key, value);
            }
        }
    }
    if !ballots.is_empty() {
        shell.insert("votes".into(), Value::Object(ballots));
    }
    shell.entry("name").or_insert_with(|| json!("Untitled"));
    shell.insert("pages".into(), json!([]));
    shell.insert("nodes".into(), json!({}));
    shell.insert("next_id".into(), json!(1));
    let store_page = shell.remove(STORE_PAGE);
    let mut doc = settings_from(shell)?;

    // The layer tree, then the library store's. A layer changed when its object or one of its
    // paint and effect objects did.
    let items_changed: BTreeSet<NodeId> =
        reuse.as_ref().map(|r| r.changed.iter().filter_map(|o| r.ids.item_owner.get(o).copied()).collect()).unwrap_or_default();
    let changed = |o: &ObjectId| reuse.as_ref().is_none_or(|r| r.changed.contains(o) || r.ids.node_of(o).is_some_and(|k| items_changed.contains(&k)));
    let entries = replica.tree(&root, "nodes")?;
    let store_entries = replica.tree(&root, STORE_TREE)?;
    let mut parent_of: HashMap<NodeId, Option<NodeId>> = HashMap::with_capacity(entries.len() + store_entries.len());
    let mut children: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    let mut roots: Vec<NodeId> = Vec::new();
    let mut order: Vec<(NodeId, ObjectId)> = Vec::with_capacity(entries.len() + store_entries.len());
    // The store's root: the setting's id; one the repair gives (V10) when its layers outlive the
    // setting.
    let has_store_layers = store_entries.iter().any(|e| replica.exists(&e.object));
    let store = match doc.libraries.store {
        Some(s) => Some(s),
        None if has_store_layers || store_page.is_some() => Some(store::repair_store_key()),
        None => None,
    };
    let store_node = store.filter(|_| has_store_layers || store_page.is_some());
    let mut store_kids: Vec<NodeId> = Vec::new();
    for (tree, in_store) in [(&entries, false), (&store_entries, true)] {
        // The entries are in pre-order: a node's parent is on the stack of its ancestors.
        let mut stack: Vec<(kition_kit::kdm::NodeRef, NodeId)> = Vec::new();
        for entry in tree {
            // A layer whose object was deleted (a delete merged with a concurrent edit): skipped with
            // everything under it.
            if !replica.exists(&entry.object) {
                continue;
            }
            let known = reuse.as_ref().filter(|_| !changed(&entry.object)).and_then(|r| r.ids.node_of(&entry.object));
            let key = match known {
                Some(k) => k,
                None => match replica.get(&entry.object, "key")?.as_u64() {
                    Some(k) => NodeId(k),
                    None => continue,
                },
            };
            // A key twice (two sessions in one id space, before the repair): the first one wins.
            if parent_of.contains_key(&key) || Some(key) == store_node {
                continue;
            }
            let parent = match &entry.parent {
                None => {
                    stack.clear();
                    None
                }
                Some(p) => match stack.iter().rposition(|(r, _)| r == p) {
                    Some(at) => {
                        stack.truncate(at + 1);
                        stack.get(at).map(|(_, k)| *k)
                    }
                    // Under a layer that was skipped: skipped too.
                    None => continue,
                },
            };
            stack.push((entry.node.clone(), key));
            match (parent, in_store) {
                (Some(p), _) => {
                    parent_of.insert(key, Some(p));
                    children.entry(p).or_default().push(key);
                }
                (None, false) => {
                    parent_of.insert(key, None);
                    roots.push(key);
                }
                (None, true) => {
                    parent_of.insert(key, store_node);
                    store_kids.push(key);
                }
            }
            ids.insert_node(key, entry.object.clone(), entry.node.clone(), in_store);
            order.push((key, entry.object.clone()));
        }
    }

    let mut nodes: Vec<Arc<Node>> = Vec::with_capacity(order.len());
    for (key, object) in &order {
        let parent = parent_of.get(key).copied().flatten();
        let kids = children.remove(key).unwrap_or_default();
        let previous = reuse.as_ref().filter(|_| !changed(object)).and_then(|r| r.view.node_arc(*key).map(|n| (n, r.view)));
        match previous {
            Some((n, view)) => {
                if let Some(r) = reuse.as_ref() {
                    ids.set_items(*key, r.ids.items_of(*key));
                }
                if n.parent == parent && n.children == kids {
                    nodes.push(n.clone());
                } else {
                    let mut m = (**n).clone();
                    m.parent = parent;
                    m.children = kids;
                    nodes.push(Arc::new(m));
                }
                if let Some(s) = view.dev_snapshots.get(key) {
                    doc.dev_snapshots.insert(*key, s.clone());
                }
            }
            None => {
                let (node, snapshot, objects) = read_node(replica, object, *key, parent, kids)?;
                ids.set_items(*key, objects);
                if let Some(s) = snapshot.and_then(|s| serde_json::from_value::<DevSnapshot>(s).ok()) {
                    doc.dev_snapshots.insert(*key, Arc::new(s));
                }
                nodes.push(Arc::new(node));
            }
        }
    }
    if nodes.len() > MAX_DEPTH * 10_000 {
        return Err(kdm_err("too many layers"));
    }
    if let Some(s) = store_node {
        nodes.push(read_store(s, store_page.as_ref(), store_kids, reuse.as_ref().map(|r| r.view))?);
        if doc.libraries.store != Some(s) {
            doc.libraries_mut().store = Some(s);
        }
    }
    let page_kind: BTreeSet<NodeId> = nodes.iter().filter(|n| matches!(n.kind, NodeKind::Page { .. })).map(|n| n.id).collect();
    doc.pages = roots.iter().copied().filter(|r| Some(*r) != store && page_kind.contains(r)).collect();
    doc.put_nodes_raw(nodes);

    doc.styles = read_styles(replica, &mut ids)?;
    doc.variables = read_variables(replica, &mut ids)?;
    doc.images = read_images(replica, blobs, &mut ids, reuse.as_ref().map(|r| r.view))?;
    doc.comments = read_comments(replica, &mut ids)?;
    if let Some(view) = reuse.as_ref().map(|r| r.view) {
        keep_read_state(&mut doc.comments, &view.comments);
    }
    let links = read_libraries(replica, &mut ids)?;
    if links != doc.libraries.enabled {
        doc.libraries_mut().enabled = links;
    }
    let copies = read_copies(replica, &mut ids)?;
    if !copies.is_empty() {
        store::set_rows(doc.libraries_mut(), copies);
    }
    doc.take_dirty();
    Ok((doc, ids))
}

/// The document from its settings (`shell`: the settings with the tree's keys emptied). Every
/// setting is a register any peer may have written: a key the engine cannot read (with the keys
/// before it, in key order) is left out, so one bad value never hides the document.
fn settings_from(shell: Map<String, Value>) -> KdmResult<Document> {
    if let Ok(doc) = serde_json::from_value::<Document>(Value::Object(shell.clone())) {
        return Ok(doc);
    }
    let mut kept = Map::new();
    for key in ["name", "pages", "nodes", "next_id"] {
        if let Some(v) = shell.get(key) {
            kept.insert(key.to_owned(), v.clone());
        }
    }
    if serde_json::from_value::<Document>(Value::Object(kept.clone())).is_err() {
        kept.insert("name".into(), json!("Untitled"));
    }
    let mut keys: Vec<&String> = shell.keys().filter(|k| !kept.contains_key(*k)).collect();
    keys.sort();
    for key in keys {
        let Some(value) = shell.get(key) else { continue };
        kept.insert(key.clone(), value.clone());
        if serde_json::from_value::<Document>(Value::Object(kept.clone())).is_err() {
            kept.remove(key);
        }
    }
    serde_json::from_value(Value::Object(kept)).map_err(|e| kdm_err(format!("document settings: {e}")))
}

fn read_styles(replica: &Replica, ids: &mut Ids) -> KdmResult<Vec<Style>> {
    let mut out = Vec::new();
    for object in replica.list(&root_id(), "styles")? {
        let Some(key) = replica.get(&object, "key")?.as_str().map(str::to_owned) else { continue };
        if ids.styles.contains_key(&key) {
            continue;
        }
        let regs = registers(replica, &object, &["name", "value", "props"])?;
        let Ok(style) = fields::style_from(&key, &regs) else { continue };
        ids.styles.insert(key, object);
        out.push(style);
    }
    Ok(out)
}

fn read_variables(replica: &Replica, ids: &mut Ids) -> KdmResult<Variables> {
    let mut vars = Variables::default();
    for object in replica.list(&root_id(), "collections")? {
        let Some(key) = replica.get(&object, "key")?.as_str().map(str::to_owned) else { continue };
        if ids.collections.contains_key(&key) {
            continue;
        }
        let regs = registers(replica, &object, &["name", "props"])?;
        let mut entry = CollectionIds { object: object.clone(), modes: HashMap::new() };
        let mut modes = Vec::new();
        for mode in replica.list(&object, "modes")? {
            let Some(id) = replica.get(&mode, "key")?.as_str().map(str::to_owned) else { continue };
            if entry.modes.contains_key(&id) {
                continue;
            }
            let name = replica.get(&mode, "name")?.as_str().unwrap_or_default().to_owned();
            entry.modes.insert(id.clone(), mode);
            modes.push((id, name));
        }
        let Ok(collection) = fields::collection_from(&key, &regs, modes) else { continue };
        ids.collections.insert(key, entry);
        vars.collections.push(collection);
    }
    for object in replica.list(&root_id(), "variables")? {
        let Some(key) = replica.get(&object, "key")?.as_str().map(str::to_owned) else { continue };
        if ids.variables.contains_key(&key) {
            continue;
        }
        let regs = registers(replica, &object, &["name", "collection", "type", "props"])?;
        let values = replica.map_entries(&object, "values")?;
        let Ok(variable): Result<Variable, KdmError> = fields::variable_from(&key, &regs, values) else { continue };
        ids.variables.insert(key, object);
        vars.variables.push(variable);
    }
    Ok(vars)
}

fn read_images(replica: &Replica, blobs: &dyn BlobSource, ids: &mut Ids, view: Option<&Document>) -> KdmResult<BTreeMap<String, ImageData>> {
    let mut out = BTreeMap::new();
    for object in replica.list(&root_id(), "images")? {
        let Some(key) = replica.get(&object, "key")?.as_str().map(str::to_owned) else { continue };
        ids.images.insert(key.clone(), object.clone());
        if let Some(img) = view.and_then(|v| v.images.get(&key)) {
            out.insert(key, img.clone());
            continue;
        }
        let blob = replica.get(&object, "blob")?;
        let Some(bytes) = blob.get("hash").and_then(Value::as_str).and_then(|hash| blobs.get(hash)) else { continue };
        let size = replica.get(&object, "size")?;
        let dim = |k: &str| size.get(k).and_then(Value::as_u64).and_then(|v| u32::try_from(v).ok()).unwrap_or(0);
        out.insert(key, ImageData { bytes: Arc::new(bytes.to_vec()), width: dim("width"), height: dim("height") });
    }
    Ok(out)
}

/// The store's root under `id`: its registers from the `libraryStore` setting (the engine's new
/// store when there is none), its children the roots of `Design.libraryStore`. The previous view's
/// node is kept when it is the same.
fn read_store(id: NodeId, page: Option<&Value>, children: Vec<NodeId>, view: Option<&Document>) -> KdmResult<Arc<Node>> {
    let mut node = match page {
        Some(Value::Object(map)) => {
            let regs: BTreeMap<&str, Value> = NODE_REGISTERS.iter().filter_map(|name| map.get(*name).map(|v| (*name, v.clone()))).collect();
            fields::node_from_lenient(id, &regs, None, Vec::new())?.0
        }
        _ => store::default_store(id).ok_or_else(|| kdm_err("no library store"))?,
    };
    node.children = children;
    if let Some(prev) = view.and_then(|v| v.node_arc(id))
        && **prev == node
    {
        return Ok(prev.clone());
    }
    Ok(Arc::new(node))
}

/// The records of the copies taken from libraries, in order (a kind and local id once: the first).
fn read_copies(replica: &Replica, ids: &mut Ids) -> KdmResult<Vec<(String, Value)>> {
    let mut out = Vec::new();
    for object in replica.list(&root_id(), COPIES)? {
        let (Some(kind), Some(local)) =
            (replica.get(&object, "kind")?.as_str().map(str::to_owned), replica.get(&object, "local")?.as_str().map(str::to_owned))
        else {
            continue;
        };
        let id = (kind.clone(), local);
        if ids.copies.contains_key(&id) {
            continue;
        }
        out.push((kind, replica.get(&object, "record")?));
        ids.copies.insert(id, object);
    }
    Ok(out)
}

/// The library links, in order (a library id once: the first).
fn read_libraries(replica: &Replica, ids: &mut Ids) -> KdmResult<Vec<LibraryLink>> {
    let mut out = Vec::new();
    for object in replica.list(&root_id(), "libraries")? {
        let Some(key) = replica.get(&object, "key")?.as_str().map(str::to_owned) else { continue };
        if ids.libraries.contains_key(&key) {
            continue;
        }
        let regs = registers(replica, &object, &["name", "path", "version"])?;
        out.push(fields::library_from(&key, &regs));
        ids.libraries.insert(key, object);
    }
    Ok(out)
}

/// Who has read each thread, from the previous view (it is each person's own, never replicated).
/// A thread that got a message since is unread for everyone but its author, as the engine marks
/// a thread a person writes in ([`Comment::touched_by`]).
pub(crate) fn keep_read_state(comments: &mut [Comment], view: &[Comment]) {
    for c in comments {
        let Some(was) = view.iter().find(|v| v.id == c.id) else { continue };
        let before = was.messages().count();
        let now = c.messages().count();
        if now > before {
            if let Some(last) = c.messages().last().map(|m| m.author.clone()) {
                c.touched_by(&last);
            }
        } else {
            c.read_by.clone_from(&was.read_by);
        }
    }
}

fn read_comments(replica: &Replica, ids: &mut Ids) -> KdmResult<Vec<Comment>> {
    let mut out = Vec::new();
    for object in replica.list(&root_id(), "comments")? {
        let anchor = replica.get(&object, "anchor")?;
        let resolved = replica.get(&object, "resolved")?.as_bool().unwrap_or(false);
        let mut messages = Vec::new();
        let mut objects = Vec::new();
        for m in replica.list(&object, "comments")? {
            let regs = registers(replica, &m, &["author", "created", "edited", "reactions"])?;
            let body = replica.text(&m, "body")?;
            messages.push(fields::message_from(&regs, body));
            objects.push(m);
        }
        let Some(comment) = fields::comment_from(&anchor, resolved, messages) else { continue };
        if ids.threads.contains_key(&comment.id) {
            continue;
        }
        ids.threads.insert(comment.id, Thread { object, messages: objects });
        out.push(comment);
    }
    Ok(out)
}
