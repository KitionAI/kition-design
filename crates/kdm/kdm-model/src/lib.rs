//! Bridges the Design engine's [`Document`] ([`kition_design_doc`]) to a `kit.design` KDM replica
//! (`docs/features/kdm.md`).
//!
//! - [`write_document`] writes a whole document into a replica's creating transaction and
//!   [`write_changes`] writes only what differs between two documents (the engine's command
//!   bridge: the engine edits its own document and the difference is the transaction);
//! - [`read_document`] reads a replica back into a document, reusing every node of a previous
//!   view whose object did not change ([`read_with`]);
//! - [`validate`] is the deterministic repair after a merge (`docs/features/kdm.md` section 5);
//! - [`DesignSession`] is one document's replica, its view and its blobs, for the engine;
//! - [`file`] keeps the replica in a `.kdesign` file's `kdm/` entries.
//!
//! **What lives where** (`model/schema.json`): every layer of every page is a `Node` object named
//! by one node of the `Design.nodes` tree (pages are its roots), and the library store's layers by
//! nodes of `Design.libraryStore`; a node's registers group its fields by coupling ([`fields`]),
//! its paints and effects are objects of its lists ([`items`]) and a text layer's characters its
//! rich `text` field ([`text`]).
//! Styles, variable collections (with their modes), variables, images (blobs), comment threads
//! (KitModel M23), library links and the records of library copies are objects in lists of the
//! root; every other document-level value is one key of the `Design.settings` map.
//!
//! **Ids are data.** The engine's ids (node ids, style, variable, collection and mode ids, comment
//! ids) are stored in the objects (`key`), so every replica reads the same ids and every reference
//! inside a register means the same layer everywhere. New ids come from the session's own id
//! space ([`Document::set_client_id`]), which the engine's bridge sets.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod fields;
pub mod file;
mod ids;
pub mod items;
mod list;
mod merge;
mod read;
mod session;
mod store;
pub mod text;
mod validate;
mod write;

use std::sync::{Arc, OnceLock};

use kition_design_doc::Document;
use kition_kit::kdm::{KdmError, ObjectId, Schema};
use kition_kit::session::KitError;

pub use ids::Ids;
pub use read::{read_document, read_with};
pub use session::{DesignSession, blobs_of, undoable};
pub use validate::{StoreTexts, repair_comment_ids, store_texts, validate};
pub use write::{write_changes, write_document};

/// Re-exported so the engine and the bindings share this crate's exact kition-sdk pin.
pub use kition_kit;

pub type Result<T> = std::result::Result<T, KitError>;
pub(crate) type KdmResult<T> = std::result::Result<T, KdmError>;

/// The schema file, as the package ships it.
pub const SCHEMA_JSON: &str = include_str!("../model/schema.json");

/// The kit id the schema belongs to.
pub const KIT: &str = "kit.design";

/// How deep layers may nest for the bridge (writes refuse deeper trees, reads stop there).
pub(crate) const MAX_DEPTH: usize = 512;

/// The `kit.design` schema, parsed once.
pub fn schema() -> Result<Arc<Schema>> {
    static PARSED: OnceLock<std::result::Result<Arc<Schema>, String>> = OnceLock::new();
    PARSED.get_or_init(|| Schema::parse(SCHEMA_JSON).map(Arc::new).map_err(|error| error.to_string())).clone().map_err(KitError::Kit)
}

/// The `Design` object's id: one per replica, made with [`kition_kit::kdm::Tx::create_with_id`] so
/// every reader finds it without an index.
pub(crate) fn root_id() -> ObjectId {
    ObjectId::from_string("design")
}

pub(crate) fn kit_err(error: impl std::fmt::Display) -> KitError {
    KitError::new(error.to_string())
}

pub(crate) fn kdm_err(error: impl std::fmt::Display) -> KdmError {
    KdmError::Engine(error.to_string())
}

/// The part of `doc` a replica holds is all of it but what the file keeps for itself (the
/// published library) and the editing person's own state (the current page, the id counter): the
/// writer skips those, so no copy is needed.
pub fn same_content(a: &Document, b: &Document) -> bool {
    read::comparable(a) == read::comparable(b)
}

#[cfg(test)]
mod tests;
