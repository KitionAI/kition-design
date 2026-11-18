//! Project tasks: `cargo xtask <task>`.
//!
//! - `ci`      fmt check, clippy -D warnings, tests, layers, assets, wasm check
//! - `test`    the workspace tests (nextest when installed) and doctests
//! - `layers`  crate dependency layering check
//! - `assets`  every file under assets/ and docs/ has a row in docs/attribution.md
//! - `parity`  menu-catalog coverage → docs/parity/parity-checklist.md
//! - `version` print the version, or `version set X.Y.Z`
//! - `wasm`    check the engine crates for wasm32
//! - `eval`    the layer: eval cases (kition-sdk's tools/eval-cases.mjs; real models). Only a maintainer
//!   starts it (docs/quality.md, F-QA-16); agents never do. `eval --dry-run` calls no model

mod layers;

use std::process::{Command, ExitCode, Stdio};

fn run(cmd: &mut Command) -> bool {
    eprintln!("$ {cmd:?}");
    cmd.status().map(|s| s.success()).unwrap_or(false)
}

fn cargo() -> Command {
    Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
}

/// The repository root: two levels above this crate, which lives in `tools/xtask`.
fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).map(std::path::Path::to_path_buf).unwrap_or_default()
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let task = args.first().map(String::as_str).unwrap_or("help");
    let ok = match task {
        "ci" => ci(),
        "test" => test(),
        "layers" => layers::check(&root()),
        "assets" => assets(),
        "parity" => parity(),
        "version" => version(args.get(1).map(String::as_str), args.get(2).map(String::as_str)),
        "wasm" => wasm(),
        "eval" => eval(&args[1..]),
        _ => {
            eprintln!("tasks: ci, test, layers, assets, parity, version [set X.Y.Z], wasm, eval");
            task == "help"
        }
    };
    if ok { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

fn ci() -> bool {
    run(cargo().args(["fmt", "--all", "--check"]))
        && run(cargo().args(["clippy", "--workspace", "--all-targets", "--keep-going", "--", "-D", "warnings"]))
        && test()
        && layers::check(&root())
        && assets()
        && wasm()
}

/// Unit and integration tests with nextest when it is installed (`cargo install cargo-nextest`),
/// else with `cargo test`; then the doctests, which nextest does not run. Examples are not built:
/// clippy's `--all-targets` already checks them.
fn test() -> bool {
    let nextest = cargo().args(["nextest", "--version"]).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success());
    let units = if nextest {
        run(cargo().args(["nextest", "run", "--workspace", "--lib", "--bins", "--tests"]))
    } else {
        eprintln!("xtask: cargo-nextest not installed; running unit and integration tests with `cargo test`");
        run(cargo().args(["test", "--workspace", "--lib", "--bins", "--tests"]))
    };
    units && run(cargo().args(["test", "--workspace", "--doc"]))
}

fn wasm() -> bool {
    run(cargo().args(["check", "--keep-going", "--target", "wasm32-unknown-unknown", "-p", "kition-design-collab", "-p", "kition-design-engine"]))
}

/// Every asset file must be listed in docs/attribution.md.
fn assets() -> bool {
    let root = root();
    let attribution = std::fs::read_to_string(root.join("docs/attribution.md")).unwrap_or_default();
    let mut missing = Vec::new();
    walk(&root.join("assets"), &mut |p| {
        let rel = p.strip_prefix(&root).map(|r| r.to_string_lossy().replace('\\', "/")).unwrap_or_default();
        let name = p.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
        if name.starts_with('.') || name.ends_with(".md") || name.ends_with(".txt") {
            return;
        }
        // Listed by path, or by any ancestor directory written as `dir/`.
        let mut listed = attribution.contains(&format!("`{rel}`"));
        let mut cur = rel.as_str();
        while let Some((parent, _)) = cur.rsplit_once('/') {
            if attribution.contains(&format!("`{parent}/`")) {
                listed = true;
                break;
            }
            cur = parent;
        }
        if !listed {
            missing.push(rel);
        }
    });
    if !missing.is_empty() {
        eprintln!("assets without a docs/attribution.md row:\n  {}", missing.join("\n  "));
        return false;
    }
    eprintln!("assets: ok");
    true
}

fn walk(dir: &std::path::Path, f: &mut dyn FnMut(&std::path::Path)) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let Ok(ft) = e.file_type() else { continue };
        let p = e.path();
        if ft.is_dir() && !ft.is_symlink() {
            walk(&p, f);
        } else if !ft.is_dir() {
            f(&p);
        }
    }
}

fn parity() -> bool {
    let extra = [
        "file.open",
        "file.import",
        "file.placeImage",
        "file.save",
        "file.saveAs",
        "file.export",
        "file.exportSvg",
        "edit.copyAsText",
        "edit.copyAsCss",
        "edit.copyAsSvg",
        "edit.copyAsPng",
        "edit.copyLink",
    ];
    let (done, total, missing) = kition_design_engine::catalog::parity(&extra);
    let pct = done as f64 * 100.0 / total.max(1) as f64;
    // The generation date: the date of the last commit (no clock dependency in xtask).
    let date = Command::new("git")
        .args(["log", "-1", "--format=%cs"])
        .current_dir(root())
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|d| d.trim().to_string())
        .unwrap_or_default();
    let mut md = format!(
        "# Parity checklist\n\n> **Generated:** {date} by `cargo xtask parity`. Do not edit by hand.\n\nKition Design commands registered for the incumbent's main menu and toolbar ({} commands in the engine).\n\n**{done} / {total} menu items ({pct:.1}%)**\n\n",
        kition_design_engine::all_commands().len()
    );
    if missing.is_empty() {
        md.push_str("Every catalog item has a command.\n");
    } else {
        md.push_str("## Missing\n\n");
        for m in &missing {
            md.push_str(&format!("- `{m}`\n"));
        }
    }
    md.push_str("\nMenu coverage measures breadth only; see docs/parity/target-app-parity.md for each feature's status.\n");
    let path = root().join("docs/parity/parity-checklist.md");
    if let Err(e) = std::fs::write(&path, md) {
        eprintln!("{}: {e}", path.display());
        return false;
    }
    eprintln!("parity: {done}/{total} ({pct:.1}%) → docs/parity/parity-checklist.md");
    true
}

fn version(sub: Option<&str>, value: Option<&str>) -> bool {
    let path = root().join("Cargo.toml");
    let Ok(text) = std::fs::read_to_string(&path) else { return false };
    let cur = text
        .lines()
        .skip_while(|l| !l.starts_with("[workspace.package]"))
        .find_map(|l| l.strip_prefix("version = \"").and_then(|r| r.strip_suffix('"')))
        .unwrap_or("0.0.0")
        .to_string();
    match (sub, value) {
        (Some("set"), Some(v)) => {
            let valid =
                v.split(['-', '+']).next().is_some_and(|core| core.split('.').count() == 3 && core.split('.').all(|p| p.parse::<u64>().is_ok()));
            if !valid {
                eprintln!("version must look like 1.2.3 or 1.2.3-rc.1");
                return false;
            }
            let new = text.replacen(&format!("version = \"{cur}\""), &format!("version = \"{v}\""), 1);
            if std::fs::write(&path, new).is_err() {
                return false;
            }
            eprintln!("version {cur} → {v}");
            run(cargo().args(["update", "--workspace"]))
        }
        _ => {
            println!("{cur}");
            true
        }
    }
}

/// The `layer: eval` cases through kition-sdk's runner (`tools/eval-cases.mjs`); its guard asks the
/// person at the terminal before any model call.
fn eval(args: &[String]) -> bool {
    let root = root();
    let sdk = std::env::var_os("KITION_SDK_DIR").map(std::path::PathBuf::from).unwrap_or_else(|| root.join("..").join("kition-sdk"));
    run(Command::new("node").arg(sdk.join("tools").join("eval-cases.mjs")).arg("--repo").arg(&root).args(args))
}
