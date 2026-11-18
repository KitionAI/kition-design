//! `PMap` against `BTreeMap`: the same contents after any sequence of writes, versions that stay
//! independent, a diff that equals a full comparison, and a tree that keeps its shape.

use crate::pmap::{DiffItem, PMap};
use std::collections::BTreeMap;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
}

type Change = (u64, Option<u32>, Option<u32>);

/// Every key whose value differs, with the old and new value, ascending.
fn brute_force(a: &BTreeMap<u64, u32>, b: &BTreeMap<u64, u32>) -> Vec<Change> {
    let mut keys: Vec<u64> = a.keys().chain(b.keys()).copied().collect();
    keys.sort();
    keys.dedup();
    keys.into_iter().map(|k| (k, a.get(&k).copied(), b.get(&k).copied())).filter(|(_, x, y)| x != y).collect()
}

fn diff(a: &PMap<u64, u32>, b: &PMap<u64, u32>) -> Vec<Change> {
    a.diff(b)
        .map(|d| match d {
            DiffItem::Add(k, v) => (*k, None, Some(*v)),
            DiffItem::Remove(k, v) => (*k, Some(*v), None),
            DiffItem::Update { old: (k, x), new: (k2, y) } => {
                assert_eq!(k, k2);
                (*k, Some(*x), Some(*y))
            }
        })
        .collect()
}

fn same(p: &PMap<u64, u32>, m: &BTreeMap<u64, u32>) {
    p.check().unwrap();
    assert_eq!(p.len(), m.len());
    assert_eq!(p.iter().len(), m.len());
    assert!(p.iter().map(|(k, v)| (*k, *v)).eq(m.iter().map(|(k, v)| (*k, *v))));
}

/// One random write to both maps.
fn write(p: &mut PMap<u64, u32>, m: &mut BTreeMap<u64, u32>, rng: &mut Rng, keys: u64) {
    let k = rng.below(keys);
    match rng.below(10) {
        0..=3 => {
            let v = rng.below(1000) as u32;
            assert_eq!(p.insert(k, v), m.insert(k, v), "insert {k}");
        }
        4..=6 => assert_eq!(p.remove(&k), m.remove(&k), "remove {k}"),
        7 => {
            let v = rng.below(1000) as u32;
            if let Some(x) = p.get_mut(&k) {
                *x = v;
            }
            if let Some(x) = m.get_mut(&k) {
                *x = v;
            }
        }
        // Write the value it already has: a copied path that is equal in value.
        8 => {
            if let Some(x) = p.get_mut(&k) {
                *x += 0;
            }
        }
        _ => assert_eq!(p.get(&k), m.get(&k), "get {k}"),
    }
}

#[test]
fn random_writes_match_a_btree_map_and_versions_stay_independent() {
    let mut rng = Rng(0x1234_5678_9abc_def1);
    for (keys, steps) in [(40u64, 4_000usize), (3_000, 30_000), (60_000, 30_000)] {
        let mut p = PMap::new();
        let mut m = BTreeMap::new();
        let mut versions: Vec<(PMap<u64, u32>, BTreeMap<u64, u32>)> = Vec::new();
        for step in 0..steps {
            write(&mut p, &mut m, &mut rng, keys);
            if step % 997 == 0 {
                versions.push((p.clone(), m.clone()));
            }
        }
        same(&p, &m);
        // Every saved version still holds what it held, though later writes went on from it.
        for (vp, vm) in &versions {
            same(vp, vm);
        }
        // Diffs between versions equal a full comparison, both ways.
        for (i, (ap, am)) in versions.iter().enumerate() {
            for (bp, bm) in versions.iter().skip(i).take(4).map(|(a, b)| (a, b)).chain(std::iter::once((&p, &m))) {
                assert_eq!(diff(ap, bp), brute_force(am, bm), "{keys} keys");
                assert_eq!(diff(bp, ap), brute_force(bm, am), "{keys} keys, reversed");
                assert_eq!(*ap == *bp, am == bm);
            }
        }
    }
}

#[test]
fn diffs_after_few_or_many_edits_match_a_full_comparison() {
    let mut rng = Rng(7);
    let base: PMap<u64, u32> = (0..50_000u64).map(|k| (k * 3, k as u32)).collect();
    base.check().unwrap();
    let base_m: BTreeMap<u64, u32> = base.iter().map(|(k, v)| (*k, *v)).collect();
    for round in 0..50 {
        let (mut p, mut m) = (base.clone(), base_m.clone());
        for _ in 0..1 + rng.below(if round % 5 == 0 { 2_000 } else { 5 }) {
            write(&mut p, &mut m, &mut rng, 150_000);
        }
        same(&p, &m);
        same(&base, &base_m);
        assert_eq!(diff(&base, &p), brute_force(&base_m, &m), "round {round}");
        assert_eq!(diff(&p, &base), brute_force(&m, &base_m), "round {round}, reversed");
    }
    // Maps built separately, sharing nothing, compare by value.
    let other: PMap<u64, u32> = (0..50_000u64).rev().map(|k| (k * 3, k as u32)).collect();
    assert!(!other.ptr_eq(&base));
    assert_eq!(other, base);
    assert_eq!(diff(&other, &base), vec![]);
}

#[test]
fn building_keeps_the_last_of_repeated_keys_and_any_size_has_its_shape() {
    for n in [0usize, 1, 2, 7, 8, 23, 24, 25, 31, 32, 33, 100, 577, 1_000, 24 * 24, 24 * 24 + 1, 40_000] {
        let p: PMap<u64, u32> = (0..n as u64).map(|k| (k, k as u32)).collect();
        p.check().unwrap();
        assert_eq!(p.len(), n);
        assert!(p.keys().copied().eq(0..n as u64));
        // And then shrinking it one entry at a time keeps the shape.
        let mut q = p.clone();
        for k in (0..n as u64).step_by(2).chain((1..n as u64).step_by(2)) {
            assert_eq!(q.remove(&k), Some(k as u32));
            if k % 101 == 0 {
                q.check().unwrap();
            }
        }
        assert!(q.is_empty());
        q.check().unwrap();
        assert_eq!(p.len(), n, "the original is untouched");
    }
    let p: PMap<u64, &str> = [(3, "a"), (1, "b"), (3, "c"), (2, "d"), (1, "e")].into_iter().collect();
    assert_eq!(p.iter().map(|(k, v)| (*k, *v)).collect::<Vec<_>>(), vec![(1, "e"), (2, "d"), (3, "c")]);
}

#[test]
fn last_at_or_below_finds_the_greatest_key_in_range() {
    let mut p: PMap<u64, ()> = (0..10_000u64).map(|k| (k * 10, ())).collect();
    // Remove whole runs so some separators no longer name a key.
    for k in 2_000..4_000u64 {
        p.remove(&(k * 10));
    }
    p.check().unwrap();
    let m: BTreeMap<u64, ()> = p.iter().map(|(k, v)| (*k, *v)).collect();
    let mut rng = Rng(99);
    for hi in (0..20).chain((0..2_000).map(|_| rng.below(110_000))) {
        assert_eq!(p.last_at_or_below(&hi).map(|(k, _)| *k), m.range(..=hi).next_back().map(|(k, _)| *k), "{hi}");
    }
    assert_eq!(PMap::<u64, ()>::new().last_at_or_below(&5), None);
}

#[test]
fn serializes_as_a_map_in_key_order() {
    let p: PMap<u64, String> = [(10, "ten".to_string()), (9, "nine".to_string()), (100, "hundred".to_string())].into_iter().collect();
    let json = serde_json::to_string(&p).unwrap();
    assert_eq!(json, r#"{"9":"nine","10":"ten","100":"hundred"}"#);
    assert_eq!(serde_json::from_str::<PMap<u64, String>>(&json).unwrap(), p);
    let dup = serde_json::from_str::<PMap<u64, String>>(r#"{"100":"x","9":"nine","100":"hundred","10":"ten"}"#).unwrap();
    assert_eq!(dup, p);
    assert_eq!(format!("{p:?}"), r#"{9: "nine", 10: "ten", 100: "hundred"}"#);
}

#[test]
fn ascending_and_descending_inserts_keep_the_shape() {
    // Ascending keys (new ids) split the right edge unevenly; the tree stays valid and compact.
    let mut up = PMap::new();
    let mut down = PMap::new();
    for k in 0..20_000u64 {
        up.insert(k, k as u32);
        down.insert(u64::MAX - k, k as u32);
        if k % 1_009 == 0 {
            up.check().unwrap();
            down.check().unwrap();
        }
    }
    up.check().unwrap();
    down.check().unwrap();
    assert!(up.keys().copied().eq(0..20_000u64));
    assert!(down.values().copied().eq((0..20_000u32).rev()));
    // A version taken midway shares the old part and diffs as the added keys only.
    let half: PMap<u64, u32> = (0..10_000u64).map(|k| (k, k as u32)).collect();
    let mut grown = half.clone();
    for k in 10_000..10_500u64 {
        grown.insert(k, 0);
    }
    let added: Vec<u64> = half.diff(&grown).map(|d| *d.key()).collect();
    assert_eq!(added, (10_000..10_500u64).collect::<Vec<_>>());
    // `diff_by` decides equality of the values both versions hold.
    let mut bumped = half.clone();
    *bumped.get_mut(&7).unwrap() += 2;
    *bumped.get_mut(&8).unwrap() += 1;
    let odd_changes: Vec<u64> = half.diff_by(&bumped, |a, b| (a % 2) == (b % 2)).map(|d| *d.key()).collect();
    assert_eq!(odd_changes, vec![8]);
}

#[test]
fn extend_is_inserting_each_entry_in_turn() {
    let entries: Vec<(u64, u32)> = (0..5_000u64).map(|k| ((k * 7919) % 6_000, k as u32)).collect();
    let mut want = BTreeMap::new();
    for (k, v) in &entries {
        want.insert(*k, *v);
    }
    // Into an empty map (built in one go) and into a map that has entries (inserted one by one).
    let mut empty = PMap::new();
    empty.extend(entries.iter().copied());
    same(&empty, &want);
    let mut some: PMap<u64, u32> = [(3, 1), (100_000, 2)].into_iter().collect();
    some.extend(entries.iter().copied());
    want.insert(100_000, 2);
    if !entries.iter().any(|(k, _)| *k == 3) {
        want.insert(3, 1);
    }
    same(&some, &want);
    // A missing key edits nothing.
    let base = some.clone();
    assert!(some.get_mut(&99_999).is_none());
    assert_eq!(some, base);
    assert_eq!(diff(&base, &some), vec![]);
}
