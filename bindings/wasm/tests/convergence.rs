//! Kition Design against the convergence suite (L6, kition-sdk `kition-convergence`): two to five
//! replicas of a design, edited through the engine's own commands (`docs/features/kdm.md` section 8).

mod common;

use std::path::Path;

use common::{DesignKit, NAME};
use kition_convergence::wasm::{self, WasmEngine};
use kition_convergence::{Options, Outcome};

#[test]
fn converges() {
    let report = kition_convergence::run(&DesignKit, &Options::from_env());
    println!("{}", report.render());
    report.save_if_requested().unwrap();
    assert!(report.level6(), "{}", report.render());
    // A text layer's characters are one register (`Node.kind`), not a KDM `text` field, so the
    // adapter offers no text to type into concurrently: the text scenarios skip, as Vector's do.
    assert!(matches!(report.outcome("text-position"), Some(Outcome::Skipped(_))));
}

/// Native and WebAssembly engines reach one state from one change log. Set `KITION_WASM_ENGINE`
/// to a folder with `kition_design_kit.js` and `kition_design_kit_bg.wasm` (a package's `engine/`, or
/// `web/public/engine` after `sh web/scripts/build-engine.sh`).
#[test]
fn wasm_parity() {
    let Some(engine) = WasmEngine::from_env("kition_design_kit", "DesignDocument", NAME) else {
        println!("SKIP wasm_parity: KITION_WASM_ENGINE is not set");
        return;
    };
    wasm::check_parity(&DesignKit, &engine, Path::new(env!("CARGO_TARGET_TMPDIR")), 1).unwrap();
}
