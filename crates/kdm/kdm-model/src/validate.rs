//! The repair after a merge, an undo or a redo (engine contract 2 E14; `docs/features/kdm.md` section 5).
//!
//! Every rule depends on the state alone (never on who wrote what or in which order), so every
//! replica writes the same repair and recognises another's as done. A repair drops, clears,
//! renames or moves; it inserts only under an object id derived from the state (V8, V12), so
//! replicas repairing one state make one object.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use kition_design_doc::{Document, NodeId, NodeKind};
use kition_kit::kdm::{NodeRef, ObjectId, Replica, Tx};
use kition_kit::session::TextRun;
use serde_json::{Value, json};

use crate::store::{self, BY_ASSET, COPIES, STORE_PAGE, STORE_TREE};
use crate::{KdmResult, Result, kit_err, root_id};

/// Node keys a repair gives (V3) lie in this id space, which no session is given.
pub const REPAIR_SPACE: u64 = (1 << 21) - 1;

/// FNV-1a, 32 bits: a stable number from an object id.
pub(crate) fn fnv32(text: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in text.bytes() {
        h ^= u32::from(b);
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// A node key of the repair space derived from `text` that `taken` does not hold: the hash of
/// `text`, else of `text#1`, `text#2`, … (at most 64 tries). Depends on its arguments alone.
pub(crate) fn free_repair_key(text: &str, taken: impl Fn(NodeId) -> bool) -> NodeId {
    let mut salt = 0u32;
    loop {
        let seed = if salt == 0 { text.to_owned() } else { format!("{text}#{salt}") };
        let key = NodeId((REPAIR_SPACE << 32) | u64::from(fnv32(&seed)));
        if !taken(key) || salt >= 64 {
            return key;
        }
        salt += 1;
    }
}

/// Numeric tool ids share one namespace at the guard. Older documents can have a thread and
/// layer with the same id. Repair before creating their first replica, using the saved id
/// (not a freshly allocated KDM object id) so independent imports agree.
pub fn repair_comment_ids(doc: &mut Document) {
    let mut taken: BTreeSet<u64> = doc.ids().map(|n| n.0).chain(doc.comments.iter().map(|c| c.id)).collect();
    let nodes: BTreeSet<u64> = doc.ids().map(|n| n.0).collect();
    let mut seen = BTreeSet::new();
    for comment in &mut doc.comments {
        if nodes.contains(&comment.id) || !seen.insert(comment.id) {
            comment.id = free_repair_key(&format!("comment/{}", comment.id), |n| taken.contains(&n.0)).0;
            taken.insert(comment.id);
            seen.insert(comment.id);
        }
    }
}

/// The key the reader gives the library store's root: the `libraries` setting's `store`, else
/// (V10) [`store::repair_store_key`]; none when the design has no store (no store layer and no
/// `libraryStore` setting), as `read::read_with` decides.
pub(crate) fn store_root_key(tx: &Tx<'_>, store_entries: &[kition_kit::kdm::TreeEntry]) -> Option<NodeId> {
    let root = root_id();
    let has_layers = store_entries.iter().any(|e| tx.exists(&e.object));
    if !has_layers && !tx.map_get(&root, "settings", STORE_PAGE).is_ok_and(|v| !v.is_null()) {
        return None;
    }
    let set = tx
        .map_get(&root, "settings", "libraries")
        .ok()
        .and_then(|v| serde_json::from_value::<kition_design_doc::Libraries>(v).ok())
        .and_then(|l| l.store);
    Some(set.unwrap_or_else(store::repair_store_key))
}

/// The keys of every live layer of both trees, and the store root's.
pub(crate) fn live_keys(tx: &Tx<'_>) -> KdmResult<BTreeSet<NodeId>> {
    let root = root_id();
    let store_entries = tx.tree(&root, STORE_TREE)?;
    let mut keys: BTreeSet<NodeId> = BTreeSet::new();
    for e in tx.tree(&root, "nodes")?.iter().chain(&store_entries) {
        if tx.exists(&e.object)
            && let Some(k) = tx.get(&e.object, "key").ok().and_then(|v| v.as_u64())
        {
            keys.insert(NodeId(k));
        }
    }
    keys.extend(store_root_key(tx, &store_entries));
    Ok(keys)
}

/// The text fields of the library store's layers (`Node.text`, by object), read before the
/// repair's transaction: a transaction cannot read a text field, and V12 copies them.
pub type StoreTexts = HashMap<ObjectId, Vec<TextRun>>;

/// [`StoreTexts`] of `replica`: every live store layer whose text is not empty.
pub fn store_texts(replica: &Replica) -> StoreTexts {
    let mut out = StoreTexts::new();
    for entry in replica.tree(&root_id(), STORE_TREE).unwrap_or_default() {
        if !replica.exists(&entry.object) {
            continue;
        }
        if let Ok(runs) = replica.text_delta(&entry.object, "text")
            && !runs.is_empty()
        {
            out.insert(entry.object, runs);
        }
    }
    out
}

/// Runs the rules; answers whether it wrote anything. `texts` is [`store_texts`] of the state the
/// transaction starts from.
pub fn validate(tx: &mut Tx<'_>, _changed: &BTreeSet<ObjectId>, texts: &StoreTexts) -> Result<bool> {
    repair(tx, texts).map_err(kit_err)
}

fn repair(tx: &mut Tx<'_>, texts: &StoreTexts) -> KdmResult<bool> {
    let mut wrote = false;
    let root = root_id();
    if !tx.exists(&root) {
        return Ok(false);
    }
    // V7: an object two tree nodes name (a redo made a new node for a layer while another
    // replica's concurrent move kept the old one): the KDM keeps one of them (M10b).
    wrote |= tx.tree_dedupe(&root, "nodes")? > 0;
    wrote |= tx.tree_dedupe(&root, STORE_TREE)? > 0;
    let entries = tx.tree(&root, "nodes")?;
    let mut store_entries = tx.tree(&root, STORE_TREE)?;

    // V5: layers no tree node names (made under a layer deleted concurrently).
    let named: HashSet<&ObjectId> = entries.iter().chain(&store_entries).map(|e| &e.object).collect();
    for object in tx.objects_of_type("Node") {
        if !named.contains(&object) {
            tx.delete(&object)?;
            wrote = true;
        }
    }

    // V6: threads without comments.
    for thread in tx.objects_of_type("CommentThread") {
        if tx.list_items(&thread, "comments")?.iter().all(|c| !tx.exists(c)) {
            tx.delete(&thread)?;
            wrote = true;
        }
    }

    // V11: copies of one library asset taken at once (before V1 and V3, so V3 never renames a
    // copy into a style or variable of the design's own).
    wrote |= one_copy_per_asset(tx)?;
    // V12: copies of one library component placed at once become one (after V11, which leaves
    // one record per copy; before V1, V3 and V10, which see the new copy).
    if crate::merge::one_copy_per_component(tx, texts)? {
        wrote = true;
        store_entries = tx.tree(&root, STORE_TREE)?;
    }

    // V1, V2: list entries naming nothing, or an object twice.
    let mut lists: Vec<(ObjectId, &str)> =
        ["styles", "collections", "variables", "images", "comments", "libraries", COPIES].iter().map(|f| (root.clone(), *f)).collect();
    lists.extend(tx.objects_of_type("Collection").into_iter().map(|c| (c, "modes")));
    lists.extend(tx.objects_of_type("CommentThread").into_iter().map(|t| (t, "comments")));
    for (owner, field) in &lists {
        let items = tx.list_items(owner, field)?;
        let mut seen = HashSet::new();
        let drop: Vec<usize> =
            items.iter().enumerate().filter(|(_, item)| !tx.exists(item) || !seen.insert((*item).clone())).map(|(i, _)| i).collect();
        for i in drop.into_iter().rev() {
            tx.list_remove(owner, field, i)?;
            wrote = true;
        }
    }

    // V3: one key on two objects of a type.
    let all: Vec<kition_kit::kdm::TreeEntry> = entries.iter().chain(&store_entries).cloned().collect();
    let store_key = store_root_key(tx, &store_entries);
    wrote |= unique_node_keys(tx, &all, store_key)?;
    for (ty, field) in [("Style", "key"), ("Collection", "key"), ("Variable", "key")] {
        let objects = tx.objects_of_type(ty);
        wrote |= unique_text_keys(tx, &objects, field)?;
    }
    for collection in tx.objects_of_type("Collection") {
        let modes = tx.list_items(&collection, "modes")?;
        wrote |= unique_text_keys(tx, &modes, "key")?;
    }
    wrote |= unique_images(tx)?;
    wrote |= unique_threads(tx)?;
    // V9: one library added twice (two people added the same library file at once).
    wrote |= unique_libraries(tx)?;

    // V13: paint and effect objects no live layer lists (added to a layer deleted at once), and
    // list entries naming a deleted item or one item twice.
    wrote |= items_of_live_layers(tx)?;

    // V4: layers under a layer that cannot hold children (a concurrent type change).
    let moved = into_containers(tx, "nodes", &entries)?;
    wrote |= moved;
    wrote |= into_containers(tx, STORE_TREE, &store_entries)?;

    // V8: no page left to open (two people deleted the last two pages at once).
    let added = keep_a_page(tx)?;
    wrote |= added;
    // V4 for the roots of `Design.nodes`: a layer there that is no page (after V8, so a page exists).
    // The tree read above still holds unless V4 or V8 changed it.
    wrote |= roots_into_a_page(tx, (!moved && !added).then_some(&entries[..]))?;
    // V10: the store's layers outlive the setting that names the store's root.
    wrote |= keep_the_store(tx, &store_entries)?;
    Ok(wrote)
}

/// V13: every paint and effect object belongs to one live layer. Entries of a layer's `fills`,
/// `strokes` and `effects` that name a deleted object or an object listed before (by this layer or
/// an earlier one, in tree order) are dropped; `Paint` and `Effect` objects no live layer lists
/// are deleted.
fn items_of_live_layers(tx: &mut Tx<'_>) -> KdmResult<bool> {
    let root = root_id();
    let mut wrote = false;
    let mut listed: HashSet<ObjectId> = HashSet::new();
    let mut layers: Vec<ObjectId> = Vec::new();
    for field in ["nodes", STORE_TREE] {
        for e in tx.tree(&root, field)? {
            if tx.exists(&e.object) {
                layers.push(e.object);
            }
        }
    }
    for layer in &layers {
        for (field, _, _) in crate::items::LISTS {
            let entries = tx.list_items(layer, field)?;
            let drop: Vec<usize> =
                entries.iter().enumerate().filter(|(_, o)| !tx.exists(o) || !listed.insert((*o).clone())).map(|(i, _)| i).collect();
            for i in drop.into_iter().rev() {
                tx.list_remove(layer, field, i)?;
                wrote = true;
            }
        }
    }
    for ty in ["Paint", "Effect"] {
        for object in tx.objects_of_type(ty) {
            if !listed.contains(&object) {
                tx.delete(&object)?;
                wrote = true;
            }
        }
    }
    Ok(wrote)
}

/// V4 for one tree: layers under a layer that cannot hold children move to the nearest ancestor
/// that can (a root of the tree when none can), last.
fn into_containers(tx: &mut Tx<'_>, field: &str, entries: &[kition_kit::kdm::TreeEntry]) -> KdmResult<bool> {
    let root = root_id();
    let mut wrote = false;
    let mut container: HashMap<ObjectId, bool> = HashMap::new();
    let mut stack: Vec<(NodeRef, ObjectId)> = Vec::new();
    let mut moves: Vec<(NodeRef, Option<NodeRef>)> = Vec::new();
    for entry in entries {
        if !tx.exists(&entry.object) {
            continue;
        }
        let Some(parent) = &entry.parent else {
            stack.clear();
            stack.push((entry.node.clone(), entry.object.clone()));
            continue;
        };
        let Some(at) = stack.iter().rposition(|(r, _)| r == parent) else { continue };
        stack.truncate(at + 1);
        let holds = |tx: &Tx<'_>, cache: &mut HashMap<ObjectId, bool>, object: &ObjectId| -> bool {
            *cache.entry(object.clone()).or_insert_with(|| {
                tx.get(object, "kind").ok().and_then(|k| serde_json::from_value::<NodeKind>(k).ok()).is_none_or(|k| k.is_container())
            })
        };
        let parent_object = stack.get(at).map(|(_, o)| o.clone());
        if let Some(po) = parent_object
            && !holds(tx, &mut container, &po)
        {
            let target = stack.iter().rev().skip(1).find(|(_, o)| holds(tx, &mut container, o)).map(|(r, _)| r.clone());
            moves.push((entry.node.clone(), target));
        }
        stack.push((entry.node.clone(), entry.object.clone()));
    }
    for (node, target) in moves {
        if tx.tree_move(&root, field, &node, target.as_ref()).is_ok() {
            wrote = true;
        }
    }
    Ok(wrote)
}

/// Whether the live layer `object`'s `kind` reads as a page (as the reader reads it: a `kind`
/// that does not read is a frame).
fn is_page(tx: &Tx<'_>, object: &ObjectId) -> bool {
    let Ok(kind) = tx.get(object, "kind") else { return false };
    // Most layers are not pages: their tag says so without reading the rest.
    kind.get("type").and_then(Value::as_str) == Some("page")
        && serde_json::from_value::<NodeKind>(kind).is_ok_and(|k| matches!(k, NodeKind::Page { .. }))
}

/// V4 for the roots of `Design.nodes`: every live root that is no page (a layer V4 found no
/// container ancestor for, or one a peer wrote there) moves into the first page that is not a
/// divider (else the first page), last, in tree order. Nothing when the tree has no page.
fn roots_into_a_page(tx: &mut Tx<'_>, entries: Option<&[kition_kit::kdm::TreeEntry]>) -> KdmResult<bool> {
    let root = root_id();
    let read;
    let entries = match entries {
        Some(e) => e,
        None => {
            read = tx.tree(&root, "nodes")?;
            &read[..]
        }
    };
    let live_roots: Vec<&kition_kit::kdm::TreeEntry> = entries.iter().filter(|e| e.parent.is_none() && tx.exists(&e.object)).collect();
    let (pages, strays): (Vec<&kition_kit::kdm::TreeEntry>, Vec<&kition_kit::kdm::TreeEntry>) =
        live_roots.into_iter().partition(|e| is_page(tx, &e.object));
    if strays.is_empty() {
        return Ok(false);
    }
    let open = |e: &&&kition_kit::kdm::TreeEntry| {
        let name = tx.get(&e.object, "name").ok().and_then(|n| n.as_str().map(str::to_owned)).unwrap_or_default();
        !name.starts_with('-') || entries.iter().any(|c| c.parent.as_ref() == Some(&e.node) && tx.exists(&c.object))
    };
    let Some(target) = pages.iter().find(open).or(pages.first()).map(|e| e.node.clone()) else { return Ok(false) };
    let mut wrote = false;
    for stray in strays {
        if tx.tree_move(&root, "nodes", &stray.node, Some(&target)).is_ok() {
            wrote = true;
        }
    }
    Ok(wrote)
}

/// V10: layers in the library store but no store root named in the settings (an undo of the edit
/// that made the store while another person added copies to it). The setting names a root under
/// a key derived from nothing but the rule ([`store::repair_store_key`]), with the engine's new
/// store's registers, so replicas repairing at once write the same values.
fn keep_the_store(tx: &mut Tx<'_>, store_entries: &[kition_kit::kdm::TreeEntry]) -> KdmResult<bool> {
    if !store_entries.iter().any(|e| tx.exists(&e.object)) {
        return Ok(false);
    }
    let root = root_id();
    let mut wrote = false;
    // As the writer writes it (`write::settings`) when the setting is missing.
    let fresh = || serde_json::to_value(kition_design_doc::Libraries::default()).unwrap_or_else(|_| json!({}));
    let mut libraries = tx.map_get(&root, "settings", "libraries").ok().filter(Value::is_object).unwrap_or_else(fresh);
    let key = match libraries.get("store").and_then(Value::as_u64) {
        Some(k) => NodeId(k),
        None => {
            let key = store::repair_store_key();
            libraries["store"] = json!(key.0);
            tx.map_set(&root, "settings", "libraries", libraries)?;
            wrote = true;
            key
        }
    };
    if !tx.map_get(&root, "settings", STORE_PAGE).is_ok_and(|v| v.is_object())
        && let Some(page) = store::default_store(key)
    {
        tx.map_set(&root, "settings", STORE_PAGE, store::store_page(&page)?)?;
        wrote = true;
    }
    Ok(wrote)
}

/// V11: copies of one library asset taken at once by two people.
///
/// - Two records of one copy (a kind and local id): the one of the lowest library version stays
///   (the object whose id sorts first among those), so an update is offered when either side's
///   copy is older; the others are deleted and their list entries dropped.
/// - Style, variable collection and variable copies have ids derived from the asset
///   (`<library>:<key>`): two objects of one such id that a record names are one copy. The object
///   whose id sorts first stays (a collection with its modes); the others are deleted and their
///   list entries dropped.
fn one_copy_per_asset(tx: &mut Tx<'_>) -> KdmResult<bool> {
    let root = root_id();
    let mut wrote = false;
    let mut records: BTreeMap<(String, String), Vec<(u64, ObjectId)>> = BTreeMap::new();
    for object in tx.objects_of_type("LibraryCopy") {
        let text = |tx: &Tx<'_>, f: &str| tx.get(&object, f).ok().and_then(|v| v.as_str().map(str::to_owned));
        let (Some(kind), Some(local)) = (text(tx, "kind"), text(tx, "local")) else { continue };
        let version = tx.get(&object, "record").ok().and_then(|r| r.get("version").and_then(Value::as_u64)).unwrap_or(0);
        records.entry((kind, local)).or_default().push((version, object));
    }
    for (_, mut objects) in records.iter().filter(|(_, o)| o.len() > 1).map(|(k, o)| (k, o.clone())) {
        objects.sort();
        for (_, object) in objects.iter().skip(1) {
            wrote |= drop_from(tx, &root, COPIES, object)?;
        }
    }
    for (kind, ty) in BY_ASSET {
        let field = match *ty {
            "Style" => "styles",
            "Collection" => "collections",
            _ => "variables",
        };
        let copied: BTreeSet<&str> = records.keys().filter(|(k, _)| k == kind).map(|(_, local)| local.as_str()).collect();
        let mut by_key: BTreeMap<String, Vec<ObjectId>> = BTreeMap::new();
        for object in tx.objects_of_type(ty) {
            if let Some(k) = tx.get(&object, "key").ok().and_then(|v| v.as_str().map(str::to_owned))
                && copied.contains(k.as_str())
            {
                by_key.entry(k).or_default().push(object);
            }
        }
        for (_, mut objects) in by_key.into_iter().filter(|(_, o)| o.len() > 1) {
            objects.sort();
            for object in objects.iter().skip(1) {
                if *ty == "Collection" {
                    for mode in tx.list_items(object, "modes")? {
                        if tx.exists(&mode) {
                            tx.delete(&mode)?;
                        }
                    }
                }
                wrote |= drop_from(tx, &root, field, object)?;
            }
        }
    }
    Ok(wrote)
}

/// Deletes `object` and drops its entries from `owner.field`.
fn drop_from(tx: &mut Tx<'_>, owner: &ObjectId, field: &str, object: &ObjectId) -> KdmResult<bool> {
    while let Some(i) = tx.list_items(owner, field)?.iter().position(|o| o == object) {
        tx.list_remove(owner, field, i)?;
    }
    if tx.exists(object) {
        tx.delete(object)?;
    }
    Ok(true)
}

/// FNV-1a, 64 bits.
pub(crate) fn fnv64(text: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// V8: a design keeps a page that is not a divider (the engine refuses to delete the last one
/// locally; concurrent deletes can still remove them all). The repair adds an empty page under an
/// object id derived from the roots the tree holds (deleted pages included), so every replica that
/// repairs this state makes the same object: the KDM keeps one of the objects one id names, and
/// M10b (V7) keeps one of the tree nodes.
fn keep_a_page(tx: &mut Tx<'_>) -> KdmResult<bool> {
    let root = root_id();
    let entries = tx.tree(&root, "nodes")?;
    let live = |tx: &Tx<'_>, object: &ObjectId| tx.exists(object);
    let mut roots: Vec<&ObjectId> = Vec::new();
    for entry in entries.iter().filter(|e| e.parent.is_none()) {
        roots.push(&entry.object);
        if !live(tx, &entry.object) {
            continue;
        }
        if !is_page(tx, &entry.object) {
            continue;
        }
        let name = tx.get(&entry.object, "name").ok().and_then(|n| n.as_str().map(str::to_owned)).unwrap_or_default();
        let has_children = entries.iter().any(|e| e.parent.as_ref() == Some(&entry.node) && live(tx, &e.object));
        if has_children || !name.starts_with('-') {
            return Ok(false);
        }
    }
    roots.sort();
    let seed: Vec<&str> = roots.iter().map(|o| o.as_str()).collect();
    let id = ObjectId::from_string(format!("page-{:016x}", fnv64(&seed.join(","))));
    if tx.exists(&id) {
        return Ok(false);
    }
    let taken = live_keys(tx)?;
    let key = free_repair_key(id.as_str(), |k| taken.contains(&k));
    // The engine's own new page (as a new design has), under the repair's key.
    let fresh = Document::new("");
    let Some(mut page) = fresh.first_page().and_then(|p| fresh.node(p)).cloned() else { return Ok(false) };
    page.id = key;
    page.name = "Page 1".into();
    let mut fields: serde_json::Map<String, Value> =
        crate::fields::node_fields(&page, None)?.into_iter().filter(|(_, v)| !v.is_null()).map(|(k, v)| (k.to_owned(), v)).collect();
    fields.insert("key".into(), json!(key.0));
    // An id taken by an object deleted earlier: leave the state as it is rather than fail the repair.
    if tx.create_with_id("Node", &id, fields).is_err() {
        return Ok(false);
    }
    tx.tree_create(&root, "nodes", None, &id)?;
    Ok(true)
}

/// V3 for layers: of the live layers of one key, the one whose object id sorts first keeps it
/// (none when the key is the library store root's, `store`); the others get keys of the repair
/// space that no live layer, the store root, nor an earlier repaired layer has.
fn unique_node_keys(tx: &mut Tx<'_>, entries: &[kition_kit::kdm::TreeEntry], store: Option<NodeId>) -> KdmResult<bool> {
    let mut by_key: BTreeMap<u64, Vec<ObjectId>> = BTreeMap::new();
    for e in entries {
        if !tx.exists(&e.object) {
            continue;
        }
        if let Some(k) = tx.get(&e.object, "key").ok().and_then(|v| v.as_u64()) {
            by_key.entry(k).or_default().push(e.object.clone());
        }
    }
    let mut taken: BTreeSet<NodeId> = by_key.keys().copied().map(NodeId).chain(store).collect();
    let mut wrote = false;
    // A layer without a key (a model batch made it: `key` is the kit's tool id, read-only to
    // batches) gets one of the repair space, derived from its object id.
    let mut keyless: Vec<ObjectId> = entries
        .iter()
        .filter(|e| tx.exists(&e.object) && tx.get(&e.object, "key").ok().and_then(|v| v.as_u64()).is_none())
        .map(|e| e.object.clone())
        .collect();
    keyless.sort();
    keyless.dedup();
    for object in keyless {
        let key = free_repair_key(object.as_str(), |k| taken.contains(&k));
        taken.insert(key);
        tx.set(&object, "key", json!(key.0))?;
        wrote = true;
    }
    for (key, mut objects) in by_key.into_iter().filter(|(k, o)| o.len() > 1 || store == Some(NodeId(*k))) {
        objects.sort();
        objects.dedup();
        let keeps = usize::from(store != Some(NodeId(key)));
        for object in objects.iter().skip(keeps) {
            let key = free_repair_key(object.as_str(), |k| taken.contains(&k));
            taken.insert(key);
            tx.set(object, "key", json!(key.0))?;
            wrote = true;
        }
    }
    Ok(wrote)
}

fn unique_text_keys(tx: &mut Tx<'_>, objects: &[ObjectId], field: &str) -> KdmResult<bool> {
    let mut by_key: BTreeMap<String, Vec<ObjectId>> = BTreeMap::new();
    for object in objects {
        if let Some(k) = tx.get(object, field).ok().and_then(|v| v.as_str().map(str::to_owned)) {
            by_key.entry(k).or_default().push(object.clone());
        }
    }
    let mut taken: BTreeSet<String> = by_key.keys().cloned().collect();
    let mut wrote = false;
    for (key, mut dupes) in by_key.into_iter().filter(|(_, o)| o.len() > 1) {
        dupes.sort();
        for object in dupes.iter().skip(1) {
            // `key~suffix`, else `key~suffix~1`, `~2`, … : never a key another object has.
            let suffix: String = object.as_str().chars().filter(char::is_ascii_alphanumeric).take(8).collect();
            let base = format!("{key}~{suffix}");
            let mut fresh = base.clone();
            let mut salt = 0u32;
            while taken.contains(&fresh) && salt < 64 {
                salt += 1;
                fresh = format!("{base}~{salt}");
            }
            taken.insert(fresh.clone());
            tx.set(object, field, Value::String(fresh))?;
            wrote = true;
        }
    }
    Ok(wrote)
}

/// Two image objects for one content hash (two people placed the same picture): one is kept.
fn unique_images(tx: &mut Tx<'_>) -> KdmResult<bool> {
    let mut by_key: BTreeMap<String, Vec<ObjectId>> = BTreeMap::new();
    for object in tx.objects_of_type("Image") {
        if let Some(k) = tx.get(&object, "key").ok().and_then(|v| v.as_str().map(str::to_owned)) {
            by_key.entry(k).or_default().push(object);
        }
    }
    let mut wrote = false;
    let root = root_id();
    for (_, mut objects) in by_key.into_iter().filter(|(_, o)| o.len() > 1) {
        objects.sort();
        for object in objects.iter().skip(1) {
            if let Some(i) = tx.list_items(&root, "images")?.iter().position(|o| o == object) {
                tx.list_remove(&root, "images", i)?;
            }
            tx.delete(object)?;
            wrote = true;
        }
    }
    Ok(wrote)
}

/// Two library links of one library id: the object whose id sorts first stays.
fn unique_libraries(tx: &mut Tx<'_>) -> KdmResult<bool> {
    let mut by_key: BTreeMap<String, Vec<ObjectId>> = BTreeMap::new();
    for object in tx.objects_of_type("Library") {
        if let Some(k) = tx.get(&object, "key").ok().and_then(|v| v.as_str().map(str::to_owned)) {
            by_key.entry(k).or_default().push(object);
        }
    }
    let mut wrote = false;
    let root = root_id();
    for (_, mut objects) in by_key.into_iter().filter(|(_, o)| o.len() > 1) {
        objects.sort();
        for object in objects.iter().skip(1) {
            if let Some(i) = tx.list_items(&root, "libraries")?.iter().position(|o| o == object) {
                tx.list_remove(&root, "libraries", i)?;
            }
            tx.delete(object)?;
            wrote = true;
        }
    }
    Ok(wrote)
}

/// Threads and layers share a numeric tool-id namespace; the thread moves on collision.
fn unique_threads(tx: &mut Tx<'_>) -> KdmResult<bool> {
    let mut by_key: BTreeMap<u64, Vec<ObjectId>> = BTreeMap::new();
    for object in tx.objects_of_type("CommentThread") {
        if let Some(k) = tx.get(&object, "anchor").ok().and_then(|v| v.get("id").and_then(Value::as_u64)) {
            by_key.entry(k).or_default().push(object);
        }
    }
    let nodes: BTreeSet<u64> = live_keys(tx)?.into_iter().map(|n| n.0).collect();
    let mut taken: BTreeSet<u64> = by_key.keys().copied().chain(nodes.iter().copied()).collect();
    let mut wrote = false;
    for (key, mut objects) in by_key {
        objects.sort();
        for (index, object) in objects.iter().enumerate() {
            let mut anchor = tx.get(object, "anchor")?;
            let id = if index > 0 || nodes.contains(&key) {
                let id = free_repair_key(&format!("comment/{key}"), |k| taken.contains(&k.0)).0;
                taken.insert(id);
                anchor["id"] = json!(id);
                tx.set(object, "anchor", anchor)?;
                wrote = true;
                id
            } else {
                key
            };
            if tx.get(object, "key")?.as_u64() != Some(id) {
                tx.set(object, "key", json!(id))?;
                wrote = true;
            }
        }
    }
    Ok(wrote)
}
