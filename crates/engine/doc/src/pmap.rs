//! [`PMap`]: a persistent ordered map, a B+ tree whose nodes are shared behind `Arc`s.
//!
//! Cloning a map copies one pointer. A write copies only the nodes on the path to the entry it
//! changes (copy-on-write, one allocation per node), so a clone and the original share every other
//! node. [`PMap::diff`] walks two versions side by side and skips every subtree they share, so
//! comparing an edit with the version it came from costs about the size of the edit, not of the
//! map.

use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cmp::Ordering;
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

/// The most entries a leaf holds and the most children a branch has.
const MAX: usize = 16;
/// A node left with fewer (after a removal) merges with a neighbour, splitting again when the two
/// are too many for one node. Nodes other than the root never have fewer.
const MIN: usize = MAX / 4;
/// How full a map built in one go fills its nodes, leaving room for inserts.
const FILL: usize = MAX * 3 / 4;

/// A persistent ordered map (see the [module documentation](self)).
pub struct PMap<K, V> {
    root: Option<Tree<K, V>>,
    len: usize,
}

/// A node. A leaf holds entries in ascending key order. A branch holds its children, each with
/// the least key it may hold: child `i` holds the keys from its own bound up to the next child's.
/// The first child's bound is never read. Every leaf is at the same depth.
enum Tree<K, V> {
    Leaf(Arc<[(K, V)]>),
    Branch(Arc<[(K, Tree<K, V>)]>),
}

impl<K, V> Clone for Tree<K, V> {
    fn clone(&self) -> Self {
        match self {
            Tree::Leaf(a) => Tree::Leaf(a.clone()),
            Tree::Branch(a) => Tree::Branch(a.clone()),
        }
    }
}

/// Which child of `children` may hold `key`.
fn child_index<K: Ord, T>(children: &[(K, T)], key: &K) -> usize {
    children.get(1..).map_or(0, |rest| rest.partition_point(|(bound, _)| bound <= key))
}

impl<K, V> Tree<K, V> {
    fn size(&self) -> usize {
        match self {
            Tree::Leaf(a) => a.len(),
            Tree::Branch(a) => a.len(),
        }
    }

    fn ptr_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Tree::Leaf(a), Tree::Leaf(b)) => Arc::ptr_eq(a, b),
            (Tree::Branch(a), Tree::Branch(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }

    fn first_key(&self) -> Option<&K> {
        let mut t = self;
        loop {
            match t {
                Tree::Leaf(a) => return a.first().map(|(k, _)| k),
                Tree::Branch(a) => t = &a.first()?.1,
            }
        }
    }

    /// Levels below this node (0 for a leaf).
    fn height(&self) -> usize {
        let mut t = self;
        let mut h = 0;
        while let Tree::Branch(a) = t {
            let Some((_, c)) = a.first() else { break };
            t = c;
            h += 1;
        }
        h
    }
}

impl<K: Ord, V> Tree<K, V> {
    fn get(&self, key: &K) -> Option<&V> {
        let mut t = self;
        loop {
            match t {
                Tree::Leaf(a) => return a.binary_search_by(|(k, _)| k.cmp(key)).ok().and_then(|i| a.get(i)).map(|(_, v)| v),
                Tree::Branch(a) => t = &a.get(child_index(a, key))?.1,
            }
        }
    }

    fn last_at_or_below(&self, hi: &K) -> Option<(&K, &V)> {
        match self {
            Tree::Leaf(a) => {
                let i = a.partition_point(|(k, _)| k <= hi);
                a.get(i.checked_sub(1)?).map(|(k, v)| (k, v))
            }
            // The child that may hold `hi` can have nothing at or below it (its least keys were
            // removed); the one before it then has.
            Tree::Branch(a) => a.get(..=child_index(a, hi))?.iter().rev().find_map(|(_, c)| c.last_at_or_below(hi)),
        }
    }
}

/// A node's entries, and the upper half (with its first key) when it had to split.
type Inserted<K, T> = (Arc<[(K, T)]>, Option<(K, Arc<[(K, T)]>)>);

/// `items` with `item` inserted at `i`: one node, or two when that is more than [`MAX`]. Appending
/// at the right edge of the map splits unevenly, leaving the lower node nearly full, so maps grown
/// by ascending keys stay compact.
fn insert_at<K: Clone, T: Clone>(items: &[(K, T)], i: usize, item: (K, T), right_edge: bool) -> Inserted<K, T> {
    let (before, after) = items.split_at(i.min(items.len()));
    let all = before.iter().cloned().chain(std::iter::once(item)).chain(after.iter().cloned());
    if items.len() < MAX {
        return (all.collect(), None);
    }
    let mut all: Vec<(K, T)> = all.collect();
    let cut = if right_edge && after.is_empty() { all.len() - MIN } else { all.len() / 2 };
    let upper = all.split_off(cut);
    match upper.first() {
        Some((sep, _)) => {
            let sep = sep.clone();
            (all.into(), Some((sep, upper.into())))
        }
        None => (all.into(), None),
    }
}

/// The upper half of a node that split, with its first key.
type Upper<K, V> = (K, Tree<K, V>);

impl<K: Ord + Clone, V: Clone> Tree<K, V> {
    /// Insert or replace. A node that would grow past [`MAX`] splits: the new upper half and its
    /// first key are returned for the parent to add.
    fn insert(&mut self, key: K, value: V, right_edge: bool) -> (Option<V>, Option<Upper<K, V>>) {
        match self {
            Tree::Leaf(a) => match a.binary_search_by(|(k, _)| k.cmp(&key)) {
                Ok(i) => (Arc::make_mut(a).get_mut(i).map(|slot| std::mem::replace(&mut slot.1, value)), None),
                Err(i) => {
                    let (lower, upper) = insert_at(a, i, (key, value), right_edge);
                    *a = lower;
                    (None, upper.map(|(sep, u)| (sep, Tree::Leaf(u))))
                }
            },
            Tree::Branch(a) => {
                let i = child_index(a, &key);
                let last = i + 1 == a.len();
                let Some((_, child)) = Arc::make_mut(a).get_mut(i) else {
                    return (None, None);
                };
                let (old, split) = child.insert(key, value, right_edge && last);
                let Some((sep, upper)) = split else {
                    return (old, None);
                };
                let (lower, upper) = insert_at(a, i + 1, (sep, upper), right_edge && last);
                *a = lower;
                (old, upper.map(|(sep, u)| (sep, Tree::Branch(u))))
            }
        }
    }

    fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        let mut t = self;
        loop {
            match t {
                Tree::Leaf(a) => {
                    let i = a.binary_search_by(|(k, _)| k.cmp(key)).ok()?;
                    return Arc::make_mut(a).get_mut(i).map(|(_, v)| v);
                }
                Tree::Branch(a) => {
                    let i = child_index(a, key);
                    t = &mut Arc::make_mut(a).get_mut(i)?.1;
                }
            }
        }
    }

    fn remove(&mut self, key: &K) -> Option<V> {
        match self {
            Tree::Leaf(a) => {
                let i = a.binary_search_by(|(k, _)| k.cmp(key)).ok()?;
                let out = a.get(i)?.1.clone();
                let (before, after) = (a.get(..i).unwrap_or_default(), a.get(i + 1..).unwrap_or_default());
                *a = before.iter().chain(after).cloned().collect();
                Some(out)
            }
            Tree::Branch(a) => {
                let i = child_index(a, key);
                let (_, child) = Arc::make_mut(a).get_mut(i)?;
                let out = child.remove(key)?;
                if child.size() < MIN {
                    rebalance(a, i);
                }
                Some(out)
            }
        }
    }
}

/// Merge the underfull child `i` of a branch with a neighbour, splitting the two again evenly when
/// they are too many for one node.
fn rebalance<K: Clone, V: Clone>(children: &mut Arc<[(K, Tree<K, V>)]>, i: usize) {
    let n = children.len();
    if n < 2 {
        return;
    }
    let l = if i + 1 < n { i } else { n - 2 };
    let (Some((lbound, left)), Some((rbound, right))) = (children.get(l), children.get(l + 1)) else {
        return;
    };
    let merged: Tree<K, V> = match (left, right) {
        (Tree::Leaf(x), Tree::Leaf(y)) => Tree::Leaf(x.iter().chain(y.iter()).cloned().collect()),
        // The right node's first child gets the right node's bound (its own was never read).
        (Tree::Branch(x), Tree::Branch(y)) => {
            let moved = y.iter().enumerate().map(|(j, (b, c))| (if j == 0 { rbound.clone() } else { b.clone() }, c.clone()));
            Tree::Branch(x.iter().cloned().chain(moved).collect())
        }
        // Siblings are always at the same depth; leave the tree as it is if they are not.
        _ => return,
    };
    let lbound = lbound.clone();
    let halves = match &merged {
        Tree::Leaf(m) if m.len() > MAX => {
            let (lo, hi) = m.split_at(m.len() / 2);
            hi.first().map(|(sep, _)| (Tree::Leaf(lo.into()), sep.clone(), Tree::Leaf(hi.into())))
        }
        Tree::Branch(m) if m.len() > MAX => {
            let (lo, hi) = m.split_at(m.len() / 2);
            hi.first().map(|(sep, _)| (Tree::Branch(lo.into()), sep.clone(), Tree::Branch(hi.into())))
        }
        _ => None,
    };
    let replacement: Vec<(K, Tree<K, V>)> = match halves {
        Some((lo, sep, hi)) => vec![(lbound, lo), (sep, hi)],
        None => vec![(lbound, merged)],
    };
    let (before, after) = (children.get(..l).unwrap_or_default(), children.get(l + 2..).unwrap_or_default());
    *children = before.iter().cloned().chain(replacement).chain(after.iter().cloned()).collect();
}

impl<K, V> PMap<K, V> {
    pub const fn new() -> Self {
        PMap { root: None, len: 0 }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Entries in ascending key order.
    pub fn iter(&self) -> Iter<'_, K, V> {
        let (stack, leaf) = match &self.root {
            None => (Vec::new(), [].iter()),
            Some(Tree::Leaf(a)) => (Vec::new(), a.iter()),
            Some(Tree::Branch(a)) => (vec![a.iter()], [].iter()),
        };
        Iter { stack, leaf, left: self.len }
    }

    pub fn keys(&self) -> impl ExactSizeIterator<Item = &K> + '_ {
        self.iter().map(|(k, _)| k)
    }

    pub fn values(&self) -> impl ExactSizeIterator<Item = &V> + '_ {
        self.iter().map(|(_, v)| v)
    }

    /// Whether both maps are the same version (share their whole tree).
    pub fn ptr_eq(&self, other: &Self) -> bool {
        match (&self.root, &other.root) {
            (Some(a), Some(b)) => a.ptr_eq(b),
            (None, None) => true,
            _ => false,
        }
    }
}

impl<K: Ord, V> PMap<K, V> {
    pub fn get(&self, key: &K) -> Option<&V> {
        self.root.as_ref()?.get(key)
    }

    pub fn contains_key(&self, key: &K) -> bool {
        self.get(key).is_some()
    }

    /// The entry with the greatest key at or below `hi`.
    pub fn last_at_or_below(&self, hi: &K) -> Option<(&K, &V)> {
        self.root.as_ref()?.last_at_or_below(hi)
    }

    /// What changed from `self` to `other`, in ascending key order: entries only `other` has
    /// ([`DiffItem::Add`]), entries only `self` has ([`DiffItem::Remove`]), and entries both have
    /// with values that differ ([`DiffItem::Update`]). Subtrees the two share are skipped.
    pub fn diff<'a>(&'a self, other: &'a Self) -> Diff<'a, K, V, fn(&V, &V) -> bool>
    where
        V: PartialEq,
    {
        self.diff_by(other, <V as PartialEq>::eq)
    }

    /// [`diff`](Self::diff) with `same` telling whether two values of one key are equal (e.g. a
    /// pointer comparison before a deep one).
    pub fn diff_by<'a, F: FnMut(&V, &V) -> bool>(&'a self, other: &'a Self, same: F) -> Diff<'a, K, V, F> {
        let mut d = Diff { old: Vec::new(), new: Vec::new(), same };
        if !self.ptr_eq(other) {
            d.old.extend(self.root.as_ref().map(|t| Item::Tree(t, t.height())));
            d.new.extend(other.root.as_ref().map(|t| Item::Tree(t, t.height())));
        }
        d
    }
}

impl<K: Ord + Clone, V: Clone> PMap<K, V> {
    /// Insert or replace, returning the value replaced.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        let Some(root) = self.root.as_mut() else {
            self.root = Some(Tree::Leaf(Arc::from([(key, value)])));
            self.len = 1;
            return None;
        };
        let (old, split) = root.insert(key, value, true);
        if let Some((sep, upper)) = split
            && let Some(lower) = self.root.take()
        {
            let bound = lower.first_key().cloned().unwrap_or_else(|| sep.clone());
            self.root = Some(Tree::Branch(Arc::from([(bound, lower), (sep, upper)])));
        }
        if old.is_none() {
            self.len += 1;
        }
        old
    }

    /// Mutable access to a value; copies the nodes on its path that another version shares (also
    /// when the key turns out to be absent: the copies are equal in value, so nothing reads
    /// differently, and an edit, the common case, looks the key up only once).
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.root.as_mut()?.get_mut(key)
    }

    /// Remove an entry (copying shared nodes on its path, as [`get_mut`](Self::get_mut) does).
    pub fn remove(&mut self, key: &K) -> Option<V> {
        let out = self.root.as_mut()?.remove(key);
        // An emptied leaf leaves the map empty; a branch with one child gives way to it.
        loop {
            let next = match &self.root {
                Some(Tree::Leaf(a)) if a.is_empty() => None,
                Some(Tree::Branch(a)) if a.len() == 1 => a.first().map(|(_, c)| c.clone()),
                _ => break,
            };
            self.root = next;
        }
        if out.is_some() {
            self.len = self.len.saturating_sub(1);
        }
        out
    }

    /// A map of `entries` in ascending key order with no key repeated, built bottom-up.
    fn from_sorted(entries: Vec<(K, V)>) -> Self {
        let len = entries.len();
        let mut level: Vec<(K, Tree<K, V>)> =
            even_chunks(entries).into_iter().filter_map(|leaf| Some((leaf.first()?.0.clone(), Tree::Leaf(leaf.into())))).collect();
        while level.len() > 1 {
            level = even_chunks(level).into_iter().filter_map(|group| Some((group.first()?.0.clone(), Tree::Branch(group.into())))).collect();
        }
        PMap { root: level.pop().map(|(_, t)| t), len }
    }

    /// Checks the tree's shape: keys in order, bounds that hold, every leaf at the same depth, no
    /// node but the root below [`MIN`] or any above [`MAX`], and the length.
    #[cfg(test)]
    pub(crate) fn check(&self) -> Result<(), String> {
        fn walk<K: Ord, V>(
            t: &Tree<K, V>,
            root: bool,
            lo: Option<&K>,
            hi: Option<&K>,
            depth: usize,
            leaves: &mut Vec<usize>,
        ) -> Result<usize, String> {
            let n = t.size();
            if n > MAX || (!root && n < MIN) || n == 0 {
                return Err(format!("node of {n} at depth {depth}"));
            }
            let in_range = |k: &K| lo.is_none_or(|lo| k >= lo) && hi.is_none_or(|hi| k < hi);
            match t {
                Tree::Leaf(a) => {
                    if !a.windows(2).all(|w| w[0].0 < w[1].0) || !a.iter().all(|(k, _)| in_range(k)) {
                        return Err(format!("leaf keys out of order at depth {depth}"));
                    }
                    leaves.push(depth);
                    Ok(n)
                }
                Tree::Branch(a) => {
                    if n < 2 {
                        return Err(format!("branch of one child at depth {depth}"));
                    }
                    let bounds: Vec<&K> = a.iter().skip(1).map(|(b, _)| b).collect();
                    if !bounds.windows(2).all(|w| w[0] < w[1]) || !bounds.iter().all(|b| in_range(b)) {
                        return Err(format!("bounds out of order at depth {depth}"));
                    }
                    let mut count = 0;
                    for (i, (_, c)) in a.iter().enumerate() {
                        let clo = if i == 0 { lo } else { bounds.get(i - 1).copied() };
                        let chi = bounds.get(i).copied().or(hi);
                        count += walk(c, false, clo, chi, depth + 1, leaves)?;
                    }
                    Ok(count)
                }
            }
        }
        let Some(root) = &self.root else {
            return if self.len == 0 { Ok(()) } else { Err(format!("empty with length {}", self.len)) };
        };
        let mut leaves = Vec::new();
        let count = walk(root, true, None, None, 0, &mut leaves)?;
        if count != self.len {
            return Err(format!("{count} entries, length {}", self.len));
        }
        if !leaves.windows(2).all(|w| w[0] == w[1]) {
            return Err("leaves at different depths".into());
        }
        Ok(())
    }
}

/// `items` in consecutive groups of at most [`FILL`], as even as possible (so each has at least
/// `FILL / 2` when there is more than one).
fn even_chunks<T>(items: Vec<T>) -> Vec<Vec<T>> {
    let n = items.len();
    let groups = n.div_ceil(FILL).max(1);
    let (base, extra) = (n / groups, n % groups);
    let mut it = items.into_iter();
    (0..groups).map(|g| it.by_ref().take(base + usize::from(g < extra)).collect()).collect()
}

impl<K, V> Clone for PMap<K, V> {
    fn clone(&self) -> Self {
        PMap { root: self.root.clone(), len: self.len }
    }
}

impl<K, V> Default for PMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Ord, V: PartialEq> PartialEq for PMap<K, V> {
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len && self.diff(other).next().is_none()
    }
}

impl<K: Ord, V: Eq> Eq for PMap<K, V> {}

impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for PMap<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

/// Later entries replace earlier ones with the same key.
impl<K: Ord + Clone, V: Clone> FromIterator<(K, V)> for PMap<K, V> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let mut entries: Vec<(K, V)> = iter.into_iter().collect();
        // Stable: entries with the same key stay in the order given, and the last one is kept.
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        let mut out: Vec<(K, V)> = Vec::with_capacity(entries.len());
        for e in entries {
            match out.last_mut() {
                Some(last) if last.0 == e.0 => *last = e,
                _ => out.push(e),
            }
        }
        Self::from_sorted(out)
    }
}

/// Like inserting each entry in turn; into an empty map, built in one go.
impl<K: Ord + Clone, V: Clone> Extend<(K, V)> for PMap<K, V> {
    fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, iter: I) {
        if self.is_empty() {
            *self = iter.into_iter().collect();
            return;
        }
        for (k, v) in iter {
            self.insert(k, v);
        }
    }
}

impl<'a, K, V> IntoIterator for &'a PMap<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V>;
    fn into_iter(self) -> Iter<'a, K, V> {
        self.iter()
    }
}

/// A map, written in ascending key order.
impl<K: Serialize, V: Serialize> Serialize for PMap<K, V> {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        let mut m = ser.serialize_map(Some(self.len))?;
        for (k, v) in self.iter() {
            m.serialize_entry(k, v)?;
        }
        m.end()
    }
}

/// A map in any order; a repeated key keeps its last value.
impl<'de, K: Deserialize<'de> + Ord + Clone, V: Deserialize<'de> + Clone> Deserialize<'de> for PMap<K, V> {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        struct MapVisitor<K, V>(PhantomData<(K, V)>);
        impl<'de, K: Deserialize<'de> + Ord + Clone, V: Deserialize<'de> + Clone> Visitor<'de> for MapVisitor<K, V> {
            type Value = PMap<K, V>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a map")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<PMap<K, V>, A::Error> {
                let mut entries = Vec::with_capacity(access.size_hint().unwrap_or(0).min(4096));
                while let Some(e) = access.next_entry()? {
                    entries.push(e);
                }
                Ok(entries.into_iter().collect())
            }
        }
        de.deserialize_map(MapVisitor(PhantomData))
    }
}

/// The entries of a [`PMap`] in ascending key order.
pub struct Iter<'a, K, V> {
    /// The children still to visit, one iterator per branch level above the current leaf.
    stack: Vec<std::slice::Iter<'a, (K, Tree<K, V>)>>,
    leaf: std::slice::Iter<'a, (K, V)>,
    left: usize,
}

impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some((k, v)) = self.leaf.next() {
                self.left = self.left.saturating_sub(1);
                return Some((k, v));
            }
            loop {
                let top = self.stack.last_mut()?;
                match top.next() {
                    None => {
                        self.stack.pop();
                    }
                    Some((_, Tree::Leaf(a))) => {
                        self.leaf = a.iter();
                        break;
                    }
                    Some((_, Tree::Branch(a))) => self.stack.push(a.iter()),
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.left, Some(self.left))
    }
}

impl<K, V> ExactSizeIterator for Iter<'_, K, V> {}

impl<K, V> std::iter::FusedIterator for Iter<'_, K, V> {}

/// One difference [`PMap::diff`] finds.
#[derive(Debug, PartialEq)]
pub enum DiffItem<'a, K, V> {
    Add(&'a K, &'a V),
    Remove(&'a K, &'a V),
    Update { old: (&'a K, &'a V), new: (&'a K, &'a V) },
}

impl<'a, K, V> DiffItem<'a, K, V> {
    pub fn key(&self) -> &'a K {
        match self {
            DiffItem::Add(k, _) | DiffItem::Remove(k, _) | DiffItem::Update { new: (k, _), .. } => k,
        }
    }
}

/// Pending work of one side of a [`Diff`]: a subtree (and its height) or a single entry.
enum Item<'a, K, V> {
    Tree(&'a Tree<K, V>, usize),
    Entry(&'a K, &'a V),
}

impl<K, V> Clone for Item<'_, K, V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K, V> Copy for Item<'_, K, V> {}

/// The differences between two versions of a [`PMap`] (see [`PMap::diff`]).
pub struct Diff<'a, K, V, F> {
    /// Each side's pending items, the next one last; together they cover the rest of that side
    /// in ascending key order.
    old: Vec<Item<'a, K, V>>,
    new: Vec<Item<'a, K, V>>,
    same: F,
}

/// Replace the subtree on top of `stack` with its children or entries.
fn expand<K, V>(stack: &mut Vec<Item<'_, K, V>>) {
    if let Some(Item::Tree(t, h)) = stack.pop() {
        match t {
            Tree::Leaf(a) => stack.extend(a.iter().rev().map(|(k, v)| Item::Entry(k, v))),
            Tree::Branch(a) => stack.extend(a.iter().rev().map(|(_, c)| Item::Tree(c, h.saturating_sub(1)))),
        }
    }
}

impl<'a, K: Ord, V, F: FnMut(&V, &V) -> bool> Iterator for Diff<'a, K, V, F> {
    type Item = DiffItem<'a, K, V>;

    fn next(&mut self) -> Option<DiffItem<'a, K, V>> {
        loop {
            match (self.old.last().copied(), self.new.last().copied()) {
                (None, None) => return None,
                (Some(Item::Entry(k, v)), None) => {
                    self.old.pop();
                    return Some(DiffItem::Remove(k, v));
                }
                (None, Some(Item::Entry(k, v))) => {
                    self.new.pop();
                    return Some(DiffItem::Add(k, v));
                }
                (Some(Item::Tree(..)), None) => expand(&mut self.old),
                (None, Some(Item::Tree(..))) => expand(&mut self.new),
                (Some(Item::Tree(a, ha)), Some(Item::Tree(b, hb))) => {
                    if a.ptr_eq(b) {
                        self.old.pop();
                        self.new.pop();
                        continue;
                    }
                    // Open the subtree that starts first; when both start at the same key, the
                    // taller one, or both, so shared subtrees line up again.
                    let order = match (a.first_key(), b.first_key()) {
                        (Some(ka), Some(kb)) => ka.cmp(kb).then(hb.cmp(&ha)),
                        (None, _) => Ordering::Less,
                        (_, None) => Ordering::Greater,
                    };
                    if order != Ordering::Greater {
                        expand(&mut self.old);
                    }
                    if order != Ordering::Less {
                        expand(&mut self.new);
                    }
                }
                (Some(Item::Tree(a, _)), Some(Item::Entry(k, v))) => {
                    if a.first_key().is_some_and(|first| k < first) {
                        self.new.pop();
                        return Some(DiffItem::Add(k, v));
                    }
                    expand(&mut self.old);
                }
                (Some(Item::Entry(k, v)), Some(Item::Tree(b, _))) => {
                    if b.first_key().is_some_and(|first| k < first) {
                        self.old.pop();
                        return Some(DiffItem::Remove(k, v));
                    }
                    expand(&mut self.new);
                }
                (Some(Item::Entry(ka, va)), Some(Item::Entry(kb, vb))) => match ka.cmp(kb) {
                    Ordering::Less => {
                        self.old.pop();
                        return Some(DiffItem::Remove(ka, va));
                    }
                    Ordering::Greater => {
                        self.new.pop();
                        return Some(DiffItem::Add(kb, vb));
                    }
                    Ordering::Equal => {
                        self.old.pop();
                        self.new.pop();
                        if !(self.same)(va, vb) {
                            return Some(DiffItem::Update { old: (ka, va), new: (kb, vb) });
                        }
                    }
                },
            }
        }
    }
}

impl<K: Ord, V, F: FnMut(&V, &V) -> bool> std::iter::FusedIterator for Diff<'_, K, V, F> {}
