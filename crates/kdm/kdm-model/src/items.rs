//! A layer's paints and effects as objects (`Node.fills` and `Node.strokes`: lists of `Paint`;
//! `Node.effects`: a list of `Effect`; `docs/features/kdm.md` section 2.1).
//!
//! Each item is one object: its value (the engine's serde form of the paint or effect) and its
//! variable bindings, a map from the field inside the item (`color`, `stops/1/color`, `radius`) to
//! the variable id. The engine names a binding by the item's place (`fills/0/color`); the bridge
//! keeps it on the item's object, so a binding moves with its paint when two people reorder, add
//! or remove paints at once, and a concurrent edit of one paint never touches another.
//!
//! The engine's paints have no ids of their own: [`pair`] tells which object an item of the new
//! list is (an equal item first, else the one at its place), so recolouring paint 1 writes its
//! object's value, adding one creates one object and reordering moves objects.

use std::collections::BTreeMap;

use kition_design_doc::Node;
use serde_json::Value;

use crate::{KdmResult, kdm_err};

/// The item lists of a node: `(field, object type, value field)`; the field is also the engine's
/// serde key and binding prefix.
pub const LISTS: [(&str, &str, &str); 3] = [("fills", "Paint", "paint"), ("strokes", "Paint", "paint"), ("effects", "Effect", "effect")];

/// The bindings map of an item object.
pub const BOUND: &str = "bound";

/// One paint or effect: its value and its bindings (field inside the item → variable id).
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub value: Value,
    pub bound: BTreeMap<String, Value>,
}

/// How many items `n` has in list `field`.
pub fn count(n: &Node, field: &str) -> usize {
    match field {
        "fills" => n.fills.len(),
        "strokes" => n.strokes.len(),
        "effects" => n.effects.len(),
        _ => 0,
    }
}

/// The item a binding key names: `(list, index, field inside the item)` for `fills/<i>/<field>`
/// (and `strokes/…`, `effects/…`) when `n` has item `i`; other keys (`fill`, `width`) stay the
/// node's.
pub fn item_key<'k>(n: &Node, key: &'k str) -> Option<(&'static str, usize, &'k str)> {
    LISTS.iter().find_map(|(field, _, _)| {
        let rest = key.strip_prefix(field)?.strip_prefix('/')?;
        let (digits, inner) = rest.split_once('/')?;
        let index: usize = digits.parse().ok()?;
        (!inner.is_empty() && index < count(n, field) && index.to_string() == digits).then_some((*field, index, inner))
    })
}

/// The items of `n`'s list `field`: values and the bindings that name them.
pub fn items(n: &Node, field: &str) -> KdmResult<Vec<Item>> {
    let values = match field {
        "fills" => serde_json::to_value(&n.fills),
        "strokes" => serde_json::to_value(&n.strokes),
        "effects" => serde_json::to_value(&n.effects),
        _ => Ok(Value::Array(Vec::new())),
    }
    .map_err(kdm_err)?;
    let Value::Array(values) = values else { return Err(kdm_err(format!("{field} is not a list"))) };
    let mut out: Vec<Item> = values.into_iter().map(|value| Item { value, bound: BTreeMap::new() }).collect();
    for (key, variable) in &n.bound {
        if let Some((list, index, inner)) = item_key(n, key)
            && list == field
            && let Some(item) = out.get_mut(index)
        {
            item.bound.insert(inner.to_owned(), Value::String(variable.clone()));
        }
    }
    Ok(out)
}

/// Adds the bindings of items read from the replica to `n` (as the engine names them).
pub fn bind(n: &mut Node, field: &str, items: &[Item]) {
    for (index, item) in items.iter().enumerate() {
        for (inner, variable) in &item.bound {
            if let Some(v) = variable.as_str() {
                n.bound.insert(format!("{field}/{index}/{inner}"), v.to_owned());
            }
        }
    }
}

/// Which old item each new item is: for each of `new`, `Some(index into old)` or `None` (a new
/// object). An equal old item first (in order, each once); else the old item at the same place
/// when no other new item took it. Depends on the two lists alone.
pub fn pair(old: &[Item], new: &[Item]) -> Vec<Option<usize>> {
    let mut used = vec![false; old.len()];
    let mut out: Vec<Option<usize>> = new
        .iter()
        .map(|item| {
            let found = old.iter().enumerate().position(|(i, o)| !used.get(i).copied().unwrap_or(true) && o == item);
            if let Some(i) = found
                && let Some(u) = used.get_mut(i)
            {
                *u = true;
            }
            found
        })
        .collect();
    for (at, slot) in out.iter_mut().enumerate() {
        if slot.is_none()
            && let Some(u) = used.get_mut(at)
            && !*u
        {
            *u = true;
            *slot = Some(at);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn item(v: i64) -> Item {
        Item { value: json!(v), bound: BTreeMap::new() }
    }

    #[test]
    fn pairing_keeps_objects_through_edits_moves_and_inserts() {
        let old = [item(1), item(2), item(3)];
        // Paint 1 recoloured: the same object, at the same place.
        assert_eq!(pair(&old, &[item(1), item(9), item(3)]), [Some(0), Some(1), Some(2)]);
        // Reordered: every object kept.
        assert_eq!(pair(&old, &[item(3), item(1), item(2)]), [Some(2), Some(0), Some(1)]);
        // One added in front: a new object, the others kept.
        assert_eq!(pair(&old, &[item(0), item(1), item(2), item(3)]), [None, Some(0), Some(1), Some(2)]);
        // One removed: the others kept.
        assert_eq!(pair(&old, &[item(1), item(3)]), [Some(0), Some(2)]);
    }

    #[test]
    fn binding_keys_name_items_only_when_the_item_is_there() {
        let mut n = Node::new(kition_design_doc::NodeId(1), kition_design_doc::NodeKind::Rectangle, "r", 10.0, 10.0);
        n.fills = vec![kition_design_doc::color::Paint::solid(kition_design_doc::color::Color::BLACK)];
        assert_eq!(item_key(&n, "fills/0/color"), Some(("fills", 0, "color")));
        assert_eq!(item_key(&n, "fills/0/stops/1/color"), Some(("fills", 0, "stops/1/color")));
        assert_eq!(item_key(&n, "fills/1/color"), None);
        assert_eq!(item_key(&n, "fills/00/color"), None);
        assert_eq!(item_key(&n, "fill"), None);
        assert_eq!(item_key(&n, "fills/0"), None);
        assert_eq!(item_key(&n, "strokes/0/color"), None);
    }
}
