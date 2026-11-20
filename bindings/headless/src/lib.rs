//! What the `kition-design` executable serves beside the engine: where a `.kdesign` file keeps the
//! design's replica (`document.identity` and `document.fork`, Kit Process Protocol section 5.6).

use std::sync::Arc;

use kition_kit::kdm::Schema;
use kition_kit_server::{ErrorReason, RpcError};

/// Where a `.kdesign` file keeps the design's replica (`docs/features/kdm.md` section 3): its
/// `kdm/` entries, `snapshot.bin` and the change entries `meta.json` lists. A fork's file holds the
/// SDK's copy as `snapshot.bin` alone, with `meta.json`'s copy of the docId the copy's
/// (`kition_design_kdm::file::fork_entries`), no version history or branches, and only the images
/// its state, its `document.json` and its published library use
/// (`kition_design_format::replace_replica`).
pub struct DesignFiles(Arc<Schema>);

impl DesignFiles {
    /// With the `kit.design` schema the engine ships; errs only on a build whose schema is broken.
    pub fn new() -> Result<Self, String> {
        kition_design_kdm::schema().map(Self).map_err(|error| format!("the kit.design schema: {error}"))
    }
}

fn unsupported(error: impl std::fmt::Display) -> RpcError {
    RpcError::kit_detail(ErrorReason::UnsupportedFormat, error.to_string())
}

impl kition_headless::EmbeddedReplica for DesignFiles {
    fn schema(&self) -> Arc<Schema> {
        Arc::clone(&self.0)
    }

    /// The snapshot with the listed changes imported, or `None` for a file without replica entries
    /// (an empty file, a `.kdesign` written without the bridge, a bare `document.json`, an import).
    fn replica(&self, file: &[u8]) -> Result<Option<Vec<u8>>, RpcError> {
        let entries = kition_design_format::replica_entries(file).map_err(unsupported)?;
        kition_design_kdm::file::embedded_replica(&entries).map_err(unsupported)
    }

    fn replace(&self, file: &[u8], snapshot: &[u8]) -> Result<Vec<u8>, RpcError> {
        let (entries, images) = kition_design_kdm::file::fork_entries(snapshot).map_err(unsupported)?;
        kition_design_format::replace_replica(file, &entries, &images).map_err(unsupported)
    }
}
