//! # kition-design-kit
//!
//! The Design engine for Kition's in-page editor, on the Kition SDK (engine contract 1).
//! [`DesignDocument`] implements [`KitDocument`] for a Kition Design design (`.kdesign`, and the files the
//! engine imports from bytes: `.fig`, REST API file `.json`, `.sketch`, `.svg`, and raster
//! images), and [`KitTools`](kition_kit::tools::KitTools) for the engine's own pointer tools, so
//! the host runs the Move, Frame, shape, Pen, Pencil, Text and Comment tools unchanged.
//!
//! **Document pixels.** A Design page is an infinite canvas. The document frame ([`area::Area`]) is
//! the current page's content (its top-level layers' render bounds) with a margin of
//! [`area::MARGIN`] canvas units on every side, out to whole [`area::CHUNK`]s of 1000 × 1000
//! canvas units, at least [`area::MIN_WIDTH`] × [`area::MIN_HEIGHT`]; an empty page is that size
//! at the canvas origin. Document pixel (0, 0) is
//! canvas point (`area.x`, `area.y`) and one document pixel is one canvas unit; `inspect().area`
//! says where the frame is, and the frame moves when the content grows past it. The frame depends
//! on the document only (never on the selection, the view or the tool), so a saved file reopens to
//! the same pixels.
//!
//! The engine draws its canvas chrome in its UI upstream; here selection outlines, handles, the
//! hover outline, the marquee, smart guides, pen and vector edit points and the text caret are
//! engine overlays ([`overlays`]), and what overlays cannot express (frame name labels, the size
//! pill, auto layout padding and gap handles, rulers) is the `chrome` extra ([`chrome`]). The
//! editor's panels read the engine through the `query` extra ([`query`]), with canvas marks the
//! upstream UI computed itself (prototype connections, comment pins, gradient, crop and blur
//! handles) as queries too ([`canvas`]), and pictures only the engine can draw (layer thumbnails,
//! text in its own font, the eyedropper's colour) as extras ([`preview`]).
//!
//! **Gestures and the view** ([`view`]): `beginInteraction(label)`, `commitInteraction()` and
//! `cancelInteraction()` make a continuous edit (a number scrub, a colour drag, a handle drag) one
//! undo step; `setViewport(x, y, width, height, zoom)` tells the engine the editor's view, and
//! `viewTarget(command, params)` answers where a View menu zoom command takes it.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod acts;
mod addressing;
pub mod agent;
pub mod agent_text;
pub mod area;
pub mod canvas;
pub mod check;
pub mod chrome;
pub mod collab;
mod commands;
mod context;
mod files;
pub mod libraries;
pub mod lists;
pub mod overlays;
pub mod panels;
mod params;
pub mod player;
pub mod preview;
pub mod query;
mod render;
mod rests;
mod tools;
pub mod view;

use std::sync::Arc;

use kition_design_engine::Session;
use kition_design_engine::file_access::{Roots, confined};
use kition_kit::{KitCommand, KitDocument};
use serde_json::{Value, json};

pub use area::Area;

/// Commands whose file parameters the engine reads through lent files (`kition-design`'s SANDBOXED
/// list): none. The engine never reaches the file system here (every call runs confined to no
/// folder at all, [`no_files`]), so a command naming a path fails.
pub const SANDBOXED: &[&str] = &[];

/// The editor's state in [`KitDocument::inspect`], as JSON Pointers: the current page and what
/// inspect reports of it (its layer tree, its background, the document frame and the area the
/// editor opens on, its content bounds), the selection and its bounds, the tool, the text and
/// vector edit modes, the interaction under way and the hovered layer. It describes the editing
/// session, not the design, so a headless host leaves it out of what an agent reads
/// (`kition_headless::KitInfo::volatile`) and `testing::check_context` does not compare it; every
/// page's own facts are in `/pages`, and `page.inspect` lists any page's layers.
pub const EDITOR_STATE: &[&str] = &[
    "/page",
    "/tree",
    "/background",
    "/area",
    "/initialArea",
    "/contentBounds",
    "/selection",
    "/selectionBounds",
    "/tool",
    "/editing",
    "/textEdit",
    "/vectorEdit",
    "/interaction",
    "/hover",
];

/// The native extension.
pub const EXTENSION: &str = "kdesign";

/// One open Kition Design design.
pub struct DesignDocument {
    session: Session,
    /// The zoom of the last tool event, for the overlays a command run since needs.
    zoom: f64,
    /// The document frame held while a pointer gesture runs, so the document does not move under
    /// the pointer when the gesture grows the content; the frame follows the content again when
    /// the gesture ends.
    held: Option<Area>,
    /// The editor opened the engine's interaction ([`DesignDocument::begin_interaction`]).
    interaction: bool,
    /// The person's commands since the editor's interaction began, to run again on top of a change
    /// made by someone else meanwhile ([`DesignDocument::aside`]).
    pub(crate) replay: Vec<(String, Value)>,
    /// The editor told the engine its view ([`DesignDocument::set_viewport`]): the view's centre is
    /// the editor's, not the last pointer position.
    viewport: bool,
    /// The prototype player, while one plays ([`player`]).
    pub(crate) player: Option<Box<player::Player>>,
    /// The `kit.design` KDM schema the document's replica reads ([`collab`]).
    pub(crate) schema: Arc<kition_kit::kdm::Schema>,
    /// The library files the engine reads: what the editor lent, else the host's lent files
    /// ([`libraries`]).
    pub(crate) libraries: Arc<libraries::LentLibraries>,
    /// The editor states [`KitDocument::editor_state`] answered, for
    /// [`KitDocument::restore_editor_state`] ([`context`]); shared with the design's file read
    /// again and with its planning copies.
    kept: context::Kept,
}

/// Runs `f` with the engine's file access confined to no folder: in a page there is no file system
/// anyway, and the headless kit must never read or write files on an agent's behalf.
pub(crate) fn no_files<R>(f: impl FnOnce() -> R) -> R {
    let none = Roots::new(&[], &[]).ok().map(Arc::new);
    confined(none.as_ref(), f)
}

fn engine_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

/// The lower-case extension of `name`.
pub(crate) fn extension(name: &str) -> String {
    std::path::Path::new(name).extension().map(|ext| ext.to_string_lossy().to_ascii_lowercase()).unwrap_or_default()
}

/// The file name without folder and extension.
fn title_of(name: &str) -> String {
    let stem = std::path::Path::new(name).file_stem().map(|stem| stem.to_string_lossy().into_owned()).unwrap_or_default();
    if stem.trim().is_empty() { "Untitled".to_owned() } else { stem }
}

/// The extensions [`DesignDocument::open`] reads.
pub const OPENS: &[&str] = &["kdesign", "fig", "json", "sketch", "svg", "png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff"];

/// The largest file opened (the engine's own `.fig` and zip readers cap what they inflate).
const MAX_FILE: usize = 1 << 30;

impl DesignDocument {
    /// Refuses a parameter `command` does not take (a typo or a guessed name) before it runs,
    /// naming the one it resembles, instead of running without it (engine rule E7a). The schemas
    /// are the ones the catalogue offers agents, read once.
    pub(crate) fn refuse_unknown(&self, command: &str, params: &Value) -> Result<(), String> {
        static SCHEMAS: std::sync::OnceLock<std::collections::HashMap<String, Value>> = std::sync::OnceLock::new();
        let schemas = SCHEMAS.get_or_init(|| self.commands().into_iter().filter_map(|command| Some((command.id, command.schema?))).collect());
        schemas.get(command).map_or(Ok(()), |schema| kition_kit::schema::refuse_unknown(command, schema, params))
    }

    /// The document of an opened session, with its KDM replica (engine contract 2, [`collab`]).
    fn from_session(mut session: Session) -> Result<Self, String> {
        let schema = kition_design_kdm::schema().map_err(engine_error)?;
        if let Ok(ds) = session.ds_mut() {
            let mut taken: std::collections::HashSet<_> = ds.doc.ids().map(|n| n.0).collect();
            if ds.doc.comments.iter().any(|c| !taken.insert(c.id)) {
                kition_design_kdm::repair_comment_ids(Arc::make_mut(&mut ds.doc));
                ds.mark_saved();
            }
        }
        session.enable_kdm();
        let libraries = Arc::new(libraries::LentLibraries::default());
        session.libraries.lend(Some(libraries.clone() as Arc<dyn kition_design_engine::library::LibraryFiles>));
        Ok(Self {
            session,
            zoom: 1.0,
            held: None,
            interaction: false,
            replay: Vec::new(),
            viewport: false,
            player: None,
            schema,
            libraries,
            kept: context::Kept::default(),
        })
    }

    /// The engine session (read access, for tests and hosts).
    pub fn session(&self) -> &Session {
        &self.session
    }

    pub(crate) fn ds(&self) -> Result<&kition_design_engine::DocState, String> {
        self.session.ds().map_err(engine_error)
    }

    /// The document frame on the current page.
    pub fn area(&self) -> Area {
        self.held.unwrap_or_else(|| self.ds().map_or_else(|_| Area::empty(), |ds| Area::of(&ds.doc, ds.page)))
    }

    /// The document frame on `page`: the current page's ([`DesignDocument::area`]) for the current
    /// page or none, else that page's own (what it is once that page is the current one).
    pub fn area_on(&self, page: Option<kition_design_doc::NodeId>) -> Area {
        match (page, self.ds()) {
            (Some(page), Ok(ds)) if page != ds.page && ds.doc.pages.contains(&page) => Area::of(&ds.doc, page),
            _ => self.area(),
        }
    }

    /// Runs an engine command (file access confined to nothing; a panic in the command is the
    /// engine's `Internal` error and the document is rolled back), with the UI requests it made.
    fn run(&mut self, command: &str, params: &Value) -> Result<Value, String> {
        let session = &mut self.session;
        let result = no_files(|| session.execute(command, params)).map_err(engine_error);
        let requests = std::mem::take(&mut self.session.ui_requests);
        let mut value = result?;
        if !requests.is_empty() {
            let shown: Vec<Value> = requests.into_iter().map(|request| json!({ "kind": request.kind, "data": request.data })).collect();
            match value.as_object_mut() {
                Some(object) => {
                    object.insert("uiRequests".into(), Value::Array(shown));
                }
                None => value = json!({ "result": value, "uiRequests": shown }),
            }
        }
        Ok(value)
    }

    /// `edit.undo` / `edit.redo` in an undo scope (engine contract 2 E14a): `scope` (`person` or
    /// `agent`), else the scope of who runs it ([`collab`]); the agent's undo takes back only the
    /// agent's steps and keeps the person's later work, and the other way round.
    fn history(&mut self, command: &str, params: &Value) -> Result<Value, String> {
        let scope = match params.get("scope") {
            None | Some(Value::Null) => Value::Null,
            Some(Value::String(scope)) if scope == "person" || scope == "agent" => Value::String(scope.clone()),
            Some(other) => return Err(format!("invalid-params: `scope` must be \"person\" or \"agent\", not {other}")),
        };
        let result = self.run(command, &json!({ "scope": scope }))?;
        let done = result.get("undone").or_else(|| result.get("redone")).and_then(Value::as_bool).unwrap_or(false);
        Ok(json!({ "done": done }))
    }
}

/// Opens `name` (`bytes`) in a new session as [`KitDocument::open`] does, the bridge still off and
/// the document clean.
pub(crate) fn open_session(name: &str, bytes: &[u8]) -> Result<Session, String> {
    let ext = extension(name);
    if !OPENS.contains(&ext.as_str()) {
        return Err(format!("Design opens {} files, not {name:?}", OPENS.iter().map(|e| format!(".{e}")).collect::<Vec<_>>().join(", ")));
    }
    if bytes.is_empty() {
        return Err(format!("{name} is empty"));
    }
    if bytes.len() > MAX_FILE {
        return Err(format!("{name} is larger than {} MiB", MAX_FILE >> 20));
    }
    let mut session = Session::empty();
    let data = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes);
    no_files(|| session.execute("file.open", &json!({ "name": name, "data": data }))).map_err(engine_error)?;
    session.ui_requests.clear();
    if session.ds().is_err() {
        return Err(format!("{name} opened no document"));
    }
    // An import is a new document from the file; the native file is as it was read.
    if let Ok(ds) = session.ds_mut() {
        ds.mark_saved();
    }
    Ok(session)
}

impl KitDocument for DesignDocument {
    /// Opens a `.kdesign` design, or imports a `.fig` file, a REST API file `.json`, a `.sketch`
    /// file, an SVG, or a picture: `name`'s extension picks the reader (`.kdesign` also tries the
    /// import formats, as upstream's open does).
    fn open(name: &str, bytes: &[u8]) -> Result<Self, String> {
        Self::from_session(open_session(name, bytes)?)
    }

    /// A new design titled after `name`: one page holding one `width × height` frame at the
    /// canvas origin (upstream's "New design file" with a first frame, so the document has a
    /// size).
    fn create(name: &str, width: u32, height: u32) -> Result<Self, String> {
        let mut session = Session::with_document(kition_design_doc::Document::new(title_of(name)));
        let (w, h) = (f64::from(width.clamp(1, 100_000)), f64::from(height.clamp(1, 100_000)));
        no_files(|| session.execute("frame.create", &json!({ "x": 0, "y": 0, "width": w, "height": h }))).map_err(engine_error)?;
        session.ui_requests.clear();
        session.set_selection(Vec::new());
        if let Ok(ds) = session.ds_mut() {
            ds.history.undo.clear();
            ds.history.redo.clear();
            ds.mark_saved();
        }
        session.journal.clear();
        session.last_command = None;
        Self::from_session(session)
    }

    fn width(&self) -> u32 {
        self.area().width
    }

    fn height(&self) -> u32 {
        self.area().height
    }

    fn revision(&self) -> u64 {
        self.ds().map_or(0, |ds| ds.revision)
    }

    fn dirty(&self) -> bool {
        self.ds().is_ok_and(kition_design_engine::DocState::is_dirty)
    }

    fn render(&self, x: f64, y: f64, scale: f64, width: u32, height: u32) -> Result<Vec<u8>, String> {
        render::render(self, x, y, scale, width, height)
    }

    fn render_area(&self, x0: i32, y0: i32, x1: i32, y1: i32, width: u32, height: u32) -> Result<Vec<u8>, String> {
        render::render_area(self, x0, y0, x1, y1, width, height)
    }

    /// Runs `command`. An agent's command that names what it acts on (layers, a parent, a page)
    /// acts on the page it names and leaves the current page, the selection, the view, the edit
    /// modes and the tool as they were, also when it fails; what it made on the current page
    /// becomes the selection, for the run's next step. Only the commands whose work is moving the
    /// editor ([`context::moves_the_editor`]: `select.*`, `page.select`, `page.go`, `view.*`,
    /// `tool.*`, the edit modes) keep their effect. A person's commands run as they always did.
    fn execute(&mut self, command: &str, params: Value) -> Result<Value, String> {
        let params = if params.is_null() { json!({}) } else { params };
        if !params.is_object() {
            return Err("invalid-params: params must be an object".into());
        }
        self.refuse_unknown(command, &params)?;
        if agent::owns(command) {
            return agent::run(self, command, &params);
        }
        if self.is_agent() && !context::moves_the_editor(command) && context::names_target(&params) {
            let named = params.clone();
            return self.bracketed(&named, |doc| doc.run_offered(command, params));
        }
        self.run_offered(command, params)
    }

    /// The editor's state besides the design (the current page, the selection, the view, the
    /// text and vector edit modes, the tool), kept by the document and answered as
    /// `{snapshot, page, selection}` ([`context`]).
    fn editor_state(&self) -> Value {
        self.take_editor_state()
    }

    /// Puts back what [`KitDocument::editor_state`] answered; no edit. A page or layer the run
    /// deleted is dropped. A state taken on the design this one replaces or copies (its file read
    /// again, a planning copy) is refused unless every active page, target and mode resolves here
    /// (`deleted active target 12`).
    fn restore_editor_state(&mut self, saved: &Value) -> Result<(), String> {
        self.put_back(saved)
    }

    /// The design's file read again (`self`) takes the editor states kept by the design it
    /// replaces, so the host can restore the previous editor state here and every active target
    /// is checked against the file as it is now.
    fn carry_replica_objects(&mut self, previous: &Self) -> Result<(), String> {
        self.share_kept(previous);
        Ok(())
    }

    /// Takes in another program's save of this clean design: the editor state must resolve on
    /// `fresh` (else the reason, nothing changed), then only the file's difference is written,
    /// as one `import` transaction outside both undo histories, which stay
    /// (`Session::reconcile_reread`).
    fn reconcile_reread(&mut self, fresh: &Self) -> Result<bool, String> {
        self.state_resolves_on(fresh)?;
        self.session.reconcile_reread(fresh.ds()?).map_err(engine_error)?;
        self.share_kept(fresh);
        Ok(true)
    }

    /// A copy for planning a guarded run: the same pages, layers and their ids, the same replica
    /// identities and blobs, the editor state, clipboard, preferences and lent libraries; its own
    /// replica, an empty undo history, no player and no file access.
    fn fork_for_premises(&self) -> Result<Self, String> {
        let kdm = self.session.kdm().ok_or("premise planning needs the design's replica")?;
        let snapshot = kdm.snapshot().map_err(engine_error)?;
        let blobs = kdm.blobs().clone();
        let mut copy = rests::scratch(self)?;
        copy.session.attach_replica(&snapshot, blobs).map_err(engine_error)?;
        copy.session.enable_kdm();
        if let Ok(ds) = copy.session.ds_mut() {
            ds.history.undo.clear();
            ds.history.redo.clear();
        }
        copy.share_kept(self);
        Ok(copy)
    }

    /// Sets the page a call acts on (`part`: `{pageId}` or `{page}`) and the layers it acts on
    /// there (`on`), as [`context`] describes; refuses what the design lacks and `time`. No edit.
    fn set_context(&mut self, context: &kition_kit::CallContext) -> Result<(), String> {
        self.set_call_context(context)
    }

    /// The design as JSON (cheap: it runs after every change): history, the document frame, the
    /// current page, the pages, the selection and its bounds in document pixels, the tool and the
    /// editing modes, the area the editor opens on, the hovered layer (`hover`: id or null, shared
    /// by the canvas and the layers panel, see [`DesignDocument::set_hover`]) and the page's content
    /// (`contentBounds`: the top-level layers' bounds in document pixels, null on an empty page).
    fn inspect(&self) -> Result<Value, String> {
        let ds = self.ds()?;
        let doc = &ds.doc;
        let area = self.area();
        let (undo, redo) = self.session.undo_depth();
        let pages: Vec<Value> = doc
            .pages
            .iter()
            .enumerate()
            .filter_map(|(index, id)| doc.node(*id).map(|page| json!({ "id": id, "index": index, "name": page.name, "layers": page.children.len() })))
            .collect();
        let selection_bounds = doc.bounds_of(&ds.selection).map(|b| area.rect_json(b));
        let first_frame = doc
            .node(ds.page)
            .and_then(|page| page.children.iter().find_map(|c| doc.node(*c).filter(|n| n.kind.is_frame_like() && n.visible).map(|n| n.id)))
            .and_then(|id| doc.world_bounds(id));
        Ok(json!({
            "canUndo": undo > 0,
            "canRedo": redo > 0,
            "name": doc.name,
            "kind": doc.kind.id(),
            "area": area.to_json(),
            "page": ds.page,
            "pages": pages,
            "background": query::page_background(doc, ds.page).to_css_hex(),
            "selection": ds.selection,
            "selectionBounds": selection_bounds,
            "layers": doc.nodes().filter(|n| !n.is_page()).count(),
            // The current page's layers as the Layers panel lists them (at most 500).
            "tree": kition_design_engine::inspect::layer_tree(doc, ds.page, 500).0,
            "tool": self.session.tool.tool.id(),
            "editing": if ds.text_edit.is_some() { "text" } else if ds.vector_edit.is_some() { "vector" } else { "none" },
            "textEdit": ds.text_edit.as_ref().map(|te| json!({ "id": te.id, "caret": te.caret, "anchor": te.anchor })),
            "vectorEdit": ds.vector_edit.as_ref().map(|ve| json!({ "id": ve.id, "tool": ve.tool.id() })),
            "initialArea": first_frame.or_else(|| doc.page_content_bounds(ds.page)).map(|b| area.rect_json(b)),
            "colorProfile": doc.color_profile.id(),
            "undo": self.session.undo_labels(1).first(),
            "interaction": self.interaction_label(),
            "hover": ds.hover,
            "contentBounds": doc.page_content_bounds(ds.page).map(|b| area.rect_json(b)),
        }))
    }

    /// The engine's commands with the upstream menus' titles, paths and shortcuts. Board-only
    /// commands (`board.*`, the board tools) stay listed for board files and are disabled on
    /// designs ([`commands`]).
    fn commands(&self) -> Vec<KitCommand> {
        let mut commands: Vec<kition_kit::KitCommand> = {
            let mut list = commands::list(&self.session);
            list.extend(agent::commands(self.session.ds().is_ok()));
            list
        };
        agent_text::apply(&mut commands);
        for command in &mut commands {
            if let Some(schema) = command.schema.as_mut() {
                params::complete(&command.id, schema);
            }
            command.reads = acts::reads_any(&command.id);
            command.targets = acts::targets_any(&command.id);
            addressing::declare(command);
            if command.id == "library.place" {
                command.places = &["component", "x", "y"];
            } else if matches!(command.id.as_str(), "instance.swapAt" | "instance.place" | "library.swapAt" | "tool.place") {
                command.places = &["x", "y"];
                if command.id == "tool.place" {
                    command.places = &["tool", "x", "y"];
                }
            } else if command.id.starts_with("proto.") || command.id.starts_with("variable.") {
                command.places = &["expression", "condition", "expr", "action", "actions", "interaction", "blocks", "value", "values"];
            } else if command.id == "node.set" {
                // The pinned SDK reads place keys at the top level; nested paths are not followed.
                command.places = &["props"];
            } else if command.id == "style.renameFolder" {
                command.places = &["folder"];
            }
            if matches!(command.id.as_str(), "node.create" | "node.reparent" | "node.reorder" | "page.move") {
                command.places = &["index"];
            }
        }
        kition_kit::files::declare(&mut commands, files::FILES);
        commands
    }

    fn disabled_commands(&self) -> Vec<String> {
        commands::disabled(&self.session)
    }

    fn under(&self, command: &str, params: &Value) -> Result<Option<kition_kit::Under>, String> {
        rests::under(self, command, params)
    }

    fn covered(&self, command: &str, params: &Value, objects: &[kition_kit::CoveredAsk]) -> Result<Option<kition_kit::CoveredAt>, String> {
        rests::resolve::covered(self, command, params, objects)
    }

    fn derived(&self, ids: &[Value], names: &[String]) -> Result<Vec<kition_kit::DerivedValues>, String> {
        rests::resolve::derived(self, ids, names)
    }

    fn carried(&self, ids: &[Value], changes: &[Value]) -> Result<Vec<kition_kit::Carried>, String> {
        rests::resolve::carried(self, ids, changes)
    }

    /// The design as `.kdesign` bytes: its content (the replica's sealed content snapshot: no
    /// history, rule M16b), thumbnail and branches. Hosts call [`KitDocument::prepare_save`]
    /// first; a session nobody sealed saves the content of a sealed copy, whose seal
    /// [`KitDocument::mark_saved`] takes in (`kition_design_kdm::file::save`). Other extensions
    /// are exports (`file.export`), not saves.
    fn save(&self, name: Option<&str>) -> Result<Vec<u8>, String> {
        if let Some(name) = name {
            let ext = extension(name);
            if !ext.is_empty() && ext != EXTENSION {
                return Err(format!("saving as .{ext} is an export (file.export); save the design as .{EXTENSION}"));
            }
        }
        let ds = self.ds()?;
        kition_design_engine::cmd::file::file_bytes(ds).map_err(engine_error)
    }

    /// Seals the design's replica (engine contract E4, rule M16b) so `save` writes its content
    /// snapshot and the seal is in the replica the host stores: no edit, no undo step.
    fn prepare_save(&mut self) -> Result<(), String> {
        let origin = self.session.origin();
        self.session.ds_mut().map_err(engine_error)?.prepare_save(&origin).map_err(engine_error)
    }

    fn mark_saved(&mut self) {
        if let Ok(ds) = self.session.ds_mut() {
            ds.mark_saved();
        }
    }

    /// What is wrong with the design, errors first, every code in [`check::CATALOGUE`] ([`check`]).
    fn check(&self) -> Vec<kition_kit::diagnostics::Diagnostic> {
        let unreadable = libraries::unreadable(self);
        self.ds().map(|ds| check::check_libraries(&ds.doc, &unreadable)).unwrap_or_default()
    }
}

impl DesignDocument {
    /// Runs an offered command (not one of the binding's own, [`agent`]).
    fn run_offered(&mut self, command: &str, params: Value) -> Result<Value, String> {
        if !commands::offered(command) {
            return Err(format!("unknown command {command}"));
        }
        if command == "command.batch" {
            agent::screen_batch(&params)?;
        }
        let result = match command {
            "edit.undo" | "edit.redo" => {
                self.end_open_interaction();
                self.history(command, &params)
            }
            _ => self.run(command, &params),
        };
        if self.interaction && result.is_ok() && self.replay.len() < view::MAX_REPLAY {
            self.replay.push((command.to_owned(), params));
        }
        if !self.session.tool.busy() {
            self.held = None;
        }
        result
    }
}

kition_kit::export_kit_document!(DesignDocument as "DesignDocument");
kition_kit::export_kit_tools!(DesignDocument as "DesignDocument");

/// Lays out and draws text with the fonts built into the kit alone (the kition-fonts faces the
/// build embeds), never the machine's: what the headless kit does, so its previews and text sizes
/// do not depend on the machine it runs on (task card C4/C5/C6-design).
pub fn pin_fonts() {
    kition_design_text::opt_out_of_system_fonts();
}

/// Adds a font file's faces for text the embedded faces do not cover (standard E9); answers how
/// many families it added.
pub fn add_font(bytes: &[u8]) -> usize {
    kition_design_text::fontdb::load_font_bytes(bytes.to_vec()).len()
}

impl DesignDocument {
    /// `query(name, params)`: the editor's panels' answers ([`query`]).
    pub fn query(&self, name: &str, params: &Value) -> Result<Value, String> {
        query::query(self, name, params)
    }

    /// `chrome(zoom)`: the canvas chrome in document pixels ([`chrome`]). It also tells the engine
    /// where the frame labels are, so presses on them select their frames.
    pub fn chrome(&mut self, zoom: f64) -> Value {
        let zoom = if zoom.is_finite() && zoom > 0.0 { zoom } else { 1.0 };
        let (value, labels) = chrome::chrome(self, zoom);
        self.session.tool.labels = labels;
        value
    }

    /// One layer exported as `format` (`png`, `jpg`, `webp`, `svg`, `pdf`) at `scale`: the file
    /// bytes, as the Export section's button makes them. Never changes the document.
    pub fn export_node(&self, id: &str, format: &str, scale: f64) -> Result<Vec<u8>, String> {
        query::export_node(self, id, format, scale)
    }

    /// One layer exported with an export setting's options ([`query::export_node_with`]).
    pub fn export_node_with(&self, id: &str, params: &Value) -> Result<Vec<u8>, String> {
        query::export_node_with(self, id, params)
    }

    /// A brush's outline along a short line as SVG path data, as the brush menu's rows draw it
    /// ([`panels::stroke_preview_outline`]).
    pub fn stroke_preview_outline(&self, params: &Value) -> Result<String, String> {
        panels::stroke_preview_outline(params)
    }

    /// A PNG thumbnail of the first page (what `.kdesign` files carry).
    pub fn thumbnail(&self) -> Result<Vec<u8>, String> {
        let ds = self.ds()?;
        kition_design_engine::cmd::file::thumbnail(&ds.doc).ok_or_else(|| "the first page is empty".to_owned())
    }

    /// Layer `id` as a PNG fitting a `size` × `size` square ([`preview::node_thumbnail`]).
    pub fn node_thumbnail(&self, id: &str, size: f64) -> Result<Vec<u8>, String> {
        preview::node_thumbnail(self, id, size)
    }

    /// Text in its own font as a PNG ([`preview::text_preview`]).
    pub fn text_preview(&self, params: &Value) -> Result<Vec<u8>, String> {
        preview::text_preview(self, params)
    }

    /// The colour the canvas shows at a document pixel ([`preview::sample_pixel`]).
    pub fn sample_pixel(&self, x: f64, y: f64, zoom: f64) -> Result<String, String> {
        preview::sample_pixel(self, x, y, zoom)
    }
}

/// Design's own additions to the exported class, for its editor.
#[cfg(target_arch = "wasm32")]
pub mod web {
    use kition_kit::wasm_bindgen::prelude::*;

    use crate::__kition_kit_export::WebDocument;

    fn error(message: String) -> JsError {
        JsError::new(&message)
    }

    /// Adds a font file's faces (standard E9); answers how many families it added.
    #[wasm_bindgen(js_name = addFont, wasm_bindgen = kition_kit::wasm_bindgen)]
    pub fn add_font(bytes: &[u8]) -> u32 {
        u32::try_from(crate::add_font(bytes)).unwrap_or(u32::MAX)
    }

    #[wasm_bindgen(js_class = "DesignDocument", wasm_bindgen = kition_kit::wasm_bindgen)]
    impl WebDocument {
        /// The answer of panel query `name` (see `query.rs`) as JSON; `params` is a JSON object
        /// (an empty string is `{}`).
        pub fn query(&self, name: &str, params: &str) -> Result<String, JsError> {
            let params = kition_kit::parse_params(params).map_err(error)?;
            self.0.query(name, &params).map(|value| value.to_string()).map_err(error)
        }

        /// The canvas chrome at `zoom` (screen pixels per document pixel) as JSON (see
        /// `chrome.rs`).
        pub fn chrome(&mut self, zoom: f64) -> String {
            self.0.chrome(zoom).to_string()
        }

        /// Layer `id` exported as `format` at `scale`: the file's bytes.
        #[wasm_bindgen(js_name = exportNode)]
        pub fn export_node(&self, id: &str, format: &str, scale: f64) -> Result<Vec<u8>, JsError> {
            self.0.export_node(id, format, scale).map_err(error)
        }

        /// Layer `id` exported with an export setting's options; `params` is a JSON object
        /// `{ index?, setting?, format?, scale? }` (see `query.rs` `export_node_with`).
        #[wasm_bindgen(js_name = exportNodeWith)]
        pub fn export_node_with(&self, id: &str, params: &str) -> Result<Vec<u8>, JsError> {
            let params = kition_kit::parse_params(params).map_err(error)?;
            self.0.export_node_with(id, &params).map_err(error)
        }

        /// A brush's outline along a short line as SVG path data in CSS pixels; `params` is a
        /// JSON object `{ preset, width?, height? }` (see `panels.rs`).
        #[wasm_bindgen(js_name = strokePreviewOutline)]
        pub fn stroke_preview_outline(&self, params: &str) -> Result<String, JsError> {
            let params = kition_kit::parse_params(params).map_err(error)?;
            self.0.stroke_preview_outline(&params).map_err(error)
        }

        /// Outlines layer `id` on the canvas as the hover does (null or an empty string clears
        /// it); `inspect().hover` reads it back. Never moves the revision. `true` when it changed.
        #[wasm_bindgen(js_name = setHover)]
        pub fn set_hover(&mut self, id: Option<String>) -> Result<bool, JsError> {
            self.0.set_hover(id.as_deref()).map_err(error)
        }

        /// Turns Dev Mode's tool state on or off (measure on hover, no handles, no editing of
        /// text or vectors); `true` when it changed.
        #[wasm_bindgen(js_name = setDevMode)]
        pub fn set_dev_mode(&mut self, on: bool) -> Result<bool, JsError> {
            self.0.set_dev_mode(on).map_err(error)
        }

        /// Lends the library file `path` (as the document names it, `query('library.files')`)
        /// that the editor read with the host: its `bytes`, or `null` when it can't be read.
        /// Never an edit; `true` when what the path reads changed (see `libraries.rs`).
        #[wasm_bindgen(js_name = lendLibrary)]
        pub fn lend_library(&mut self, path: &str, bytes: Option<Vec<u8>>) -> Result<bool, JsError> {
            self.0.lend_library(path, bytes.as_deref()).map_err(error)
        }

        /// Component (or set) `id` of enabled library `library` drawn to fit a `size` × `size`
        /// square, as a PNG (the Assets panel's library tiles).
        #[wasm_bindgen(js_name = libraryThumbnail)]
        pub fn library_thumbnail(&self, library: &str, id: &str, size: f64) -> Result<Vec<u8>, JsError> {
            self.0.library_thumbnail(library, id, size).map_err(error)
        }

        /// A PNG thumbnail of the first page.
        pub fn thumbnail(&self) -> Result<Vec<u8>, JsError> {
            self.0.thumbnail().map_err(error)
        }

        /// Starts an interaction named `label`: the edits until `commitInteraction` are one undo
        /// step. `true` when this call opened it, `false` when one was open already (see `view.rs`).
        #[wasm_bindgen(js_name = beginInteraction)]
        pub fn begin_interaction(&mut self, label: &str) -> Result<bool, JsError> {
            self.0.begin_interaction(label).map_err(error)
        }

        /// Ends the editor's interaction as one undo step: `true` when it recorded one.
        #[wasm_bindgen(js_name = commitInteraction)]
        pub fn commit_interaction(&mut self) -> Result<bool, JsError> {
            self.0.commit_interaction().map_err(error)
        }

        /// Abandons the editor's interaction, restoring the document: `true` when there was one.
        #[wasm_bindgen(js_name = cancelInteraction)]
        pub fn cancel_interaction(&mut self) -> Result<bool, JsError> {
            self.0.cancel_interaction().map_err(error)
        }

        /// Tells the engine the editor's view: document pixel (`x`, `y`) at the viewport's
        /// top-left, `width` × `height` screen pixels, `zoom`.
        #[wasm_bindgen(js_name = setViewport)]
        pub fn set_viewport(&mut self, x: f64, y: f64, width: f64, height: f64, zoom: f64) {
            self.0.set_viewport(x, y, width, height, zoom);
        }

        /// Where View menu command `command` takes the view, as JSON (see `view.rs`); `params` is
        /// a JSON object (an empty string is `{}`).
        #[wasm_bindgen(js_name = viewTarget)]
        pub fn view_target(&self, command: &str, params: &str) -> Result<String, JsError> {
            let params = kition_kit::parse_params(params).map_err(error)?;
            self.0.view_target(command, &params).map(|value| value.to_string()).map_err(error)
        }

        /// Layer `id` as a PNG fitting a `size` × `size` square.
        #[wasm_bindgen(js_name = nodeThumbnail)]
        pub fn node_thumbnail(&self, id: &str, size: f64) -> Result<Vec<u8>, JsError> {
            self.0.node_thumbnail(id, size).map_err(error)
        }

        /// Text in its own font as a PNG; `params` is a JSON object (see `preview.rs`).
        #[wasm_bindgen(js_name = textPreview)]
        pub fn text_preview(&self, params: &str) -> Result<Vec<u8>, JsError> {
            let params = kition_kit::parse_params(params).map_err(error)?;
            self.0.text_preview(&params).map_err(error)
        }

        /// The colour the canvas shows at document pixel (`x`, `y`) at `zoom`: `#rrggbb`.
        #[wasm_bindgen(js_name = samplePixel)]
        pub fn sample_pixel(&self, x: f64, y: f64, zoom: f64) -> Result<String, JsError> {
            self.0.sample_pixel(x, y, zoom).map_err(error)
        }
    }
}
