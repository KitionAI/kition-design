//! `kition-design`: the Design kit for Kition's agent. The same document the in-page editor runs
//! (`kition-design-kit`), served headless over the Kit Protocol by `kition-headless`, as the kit's
//! `kit.json` declares it, with the object model (`model.describe`, `model.batch`) over the document's
//! change exchange. It never shows a window, and the engine never reads or writes files on
//! the agent's behalf (commands run confined to no folder).
//!
//! It lays out and draws text with the fonts built into it alone (`kition_design_kit::pin_fonts`):
//! never the machine's fonts nor a font folder named in the environment, so a preview and a text
//! layer's size are the same on every machine for the same build.

fn main() -> std::process::ExitCode {
    kition_design_kit::pin_fonts();
    kition_headless::run_manifest_model::<kition_design_kit::DesignDocument>(
        include_str!("../../../kit.json"),
        kition_design_kit::SANDBOXED,
        kition_design_kit::EDITOR_STATE,
    )
}
