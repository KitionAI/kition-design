//! No network (KHP A37): the kit never opens a connection. The engine's live sessions, online
//! files, team libraries kept on a server and plug-in directory go only through a link an app
//! installs (`kition_design_engine` `Lives::install`), and the kit installs none; their commands are
//! left out of the catalog, and `command.batch` refuses steps the catalog does not offer, so neither
//! the agent nor the editor can reach them. Libraries from files are offered: they are read through
//! the host's lent files, and a library named by a server link fails without a network link. The headless kit's dependencies hold no network client
//! (`cargo tree -p kition-design`: only kition-kit-server's local socket to its host).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;
use serde_json::json;

/// Command families that need the network upstream.
const ONLINE: &[&str] = &["collab.", "online.", "live.", "invite.", "plugin.", "plugins."];
/// The library commands that need a server (team libraries kept on one).
const SERVER_LIBRARY: &[&str] = &["library.publishToServer", "library.usage"];

#[test]
fn no_offered_command_reaches_the_network() {
    let doc = DesignDocument::create("Design.kdesign", 400, 300).unwrap();
    let offered: Vec<String> = doc.commands().into_iter().map(|c| c.id).collect();
    for id in &offered {
        assert!(!ONLINE.iter().any(|prefix| id.starts_with(prefix)), "{id} is offered");
        assert!(!SERVER_LIBRARY.contains(&id.as_str()), "{id} is offered");
    }
    assert!(!offered.iter().any(|id| id == "pref.publishServer" || id == "text.loadFont"));
    let mut doc = doc;
    let online: Vec<&str> = kition_design_engine::all_commands()
        .iter()
        .map(|c| c.id)
        .filter(|id| ONLINE.iter().any(|p| id.starts_with(p)) || SERVER_LIBRARY.contains(id))
        .collect();
    assert!(online.len() > 10, "the engine has its online commands: {online:?}");
    for id in online {
        let error = doc.execute(id, json!({})).unwrap_err();
        assert!(error.starts_with("unknown command"), "{id}: {error}");
    }
}

#[test]
fn the_kit_installs_no_network_link() {
    let doc = DesignDocument::create("Design.kdesign", 400, 300).unwrap();
    assert!(!doc.session().live.available(), "no live session link");
    assert!(!doc.session().live.can_host());
}

#[test]
fn a_team_library_link_reaches_no_server() {
    let mut doc = DesignDocument::create("Design.kdesign", 400, 300).unwrap();
    let (revision, saved) = (doc.revision(), doc.seal_and_save(None).unwrap());
    for link in ["kition-design://example.com/file/abc?token=x", "wss://example.com/file/abc"] {
        assert!(doc.execute("library.add", json!({ "path": link })).is_err(), "{link} must fail without a network link");
    }
    assert_eq!(doc.revision(), revision, "nothing was added");
    assert_eq!(doc.seal_and_save(None).unwrap(), saved);
}

#[test]
fn a_batch_cannot_run_what_the_kit_leaves_out() {
    let mut doc = DesignDocument::create("Design.kdesign", 400, 300).unwrap();
    let (revision, saved) = (doc.revision(), doc.seal_and_save(None).unwrap());
    for step in ["collab.share", "collab.publish", "library.enable", "plugin.run", "file.open"] {
        let batch = json!({ "commands": [{ "command": "shape.rectangle", "params": { "x": 0, "y": 0, "width": 10, "height": 10 } }, { "command": step, "params": {} }] });
        let error = doc.execute("command.batch", batch).unwrap_err();
        assert!(error.starts_with("invalid-params:") && error.contains(step), "{step}: {error}");
        assert_eq!(doc.revision(), revision, "{step}: nothing ran");
    }
    assert_eq!(doc.seal_and_save(None).unwrap(), saved);
    let ok = json!({ "commands": [{ "command": "shape.rectangle", "params": { "x": 0, "y": 0, "width": 10, "height": 10 } }] });
    doc.execute("command.batch", ok).unwrap();
    assert_ne!(doc.revision(), revision);
}

/// Review #2: a `command.batch` step that is itself a batch is screened too, at any depth.
#[test]
fn a_nested_batch_cannot_run_what_the_kit_leaves_out() {
    let mut doc = DesignDocument::create("Design.kdesign", 400, 300).unwrap();
    let (revision, saved) = (doc.revision(), doc.seal_and_save(None).unwrap());
    let nest = |inner: serde_json::Value| json!({ "commands": [{ "command": "command.batch", "params": inner }] });
    for step in ["file.new", "file.close", "plugin.run", "collab.share"] {
        let leaf = json!({ "commands": [{ "command": step, "params": {} }] });
        for batch in [nest(leaf.clone()), nest(nest(leaf.clone()))] {
            let error = doc.execute("command.batch", batch).unwrap_err();
            assert!(error.starts_with("invalid-params:") && error.contains(step), "{step}: {error}");
            assert_eq!(doc.revision(), revision, "{step}: nothing ran");
        }
    }
    assert_eq!(doc.seal_and_save(None).unwrap(), saved);
    assert_eq!(doc.inspect().unwrap()["layers"], json!(1), "the design is still the one open");
    let ok = nest(json!({ "commands": [{ "command": "shape.rectangle", "params": { "x": 0, "y": 0, "width": 10, "height": 10 } }] }));
    doc.execute("command.batch", ok).unwrap();
    assert_ne!(doc.revision(), revision);
}
