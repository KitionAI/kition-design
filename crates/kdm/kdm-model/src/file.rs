//! The replica inside a `.kdesign` file (`docs/features/kdm.md` section 3): the zip's `kdm/` entries.
//!
//! | Entry | Content |
//! | --- | --- |
//! | `kdm/meta.json` | [`Meta`] |
//! | `kdm/snapshot.bin` | a KDM snapshot envelope at the last compaction |
//! | `kdm/changes/<seq>.bin` | KDM change envelopes, in replay order (`000001`, `000002`, …) |
//!
//! The blobs are the file's own `images/<sha256>` entries. [`open`] loads the snapshot and the
//! listed changes; [`save`] re-emits the snapshot and the change entries it read or wrote byte for
//! byte and appends one entry with the changes since (P5), or writes a fresh snapshot and no
//! entries when the log has grown past [`wants_compaction`]'s thresholds.

use std::sync::Arc;

use kition_kit::kdm::{KDM_VERSION, Version};
use kition_kit::session::{KitError, MemoryBlobs};
use serde::{Deserialize, Serialize};

use crate::{DesignSession, KIT, Result, kit_err};

/// Every replica entry's name starts with this.
pub const PREFIX: &str = "kdm/";
pub const META: &str = "kdm/meta.json";
pub const SNAPSHOT: &str = "kdm/snapshot.bin";
/// `meta.json`'s `format`.
pub const FORMAT: &str = "kition.design-replica";
pub const FORMAT_VERSION: u32 = 1;
/// Most change entries before a save compacts.
pub const MAX_CHANGE_ENTRIES: usize = 200;
/// Most entries one file may list (more is a damaged file).
const MAX_LISTED: usize = 100_000;

/// `kdm/meta.json`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    pub format: String,
    pub format_version: u32,
    pub kdm: u32,
    pub schema_version: u32,
    pub doc_id: String,
    pub kit: String,
    #[serde(default)]
    pub changes: Vec<String>,
}

/// The name of change entry `seq` (from 1).
pub fn change_entry(seq: usize) -> String {
    format!("{PREFIX}changes/{seq:06}.bin")
}

/// A log smaller than this never compacts (a small document's snapshot is cheap anyway).
const MIN_COMPACTED_LOG: usize = 64 * 1024;

/// Whether a log of `changes` after a snapshot of `snapshot_len` bytes should be compacted: more
/// than [`MAX_CHANGE_ENTRIES`] entries, or more bytes than a quarter of the snapshot (and than
/// 64 KiB).
pub fn wants_compaction(snapshot_len: usize, changes: &[Arc<Vec<u8>>]) -> bool {
    let bytes: usize = changes.iter().map(|c| c.len()).sum();
    changes.len() > MAX_CHANGE_ENTRIES || bytes > (snapshot_len / 4).max(MIN_COMPACTED_LOG)
}

/// What a session's file holds, so the next save re-emits it and appends what changed since.
#[derive(Clone, Debug)]
pub struct FileBase {
    snapshot: Arc<Vec<u8>>,
    changes: Vec<Arc<Vec<u8>>>,
    /// The replica version the file's snapshot and changes add up to.
    version: Version,
    doc_id: String,
}

impl FileBase {
    pub fn version(&self) -> &Version {
        &self.version
    }

    pub fn change_entries(&self) -> usize {
        self.changes.len()
    }
}

/// The replica entries of a file, as `(name, bytes)` pairs.
pub type Entries = Vec<(String, Vec<u8>)>;

fn damaged(why: impl std::fmt::Display) -> KitError {
    KitError::new(format!("damaged: {why}"))
}

/// Opens a file's replica entries as a replica of a fresh actor, with `blobs` (the file's
/// images). Errs, opening nothing, when an entry is missing or the replica does not load.
pub fn open(entries: &[(String, Vec<u8>)], blobs: MemoryBlobs) -> Result<(DesignSession, FileBase)> {
    let entry = |name: &str| entries.iter().find(|(n, _)| n == name).map(|(_, b)| b);
    let meta: Meta = serde_json::from_slice(entry(META).ok_or_else(|| damaged("no kdm/meta.json"))?).map_err(damaged)?;
    if meta.format != FORMAT {
        return Err(damaged(format!("kdm/meta.json is a {:?}, not a {FORMAT}", meta.format)));
    }
    if meta.format_version > FORMAT_VERSION {
        return Err(KitError::new(format!("the file's replica is from a newer Kition Design (format {}); please update", meta.format_version)));
    }
    if meta.changes.len() > MAX_LISTED {
        return Err(damaged("too many change entries"));
    }
    let snapshot = entry(SNAPSHOT).ok_or_else(|| damaged("no kdm/snapshot.bin"))?.clone();
    let mut changes = Vec::with_capacity(meta.changes.len());
    for name in &meta.changes {
        if !name.starts_with(PREFIX) || name.contains("..") {
            return Err(damaged(format!("{name} is not a replica entry")));
        }
        changes.push(entry(name).ok_or_else(|| damaged(format!("{name} is listed but missing")))?.clone());
    }
    let session = DesignSession::open(&snapshot, &changes, blobs)?;
    let doc_id = session.doc_id().to_string();
    if !meta.doc_id.is_empty() && meta.doc_id != doc_id {
        return Err(damaged(format!("kdm/meta.json names document {}, the snapshot {doc_id}", meta.doc_id)));
    }
    let version = session.version();
    Ok((session, FileBase { snapshot: Arc::new(snapshot), changes: changes.into_iter().map(Arc::new).collect(), version, doc_id }))
}

/// The replica entries `session` saves, on top of `base` (what it opened from or last saved, when
/// it is the same document), and what the file then holds. A save with nothing new appends
/// nothing.
pub fn save(session: &DesignSession, base: Option<&FileBase>) -> Result<(Entries, FileBase)> {
    let doc_id = session.doc_id().to_string();
    let version = session.version();
    let base = base.filter(|b| b.doc_id == doc_id);
    let (mut snapshot, mut changes) = match base {
        Some(b) => (b.snapshot.clone(), b.changes.clone()),
        None => (Arc::new(session.snapshot()?), Vec::new()),
    };
    if let Some(b) = base
        && b.version != version
    {
        changes.push(Arc::new(session.export_changes(&b.version)?));
    }
    if base.is_some() && wants_compaction(snapshot.len(), &changes) {
        snapshot = Arc::new(session.snapshot()?);
        changes.clear();
    }
    let names: Vec<String> = (1..=changes.len()).map(change_entry).collect();
    let meta = Meta {
        format: FORMAT.into(),
        format_version: FORMAT_VERSION,
        kdm: KDM_VERSION,
        schema_version: session.replica().schema().schema_version,
        doc_id: doc_id.clone(),
        kit: KIT.into(),
        changes: names.clone(),
    };
    let mut entries: Entries = Vec::with_capacity(changes.len() + 2);
    entries.push((META.into(), serde_json::to_vec_pretty(&meta).map_err(kit_err)?));
    entries.push((SNAPSHOT.into(), (*snapshot).clone()));
    for (name, change) in names.into_iter().zip(&changes) {
        entries.push((name, (**change).clone()));
    }
    Ok((entries, FileBase { snapshot, changes, version, doc_id }))
}
