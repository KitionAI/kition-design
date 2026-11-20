//! `kition-design`: the Design kit for Kition's agent. The same document the in-page editor runs
//! (`kition-design-kit`), served headless over the Kit Protocol by `kition-headless`, as the kit's
//! `kit.json` declares it, with the object model (`model.describe`, `model.batch`) over the document's
//! change exchange, and the identity a file carries (`document.identity`, `document.fork`: a
//! `.kdesign` embeds its replica, `kition_design::DesignFiles`). It never shows a window, and the
//! engine never reads or writes files on the agent's behalf (commands run confined to no folder).
//!
//! It lays out and draws text with the fonts built into it alone (`kition_design_kit::pin_fonts`):
//! never the machine's fonts nor a font folder named in the environment, so a preview and a text
//! layer's size are the same on every machine for the same build.

use std::process::ExitCode;
use std::sync::Arc;

use kition_headless::{FileIdentity, Headless, KitInfo};

fn main() -> ExitCode {
    kition_design_kit::pin_fonts();
    let built = KitInfo::from_manifest(include_str!("../../../kit.json"), kition_design_kit::SANDBOXED, kition_design_kit::EDITOR_STATE)
        .and_then(|kit| kition_design::DesignFiles::new().map(|files| (kit, files)));
    let (kit, files) = match built {
        Ok(built) => built,
        Err(error) => {
            eprintln!("kit: {error}");
            return ExitCode::from(2);
        }
    };
    let backend =
        Headless::<kition_design_kit::DesignDocument>::with_replica_port(kit.clone()).with_file_identity(FileIdentity::Embedded(Arc::new(files)));
    kition_headless::serve_backend(&kit, backend)
}
