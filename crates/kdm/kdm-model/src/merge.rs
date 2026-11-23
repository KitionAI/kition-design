//! V12: copies of one library component that two people placed at once become one
//! (`docs/features/kdm.md` section 5).
//!
//! The rule reads the state alone. The copies of one component (the `LibraryCopy` records of one
//! library and asset whose root layer is in the store) are replaced by one new copy, made by the
//! repair under object ids derived from the merged records' ids (KitModel M8, M22), so replicas
//! that merge one state at once make the same objects and the KDM keeps one of each (V7 their
//! tree nodes). The new copy is the copy of the lowest library version (then the record whose id
//! sorts first), so an update stays offered when any copy was older. Its layers get keys in the
//! repair space. Every reference to a layer of a merged copy (an instance's main, the layers
//! inside instances that mirror the copy's, instance swap values, preferred components, the
//! copies' own nested instances) is pointed at the new copy's layer for the same library layer;
//! nothing else is written (no instance propagation, no layout: section 4.1). Overrides are
//! registers of the instances' own layers and stay as they are. The new copy's layers take the
//! kept copy's paint and effect objects (made again under ids derived from the layer's) and text
//! (read before the transaction, [`crate::validate::StoreTexts`]).
//!
//! Why a new copy rather than keeping one of them: each copy was made by one person's
//! placement, and that person's undo deletes what it made (KitModel M12). A kept copy would go
//! with its maker's undo and leave the other person's instances without a main; the repair's own
//! objects are no one's undo step and survive.
//!
//! The new record lists every merged layer (`merged`: former local id → the new copy's layer),
//! so a reference to a merged copy that arrives later (a person duplicated an instance before
//! hearing of the merge, or a concurrent edit of an instance won over the repair's) is pointed at
//! the new copy by the next repair.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use kition_design_doc::{ImportedComponent, NodeId};
use kition_kit::kdm::{NodeRef, ObjectId, TreeEntry, Tx};
use serde_json::{Map, Value, json};

use crate::fields::{NODE_REGISTERS, node_fields, node_from_lenient};
use crate::items::{self, BOUND, LISTS};
use crate::store::{COMPONENT, COPIES, STORE_TREE};
use crate::validate::{StoreTexts, fnv64, free_repair_key};
use crate::{KdmResult, kdm_err, root_id};

/// A component copy's record and where it is.
struct Copy {
    object: ObjectId,
    record: ImportedComponent,
}

/// One merge: the copies of one component, the kept one first.
struct Merge {
    copies: Vec<Copy>,
    /// The new record's object id, the record, and where its list entry goes.
    record: ObjectId,
    new_record: Option<ImportedComponent>,
    at: usize,
    /// The kept copy's layers in pre-order: (its object, its tree node, its parent's node, the new
    /// object id, the new key).
    layers: Vec<(ObjectId, NodeRef, Option<NodeRef>, ObjectId, NodeId)>,
}

/// The tree entries of the layer at `root` and every live layer under it, in pre-order (a
/// deleted layer is left out with everything under it, as the reader does).
fn subtree<'e>(entries: &'e [TreeEntry], root: &NodeRef, live: &dyn Fn(&ObjectId) -> bool) -> Vec<&'e TreeEntry> {
    let Some(start) = entries.iter().position(|e| &e.node == root) else { return Vec::new() };
    let mut out: Vec<&TreeEntry> = Vec::new();
    // `NodeRef` is only comparable for equality; copies are small.
    let mut inside: Vec<&NodeRef> = Vec::new();
    for (i, e) in entries.iter().enumerate().skip(start) {
        let member = i == start || e.parent.as_ref().is_some_and(|p| inside.contains(&p));
        if member && live(&e.object) {
            inside.push(&e.node);
            out.push(e);
        } else if i == start {
            break;
        }
    }
    out
}

/// Deletes a record the repair claimed but could not fill.
fn drop_record(tx: &mut Tx<'_>, record: &ObjectId) -> KdmResult<()> {
    if tx.exists(record) {
        tx.delete(record)?;
    }
    Ok(())
}

/// Whether `v` holds a number `map` names.
fn mentions(v: &Value, map: &BTreeMap<NodeId, NodeId>) -> bool {
    match v {
        Value::Number(n) => n.as_u64().is_some_and(|n| map.contains_key(&NodeId(n))),
        Value::Array(items) => items.iter().any(|i| mentions(i, map)),
        Value::Object(fields) => fields.values().any(|i| mentions(i, map)),
        _ => false,
    }
}

/// A node object's paint and effect objects, list by list ([`LISTS`]): the live ones in order.
fn item_objects(tx: &Tx<'_>, object: &ObjectId) -> [Vec<ObjectId>; 3] {
    let mut out: [Vec<ObjectId>; 3] = Default::default();
    for ((field, _, _), list) in LISTS.iter().zip(out.iter_mut()) {
        *list = tx.list_items(object, field).unwrap_or_default().into_iter().filter(|o| tx.exists(o)).collect();
    }
    out
}

/// A node object's registers, by name (unset: `Null`), and each item list as the array of its
/// items' values under the list's name (as [`node_from_lenient`] takes it); `items` is
/// [`item_objects`].
fn registers(tx: &Tx<'_>, object: &ObjectId, items: &[Vec<ObjectId>; 3]) -> BTreeMap<&'static str, Value> {
    let mut out: BTreeMap<&'static str, Value> = NODE_REGISTERS.iter().map(|r| (*r, tx.get(object, r).unwrap_or(Value::Null))).collect();
    for ((field, _, value_field), list) in LISTS.iter().zip(items) {
        if !list.is_empty() {
            out.insert(*field, Value::Array(list.iter().map(|o| tx.get(o, value_field).unwrap_or(Value::Null)).collect()));
        }
    }
    out
}

/// What a remap changes of one layer: its registers, as the writer writes them, and its items'
/// values (`(list, index, value)`).
struct Remapped {
    registers: Vec<(&'static str, Value)>,
    items: Vec<(&'static str, usize, Value)>,
}

/// `registers` (with the item lists, [`registers`]) with every reference `map` names pointed at
/// its entry: what changes (`None` when nothing does).
fn remapped(key: NodeId, registers: &BTreeMap<&'static str, Value>, map: &BTreeMap<NodeId, NodeId>) -> KdmResult<Option<Remapped>> {
    if !registers.values().any(|v| mentions(v, map)) {
        return Ok(None);
    }
    // Registers that do not read are left as they are (review #1).
    let (mut node, snapshot, unread) = node_from_lenient(key, registers, None, Vec::new())?;
    if !node.remap_refs(map) {
        return Ok(None);
    }
    let changed: Vec<(&'static str, Value)> = node_fields(&node, snapshot.as_ref())?
        .into_iter()
        .filter(|(field, value)| !unread.contains(field) && registers.get(field).unwrap_or(&Value::Null) != value)
        .collect();
    let mut item_values = Vec::new();
    for (field, _, _) in LISTS {
        let Some(Value::Array(was)) = registers.get(field) else { continue };
        if unread.contains(&field) {
            continue;
        }
        for (index, item) in items::items(&node, field)?.into_iter().enumerate() {
            if was.get(index) != Some(&item.value) {
                item_values.push((field, index, item.value));
            }
        }
    }
    Ok(Some(Remapped { registers: changed, items: item_values }))
}

/// Copies layer `from`'s items (`objects`, [`item_objects`]) into the new layer `to`, under ids
/// derived from `to`'s, with the values `changed` gives instead of theirs.
fn copy_items(tx: &mut Tx<'_>, objects: &[Vec<ObjectId>; 3], to: &ObjectId, changed: &[(&'static str, usize, Value)]) -> KdmResult<()> {
    for ((field, ty, value_field), list) in LISTS.iter().zip(objects) {
        for (index, item) in list.iter().enumerate() {
            let value = match changed.iter().find(|(f, i, _)| f == field && *i == index) {
                Some((_, _, v)) => v.clone(),
                None => tx.get(item, value_field)?,
            };
            let bound = tx.map_entries(item, BOUND)?;
            let id = ObjectId::from_string(format!("{}-{field}-{index}", to.as_str()));
            let mut fields = Map::new();
            fields.insert((*value_field).to_owned(), value);
            tx.create_with_id(ty, &id, fields)?;
            for (key, variable) in bound {
                tx.map_set(&id, BOUND, &key, variable)?;
            }
            tx.list_insert(to, field, index, &id)?;
        }
    }
    Ok(())
}

/// Copies a store layer's text (its runs, read before the transaction) into the new layer `to`.
fn copy_text(tx: &mut Tx<'_>, runs: Option<&Vec<kition_kit::session::TextRun>>, to: &ObjectId) -> KdmResult<()> {
    let Some(runs) = runs else { return Ok(()) };
    let all: String = runs.iter().map(|r| r.insert.as_str()).collect();
    if all.is_empty() {
        return Ok(());
    }
    tx.text_insert(to, "text", 0, &all)?;
    let mut at = 0;
    for run in runs {
        let len = run.insert.chars().count();
        for (key, value) in &run.attributes {
            tx.text_mark(to, "text", at, at + len, key, value.clone())?;
        }
        at += len;
    }
    Ok(())
}

/// V12 (run after V11, before V1 and V3). Answers whether it wrote.
pub(crate) fn one_copy_per_component(tx: &mut Tx<'_>, texts: &StoreTexts) -> KdmResult<bool> {
    let root = root_id();
    let live = |tx: &Tx<'_>, o: &ObjectId| tx.exists(o);
    let store_entries = tx.tree(&root, STORE_TREE)?;
    let node_entries = tx.tree(&root, "nodes")?;
    // Keys of live layers (the first object of a key, as the reader reads it).
    let key_of = |tx: &Tx<'_>, o: &ObjectId| tx.get(o, "key").ok().and_then(|k| k.as_u64()).map(NodeId);
    let mut store_by_key: HashMap<NodeId, &TreeEntry> = HashMap::new();
    let mut taken: BTreeSet<NodeId> = BTreeSet::new();
    for e in store_entries.iter().chain(&node_entries) {
        if !live(tx, &e.object) {
            continue;
        }
        if let Some(k) = key_of(tx, &e.object) {
            taken.insert(k);
        }
    }
    taken.extend(crate::validate::store_root_key(tx, &store_entries));
    for e in &store_entries {
        if live(tx, &e.object)
            && let Some(k) = key_of(tx, &e.object)
        {
            store_by_key.entry(k).or_insert(e);
        }
    }

    // The component copies whose root layer is in the store, by library and asset.
    let mut groups: BTreeMap<(String, NodeId), Vec<(u64, Copy)>> = BTreeMap::new();
    for object in tx.objects_of_type("LibraryCopy") {
        if !live(tx, &object) || tx.get(&object, "kind").ok().and_then(|k| k.as_str().map(str::to_owned)).as_deref() != Some(COMPONENT) {
            continue;
        }
        let Some(record) = tx.get(&object, "record").ok().and_then(|r| serde_json::from_value::<ImportedComponent>(r).ok()) else { continue };
        if !store_by_key.contains_key(&record.root) {
            continue;
        }
        groups.entry((record.library.clone(), record.key)).or_default().push((record.version, Copy { object, record }));
    }

    // Plan every merge, and the map of every reference to point elsewhere.
    let mut map: BTreeMap<NodeId, NodeId> = BTreeMap::new();
    let mut merges: Vec<Merge> = Vec::new();
    for (_, mut copies) in groups {
        if copies.len() < 2 {
            // A copy left alone: references to the copies merged into it before follow it.
            for (_, c) in &copies {
                map.extend(c.record.merged.iter().filter(|(old, _)| !taken.contains(*old)).map(|(o, n)| (*o, *n)));
            }
            continue;
        }
        copies.sort_by(|a, b| (a.0, &a.1.object).cmp(&(b.0, &b.1.object)));
        let copies: Vec<Copy> = copies.into_iter().map(|(_, c)| c).collect();
        let mut seed: Vec<&str> = copies.iter().map(|c| c.object.as_str()).collect();
        seed.sort_unstable();
        let seed = seed.join(",");
        let record = ObjectId::from_string(format!("copy-{:016x}", fnv64(&seed)));
        let Some(kept) = copies.first() else { continue };
        let Some(kept_root) = store_by_key.get(&kept.record.root) else { continue };
        if tx.exists(&record) {
            continue;
        }
        let mut layers = Vec::new();
        let mut keys: BTreeSet<NodeId> = BTreeSet::new();
        for e in subtree(&store_entries, &kept_root.node, &|o| tx.exists(o)) {
            let Some(old) = key_of(tx, &e.object) else { continue };
            let id = ObjectId::from_string(format!("copy-{:016x}", fnv64(&format!("{seed}/{}", e.object.as_str()))));
            // A key of the repair space no live layer (nor the store root) has, probed
            // deterministically.
            let key = free_repair_key(id.as_str(), |k| taken.contains(&k) || keys.contains(&k));
            keys.insert(key);
            layers.push((old, e.object.clone(), e.node.clone(), e.parent.clone(), id, key));
        }
        if layers.is_empty() {
            continue;
        }
        // The record's id first: one an object deleted earlier had is refused (M8), and the
        // merge is left out rather than fail the repair. Its record is written below.
        let mut claim = Map::new();
        claim.insert("kind".into(), json!(COMPONENT));
        claim.insert("local".into(), json!(""));
        claim.insert("record".into(), json!({}));
        if tx.create_with_id("LibraryCopy", &record, claim).is_err() {
            continue;
        }
        taken.extend(keys);
        let layers = layers
            .into_iter()
            .map(|(old, object, node, parent, id, key)| {
                map.insert(old, key);
                (object, node, parent, id, key)
            })
            .collect();
        merges.push(Merge { copies, record, new_record: None, at: usize::MAX, layers });
    }
    // The other copies' layers: to the new copy's layer for the same library layer; the copies
    // merged into them before: on to the same place.
    for m in &mut merges {
        let Some(kept) = m.copies.first() else { continue };
        let by_library: BTreeMap<NodeId, NodeId> = kept.record.ids.iter().filter_map(|(lib, local)| map.get(local).map(|n| (*lib, *n))).collect();
        for c in m.copies.iter().skip(1) {
            for (lib, local) in &c.record.ids {
                if let Some(n) = by_library.get(lib) {
                    map.insert(*local, *n);
                }
            }
        }
        let mut merged: BTreeMap<NodeId, NodeId> = BTreeMap::new();
        for c in &m.copies {
            for local in c.record.ids.values().chain(std::iter::once(&c.record.root)) {
                if let Some(n) = map.get(local) {
                    merged.insert(*local, *n);
                }
            }
            for (old, current) in &c.record.merged {
                if let Some(n) = map.get(current) {
                    merged.insert(*old, *n);
                }
            }
        }
        let Some(new_root) = map.get(&kept.record.root).copied() else { continue };
        m.at = tx.list_items(&root, COPIES)?.iter().position(|o| m.copies.iter().any(|c| &c.object == o)).unwrap_or(usize::MAX);
        m.new_record = Some(ImportedComponent {
            library: kept.record.library.clone(),
            key: kept.record.key,
            root: new_root,
            version: kept.record.version,
            ids: by_library,
            merged,
        });
    }
    for (old, new) in merges.iter().filter_map(|m| m.new_record.as_ref()).flat_map(|r| r.merged.iter()) {
        map.insert(*old, *new);
    }
    if map.is_empty() {
        return Ok(false);
    }

    let mut wrote = false;
    let mut made: BTreeSet<ObjectId> = BTreeSet::new();
    let mut gone: BTreeSet<ObjectId> = BTreeSet::new();
    for m in &merges {
        let Some(record) = &m.new_record else {
            // Never left without its record (the kept root is always among the layers).
            drop_record(tx, &m.record)?;
            continue;
        };
        // The new copy, as a root of the store.
        let mut nodes: Vec<(&NodeRef, NodeRef)> = Vec::new();
        for (object, node, parent, id, key) in &m.layers {
            let objects = item_objects(tx, object);
            let regs = registers(tx, object, &objects);
            let mut fields: Map<String, Value> =
                regs.iter().filter(|(k, v)| !v.is_null() && NODE_REGISTERS.contains(*k)).map(|(k, v)| ((*k).to_owned(), v.clone())).collect();
            let mut item_values = Vec::new();
            if let Some(changed) = remapped(*key, &regs, &map)? {
                for (field, value) in changed.registers {
                    if value.is_null() {
                        fields.remove(field);
                    } else {
                        fields.insert(field.to_owned(), value);
                    }
                }
                item_values = changed.items;
            }
            fields.insert("key".into(), json!(key.0));
            tx.create_with_id("Node", id, fields)?;
            copy_items(tx, &objects, id, &item_values)?;
            copy_text(tx, texts.get(object), id)?;
            let under = parent.as_ref().and_then(|p| nodes.iter().find(|(n, _)| *n == p).map(|(_, made)| made));
            let created = tx.tree_create(&root, STORE_TREE, under, id)?;
            nodes.push((node, created));
            made.insert(id.clone());
        }
        tx.set(&m.record, "local", json!(record.root.0.to_string()))?;
        tx.set(&m.record, "record", serde_json::to_value(record).map_err(kdm_err)?)?;
        let len = tx.list_items(&root, COPIES)?.len();
        tx.list_insert(&root, COPIES, m.at.min(len), &m.record)?;
        // The merged copies go: their records and layers.
        for c in &m.copies {
            while let Some(i) = tx.list_items(&root, COPIES)?.iter().position(|o| o == &c.object) {
                tx.list_remove(&root, COPIES, i)?;
            }
            if tx.exists(&c.object) {
                tx.delete(&c.object)?;
            }
            if let Some(e) = store_by_key.get(&c.record.root) {
                for layer in subtree(&store_entries, &e.node, &|o| tx.exists(o)) {
                    gone.insert(layer.object.clone());
                }
            }
        }
        wrote = true;
    }
    // The SDK requires children to be deleted before their tree parent.
    for entry in store_entries.iter().rev().filter(|entry| gone.contains(&entry.object)) {
        if tx.exists(&entry.object) {
            tx.delete(&entry.object)?;
        }
    }

    // Every other layer that refers to a merged copy refers to the new one.
    for e in node_entries.iter().chain(&store_entries) {
        if made.contains(&e.object) || gone.contains(&e.object) || !tx.exists(&e.object) {
            continue;
        }
        let Some(key) = key_of(tx, &e.object) else { continue };
        let objects = item_objects(tx, &e.object);
        let regs = registers(tx, &e.object, &objects);
        if let Some(changed) = remapped(key, &regs, &map)? {
            for (field, value) in changed.registers {
                tx.set(&e.object, field, value)?;
                wrote = true;
            }
            for (field, index, value) in changed.items {
                let slot = LISTS.iter().position(|(f, _, _)| *f == field);
                let Some((item, (_, _, value_field))) = slot.and_then(|s| Some((objects.get(s)?.get(index)?, LISTS.get(s)?))) else { continue };
                tx.set(item, value_field, value)?;
                wrote = true;
            }
        }
    }
    Ok(wrote)
}
