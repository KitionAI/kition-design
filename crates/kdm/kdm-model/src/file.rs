//! The replica inside a `.kdesign` file (`docs/features/kdm.md` section 3): the document's
//! **content**, not its history (kition-sdk data model 7.3, rule M16b).
//!
//! | Entry | Content |
//! | --- | --- |
//! | `kdm/meta.json` | [`Meta`]: what identifies the content |
//! | `kdm/content.bin` | the replica's sealed content snapshot: its state, and of its history only a `system` seal |
//!
//! The blobs are the file's own `images/<sha256>` entries, only those the state references. A file
//! opens as the same document (same docId, same object ids) and is repaired once; a save writes
//! the sealed content again, the same bytes while the document is at the file's version. The
//! history lives in the host's history store (the live replica's `snapshot()`), never in the file.

use std::collections::BTreeSet;
use std::sync::Arc;

use kition_design_doc::Document;
use kition_kit::kdm::{ActorId, KDM_VERSION, KdmError, Origin, OriginKind, Replica, Version};
use kition_kit::session::{KitError, MemoryBlobs};
use serde::{Deserialize, Serialize};

use crate::{DesignSession, KIT, Result, kit_err};

/// Every replica entry's name starts with this.
pub const PREFIX: &str = "kdm/";
pub const META: &str = "kdm/meta.json";
pub const CONTENT: &str = "kdm/content.bin";
/// `meta.json`'s `format`.
pub const FORMAT: &str = "kition.design-content";
pub const FORMAT_VERSION: u32 = 1;

/// `kdm/meta.json`: what identifies the content, nothing else (no title, writer, time or counter).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    pub format: String,
    pub format_version: u32,
    pub kit: String,
    pub kdm: u32,
    pub schema_version: u32,
    pub doc_id: String,
}

/// What a document's file holds of its replica: a save of this version writes the same bytes.
#[derive(Clone, Debug)]
pub struct FileBase {
    /// The replica version the file's content holds (its seal included).
    version: Version,
    doc_id: String,
}

impl FileBase {
    pub fn version(&self) -> &Version {
        &self.version
    }

    pub fn doc_id(&self) -> &str {
        &self.doc_id
    }
}

/// The replica entries of a file, as `(name, bytes)` pairs.
pub type Entries = Vec<(String, Vec<u8>)>;

/// A save: the entries, what the file then holds, and the seal the fallback made.
#[derive(Clone, Debug)]
pub struct Saved {
    pub entries: Entries,
    pub base: FileBase,
    /// The seal of the fallback in [`save`] (a replica nobody sealed): changes to import into the
    /// session once the file is stored. `None` on the direct path.
    pub seal: Option<Vec<u8>>,
}

fn damaged(why: impl std::fmt::Display) -> KitError {
    KitError::new(format!("damaged: {why}"))
}

fn meta_of(replica: &Replica) -> Meta {
    Meta {
        format: FORMAT.into(),
        format_version: FORMAT_VERSION,
        kit: KIT.into(),
        kdm: KDM_VERSION,
        schema_version: replica.schema().schema_version,
        doc_id: replica.doc_id().to_string(),
    }
}

fn entries_of(meta: &Meta, content: Vec<u8>) -> Result<Entries> {
    Ok(vec![(META.into(), serde_json::to_vec_pretty(meta).map_err(kit_err)?), (CONTENT.into(), content)])
}

/// A file's replica entries read: `meta.json` and the content. Errs when either is missing or
/// `meta.json` is not a content file this build reads.
fn read_entries(entries: &[(String, Vec<u8>)]) -> Result<(Meta, Vec<u8>)> {
    let entry = |name: &str| entries.iter().find(|(n, _)| n == name).map(|(_, b)| b);
    let meta: Meta = serde_json::from_slice(entry(META).ok_or_else(|| damaged("no kdm/meta.json"))?).map_err(damaged)?;
    if meta.format != FORMAT {
        return Err(damaged(format!("kdm/meta.json is a {:?}, not a {FORMAT}", meta.format)));
    }
    if meta.format_version > FORMAT_VERSION {
        return Err(KitError::new(format!("the file's content is from a newer Kition Design (format {}); please update", meta.format_version)));
    }
    let content = entry(CONTENT).ok_or_else(|| damaged("no kdm/content.bin"))?.clone();
    Ok((meta, content))
}

/// Loads `content` as a replica of `actor`, checked against the docId `meta` names.
fn load(meta: &Meta, content: &[u8], actor: ActorId) -> Result<Replica> {
    let replica = Replica::load(crate::schema()?, content, actor).map_err(|error| match error {
        KdmError::NewerSchema { .. } => KitError::new(format!("newer-version: {error}")),
        other => damaged(other),
    })?;
    let doc_id = replica.doc_id().to_string();
    if meta.doc_id != doc_id {
        return Err(damaged(format!("kdm/meta.json names document {}, the content {doc_id}", meta.doc_id)));
    }
    Ok(replica)
}

/// Opens a file's replica entries as a replica of a fresh actor, with `blobs` (the file's
/// images): the document the file holds, with its docId and object ids, and of its history only
/// the seal. Repaired once. Errs, opening nothing, when an entry is missing or the content does
/// not load.
pub fn open(entries: &[(String, Vec<u8>)], blobs: MemoryBlobs) -> Result<(DesignSession, FileBase)> {
    let (meta, content) = read_entries(entries)?;
    // Checks the docId before the session is made.
    load(&meta, &content, ActorId::random())?;
    let session = DesignSession::open(&content, &[], blobs)?;
    let base = FileBase { version: session.version(), doc_id: session.doc_id().to_string() };
    Ok((session, base))
}

/// The images (their SHA-256) the content of a file's replica entries names, read before its
/// blobs are (a reader reads those first).
pub fn image_keys(entries: &[(String, Vec<u8>)]) -> Result<BTreeSet<String>> {
    let (meta, content) = read_entries(entries)?;
    let replica = load(&meta, &content, ActorId::random())?;
    images_of(&replica)
}

fn images_of(replica: &Replica) -> Result<BTreeSet<String>> {
    let mut images = BTreeSet::new();
    for object in replica.list(&crate::root_id(), "images").map_err(damaged)? {
        if let Some(key) = replica.get(&object, "key").map_err(damaged)?.as_str() {
            images.insert(key.to_owned());
        }
    }
    Ok(images)
}

/// The document a file's replica entries hold, as the engine reads it (`.kdesign` opening).
pub fn read_content(entries: &[(String, Vec<u8>)], blobs: MemoryBlobs) -> Result<Document> {
    let (session, _) = open(entries, blobs)?;
    Ok((**session.document()).clone())
}

/// An actor derived from `version` alone, so the fallback seals one version the same way.
fn seal_actor(version: &Version) -> ActorId {
    let bytes = version.encode();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    ActorId::from_u64(h >> 1).unwrap_or_else(|_| ActorId::random())
}

/// The replica entries `session` saves: its sealed content snapshot (the same bytes for the same
/// version), and what the file then holds.
///
/// The direct path: the host sealed the session (`KitDocument::prepare_save`,
/// [`DesignSession::seal`]) and its own content snapshot is written.
pub fn save(session: &DesignSession) -> Result<Saved> {
    let replica = session.replica();
    let meta = meta_of(replica);
    let doc_id = meta.doc_id.clone();
    if replica.is_sealed() {
        let entries = entries_of(&meta, replica.sealed_content_snapshot().map_err(kit_err)?)?;
        return Ok(Saved { entries, base: FileBase { version: replica.version(), doc_id }, seal: None });
    }
    // FALLBACK, delete once a kit-editor release that calls prepareSave is in web/pnpm-lock.yaml:
    // until then the editor saves a session nobody sealed, and `save` sees it read-only. The
    // content of a copy sealed under an actor derived from the version (the same version gives
    // the same bytes); `mark_saved` imports the seal.
    let version = replica.version();
    let mut sealed = replica.fork(seal_actor(&version)).map_err(kit_err)?;
    sealed.seal().map_err(kit_err)?;
    let seal = sealed.export_changes(&version).map_err(kit_err)?;
    let entries = entries_of(&meta, sealed.sealed_content_snapshot().map_err(kit_err)?)?;
    Ok(Saved { entries, base: FileBase { version: sealed.version(), doc_id }, seal: Some(seal) })
}

/// The replica entries of a document no session holds (a file written without the bridge): a new
/// replica of `doc`, sealed, under a new docId.
pub fn content_entries(doc: &Arc<Document>) -> Result<Entries> {
    let mut session = DesignSession::create(doc, &Origin::system("create"))?;
    session.seal()?;
    Ok(save(&session)?.entries)
}

/// The replica entries of the same document as `entries` (a file's) whose content is now `doc`:
/// another program's edit of the design, written as one `import` transaction of the file's
/// replica and sealed, so the file keeps its document id and object ids.
pub fn rewrite(entries: &[(String, Vec<u8>)], doc: &Arc<Document>) -> Result<Entries> {
    let (mut session, _) = open(entries, crate::blobs_of(doc))?;
    session.record(&Origin::new(OriginKind::Import, "file").with_label("file"), doc)?;
    session.seal()?;
    Ok(save(&session)?.entries)
}

/// The replica a file's `kdm/` entries hold, as one KDM snapshot (`document.identity`, KitModel
/// M7a): its content, checked against `meta.json`'s docId, or `None` when the file has no replica
/// entries (an import). Errs, as [`open`] does, when the entries are damaged or of a newer format;
/// never repairs, never writes.
pub fn embedded_replica(entries: &[(String, Vec<u8>)]) -> Result<Option<Vec<u8>>> {
    if entries.is_empty() {
        return Ok(None);
    }
    let (meta, content) = read_entries(entries)?;
    load(&meta, &content, ActorId::random())?;
    Ok(Some(content))
}

/// The replica entries of a file that holds the document `snapshot` (`document.fork`, KitModel
/// M7a): `meta.json` with the snapshot's docId and its content (a copy's birth is its seal, so a
/// copy adds no change), so no earlier identity or history is left; and the images (their
/// SHA-256) its state references.
pub fn fork_entries(snapshot: &[u8]) -> Result<(Entries, BTreeSet<String>)> {
    let mut replica = Replica::load(crate::schema()?, snapshot, ActorId::random()).map_err(damaged)?;
    let images = images_of(&replica)?;
    let meta = meta_of(&replica);
    let content = replica.content_snapshot().map_err(kit_err)?;
    Ok((entries_of(&meta, content)?, images))
}
