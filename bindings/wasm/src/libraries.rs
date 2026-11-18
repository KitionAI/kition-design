//! Team libraries from files: a design uses the components, styles and variables another `.kdesign`
//! file of the workspace publishes, as upstream's library files do (`assets/library.rs` in the
//! engine; `docs/features/kdm.md` section 2.3).
//!
//! **How the files are read.** The engine never reaches a file system here (engine contract E5).
//! A document names its library files as they are stored in it (`LibraryLink.path`): a workspace
//! path relative to the document's folder, or from the workspace root with a leading `/`
//! ([`kition_design_engine::library::lent_path`]), so the design keeps working wherever the workspace
//! is. Every library read goes through [`LentLibraries`], which the binding lends the engine's
//! library cache (`Cache::lend`):
//!
//! - **bytes the editor lent** ([`DesignDocument::lend_library`], the `lendLibrary` extra): the editor
//!   reads each enabled library file with the host's `host/read`, watches its `host/stamp`, and
//!   lends the bytes again when the file changed (or `null` when it is gone). A stamp is the
//!   content's hash, so the engine parses a library again only when it changed, and
//!   `library.updates` sees a newly published version;
//! - otherwise **the host's lent files** (`kition_kit::host`): headless, `kition-design` lends the
//!   workspace through kition-headless (the same rules: relative to the document's folder, never
//!   above the root); in a page, the host's `document.lend` root.
//!
//! The library commands of the engine run as upstream's (`library.add`, `library.place`,
//! `library.acceptUpdates`, …); every library-derived change is one of those local commands, so it
//! is one transaction of the person who ran it and never runs after a merge (KDM F7).

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, PoisonError};

use kition_design_doc::{Document, LibraryLink, NodeKind, PublishedLibrary};
use kition_design_engine::library::{self, LentLibrary, LibraryFiles};
use serde_json::{Value, json};

use crate::DesignDocument;

/// The largest library file read whole (the in-page host keeps at most 256 MiB of lent files).
pub const MAX_LIBRARY_FILE: u64 = 256 * 1024 * 1024;

/// What the editor lent for one path.
#[derive(Clone, Debug)]
enum Lent {
    File(LentLibrary),
    /// The editor could not read it (the host's reason).
    Missing(String),
}

/// The library files of one open design: what the editor lent, else the host's lent files.
#[derive(Debug, Default)]
pub struct LentLibraries {
    lent: Mutex<HashMap<String, Lent>>,
}

impl LentLibraries {
    /// Lends `bytes` as the library file at `path` (`None`: the file can't be read, `missing`
    /// says why). Answers whether that changes what the path reads.
    fn lend(&self, path: &str, bytes: Option<&[u8]>, missing: &str) -> bool {
        let next = match bytes {
            Some(bytes) => {
                Lent::File(LentLibrary { stamp: format!("lent:{}", kition_design_format::image_hash(bytes)), bytes: Arc::new(bytes.to_vec()) })
            }
            None => Lent::Missing(missing.to_owned()),
        };
        let mut lent = self.lent.lock().unwrap_or_else(PoisonError::into_inner);
        let same = match (lent.get(path), &next) {
            (Some(Lent::File(a)), Lent::File(b)) => a.stamp == b.stamp,
            (Some(Lent::Missing(a)), Lent::Missing(b)) => a == b,
            _ => false,
        };
        lent.insert(path.to_owned(), next);
        !same
    }
}

impl LibraryFiles for LentLibraries {
    fn read(&self, path: &str) -> Result<LentLibrary, String> {
        if let Some(lent) = self.lent.lock().unwrap_or_else(PoisonError::into_inner).get(path) {
            return match lent {
                Lent::File(file) => Ok(file.clone()),
                Lent::Missing(why) => Err(why.clone()),
            };
        }
        let size = kition_kit::host::size(path)?;
        if size > MAX_LIBRARY_FILE {
            return Err(format!("{path} is too large to use as a library (the limit is {} MB)", MAX_LIBRARY_FILE >> 20));
        }
        let bytes = kition_kit::host::read(path)?;
        Ok(LentLibrary { stamp: format!("host:{}", kition_design_format::image_hash(&bytes)), bytes: Arc::new(bytes) })
    }
}

impl DesignDocument {
    /// `lendLibrary(path, bytes)`: the editor read the library file `path` (as the document names
    /// it: `library.files` lists them) with the host, and lends its `bytes`; `None` says the file
    /// can't be read (missing, refused). Never an edit and never moves the revision; answers
    /// whether what the path reads changed (re-run the library queries then).
    pub fn lend_library(&mut self, path: &str, bytes: Option<&[u8]>) -> Result<bool, String> {
        let path = library::lent_path(path).map_err(|e| format!("invalid-params: {e}"))?;
        if bytes.is_some_and(|b| u64::try_from(b.len()).unwrap_or(u64::MAX) > MAX_LIBRARY_FILE) {
            return Err(format!("too-large: {path} is larger than {} MB", MAX_LIBRARY_FILE >> 20));
        }
        Ok(self.libraries.lend(&path, bytes, &format!("{path} is not in the workspace")))
    }

    /// `libraryThumbnail(library, id, size)`: component (or set) `id` of enabled library `library`
    /// (its id, name or path), drawn from the library's published snapshot to fit a `size` ×
    /// `size` square, as a PNG: the Assets panel's library tiles. A set draws its default variant.
    pub fn library_thumbnail(&self, library: &str, id: &str, size: f64) -> Result<Vec<u8>, String> {
        let link = library::find_link(&self.ds()?.doc, library).cloned().ok_or_else(|| format!("not-found: no enabled library {library:?}"))?;
        let lib = read(self, &link)?;
        let node: kition_design_doc::NodeId = id.parse().map_err(|_| format!("invalid-params: {id:?} is not a layer id"))?;
        let d = lib.doc.as_ref();
        let shown = match d.node(node).map(|n| &n.kind) {
            Some(NodeKind::ComponentSet { .. }) => kition_design_engine::components::default_variant(d, node).unwrap_or(node),
            _ => node,
        };
        crate::preview::thumbnail_of(d, shown, size)
    }
}

/// The published library of enabled library `link`, read through the session's lent files.
pub(crate) fn read(doc: &DesignDocument, link: &LibraryLink) -> Result<Arc<PublishedLibrary>, String> {
    // A copy of the cache: the parsed libraries are shared, so the next read reuses this one's.
    let mut cache = doc.session().libraries.clone();
    let lib = cache.load(&link.path).map_err(|e| e.to_string())?;
    if lib.manifest.id != link.id {
        return Err(format!("the file now holds another library (\"{}\")", lib.manifest.name));
    }
    Ok(lib)
}

/// Every enabled library of the design with what reading it gave, in the document's order.
pub(crate) fn enabled(doc: &DesignDocument) -> Vec<(LibraryLink, Result<Arc<PublishedLibrary>, String>)> {
    let Ok(ds) = doc.ds() else { return Vec::new() };
    ds.doc.libraries.enabled.iter().map(|link| (link.clone(), read(doc, link))).collect()
}

/// `library.files`: the library files the design names, for the editor to read and lend:
///
/// ```text
/// { files: [{ id, name, path }] }   // path: as stored (relative to the document's folder, or /…)
/// ```
pub fn files(doc: &DesignDocument) -> Result<Value, String> {
    let d: &Document = &doc.ds()?.doc;
    let files: Vec<Value> = d
        .libraries
        .enabled
        .iter()
        .filter(|l| !kition_design_engine::library_server::is_server_link(&l.path))
        .map(|l| json!({ "id": l.id, "name": l.name, "path": l.path }))
        .collect();
    Ok(json!({ "files": files }))
}

/// The enabled libraries whose files can't be read now (library id → why), for the check.
pub(crate) fn unreadable(doc: &DesignDocument) -> BTreeMap<String, (String, String)> {
    enabled(doc).into_iter().filter_map(|(link, read)| read.err().map(|e| (link.id.clone(), (link.name.clone(), e)))).collect()
}
