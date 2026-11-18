//! Making a KDM list field hold a given order with few operations.

use std::collections::HashSet;

use kition_kit::kdm::{ObjectId, Tx};

use crate::KdmResult;

/// Makes `owner.field` hold exactly `desired` (each once), in order: entries `desired` lacks are
/// removed, then each position is fixed by a move or an insert. Answers whether it wrote.
pub(crate) fn sync_list(tx: &mut Tx<'_>, owner: &ObjectId, field: &str, desired: &[ObjectId]) -> KdmResult<bool> {
    let mut current = tx.list_items(owner, field)?;
    if current == desired {
        return Ok(false);
    }
    let want: HashSet<&ObjectId> = desired.iter().collect();
    // The first of each wanted entry stays; the rest go, from the back so indexes hold.
    let mut seen = HashSet::new();
    let keep: Vec<bool> = current.iter().map(|item| want.contains(item) && seen.insert(item.clone())).collect();
    for i in (0..current.len()).rev() {
        if !keep.get(i).copied().unwrap_or(false) {
            tx.list_remove(owner, field, i)?;
            current.remove(i);
        }
    }
    for (at, item) in desired.iter().enumerate() {
        if current.get(at) == Some(item) {
            continue;
        }
        match current.iter().position(|c| c == item) {
            Some(from) => {
                tx.list_move(owner, field, from, at)?;
                let moved = current.remove(from);
                current.insert(at.min(current.len()), moved);
            }
            None => {
                tx.list_insert(owner, field, at.min(current.len()), item)?;
                current.insert(at.min(current.len()), item.clone());
            }
        }
    }
    Ok(true)
}
