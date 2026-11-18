//! The Design kit's call context, receipts, ids and the guard end to end (task card C4/C5/C6-design):
//! the headless `kition-design` executable under the kits' MCP server's tools (`kition-kits`, the
//! library `kition-kits-mcp` serves, at the pinned kition-sdk revision), on a `.kdesign` in a
//! workspace, called by two agents.
//!
//! A run names where each step acts (`target`, `at.part`: a page by `pageId`) and what on (`on`:
//! layer ids); the person's current page is what the saved file keeps; receipts (`changes`),
//! `others` and `conflicts` name layers and pages by the numeric ids inspect lists and commands
//! take.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;
use kition_kits::tools::{self, CallOptions};
use kition_kits::{KitDefinition, KitManager};
use serde_json::{Value, json};

const ME: &str = "agent-me";
const OTHER: &str = "agent-other";
const FILE: &str = "App.kdesign";

fn kit() -> &'static KitDefinition {
    KitDefinition::from_manifest_text(include_str!("../../../kit.json")).unwrap()
}

fn id_of(answer: &Value) -> u64 {
    answer["id"].as_u64().unwrap_or_else(|| panic!("no id in {answer}"))
}

/// Page `Home`: frame `Card` holding text `Title`, rectangles `Box` and `Badge`; component `Button`
/// holding rectangle `Bg`, and an instance of it. Page `Second`: frame `Panel` holding rectangle
/// `Other`. Saved with the person on `Home`.
fn design() -> Vec<u8> {
    let mut doc = DesignDocument::create(FILE, 800, 600).unwrap();
    let inspected = doc.inspect().unwrap();
    let home = inspected["pages"][0]["id"].as_u64().unwrap();
    doc.execute("page.rename", json!({ "id": home, "name": "Home" })).unwrap();
    let card = inspected["tree"][0]["id"].as_u64().unwrap();
    doc.execute("node.set", json!({ "id": card, "props": { "name": "Card" } })).unwrap();
    let rect = |doc: &mut DesignDocument, parent: u64, name: &str, x: f64| {
        id_of(&doc.execute("node.create", json!({ "type": "rectangle", "parent": parent, "x": x, "y": 100, "width": 40, "height": 40, "name": name, "props": { "fill": "#336699" } })).unwrap())
    };
    doc.execute("text.create", json!({ "text": "Hello", "x": 20, "y": 20, "parent": card })).unwrap();
    rect(&mut doc, card, "Box", 20.0);
    rect(&mut doc, card, "Badge", 80.0);
    let button = id_of(
        &doc.execute("node.create", json!({ "type": "frame", "parent": home, "x": 0, "y": 700, "width": 120, "height": 40, "name": "Button" }))
            .unwrap(),
    );
    rect(&mut doc, button, "Bg", 0.0);
    doc.execute("component.create", json!({ "ids": [button] })).unwrap();
    doc.execute("instance.create", json!({ "component": button, "parent": home, "x": 300, "y": 700 })).unwrap();
    doc.execute("page.add", json!({ "name": "Second" })).unwrap();
    let second = doc.inspect().unwrap()["pages"][1]["id"].as_u64().unwrap();
    let panel = id_of(
        &doc.execute("node.create", json!({ "type": "frame", "parent": second, "x": 0, "y": 0, "width": 300, "height": 200, "name": "Panel" }))
            .unwrap(),
    );
    rect(&mut doc, panel, "Other", 10.0);
    doc.execute("page.select", json!({ "id": home })).unwrap();
    doc.save(None).unwrap()
}

/// The page the saved file opens on.
fn opens_on(bytes: &[u8]) -> u64 {
    DesignDocument::open(FILE, bytes).unwrap().inspect().unwrap()["page"].as_u64().unwrap()
}

struct Fixture {
    kits: KitManager,
    workspace: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let workspace = tempfile::tempdir().unwrap();
        std::fs::write(workspace.path().join(FILE), design()).unwrap();
        Self { kits: Self::server(workspace.path()), workspace }
    }

    /// A new server on `workspace` (a process of `kition-kits-mcp` is one of these).
    fn server(workspace: &Path) -> KitManager {
        KitManager::new(Some(workspace.to_path_buf()), vec![(kit(), Some(PathBuf::from(env!("CARGO_BIN_EXE_kition-design"))))])
    }

    fn file(&self) -> Vec<u8> {
        std::fs::read(self.workspace.path().join(FILE)).unwrap()
    }

    async fn call_as(&self, agent: &str, action: &str, mut input: Value) -> Result<Value, Value> {
        input["path"] = json!(FILE);
        match tools::call_as(&self.kits, &format!("design_{action}"), &input, CallOptions::default(), agent).await {
            Some(Ok(output)) => Ok(output.output),
            Some(Err(error)) => Err(error.structured()),
            None => panic!("design_{action} is no tool"),
        }
    }

    async fn ok(&self, action: &str, input: Value) -> Value {
        self.by(ME, action, input).await
    }

    async fn by(&self, agent: &str, action: &str, input: Value) -> Value {
        self.call_as(agent, action, input).await.unwrap_or_else(|error| panic!("{agent} design_{action}: {error}"))
    }

    async fn refused(&self, action: &str, input: Value) -> Value {
        match self.call_as(ME, action, input).await {
            Err(error) => error,
            Ok(answer) => panic!("design_{action} was expected to be refused: {answer}"),
        }
    }

    /// The pages as `design_inspect` lists them, and the answer's `revision`.
    async fn pages(&self) -> (Vec<Value>, String) {
        let seen = self.ok("inspect", json!({})).await;
        (seen["details"]["pages"].as_array().unwrap_or_else(|| panic!("pages: {seen}")).clone(), revision(&seen))
    }

    async fn page_id(&self, name: &str) -> u64 {
        let (pages, _) = self.pages().await;
        pages.iter().find(|page| page["name"] == name).and_then(|page| page["id"].as_u64()).unwrap_or_else(|| panic!("no page {name}: {pages:?}"))
    }

    /// The layers of page `page` (by id), as `design_inspect` lists that page.
    async fn layers(&self, page: u64) -> Value {
        let seen = self.ok("inspect", json!({ "target": { "pageId": page } })).await;
        seen["part"]["layers"].clone()
    }

    /// The id of the layer named `name` on page `page`.
    async fn id(&self, page: &str, name: &str) -> u64 {
        let page = self.page_id(page).await;
        let layers = self.layers(page).await;
        find(&layers, name).unwrap_or_else(|| panic!("no layer {name} in {layers}"))
    }

    async fn shutdown(self) {
        self.kits.shutdown().await;
    }
}

fn find(layers: &Value, name: &str) -> Option<u64> {
    layers.as_array()?.iter().find_map(|layer| if layer["name"] == name { layer["id"].as_u64() } else { find(&layer["children"], name) })
}

fn revision(answer: &Value) -> String {
    answer["revision"].as_str().unwrap_or_else(|| panic!("a revision: {answer}")).to_owned()
}

/// The receipt entry for `id` in `list` (`changes.changed`, `moved`, …).
fn entry(list: &Value, id: u64) -> Value {
    list.as_array().and_then(|items| items.iter().find(|item| item["id"] == id)).cloned().unwrap_or_else(|| panic!("no {id} in {list}"))
}

fn edit(changed: &Value, field: &str) -> Value {
    changed["edits"]
        .as_array()
        .and_then(|edits| edits.iter().find(|edit| edit["field"] == field))
        .cloned()
        .unwrap_or_else(|| panic!("no edit of {field}: {changed}"))
}

fn ids(list: &Value) -> Vec<String> {
    let mut out: Vec<String> =
        list.as_array().map(|items| items.iter().filter_map(|item| item["id"].as_u64().map(|id| id.to_string())).collect()).unwrap_or_default();
    out.sort();
    out
}

/// The component `Button` on `Home` and its layer `Bg`, and the instance's `Bg`.
async fn button(f: &Fixture) -> (u64, u64) {
    let layers = f.layers(f.page_id("Home").await).await;
    let of = |kind: &str| layers.as_array().unwrap().iter().find(|layer| layer["type"] == kind).unwrap()["children"][0]["id"].as_u64().unwrap();
    (of("COMPONENT"), of("INSTANCE"))
}

/// A refusal of a write whose premises others changed, naming `id` with `reason`.
fn assert_conflict(refused: &Value, id: u64, reason: &str) {
    assert_eq!(refused["error"]["code"], "conflict", "{refused}");
    let found = refused["conflicts"].as_array().and_then(|conflicts| conflicts.iter().find(|conflict| conflict["id"] == id));
    let found = found.unwrap_or_else(|| panic!("no conflict on {id}: {refused}"));
    assert_eq!(found["reason"], reason, "{refused}");
}

// ---- Receipts name what a call changed by the ids inspect lists ----

#[tokio::test(flavor = "multi_thread")]
async fn receipts_name_layers_by_the_ids_inspect_lists_with_before_and_after() {
    let f = Fixture::new();
    let (title, bx, badge, card, panel) = (
        f.id("Home", "Hello").await,
        f.id("Home", "Box").await,
        f.id("Home", "Badge").await,
        f.id("Home", "Card").await,
        f.id("Second", "Panel").await,
    );
    // A text change: the layer's `kind` (its characters and style), before and after.
    let ran = f.ok("run", json!({ "steps": [{ "command": "text.set", "params": { "id": title, "characters": "Welcome" } }] })).await;
    let characters = edit(&entry(&ran["changes"]["changed"], title), "kind.characters");
    assert_eq!((characters["before"].clone(), characters["after"].clone()), (json!("Hello"), json!("Welcome")), "{ran}");
    // A fill change, by `on` into the command's `ids`.
    let ran = f.ok("run", json!({ "steps": [{ "command": "node.set", "on": [bx], "params": { "props": { "fill": "#C00000" } } }] })).await;
    assert_eq!(ids(&ran["changes"]["changed"]), [bx.to_string()], "exactly that layer: {ran}");
    for channel in ["r", "g", "b"] {
        let color = edit(&entry(&ran["changes"]["changed"], bx), &format!("fills.0.color.{channel}"));
        assert_ne!(color["before"], color["after"]);
    }
    // A reparent into a frame of the other page: a move, from and to named by layer ids.
    let ran = f.ok("run", json!({ "steps": [{ "command": "node.reparent", "params": { "ids": [badge], "parent": panel } }] })).await;
    let moved = entry(&ran["changes"]["moved"], badge);
    assert_eq!((moved["from"]["parent"].clone(), moved["to"]["parent"].clone()), (json!(card.to_string()), json!(panel.to_string())), "{ran}");
    // A reorder within its parent: a move between indexes.
    let ran = f.ok("run", json!({ "steps": [{ "command": "node.reorder", "params": { "id": bx, "index": 0 } }] })).await;
    let moved = entry(&ran["changes"]["moved"], bx);
    assert_eq!(
        (moved["from"]["parent"].clone(), moved["to"]["parent"].clone(), moved["to"]["index"].clone()),
        (json!(card.to_string()), json!(card.to_string()), json!(0)),
        "{ran}"
    );
    // An edit of a component's layer: reported for it and for each instance's mirrored layer.
    let (main, mirror) = button(&f).await;
    let ran = f.ok("run", json!({ "steps": [{ "command": "node.set", "params": { "id": main, "props": { "fill": "#00FF00" } } }] })).await;
    let mut both = vec![main.to_string(), mirror.to_string()];
    both.sort();
    assert_eq!(ids(&ran["changes"]["changed"]), both, "{ran}");
    // Deleting a frame with children: the frame and every layer in it.
    let ran = f.ok("run", json!({ "steps": [{ "command": "edit.delete", "params": { "ids": [card] } }] })).await;
    let mut gone = vec![card.to_string(), title.to_string(), bx.to_string()];
    gone.sort();
    assert_eq!(ids(&ran["changes"]["deleted"]), gone, "{ran}");
    f.shutdown().await;
}

// ---- Nothing carries over between calls ----

#[tokio::test(flavor = "multi_thread")]
async fn a_run_acts_on_two_pages_and_the_saved_file_keeps_the_persons_page() {
    let f = Fixture::new();
    let (home, second) = (f.page_id("Home").await, f.page_id("Second").await);
    let (bx, other, panel) = (f.id("Home", "Box").await, f.id("Second", "Other").await, f.id("Second", "Panel").await);
    let ran = f
        .ok(
            "run",
            json!({ "target": { "pageId": second }, "steps": [
                // On the run's page: a layer with no parent goes on `Second`.
                { "command": "node.create", "params": { "type": "ellipse", "x": 400, "y": 0, "width": 20, "height": 20, "name": "Dot" } },
                // `on` a layer of the other page, with that page as the step's part.
                { "command": "arrange.roundToPixels", "on": [bx], "at": { "part": { "pageId": home } } },
                { "command": "node.set", "on": [other], "params": { "props": { "name": "Other 2" } } },
                // A parent on the run's page, given explicitly.
                { "command": "node.create", "params": { "type": "rectangle", "parent": panel, "x": 5, "y": 5, "width": 5, "height": 5, "name": "Pip" } },
            ] }),
        )
        .await;
    assert!(ran["changes"]["created"].as_array().is_some_and(|created| created.len() == 2), "{ran}");
    let second_layers = f.layers(second).await;
    assert!(
        find(&second_layers, "Dot").is_some() && find(&second_layers, "Other 2").is_some() && find(&second_layers, "Pip").is_some(),
        "{second_layers}"
    );
    assert_eq!(opens_on(&f.file()), home, "the saved file opens on the person's page, not the run's");
    // The next call starts from the person's page: a layer without a parent goes on `Home`.
    f.ok("run", json!({ "steps": [{ "command": "node.create", "params": { "type": "ellipse", "x": 0, "y": 900, "width": 20, "height": 20, "name": "Home dot" } }] })).await;
    assert!(find(&f.layers(home).await, "Home dot").is_some());
    assert_eq!(opens_on(&f.file()), home);
    f.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failing_later_step_changes_nothing_and_leaves_the_person_where_they_were() {
    let f = Fixture::new();
    let (home, second) = (f.page_id("Home").await, f.page_id("Second").await);
    let other = f.id("Second", "Other").await;
    let before = f.file();
    let refused = f
        .refused(
            "run",
            json!({ "target": { "pageId": second }, "steps": [
                { "command": "node.set", "on": [other], "params": { "props": { "name": "Renamed" } } },
                { "command": "node.set", "on": [other], "params": { "props": { "width": "wide" } } },
            ] }),
        )
        .await;
    assert_eq!(refused["error"]["index"], 1, "{refused}");
    assert_eq!(f.file(), before, "nothing changed in the file");
    assert!(find(&f.layers(second).await, "Other").is_some());
    assert_eq!(opens_on(&f.file()), home);
    f.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn inspect_lists_and_previews_any_page_by_id_without_going_there() {
    let f = Fixture::new();
    let (pages, _) = f.pages().await;
    assert_eq!(
        pages.iter().map(|page| (page["name"].clone(), page["index"].clone())).collect::<Vec<_>>(),
        [(json!("Home"), json!(0)), (json!("Second"), json!(1))]
    );
    let second = f.page_id("Second").await;
    let seen = f.ok("inspect", json!({ "target": { "pageId": second }, "preview": true })).await;
    assert_eq!((seen["part"]["id"].clone(), seen["part"]["name"].clone()), (json!(second), json!("Second")), "{seen}");
    assert_eq!(seen["part"]["layers"][0]["name"], "Panel", "{seen}");
    assert_eq!(
        (seen["preview"]["pageId"].clone(), seen["preview"]["drew"].clone()),
        (json!(second), json!("page")),
        "the preview shows that page: {seen}"
    );
    assert!(seen["details"].get("tree").is_none() && seen["details"].get("page").is_none(), "editor state is not what an agent reads");
    // The inspection did not move the person: a layer without a parent still goes on `Home`.
    f.ok("run", json!({ "steps": [{ "command": "node.create", "params": { "type": "ellipse", "x": 0, "y": 900, "width": 20, "height": 20, "name": "Here" } }] })).await;
    assert!(find(&f.layers(f.page_id("Home").await).await, "Here").is_some());
    // A layer's preview names its page.
    let other = f.id("Second", "Other").await;
    let seen = f.ok("inspect", json!({ "target": { "objectId": other.to_string() }, "preview": true })).await;
    assert_eq!((seen["preview"]["pageId"].clone(), seen["preview"]["objectId"].clone()), (json!(second), json!(other)), "{seen}");
    f.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn ids_from_an_earlier_inspect_name_the_same_layers_after_a_new_server_opens_the_saved_file() {
    let f = Fixture::new();
    let (second, other) = (f.page_id("Second").await, f.id("Second", "Other").await);
    f.ok("run", json!({ "steps": [{ "command": "page.rename", "params": { "id": second, "name": "Renamed" } }] })).await;
    f.kits.shutdown().await;
    let again = Fixture { kits: Fixture::server(f.workspace.path()), workspace: f.workspace };
    assert_eq!(again.page_id("Renamed").await, second);
    assert_eq!(again.id("Renamed", "Other").await, other);
    let ran = again.ok("run", json!({ "steps": [{ "command": "node.set", "params": { "id": other, "props": { "name": "Same" } } }] })).await;
    assert_eq!(ids(&ran["changes"]["changed"]), [other.to_string()]);
    // The object model takes them too.
    let read = again.ok("query", json!({ "follow": "nodes", "fields": ["key", "name"] })).await;
    let listed = read["objects"].as_array().unwrap().iter().find(|object| object["fields"]["name"] == "Same").cloned().unwrap();
    assert_eq!(listed["id"], other, "{listed}");
    again.shutdown().await;
}

// ---- The guard between two agents on one server ----

#[tokio::test(flavor = "multi_thread")]
async fn another_agents_edit_of_an_unrelated_layer_is_reported_and_my_write_applies() {
    let f = Fixture::new();
    let (_, basis) = f.pages().await;
    let (mine, theirs) = (f.id("Home", "Box").await, f.id("Second", "Other").await);
    f.by(OTHER, "run", json!({ "steps": [{ "command": "node.set", "params": { "id": theirs, "props": { "name": "Theirs" } } }] })).await;
    let ran =
        f.ok("run", json!({ "basis": basis, "steps": [{ "command": "node.set", "params": { "id": mine, "props": { "name": "Mine" } } }] })).await;
    assert_eq!(ids(&ran["others"]["changed"]), [theirs.to_string()], "{ran}");
    assert_eq!(ran["others"]["by"], json!([OTHER]), "{ran}");
    assert_eq!(ids(&ran["changes"]["changed"]), [mine.to_string()], "{ran}");
    f.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn overlapping_name_and_deletion_refuse_but_unrelated_fields_and_reparent_apply() {
    let f = Fixture::new();
    let (bx, badge, title, panel) =
        (f.id("Home", "Box").await, f.id("Home", "Badge").await, f.id("Home", "Hello").await, f.id("Second", "Panel").await);
    // `on`, `ids`, and scalar `id` all name the same target. Renaming rests only on its name.
    let mine = |id: u64| json!([{ "command": "node.set", "on": [id], "params": { "props": { "name": "Mine" } } }]);
    // An unrelated field on the target is preserved and does not refuse a rename.
    let (_, basis) = f.pages().await;
    f.by(OTHER, "run", json!({ "steps": [{ "command": "node.set", "params": { "id": bx, "props": { "opacity": 0.5 } } }] })).await;
    let ran = f.ok("run", json!({ "basis": basis, "steps": mine(bx) })).await;
    assert_eq!(ids(&ran["others"]["changed"]), [bx.to_string()], "{ran}");
    assert_eq!(edit(&entry(&ran["others"]["changed"], bx), "opacity")["after"], 0.5);
    let basis = revision(&ran);
    f.by(OTHER, "run", json!({ "steps": [{ "command": "node.set", "params": { "id": bx, "props": { "name": "Theirs" } } }] })).await;
    let before = f.file();
    let refused = f.refused("run", json!({ "basis": basis, "steps": mine(bx) })).await;
    assert_conflict(&refused, bx, "changed");
    assert_eq!(f.file(), before, "nothing changed in the file");
    // Reparenting preserves the identity and name a rename rests on.
    let basis = revision(&refused);
    f.by(OTHER, "run", json!({ "steps": [{ "command": "node.reparent", "params": { "ids": [badge], "parent": panel } }] })).await;
    let ran =
        f.ok("run", json!({ "basis": basis, "steps": [{ "command": "node.set", "params": { "ids": [badge], "props": { "name": "Mine" } } }] })).await;
    assert_eq!(ids(&ran["changes"]["changed"]), [badge.to_string()], "{ran}");
    // Deleted by the other agent: deleted.
    let basis = revision(&ran);
    f.by(OTHER, "run", json!({ "steps": [{ "command": "edit.delete", "params": { "ids": [title] } }] })).await;
    let gone = f.file();
    let refused = f.refused("run", json!({ "basis": basis, "steps": mine(title) })).await;
    assert_conflict(&refused, title, "deleted");
    assert_eq!(f.file(), gone, "nothing changed in the file");
    f.shutdown().await;
}
