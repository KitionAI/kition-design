//! The command catalog: the engine's commands (`all_commands`) as `kition-kit` lists them, with the
//! upstream main menu's paths and titles (`catalog::MENU`) and its default shortcuts
//! (`keymap`, macOS keys written as kits write them: `Cmd+Shift+Z`).
//!
//! Left out are the commands of the upstream app's shell that a kit has no use for or must not
//! run: tabs, windows and the home screen, opening, closing, saving and reverting files (the host
//! does those), the app's panels, theme and preferences window, the viewport's own zoom and pan
//! (the host's viewport), live sessions and servers (team libraries kept on a server among them),
//! plug-ins, and help ([`EXCLUDED`], [`EXCLUDED_PREFIXES`]). Libraries from files stay: the binding
//! lends the engine the workspace's library files ([`crate::libraries`]). `proto.present`, `proto.preview` and
//! `proto.presentOptions` stay: they only ask for the `present`, `preview` and `present.options`
//! UI requests (the editor's player, on the binding's `player` extra, draws them).
//!
//! Boards (upstream's whiteboard files: stickies, board shapes, connectors, stamps, the marker,
//! timers and voting) are deferred: the editor has no board mode yet. Their commands
//! ([`board_only`]) stay in the catalog, since the engine opens board files and the agent can run
//! them there, and are disabled while the open file is a design (the engine refuses them on designs
//! anyway).

use std::collections::BTreeSet;
use std::sync::OnceLock;

use kition_design_engine::catalog::{MENU, SEP};
use kition_design_engine::keymap::{self, Platform};
use kition_design_engine::{CommandSpec, Session, all_commands};
use kition_kit::KitCommand;

/// Shell commands left out of the kit.
pub const EXCLUDED: &[&str] = &[
    "file.new",
    "file.newBoard",
    "file.newFromTemplate",
    "file.open",
    "file.openRecent",
    "file.close",
    "file.moveTab",
    "file.recover",
    "file.duplicate",
    "file.revert",
    "file.save",
    "file.saveAs",
    "file.markSaved",
    "template.list",
    "view.zoomIn",
    "view.zoomOut",
    "view.zoom100",
    "view.zoom50",
    "view.zoom200",
    "view.zoomFit",
    "view.zoomSelection",
    "view.zoomAt",
    "view.set",
    "view.get",
    "view.pan",
    "view.toggleUi",
    "view.fullscreen",
    "view.layers",
    "view.assets",
    "view.design",
    "view.prototype",
    "view.designPrototype",
    "view.leftSidebar",
    "view.panelWidths",
    "view.nextRegion",
    "view.previousRegion",
    "view.contextMenu",
    "view.themeLight",
    "view.themeDark",
    "view.themeSystem",
    "view.multiplayerCursors",
    "pref.uiScale",
    "pref.language",
    "pref.startOnHome",
    "pref.autoSave",
    "pref.reduceMotion",
    "pref.userName",
    "pref.publishServer",
    "text.rescanFonts",
    "text.loadFont",
    "library.publishToServer",
    "library.usage",
    // Only ask for the libraries view, which the editor opens itself (`web/src/menus/catalog.ts`).
    "libraries.open",
    "libraries.publish",
];

/// Whole families left out: tabs, windows, the app, help, live sessions, plug-ins.
pub const EXCLUDED_PREFIXES: &[&str] = &["doc.", "window.", "app.", "help.", "collab.", "plugin.", "plugins.", "online.", "live.", "invite."];

/// Commands the engine does not record as undo steps that still change the file: undo and redo,
/// batches (their steps edit), widgets and data fills.
const EDITS_WITHOUT_HISTORY: &[&str] = &[
    "edit.undo",
    "edit.redo",
    "command.batch",
    "widget.insert",
    "widget.click",
    "widget.setText",
    "widget.menu",
    "data.fill",
    "board.timer",
    "board.vote",
    "tool.place",
    "vector.edit.begin",
    "vector.edit.end",
];

/// Undoable commands that change nothing in the file the editor shows: none so far; kept for the
/// rule's symmetry and its test.
const VIEWING: &[&str] = &[];

/// Whether `id` is one of the kit's commands.
pub fn offered(id: &str) -> bool {
    kition_design_engine::find_command(id).is_some() && !EXCLUDED.contains(&id) && !EXCLUDED_PREFIXES.iter().any(|prefix| id.starts_with(prefix))
}

/// Whether running `spec` changes the document (the catalog's `mutates`, which read-only documents
/// refuse): the engine's undo steps, and the few edits it keeps out of the history. Selection,
/// view, preference, clipboard and export commands do not. Tool switches can finish an active
/// pen interaction or remove empty vector-edit layers; without that state they only change tools.
fn mutates(session: &Session, spec: &CommandSpec) -> bool {
    (spec.history && !VIEWING.contains(&spec.id))
        || EDITS_WITHOUT_HISTORY.contains(&spec.id)
        || (switches_tool(spec.id)
            && (session.tool.pen.as_ref().is_some_and(|pen| pen.id.is_some()) || session.ds().is_ok_and(|ds| ds.vector_edit.is_some())))
}

pub(crate) fn switches_tool(command: &str) -> bool {
    matches!(command, "tool.set" | "tool.place" | "edit.pickColor")
        || command.strip_prefix("tool.").and_then(kition_design_engine::ToolId::from_id).is_some()
}

/// A shortcut in the engine's notation (`Shift+Cmd+Z`, `Alt+/`) as kits write it
/// (`Cmd+Shift+Z`): modifiers Cmd, Ctrl, Alt, Shift in that order, shifted symbols as their key
/// with Shift.
pub fn kit_shortcut(spec: &str) -> Option<String> {
    let (mods, key) = keymap::parse(spec)?;
    let (key, shifted) = keymap::normalize(key).unwrap_or((key, false));
    let mut out = String::new();
    for (on, name) in [(mods.cmd, "Cmd"), (mods.ctrl, "Ctrl"), (mods.alt, "Alt"), (mods.shift || shifted, "Shift")] {
        if on {
            out.push_str(name);
            out.push('+');
        }
    }
    out.push_str(key);
    Some(out)
}

/// The menu path of `id` in the upstream main menu, split at `/` (`Edit/Copy as` →
/// `["Edit", "Copy as"]`), and the item's label. The toolbar's `Tools` entries have no menu: the
/// tool rail shows them.
fn menu_of(id: &str) -> Option<(Vec<String>, &'static str)> {
    let item = MENU.iter().find(|m| m.id == id && m.label != SEP)?;
    if item.menu == "Tools" {
        return None;
    }
    Some((item.menu.split('/').map(str::to_owned).collect(), item.label))
}

/// The engine's commands in catalog order: the main menu's items first, then the rest.
fn ordered() -> &'static [&'static CommandSpec] {
    static ORDER: OnceLock<Vec<&'static CommandSpec>> = OnceLock::new();
    ORDER.get_or_init(|| {
        let mut seen = BTreeSet::new();
        let mut out = Vec::new();
        for item in MENU.iter().filter(|m| m.label != SEP) {
            if let Some(spec) = kition_design_engine::find_command(item.id).filter(|s| offered(s.id))
                && seen.insert(spec.id)
            {
                out.push(spec);
            }
        }
        for spec in all_commands().iter().filter(|s| offered(s.id)) {
            if seen.insert(spec.id) {
                out.push(spec);
            }
        }
        out
    })
}

/// Whether `id` works only on board files: `board.*` and the board tools.
pub fn board_only(id: &str) -> bool {
    id.starts_with("board.")
        || id.strip_prefix("tool.").and_then(kition_design_engine::ToolId::from_id).is_some_and(kition_design_engine::ToolId::on_boards)
}

/// Whether the open file is a board.
fn on_board(session: &Session) -> bool {
    session.doc().is_ok_and(kition_design_engine::board::is_board)
}

/// Whether `id` can run now: Undo and Redo while there is a step (or a pen anchor), board commands on boards; the
/// upstream menus leave every other command enabled and let it explain why it cannot run.
fn enabled(session: &Session, id: &str) -> bool {
    let (undo, redo) = session.undo_depth();
    // While the pen draws, Undo and Redo take back and put back its anchors.
    let (pen_undo, pen_redo) = session.pen_history();
    match id {
        "edit.undo" => undo > 0 || pen_undo,
        "edit.redo" => redo > 0 || pen_redo,
        _ if board_only(id) => on_board(session),
        _ => session.ds().is_ok(),
    }
}

/// Tool shortcuts belong to the tool rail (`tools()`), not to the commands.
fn shortcut_of(spec: &CommandSpec) -> Option<String> {
    if spec.id.starts_with("tool.") || spec.id == "edit.pickColor" {
        return None;
    }
    keymap::primary(spec.id, Platform::Mac).and_then(kit_shortcut)
}

pub fn list(session: &Session) -> Vec<KitCommand> {
    ordered()
        .iter()
        .map(|spec| {
            let menu = menu_of(spec.id);
            let title = menu.as_ref().map_or(spec.label, |(_, label)| label);
            KitCommand {
                id: spec.id.to_owned(),
                title: title.trim_end_matches('…').trim_end_matches("...").to_owned(),
                menu: menu.map(|(path, _)| path),
                shortcut: shortcut_of(spec),
                params: (!spec.params.is_empty()).then(|| spec.params.to_owned()),
                mutates: mutates(session, spec),
                enabled: enabled(session, spec.id),
                schema: own_schema(spec.id),
                ..Default::default()
            }
            .with_kit_schema()
            .without_file_paths()
        })
        .collect()
}

/// Exports whose parameter names a file or folder to write: the kit reaches no file system (every
/// command runs confined to no folder), so they answer the files' bytes and their schemas leave
/// that parameter out (KHP A28: a parameter naming a file is typed `file` or not offered).
const WRITES_TO_PATH: &[(&str, &str)] = &[
    ("file.exportSvg", "path"),
    ("file.exportFramesPdf", "path"),
    ("variable.export", "path"),
    ("file.export", "path"),
    ("file.exportLayers", "dir"),
    ("component.playground", "path"),
    ("codeConnect.export", "path"),
];

trait WithoutFilePaths {
    fn without_file_paths(self) -> Self;
}

impl WithoutFilePaths for KitCommand {
    fn without_file_paths(mut self) -> Self {
        if let Some(serde_json::Value::Object(properties)) = self.schema.as_mut() {
            for (_, key) in WRITES_TO_PATH.iter().filter(|(id, _)| *id == self.id) {
                properties.remove(*key);
            }
        }
        self
    }
}

/// Schemas for commands whose parameter notes are not in kit command notation: `node.set` lists
/// its property keys rather than its parameters (KHP A36: every agent-facing command has one).
fn own_schema(id: &str) -> Option<serde_json::Value> {
    if id == "proto.interaction.add" {
        return Some(interaction_schema(id));
    }
    (id == "node.set").then(|| {
        serde_json::json!({
            "id": { "type": "integer", "description": "The layer to change (default the selection)." },
            "ids": { "type": "array", "items": { "type": "integer" }, "description": "The layers to change (default the selection)." },
            "props": {
                "type": "object",
                "additionalProperties": true,
                "required": true,
                "description": format!("The properties to set: {}", kition_design_engine::find_command(id).map_or("", |spec| spec.params)),
            },
        })
    })
}

/// The commands that cannot run now, without building the list and its schemas.
pub fn disabled(session: &Session) -> Vec<String> {
    if session.ds().is_err() {
        return ordered().iter().map(|spec| spec.id.to_owned()).collect();
    }
    let mut out: Vec<String> = ["edit.undo", "edit.redo"].into_iter().filter(|id| !enabled(session, id)).map(str::to_owned).collect();
    if !on_board(session) {
        out.extend(ordered().iter().filter(|spec| board_only(spec.id)).map(|spec| spec.id.to_owned()));
    }
    out
}

/// The upstream main menu as catalog rows, in order, separators as `---`, only the kit's
/// commands: `[{ path: [...], label, command }]` (the `menus` query).
pub fn menu_rows() -> Vec<serde_json::Value> {
    MENU.iter()
        .filter(|m| m.menu != "Tools")
        .filter(|m| m.label == SEP || offered(m.id))
        .map(|m| {
            let path: Vec<&str> = m.menu.split('/').collect();
            if m.label == SEP {
                serde_json::json!({ "path": path, "label": "", "command": "---" })
            } else {
                serde_json::json!({ "path": path, "label": m.label, "command": m.id })
            }
        })
        .collect()
}

/// `proto.interaction.add`: its notes describe nested triggers, actions and transitions the notation
/// converter can't follow, so its parameters are written out here (KHP A5).
fn interaction_schema(id: &str) -> serde_json::Value {
    let notes = kition_design_engine::find_command(id).map_or("", |spec| spec.params);
    serde_json::json!({
        "id": { "type": "integer", "description": "The layer the interaction is on (default the selection)." },
        "ids": { "type": "array", "items": { "type": "integer" }, "description": "The layers it goes on, each its own interaction (instead of id; default the selection)." },
        "trigger": {
            "type": "object",
            "additionalProperties": true,
            "description": "What starts it: {type: onClick|onHover|onPress|onDrag|mouseEnter|mouseLeave|mouseDown|mouseUp|afterDelay {ms}|keyPress {key}} (default onClick).",
        },
        "action": {
            "type": "object",
            "additionalProperties": true,
            "description": format!("The action it runs: {{type, …}} as the notes say: {notes}"),
        },
        "actions": { "type": "array", "items": { "type": "object", "additionalProperties": true }, "description": "Several actions, in order (instead of action)." },
        "transition": {
            "type": "object",
            "additionalProperties": true,
            "description": "{type: instant|dissolve|smartAnimate|moveIn|moveOut|push|slideIn|slideOut, direction?: left|right|top|bottom}.",
        },
        "durationMs": { "type": "number", "description": "The transition's length in milliseconds." },
        "easing": {
            "oneOf": [
                { "type": "string", "enum": ["linear", "easeIn", "easeOut", "easeInOut", "easeInBack", "easeOutBack", "easeInOutBack", "gentle", "quick", "bouncy", "slow"] },
                { "type": "object", "additionalProperties": true }
            ],
            "description": "A named easing, or {customBezier {x1, y1, x2, y2}} or {customSpring {mass, stiffness, damping}}.",
        },
        "delayMs": { "type": "number", "description": "Mouse enter, leave, down and up: how long after the gesture the actions run, up to 600000." },
        "dest": { "type": "integer", "description": "A frame id: with id alone, a click that navigates there." },
    })
}
