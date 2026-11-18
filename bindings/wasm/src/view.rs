//! The editor's view and its gestures, the two things upstream's UI wrote into the session
//! directly.
//!
//! **Interactions.** Upstream groups a continuous edit (a number scrub, a colour picker drag, a
//! canvas handle drag such as auto layout padding, a slider) into one undo step with the session's
//! `begin_interaction(label)` … `commit_interaction()` (or `cancel_interaction()`), running ordinary
//! commands in between (`panels/right.rs` `field_run`, `widgets/colorpicker.rs` `write`,
//! `canvas/handles.rs`). The kit exposes the same: [`DesignDocument::begin_interaction`],
//! [`DesignDocument::commit_interaction`], [`DesignDocument::cancel_interaction`]. Every `execute` between
//! begin and commit changes the document and moves the revision (so the canvas re-renders live),
//! and commit records them as one undo step named `label` (none when nothing changed). An
//! interaction the engine already holds (text being edited, a pointer gesture) is joined rather
//! than taken over: begin answers `false` and commit and cancel leave it to its owner, as upstream's
//! typography panel runs commands straight into the text editing session. A pointer press, a key,
//! a tool switch, undo and redo first commit an interaction the editor left open.
//!
//! **The view.** The kit's canvas owns zoom and pan (kit-editor). The engine still reads the view:
//! the zoom for hit tolerances and handle sizes (every tool event carries it), and the view's
//! centre and size where it places things "in the view" (Enter with a shape tool, a frame preset on
//! an empty page). [`DesignDocument::set_viewport`] tells it. The View menu's zoom commands
//! (`view.zoomFit`, `view.zoomSelection`, `view.zoom100`, …) are the canvas's, so the catalog
//! leaves them out; [`DesignDocument::view_target`] answers where each would take the view, computed by
//! the engine's own command (its zoom steps and its 64 px fit margin), for the editor to animate its
//! viewport there.

use kition_design_doc::NodeId;
use kition_design_engine::{Session, ViewState};
use kition_design_geom::Rect;
use serde_json::{Value, json};

use crate::{DesignDocument, engine_error, no_files};

/// The most commands [`DesignDocument::aside`] runs again (an interaction longer than that keeps
/// only its first ones there).
pub const MAX_REPLAY: usize = 4096;

/// The commands [`DesignDocument::view_target`] answers.
pub const VIEW_TARGETS: &[&str] = &[
    "view.zoomIn",
    "view.zoomOut",
    "view.zoom100",
    "view.zoom50",
    "view.zoom200",
    "view.zoomFit",
    "view.zoomSelection",
    "view.zoomAt",
    "view.prevFrame",
    "view.nextFrame",
];

fn finite(v: f64) -> Option<f64> {
    v.is_finite().then_some(v)
}

fn num(params: &Value, key: &str) -> Option<f64> {
    params.get(key).and_then(Value::as_f64).and_then(finite)
}

impl DesignDocument {
    /// Starts an interaction named `label` (the undo step's name): `true` when this call opened
    /// it, `false` when one was already open (the editor's own, text editing or a pointer gesture),
    /// which the calls since join. An error when the document cannot be edited now (a version shown
    /// in place of it).
    pub fn begin_interaction(&mut self, label: &str) -> Result<bool, String> {
        let label = if label.trim().is_empty() { "Edit" } else { label.trim() };
        if self.ds()?.interaction.is_some() {
            return Ok(false);
        }
        self.session.begin_interaction(label).map_err(engine_error)?;
        self.interaction = true;
        self.replay.clear();
        Ok(true)
    }

    /// Ends the interaction the editor began as one undo step: `true` when it recorded one (the
    /// document changed), `false` when nothing changed or the editor holds no interaction.
    pub fn commit_interaction(&mut self) -> Result<bool, String> {
        self.replay.clear();
        if !std::mem::take(&mut self.interaction) || self.ds()?.interaction.is_none() {
            return Ok(false);
        }
        let before = self.session.undo_depth().0;
        self.session.commit_interaction().map_err(engine_error)?;
        Ok(self.session.undo_depth().0 > before)
    }

    /// Abandons the interaction the editor began, putting the document back as it was at begin:
    /// `true` when there was one.
    pub fn cancel_interaction(&mut self) -> Result<bool, String> {
        self.replay.clear();
        if !std::mem::take(&mut self.interaction) || self.ds()?.interaction.is_none() {
            return Ok(false);
        }
        self.session.cancel_interaction().map_err(engine_error)?;
        Ok(true)
    }

    /// Runs `f` (someone else's change: an agent's command, a collaborator's import) beside the
    /// interaction the editor holds open, as a change of its own: the interaction is set aside (the
    /// document back at its start), `f` runs on that, and the interaction begins again from the
    /// result with the person's commands since it began run again, so it stays one undo step of
    /// the person's alone, its cancel keeps `f`'s change, and the screen keeps the person's edit.
    /// Without an open interaction of the editor's, `f` just runs.
    pub(crate) fn aside<R>(&mut self, f: impl FnOnce(&mut Self) -> Result<R, String>) -> Result<R, String> {
        let open = self
            .interaction
            .then(|| self.ds().ok().and_then(|ds| ds.interaction.as_ref().map(|it| (it.label.clone(), it.selection.clone()))))
            .flatten();
        let Some((label, selection)) = open else { return f(self) };
        let replay = std::mem::take(&mut self.replay);
        self.interaction = false;
        self.session.cancel_interaction().map_err(engine_error)?;
        let result = f(self);
        let alive: Vec<NodeId> = match self.ds() {
            Ok(ds) => selection.into_iter().filter(|id| ds.doc.contains(*id)).collect(),
            Err(_) => return result,
        };
        self.session.set_selection(alive);
        if self.begin_interaction(&label).is_ok() {
            let origin = self.session.set_origin(None);
            for (command, params) in &replay {
                // A command the change made impossible (its layer gone) is left out.
                let _ = self.run(command, params);
            }
            self.session.set_origin(origin);
            self.replay = replay;
        }
        result
    }

    /// The editor's open interaction's label, if it holds one.
    pub fn interaction_label(&self) -> Option<String> {
        if !self.interaction {
            return None;
        }
        self.ds().ok().and_then(|ds| ds.interaction.as_ref().map(|it| it.label.clone()))
    }

    /// Commits an interaction the editor left open before something that ends it.
    pub(crate) fn end_open_interaction(&mut self) {
        if self.interaction {
            let _ = self.commit_interaction();
        }
    }

    /// Shares the hovered layer between the layers panel and the canvas (upstream's `ds.hover`):
    /// `Some(id)` outlines layer `id` on the canvas as the pointer's hover does (the next tool
    /// answer, `state` included, carries the outline), `None` clears it. `inspect().hover` is the
    /// hovered layer either way, so a row can show the canvas's hover. Never changes the document
    /// or its revision. An error for an unknown layer, a page, or a layer on another page; answers
    /// whether the hover changed.
    pub fn set_hover(&mut self, id: Option<&str>) -> Result<bool, String> {
        let id = match id.map(str::trim).filter(|s| !s.is_empty() && *s != "null") {
            None => None,
            Some(text) => {
                let id: NodeId = text.parse().map_err(|_| format!("invalid-params: {text:?} is not a layer id"))?;
                let ds = self.ds()?;
                let n = ds.doc.node(id).ok_or_else(|| format!("not-found: no layer {id}"))?;
                if n.is_page() || ds.doc.page_of(id) != Some(ds.page) {
                    return Err(format!("invalid-params: layer {id} is not on the current page"));
                }
                Some(id)
            }
        };
        let ds = self.session.ds_mut().map_err(engine_error)?;
        let changed = ds.hover != id;
        ds.hover = id;
        Ok(changed)
    }

    /// Turns Dev Mode's tool state on or off, as upstream does when the Inspect tab is chosen
    /// (`app/commands.rs` `sync_dev_mode`): turning it on drops a pointer gesture in progress and
    /// ends text and vector editing; while on, the canvas measures on hover without Alt and shows
    /// no handles, and `query("toolbar").devMode` is true. (`view.devMode` only asks the editor to
    /// show the Dev Mode panel.) Answers whether the state changed.
    pub fn set_dev_mode(&mut self, on: bool) -> Result<bool, String> {
        if self.session.tool.dev_mode == on {
            return Ok(false);
        }
        if on {
            self.end_open_interaction();
            let _ = self.session.drop_gesture();
            let editing = self.ds().map(|ds| (ds.text_edit.is_some(), ds.vector_edit.is_some()))?;
            for (active, command) in [(editing.0, "text.edit.end"), (editing.1, "vector.edit.end")] {
                if active {
                    self.run(command, &json!({}))?;
                }
            }
            self.held = None;
        }
        self.session.tool.dev_mode = on;
        Ok(true)
    }

    /// Tells the engine the editor's view: document pixel (`x`, `y`) is at the viewport's top-left,
    /// the viewport is `width` × `height` screen pixels, `zoom` screen pixels per document pixel.
    /// Never changes the document or its revision.
    pub fn set_viewport(&mut self, x: f64, y: f64, width: f64, height: f64, zoom: f64) {
        let zoom = finite(zoom).filter(|z| *z > 0.0).map_or(self.zoom, |z| z.clamp(ViewState::MIN_ZOOM, ViewState::MAX_ZOOM));
        self.zoom = zoom;
        let area = self.area();
        let Ok(ds) = self.session.ds_mut() else { return };
        let v = &mut ds.view;
        v.zoom = zoom;
        if let Some(w) = finite(width).filter(|w| *w > 0.0) {
            v.vw = w.clamp(10.0, 20_000.0);
        }
        if let Some(h) = finite(height).filter(|h| *h > 0.0) {
            v.vh = h.clamp(10.0, 20_000.0);
        }
        if let (Some(x), Some(y)) = (finite(x), finite(y)) {
            let centre = area.to_canvas(x + v.vw / zoom / 2.0, y + v.vh / zoom / 2.0);
            (v.cx, v.cy) = (centre.x, centre.y);
        }
        self.viewport = true;
    }

    /// Where View menu command `command` takes the view (see [`VIEW_TARGETS`]), without changing
    /// the document, the selection or the engine's view. `params`: the current view `{ zoom?, x?,
    /// y?, width?, height? }` as [`set_viewport`](Self::set_viewport) takes it (default: the last
    /// one set), and for `view.zoomAt` the point `{ atX, atY }` (document pixels) it keeps in place,
    /// `out?`, or an area `{ atX, atY, atWidth, atHeight }` to fit. The answer:
    ///
    /// ```text
    /// { command, zoom,                 // the zoom the view ends at
    ///   x, y,                          // the document pixel at the viewport's top-left then
    ///   centerX, centerY,              // the document pixel at its centre
    ///   rect: null | { x, y, width, height },   // what it fits (content, selection, frame)
    ///   selection: null | [id] }       // the selection the command leaves (frame steps select)
    /// ```
    pub fn view_target(&self, command: &str, params: &Value) -> Result<Value, String> {
        if !VIEW_TARGETS.contains(&command) {
            return Err(format!("invalid-params: viewTarget answers {}, not {command}", VIEW_TARGETS.join(", ")));
        }
        let params = if params.is_null() { json!({}) } else { params.clone() };
        if !params.is_object() {
            return Err("invalid-params: params must be an object".into());
        }
        let area = self.area();
        let mut ds = self.ds()?.clone();
        let v = &mut ds.view;
        if let Some(z) = num(&params, "zoom").filter(|z| *z > 0.0) {
            v.zoom = z.clamp(ViewState::MIN_ZOOM, ViewState::MAX_ZOOM);
        }
        if let Some(w) = num(&params, "width").filter(|w| *w > 0.0) {
            v.vw = w.clamp(10.0, 20_000.0);
        }
        if let Some(h) = num(&params, "height").filter(|h| *h > 0.0) {
            v.vh = h.clamp(10.0, 20_000.0);
        }
        if let (Some(x), Some(y)) = (num(&params, "x"), num(&params, "y")) {
            let c = area.to_canvas(x + v.vw / v.zoom / 2.0, y + v.vh / v.zoom / 2.0);
            (v.cx, v.cy) = (c.x, c.y);
        }
        let selection_before = ds.selection.clone();
        let rect: Option<Rect> = match command {
            "view.zoomFit" => ds.doc.page_content_bounds(ds.page),
            "view.zoomSelection" => ds.doc.bounds_of(&ds.selection),
            _ => None,
        };
        let mut engine_params = json!({});
        if command == "view.zoomAt" {
            let (Some(ax), Some(ay)) = (num(&params, "atX"), num(&params, "atY")) else {
                return Err("invalid-params: view.zoomAt needs `atX` and `atY` (document pixels)".into());
            };
            let at = area.to_canvas(ax, ay);
            engine_params = json!({ "x": at.x, "y": at.y, "out": params.get("out").and_then(Value::as_bool).unwrap_or(false) });
            if let (Some(w), Some(h)) = (num(&params, "atWidth"), num(&params, "atHeight")) {
                engine_params["width"] = json!(w);
                engine_params["height"] = json!(h);
            }
        }
        let mut scratch = Session::empty();
        scratch.prefs = self.session.prefs.clone();
        scratch.docs.push(ds);
        no_files(|| scratch.execute(command, &engine_params)).map_err(engine_error)?;
        let ds = scratch.ds().map_err(engine_error)?;
        let v = &ds.view;
        let rect = match command {
            "view.prevFrame" | "view.nextFrame" => ds.selection.first().and_then(|f| ds.doc.world_bounds(*f)),
            "view.zoomAt" => {
                match (num(&engine_params, "x"), num(&engine_params, "y"), num(&engine_params, "width"), num(&engine_params, "height")) {
                    (Some(x), Some(y), Some(w), Some(h)) if w > 0.0 && h > 0.0 => Some(Rect::new(x, y, x + w, y + h)),
                    _ => None,
                }
            }
            _ => rect,
        };
        let (cx, cy) = area.to_doc(kition_design_geom::Point::new(v.cx, v.cy));
        let zoom = v.zoom.max(ViewState::MIN_ZOOM);
        let selection: Option<&Vec<NodeId>> = (ds.selection != selection_before).then_some(&ds.selection);
        Ok(json!({
            "command": command,
            "zoom": zoom,
            "x": cx - v.vw / zoom / 2.0,
            "y": cy - v.vh / zoom / 2.0,
            "centerX": cx,
            "centerY": cy,
            "rect": rect.map(|r| area.rect_json(r)),
            "selection": selection,
        }))
    }
}
