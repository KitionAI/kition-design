//! What the document relies on from its node store: copies are independent and share the nodes
//! neither side touched, `changed_ids` lists exactly the ids whose node differs (ascending, the
//! limit honoured), and nodes serialize as a map in id order.

use crate::{Document, NodeId, NodeKind};
use serde::Deserialize;
use std::sync::Arc;

/// A small deterministic generator (xorshift64*), so the tests need no dependency.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// Two pages of frames with rectangles in them: `frames * per_frame` leaves.
fn big(frames: usize, per_frame: usize) -> Document {
    let mut d = Document::new("big");
    let p1 = d.first_page().unwrap();
    let p2 = d.add_page("Page 2");
    for f in 0..frames {
        let page = if f % 2 == 0 { p1 } else { p2 };
        let frame = d.create(page, NodeKind::Frame, &format!("Frame {f}"), f as f64 * 10.0, 0.0, 100.0, 100.0).unwrap();
        for r in 0..per_frame {
            d.create(frame, NodeKind::Rectangle, &format!("R {f}.{r}"), r as f64, r as f64, 5.0, 5.0).unwrap();
        }
    }
    d
}

/// Every id whose node differs between `a` and `b` by value or presence, ascending.
fn brute_force_changes(a: &Document, b: &Document) -> Vec<NodeId> {
    let mut ids: Vec<NodeId> = a.ids().chain(b.ids()).collect();
    ids.sort();
    ids.dedup();
    ids.retain(|id| a.node(*id) != b.node(*id));
    ids
}

/// One random edit of the kinds commands make.
fn random_edit(d: &mut Document, rng: &mut Rng) {
    let ids: Vec<NodeId> = d.ids().filter(|id| !d.node(*id).unwrap().is_page()).collect();
    if ids.is_empty() {
        return;
    }
    let id = ids[rng.below(ids.len())];
    match rng.below(5) {
        0 => d.get_mut(id).unwrap().name = format!("Edited {}", rng.next()),
        1 => d.get_mut(id).unwrap().opacity = 0.5,
        2 => d.remove(id).unwrap(),
        3 => {
            let parent = d.node(id).unwrap().parent.unwrap();
            d.create(parent, NodeKind::Ellipse { start: 0.0, sweep: 1.0, inner: 0.0 }, "New", 1.0, 1.0, 2.0, 2.0).unwrap();
        }
        // Copied on write but equal in value: not a change.
        _ => {
            let _ = d.get_mut(id).unwrap();
        }
    }
}

#[test]
fn copies_are_independent_and_share_untouched_nodes() {
    let mut a = big(40, 60);
    let before = serde_json::to_string(&a).unwrap();
    let count = a.node_count();
    let ids: Vec<NodeId> = a.ids().collect();
    let mut b = a.clone();
    for id in &ids {
        assert!(Arc::ptr_eq(a.node_arc(*id).unwrap(), b.node_arc(*id).unwrap()), "a fresh copy shares node {id}");
    }
    let edited = ids[ids.len() / 2];
    let edited_name = a.node(edited).unwrap().name.clone();
    b.get_mut(edited).unwrap().name = "Changed".into();
    let gone = ids[ids.len() - 1];
    b.remove(gone).unwrap();
    let page = b.first_page().unwrap();
    let added = b.create(page, NodeKind::Rectangle, "Added", 0.0, 0.0, 1.0, 1.0).unwrap();
    assert_eq!(serde_json::to_string(&a).unwrap(), before, "editing the copy leaves the original as it was");
    assert_eq!(a.node_count(), count);
    assert!(a.contains(gone) && !b.contains(gone));
    assert!(!a.contains(added) && b.contains(added));
    assert_eq!(a.node(edited).unwrap().name, edited_name);
    assert!(!Arc::ptr_eq(a.node_arc(edited).unwrap(), b.node_arc(edited).unwrap()), "the edited node was copied");
    let parent_of_gone = a.node(gone).unwrap().parent.unwrap();
    for id in ids.iter().filter(|i| ![edited, gone, page, parent_of_gone].contains(*i)) {
        assert!(Arc::ptr_eq(a.node_arc(*id).unwrap(), b.node_arc(*id).unwrap()), "untouched node {id} is still shared");
    }
    // And the other way round: editing the original leaves the copy alone.
    let b_before = serde_json::to_string(&b).unwrap();
    a.get_mut(ids[3]).unwrap().opacity = 0.25;
    a.remove(ids[ids.len() / 3]).unwrap();
    assert_eq!(serde_json::to_string(&b).unwrap(), b_before);
}

#[test]
fn changed_ids_matches_a_full_comparison() {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let base = big(30, 50);
    for round in 0..40 {
        let mut next = base.clone();
        let edits = 1 + rng.below(if round % 4 == 0 { 200 } else { 6 });
        for _ in 0..edits {
            random_edit(&mut next, &mut rng);
        }
        let want = brute_force_changes(&base, &next);
        assert_eq!(next.changed_ids(&base, usize::MAX), Some(want.clone()), "round {round}");
        assert_eq!(base.changed_ids(&next, usize::MAX), Some(want.clone()), "round {round}, reversed");
        assert_eq!(next.changed_ids(&base, want.len()), Some(want.clone()), "exactly the limit");
        if !want.is_empty() {
            assert_eq!(next.changed_ids(&base, want.len() - 1), None, "one over the limit");
        }
    }
    // Chains of versions, each compared with the one before (undo history, render frames).
    let mut prev = base.clone();
    for step in 0..60 {
        let mut next = prev.clone();
        random_edit(&mut next, &mut rng);
        assert_eq!(next.changed_ids(&prev, usize::MAX), Some(brute_force_changes(&prev, &next)), "step {step}");
        prev = next;
    }
    assert_eq!(prev.changed_ids(&base, usize::MAX), Some(brute_force_changes(&base, &prev)), "across the chain");
    // Two documents built separately (sharing nothing) compare by value.
    let other = big(30, 50);
    assert_eq!(other.changed_ids(&base, 0), Some(vec![]));
    assert_eq!(base, other);
}

/// The keys of a serialized map, in the order they were written.
struct Keys(Vec<u64>);

impl<'de> Deserialize<'de> for Keys {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = Keys;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a map")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut m: A) -> Result<Keys, A::Error> {
                let mut keys = Vec::new();
                while let Some((k, _)) = m.next_entry::<u64, serde::de::IgnoredAny>()? {
                    keys.push(k);
                }
                Ok(Keys(keys))
            }
        }
        d.deserialize_map(V)
    }
}

#[derive(Deserialize)]
struct Shape {
    nodes: Keys,
}

#[test]
fn nodes_serialize_as_a_map_in_id_order_and_load_in_any_order() {
    let d = big(12, 15);
    let json = serde_json::to_string(&d).unwrap();
    let keys = serde_json::from_str::<Shape>(&json).unwrap().nodes.0;
    let want: Vec<u64> = d.ids().map(|i| i.0).collect();
    assert_eq!(keys, want);
    assert!(keys.windows(2).all(|w| w[0] < w[1]), "ascending by number, not by text");
    assert!(keys.len() > 100 && keys.iter().any(|k| *k >= 100), "ids past two digits are in the sample");
    assert_eq!(serde_json::from_str::<Document>(&json).unwrap(), d, "round trip");
    // Written in reverse order (by another writer), it loads as the same document.
    let mut value: serde_json::Value = serde_json::from_str(&json).unwrap();
    let nodes = value["nodes"].as_object().unwrap().clone();
    let mut reversed: Vec<(String, serde_json::Value)> = nodes.into_iter().collect();
    reversed.sort_by_key(|(k, _)| std::cmp::Reverse(k.parse::<u64>().unwrap()));
    let body: Vec<String> = reversed.iter().map(|(k, v)| format!("{}:{}", serde_json::to_string(k).unwrap(), v)).collect();
    value["nodes"] = serde_json::Value::Null;
    let skeleton = serde_json::to_string(&value).unwrap();
    let with_nodes = |entries: &[String]| skeleton.replace("\"nodes\":null", &format!("\"nodes\":{{{}}}", entries.join(",")));
    assert_eq!(serde_json::from_str::<Document>(&with_nodes(&body)).unwrap(), d);
    // A repeated id: the later entry wins.
    let first = d.ids().nth(5).unwrap();
    let entry = serde_json::to_string(&serde_json::to_value(d.node(first).unwrap()).unwrap()).unwrap();
    let renamed = entry.replacen(&format!("\"name\":{}", serde_json::to_string(&d.node(first).unwrap().name).unwrap()), "\"name\":\"Later\"", 1);
    assert_ne!(entry, renamed);
    let mut dup = body.clone();
    dup.push(format!("\"{first}\":{renamed}"));
    let dup = with_nodes(&dup);
    let loaded = serde_json::from_str::<Document>(&dup).unwrap();
    assert_eq!(loaded.node(first).unwrap().name, "Later");
    assert_eq!(loaded.node_count(), d.node_count());
}

#[test]
fn client_id_allocation_continues_after_the_highest_id_of_its_space() {
    // `set_client_id` finds the highest id inside one client's id range.
    let mut d = big(3, 3);
    let page = d.first_page().unwrap();
    d.set_client_id(7);
    let a = d.create(page, NodeKind::Rectangle, "a", 0.0, 0.0, 1.0, 1.0).unwrap();
    let b = d.create(page, NodeKind::Rectangle, "b", 0.0, 0.0, 1.0, 1.0).unwrap();
    d.set_client_id(9);
    let c = d.create(page, NodeKind::Rectangle, "c", 0.0, 0.0, 1.0, 1.0).unwrap();
    assert_eq!(a.0 >> 32, 7);
    assert_eq!(c.0 >> 32, 9);
    // A fresh replica of the same file, joining as client 7 again, continues after `b`.
    let mut e = d.clone();
    e.set_client_id(0);
    e.set_client_id(7);
    assert_eq!(e.next_id(), NodeId(b.0 + 1));
    // Client 8 has no ids yet: it starts at the bottom of its range.
    e.set_client_id(8);
    assert_eq!(e.next_id(), NodeId((8u64 << 32) | 1));
}
