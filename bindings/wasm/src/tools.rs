//! The engine's own tools behind [`KitTools`]: the upstream toolbar's tools (their ids, names and
//! keys), pointer events fed to `Session::pointer` as the upstream canvas did (`canvas/input.rs`),
//! keys handled as its keyboard handler did (`shortcuts.rs`: vector edit mode, the shape builder
//! and ruler guides first, then text editing, the pen, nudging, Escape, Enter and Tab), and the
//! answer: UI requests, overlays ([`crate::overlays`]) and the cursor.

use std::sync::Arc;

use kition_design_engine::tools::{Cursor, PenCursor, PointerEvent, PointerKind, ToolId};
use kition_kit::tools::{KitTools, PointerAction, ToolInfo, ToolMods, ToolPointer, ToolRequest, ToolResponse};
use serde_json::json;

use crate::{DesignDocument, engine_error, no_files};

/// The upstream toolbar: groups of tools sharing a slot (`toolbar.rs` `GROUPS`).
pub const TOOL_GROUPS: &[&[ToolId]] = &[
    &[ToolId::Move, ToolId::Hand, ToolId::Scale],
    &[ToolId::Frame, ToolId::Section, ToolId::Slice],
    &[ToolId::Rectangle, ToolId::Line, ToolId::Arrow, ToolId::Ellipse, ToolId::Polygon, ToolId::Star],
    &[ToolId::Pen, ToolId::Pencil, ToolId::ShapeBuilder],
    &[ToolId::Text, ToolId::TextOnPath],
    &[ToolId::Comment],
];

/// The engine's cursor as a CSS cursor: a system cursor's name, or for the glyphs the upstream
/// editor draws itself (`canvas/cursors.rs`: rotation, the pen) the same drawing as an SVG
/// cursor with its hot spot, falling back to the nearest system cursor.
fn cursor_css(cursor: Cursor) -> String {
    match cursor {
        Cursor::Default => "default".into(),
        Cursor::Crosshair | Cursor::Eyedropper | Cursor::Comment => "crosshair".into(),
        Cursor::Pen(state) => drawn(&pen_glyph(state), "crosshair"),
        Cursor::Move => "move".into(),
        Cursor::Resize(angle) if angle.is_finite() => {
            let a = angle.rem_euclid(180.0);
            if !(22.5..157.5).contains(&a) {
                "ew-resize"
            } else if a < 67.5 {
                "nwse-resize"
            } else if a < 112.5 {
                "ns-resize"
            } else {
                "nesw-resize"
            }
            .into()
        }
        Cursor::Resize(_) => "default".into(),
        Cursor::Rotate(angle) => drawn(&rotate_glyph(if angle.is_finite() { angle.rem_euclid(360.0).round() } else { 0.0 }), "grab"),
        Cursor::Grab => "grab".into(),
        Cursor::Grabbing => "grabbing".into(),
        Cursor::Text => "text".into(),
        Cursor::Copy => "copy".into(),
        Cursor::Pointer => "pointer".into(),
        Cursor::NotAllowed => "not-allowed".into(),
    }
}

/// The drawn cursors' box: 32 px with the hot spot in the middle.
const GLYPH_BOX: f64 = 32.0;
const HOT: f64 = 16.0;

/// Polylines (around the hot spot at the origin) as an SVG cursor: dark lines over a light edge,
/// readable on any page (upstream `cursors::line`), with `fallback` where SVG cursors are refused.
fn drawn(lines: &[Vec<(f64, f64)>], fallback: &str) -> String {
    let points = |line: &Vec<(f64, f64)>| line.iter().map(|(x, y)| format!("{:.2},{:.2}", x + HOT, y + HOT)).collect::<Vec<_>>().join(" ");
    let mut body = String::new();
    for (width, colour) in [(3.5, "%23FFFFFF"), (1.5, "%231E1E1E")] {
        for line in lines {
            body.push_str(&format!(
                "%3Cpolyline points='{}' fill='none' stroke='{colour}' stroke-width='{width}' stroke-linecap='round' stroke-linejoin='round'/%3E",
                points(line)
            ));
        }
    }
    format!(
        "url(\"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='{b}' height='{b}' viewBox='0 0 {b} {b}'%3E{body}%3C/svg%3E\") {h} {h}, {fallback}",
        b = GLYPH_BOX,
        h = HOT,
    )
}

/// Points of the arc of radius `r` about `c` from angle `a0` to `a1` (radians).
fn arc(c: (f64, f64), r: f64, a0: f64, a1: f64) -> Vec<(f64, f64)> {
    (0..=12).map(|i| a0 + (a1 - a0) * f64::from(i) / 12.0).map(|a| (c.0 + a.cos() * r, c.1 + a.sin() * r)).collect()
}

/// The rotation cursor: an arc bulging away from the box, through the pointer, with an arrowhead
/// at each end; `degrees` is the direction from the box's centre to the corner (0 right, 90 down).
fn rotate_glyph(degrees: f64) -> Vec<Vec<(f64, f64)>> {
    let (dir, r, half) = (degrees.to_radians(), 11.0, 0.8);
    let c = (-dir.cos() * r, -dir.sin() * r);
    let mut lines = vec![arc(c, r, dir - half, dir + half)];
    for (end, sign) in [(dir - half, -1.0), (dir + half, 1.0)] {
        let tip = (c.0 + end.cos() * r, c.1 + end.sin() * r);
        let t = (-end.sin() * sign, end.cos() * sign);
        let n = (end.cos(), end.sin());
        lines.push(vec![
            (tip.0 - t.0 * 4.5 + n.0 * 3.5, tip.1 - t.1 * 4.5 + n.1 * 3.5),
            (tip.0 + t.0, tip.1 + t.1),
            (tip.0 - t.0 * 4.5 - n.0 * 3.5, tip.1 - t.1 * 4.5 - n.1 * 3.5),
        ]);
    }
    lines
}

/// The pen cursor: a cross open at the centre, and what a click does at its lower right.
fn pen_glyph(state: PenCursor) -> Vec<Vec<(f64, f64)>> {
    let mut lines = vec![vec![(-8.0, 0.0), (-2.5, 0.0)], vec![(2.5, 0.0), (8.0, 0.0)], vec![(0.0, -8.0), (0.0, -2.5)], vec![(0.0, 2.5), (0.0, 8.0)]];
    let b = (10.0, 10.0);
    match state {
        PenCursor::New => {}
        PenCursor::Close => {
            let mut ring = arc(b, 3.0, 0.0, std::f64::consts::TAU);
            ring.push((b.0 + 3.0, b.1));
            lines.push(ring);
        }
        PenCursor::Add => {
            lines.push(vec![(b.0 - 3.5, b.1), (b.0 + 3.5, b.1)]);
            lines.push(vec![(b.0, b.1 - 3.5), (b.0, b.1 + 3.5)]);
        }
        PenCursor::Bend => lines.push(arc((b.0 - 3.5, b.1 + 3.5), 7.0, -std::f64::consts::FRAC_PI_2, 0.0)),
    }
    lines
}

/// A key name from the host (`event.key`) as the engine names it.
fn key_name(key: &str) -> &str {
    match key {
        "Up" => "ArrowUp",
        "Down" => "ArrowDown",
        "Left" => "ArrowLeft",
        "Right" => "ArrowRight",
        "Esc" => "Escape",
        "Return" => "Enter",
        other => other,
    }
}

impl DesignDocument {
    /// Remembers the view's zoom where the engine's tools read it (hit tolerances, handle sizes).
    fn set_zoom(&mut self, zoom: f64) {
        let zoom = if zoom.is_finite() && zoom > 0.0 { zoom } else { 1.0 };
        self.zoom = zoom;
        if let Ok(ds) = self.session.ds_mut() {
            ds.view.zoom = zoom;
        }
    }

    /// Whether the tool must see editing keys (Enter, Escape, Delete, arrows, Tab): during a
    /// gesture, a pen path, text or vector editing, while something is selected (nudge, delete,
    /// Escape, Enter), and while a tool other than Move is chosen (Escape returns to Move, Enter
    /// places a shape).
    fn tool_busy(&self) -> bool {
        let tool = &self.session.tool;
        tool.busy()
            || tool.pen.is_some()
            || tool.tool != ToolId::Move
            || self.session.ds().is_ok_and(|ds| ds.text_edit.is_some() || ds.vector_edit.is_some() || !ds.selection.is_empty())
    }

    fn wants_text(&self) -> bool {
        self.session.ds().is_ok_and(|ds| ds.text_edit.is_some())
    }

    /// The answer after an event: what the host must do, the overlays, the cursor.
    pub(crate) fn respond(&mut self, tool_before: ToolId, zoom: f64) -> ToolResponse {
        // What the event changed goes to the document's replica now (engine contract 2).
        self.session.sync_kdm();
        if !self.session.tool.busy() {
            self.held = None;
        }
        let mut requests: Vec<ToolRequest> =
            std::mem::take(&mut self.session.ui_requests).into_iter().map(|r| ToolRequest::Dialog { name: r.kind, params: r.data }).collect();
        // A tool the editor does not know yet: changed by this event, or by a command since.
        if self.session.tool.tool != tool_before || self.session.tool.tool != self.told_tool {
            requests.push(ToolRequest::SwitchTool { id: self.session.tool.tool.id().to_owned() });
        }
        self.told_tool = self.session.tool.tool;
        if let Some(text) = self.session.message.take() {
            requests.push(ToolRequest::Status { text });
        }
        self.live_requests(&mut requests, zoom);
        let area = self.area();
        let overlays = crate::overlays::overlays(&self.session, &area, zoom);
        ToolResponse {
            requests,
            overlays,
            cursor: cursor_css(self.session.tool.overlay.cursor),
            busy: self.tool_busy(),
            wants_text: self.wants_text(),
        }
    }

    /// What the editor would otherwise ask for after every answer, in the answer, and only when it
    /// changed (R2): `canvas.hover` (`{ id }`) when the hovered layer is another; during a pointer
    /// gesture, once per revision, `canvas.chrome` (the [`DesignDocument::chrome`] at `zoom`: the
    /// frame labels and the size pill follow the drag) and `canvas.gesture` (`{ nodes: [{ id, x,
    /// y, width, height, rotation }] }`, the selected layers' position and size as the Design panel
    /// shows them, at most 200), while the editor holds its panels' queries until the gesture
    /// ends. A changed hover also brings the chrome (the hovered frame's label changes colour).
    fn live_requests(&mut self, requests: &mut Vec<ToolRequest>, zoom: f64) {
        let Ok(ds) = self.session.ds() else { return };
        let (revision, hover) = (ds.revision, ds.hover);
        let gesture = self.session.tool.busy();
        let geometry = (gesture && self.told_gesture != Some(revision)).then(|| {
            let d = &ds.doc;
            let nodes: Vec<serde_json::Value> = ds
                .selection
                .iter()
                .take(200)
                .filter_map(|id| {
                    let n = d.node(*id)?;
                    let c = d.frame_relative(*id).as_coeffs();
                    let round = |v: f64| kition_design_geom::round_to(v, 2);
                    Some(json!({
                        "id": id,
                        "x": round(c[4]),
                        "y": round(c[5]),
                        "width": round(n.width),
                        "height": round(n.height),
                        "rotation": round(kition_design_geom::rotation_degrees(n.transform)),
                    }))
                })
                .collect();
            json!({ "nodes": nodes })
        });
        self.told_gesture = gesture.then_some(revision);
        let hovered = hover != self.told_hover;
        if hovered {
            self.told_hover = hover;
            requests.push(ToolRequest::Dialog { name: "canvas.hover".into(), params: json!({ "id": hover }) });
        }
        if let Some(params) = geometry.as_ref() {
            requests.push(ToolRequest::Dialog { name: "canvas.gesture".into(), params: params.clone() });
        }
        if hovered || geometry.is_some() {
            let chrome = self.chrome(zoom);
            requests.push(ToolRequest::Dialog { name: "canvas.chrome".into(), params: chrome });
        }
    }

    /// Runs an engine command for a key, keeping its error as the answer's status.
    fn key_command(&mut self, id: &str, params: serde_json::Value) {
        let session = &mut self.session;
        if let Err(error) = no_files(|| session.execute(id, &params)) {
            self.session.message = Some(error.to_string());
        }
    }

    /// Enter on a selection (upstream `run_binding` `select.children` and `enter_text`): with a
    /// shape or frame tool, place one; on one text, edit it; on a shape, edit its points; on
    /// several shapes, edit them together; else select the children.
    fn enter(&mut self) -> Result<(), String> {
        if self.session.tool.tool.draws_shape() {
            self.key_command("tool.place", json!({}));
            return Ok(());
        }
        let sel = self.session.selection();
        if sel.len() > 1 {
            let targets = self.session.doc().ok().and_then(|d| kition_design_engine::vectoredit::enter_targets(d, &sel));
            if let Some(ids) = targets {
                self.key_command("vector.edit.begin", json!({ "ids": ids }));
                return Ok(());
            }
        }
        if let [id] = sel.as_slice() {
            let id = *id;
            match kition_design_engine::board::edit_text(&mut self.session, id) {
                Ok(true) => return Ok(()),
                Ok(false) => {}
                Err(error) => return Err(engine_error(error)),
            }
            let kind =
                self.session.doc().ok().and_then(|d| d.node(id)).map(|n| (matches!(n.kind, kition_design_doc::NodeKind::Text(_)), n.kind.is_shape()));
            match kind {
                Some((true, _)) => return kition_design_engine::textedit::begin(&mut self.session, id, None, false).map_err(engine_error),
                Some((_, true)) => {
                    self.key_command("vector.edit.begin", json!({ "id": id }));
                    return Ok(());
                }
                _ => {}
            }
        }
        self.key_command("select.children", json!({}));
        Ok(())
    }

    /// A key while editing text: the engine's text keys (macOS motions: Alt by word, Cmd to the
    /// line's ends).
    fn text_key(&mut self, name: &str, mods: ToolMods) -> Result<(), String> {
        let name = match name {
            "Backspace" | "Delete" | "Enter" | "Escape" | "Tab" | "Home" | "End" => name,
            "ArrowLeft" => "Left",
            "ArrowRight" => "Right",
            "ArrowUp" => "Up",
            "ArrowDown" => "Down",
            "a" | "A" if mods.command => "SelectAll",
            _ => return Ok(()),
        };
        let m = kition_design_engine::keymap::Mods { shift: mods.shift, alt: mods.alt, cmd: mods.command, ctrl: mods.control };
        let (word, line) = kition_design_engine::keymap::text_motion(kition_design_engine::keymap::Platform::Mac, name, m);
        if name == "Enter" && mods.command {
            return kition_design_engine::textedit::end(&mut self.session).map_err(engine_error);
        }
        kition_design_engine::textedit::key(&mut self.session, name, mods.shift, word, line).map_err(engine_error)
    }

    /// A key on the canvas outside text editing (upstream `key_event`).
    fn canvas_key(&mut self, name: &str, mods: ToolMods) -> Result<(), String> {
        // Vector edit mode, the shape builder and a selected ruler guide have their own keys.
        let session = &mut self.session;
        if no_files(|| session.tool_key(name, mods.shift, mods.alt, mods.command)).map_err(engine_error)? {
            return Ok(());
        }
        let (dx, dy) = match name {
            "ArrowLeft" => (-1.0, 0.0),
            "ArrowRight" => (1.0, 0.0),
            "ArrowUp" => (0.0, -1.0),
            "ArrowDown" => (0.0, 1.0),
            _ => (0.0, 0.0),
        };
        if (dx, dy) != (0.0, 0.0) {
            if self.session.selection().is_empty() || self.session.tool.dev_mode {
                return Ok(());
            }
            if mods.command {
                self.key_command("transform.nudgeSize", json!({ "dw": dx, "dh": dy, "big": mods.shift }));
            } else {
                self.key_command("transform.nudge", json!({ "dx": dx, "dy": dy, "big": mods.shift }));
            }
            return Ok(());
        }
        match (name, mods.shift) {
            ("Escape", _) => self.session.escape().map_err(engine_error),
            ("Enter", _) if self.session.tool.pen.is_some() => self.session.pen_end().map_err(engine_error),
            ("Enter", true) => {
                self.key_command("select.parent", json!({}));
                Ok(())
            }
            ("Enter", false) => self.enter(),
            ("Backspace" | "Delete", _) if self.session.tool.pen.is_some() => self.session.pen_finish().map_err(engine_error),
            ("Backspace" | "Delete", _) => {
                if !self.session.selection().is_empty() {
                    self.key_command("edit.delete", json!({}));
                }
                Ok(())
            }
            ("Tab", shift) => {
                self.key_command(if shift { "select.previous" } else { "select.next" }, json!({}));
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

impl KitTools for DesignDocument {
    fn tools(&self) -> Vec<ToolInfo> {
        TOOL_GROUPS
            .iter()
            .enumerate()
            .flat_map(|(group, tools)| {
                tools.iter().map(move |tool| ToolInfo {
                    id: tool.id().to_owned(),
                    label: tool.label().to_owned(),
                    shortcut: Some(tool.shortcut()).filter(|key| !key.is_empty()).and_then(crate::commands::kit_shortcut),
                    icon: tool.id().to_owned(),
                    group,
                })
            })
            .collect()
    }

    fn tool(&self) -> String {
        self.session.tool.tool.id().to_owned()
    }

    fn select_tool(&mut self, id: &str, zoom: f64) -> Result<ToolResponse, String> {
        let Some(tool) = ToolId::from_id(id).filter(|t| !t.on_boards() || id == "section") else { return Err(format!("no tool {id}")) };
        self.set_zoom(zoom);
        self.end_open_interaction();
        let before = self.session.tool.tool;
        // Choosing the tool already chosen is no change (the editor follows the engine's switches).
        if before != tool {
            self.session.set_tool(tool);
        }
        // The editor chose it: a switch to tell only if the engine took another.
        self.told_tool = tool;
        Ok(self.respond(tool, zoom))
    }

    fn pointer(&mut self, event: &ToolPointer) -> Result<ToolResponse, String> {
        if !(event.x.is_finite() && event.y.is_finite()) {
            return Err("the pointer is not at a finite position".into());
        }
        self.set_zoom(event.zoom);
        let zoom = self.zoom;
        let area = self.area();
        let at = area.to_canvas(event.x, event.y);
        let kind = match event.action {
            PointerAction::Down => PointerKind::Down,
            PointerAction::Drag => PointerKind::Move,
            PointerAction::Up => PointerKind::Up,
            PointerAction::Move => PointerKind::Hover,
            PointerAction::DoubleClick => PointerKind::DoubleClick,
        };
        if kind == PointerKind::Down {
            self.end_open_interaction();
            self.held = Some(area);
        }
        if !self.viewport
            && let Ok(ds) = self.session.ds_mut()
        {
            // Until the editor tells the engine its view (`setViewport`), Enter with a shape tool
            // places one where the pointer was last seen.
            (ds.view.cx, ds.view.cy) = (at.x, at.y);
        }
        let ev = PointerEvent {
            kind,
            x: at.x,
            y: at.y,
            sx: at.x * zoom,
            sy: at.y * zoom,
            shift: event.mods.shift,
            alt: event.mods.alt,
            cmd: event.mods.command,
            ctrl: event.mods.control,
            zoom,
            pressure: (event.pressure.is_finite() && event.pressure > 0.0 && event.pressure < 1.0).then_some(event.pressure),
        };
        let before = self.session.tool.tool;
        // Hovering changes only what the overlays show (the hover outline, guides): no new revision,
        // so the host does not render the page again for it.
        let shown = self.session.ds().ok().map(|ds| (Arc::as_ptr(&ds.doc), ds.selection.clone(), ds.page, ds.revision));
        let session = &mut self.session;
        let result = no_files(|| session.pointer(&ev));
        if kind == PointerKind::Hover
            && let Some((doc, selection, page, revision)) = shown
            && let Ok(ds) = self.session.ds_mut()
            && std::ptr::eq(Arc::as_ptr(&ds.doc), doc)
            && ds.selection == selection
            && ds.page == page
        {
            ds.revision = revision;
        }
        if let Err(error) = result {
            // A failed gesture is dropped, as upstream's toast does; the document is as it was.
            self.session.message = Some(error.to_string());
        }
        Ok(self.respond(before, zoom))
    }

    fn key(&mut self, key: &str, mods: ToolMods, zoom: f64) -> Result<ToolResponse, String> {
        self.set_zoom(zoom);
        self.end_open_interaction();
        let before = self.session.tool.tool;
        let name = key_name(key);
        let result = if self.wants_text() { self.text_key(name, mods) } else { self.canvas_key(name, mods) };
        if let Err(error) = result {
            self.session.message = Some(error);
        }
        Ok(self.respond(before, zoom))
    }

    fn text(&mut self, text: &str, zoom: f64) -> Result<ToolResponse, String> {
        self.set_zoom(zoom);
        let before = self.session.tool.tool;
        if self.wants_text() {
            kition_design_engine::textedit::insert(&mut self.session, text).map_err(engine_error)?;
        }
        Ok(self.respond(before, zoom))
    }

    fn state(&mut self, x: f64, y: f64, _mods: ToolMods, zoom: f64) -> ToolResponse {
        self.set_zoom(zoom);
        let _ = (x, y);
        let tool = self.session.tool.tool;
        self.respond(tool, zoom)
    }
}

#[cfg(test)]
mod cursor_tests {
    use super::*;

    #[test]
    fn drawn_cursors_are_svg_with_a_system_fallback() {
        let rotate = cursor_css(Cursor::Rotate(45.0));
        assert!(rotate.starts_with("url(\"data:image/svg+xml,") && rotate.ends_with("\") 16 16, grab"), "{rotate}");
        assert!(!rotate.contains('<') && !rotate.contains('#'), "escaped for a CSS url: {rotate}");
        assert!(cursor_css(Cursor::Pen(PenCursor::Add)).ends_with(", crosshair"));
        assert_eq!(cursor_css(Cursor::Resize(90.0)), "ns-resize");
        assert_eq!(cursor_css(Cursor::Rotate(f64::NAN)), cursor_css(Cursor::Rotate(0.0)));
    }
}
