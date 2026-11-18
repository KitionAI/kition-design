//! The prototype player (`player(command, params)` and `playerRender(params)`), ported from
//! upstream's presenter (`present.rs` and `present/{actions,navigate,input,layout,scroll,
//! transition}.rs` in the upstream UI) and run in the engine, so the editor's preview pane and
//! Present view draw only the chrome.
//!
//! The player works on its own copy of the document: variables set, variants changed and scroll
//! offsets change the copy, never the file or its revision. When the document changes (an edit in
//! the editor) the next call copies it again, keeping the screen, the overlays and the scroll
//! offsets that still exist, as upstream's preview pane follows edits live. Responsive prototypes
//! resize the presented frame to the device's screen (or, without a device, to the `window` the
//! editor gives) and lay it out again.
//!
//! **Screen units.** Everything the player answers is in screen units: one unit is one canvas
//! unit, with (0, 0) at the screen's top-left. The screen is the device's screen (its size in
//! points, turned for landscape), else the presented frame itself; a frame smaller than a device
//! sits centred on it. Pointer events give positions in the same units; `playerRender` draws the
//! screen at a `scale` (pixels per unit).
//!
//! **Time.** Timers (After delay, delayed mouse triggers) run on the editor's clock: every call can
//! pass `time` (seconds, any origin, never going back), and `tick` fires what is due. `nextDue`
//! in the state says when to tick next.
//!
//! **Transitions.** The player changes state at once and describes how the change animates: every
//! change of screen, overlay or variant answers a `transition` (see [`Player::transition_json`]).
//! `playerRender({ progress })` draws the screen at that point of the last transition (linear time
//! 0..1; the player applies the easing), including smart animate, which tweens matching layers.
//!
//! ## `player(command, params)`
//!
//! | command | params | does |
//! | --- | --- | --- |
//! | `start` | `{ start? (frame or flow start id; default as `proto.present`: the selected frame's top-level frame, the page's first flow, its first frame), window?: { width, height }, time? }` | starts playing (again) |
//! | `stop` | `{}` | stops: `{ stopped: bool }` |
//! | `state` | `{ time? }` | the state, without firing timers |
//! | `tick` | `{ time? }` | fires the timers due |
//! | `event` | `{ type, …, time? }` (below) | one input event |
//! | `restart` | `{ start?, time? }` | back to the start (or another flow's start) with the document as it is |
//! | `back` | `{ time? }` | closes the top overlay, else goes back one screen |
//! | `closeOverlay` | `{ time? }` | closes the top overlay |
//! | `scrollTo` | `{ x, y }` | scrolls the presented frame (clamped) |
//! | `resize` | `{ width, height }` | the editor's area changed (responsive prototypes without a device take its size) |
//! | `inspect` | `{ x?, y?, id?, colorFormat? }` | Present's Inspect (below); changes nothing |
//!
//! Events (`event`): `pointerMove { x, y }`, `pointerDown { x, y }`, `pointerUp { x, y }`,
//! `pointerLeave {}`, `pointerCancel {}` (the browser took the press away: the press and any drag
//! end without a click, mouse up or missed-click hint, and a While pressing state reverts),
//! `click { x, y }` (down and up), `wheel { x, y, dx, dy }` (dx, dy in screen
//! units, positive scrolls the content down or right, as DOM wheel deltas), `key { key (DOM
//! `KeyboardEvent.key`, or a controller button such as "Gamepad A"), shift?, ctrl?, alt?, meta? }`.
//! A drag is `pointerDown`, `pointerMove`s past 6 units (an On drag trigger fires once, its way;
//! otherwise the drag scrolls the frame under it), `pointerUp`.
//!
//! Every answer except `stop`:
//!
//! ```text
//! { handled: bool,            // the prototype used the input (keys it used should not reach the editor)
//!   changed: bool,            // what shows changed: draw again
//!   missed: bool,             // a click where nothing reacts (flash the hotspots)
//!   cursor: "default" | "pointer" | "grabbing",
//!   links: [url],             // Open link actions run (web links only); the editor asks before opening
//!   transition: null | Transition,   // the change this call made, if any
//!   state: State }
//! ```
//!
//! `State` (see [`Player::state_json`]):
//!
//! ```text
//! { start, current, name, flow: null | { start, name, description }, flows: [{ start, name, description }],
//!   screen: { width, height }, frame: { x, y, width, height },
//!   device: null | { name, width, height, landscape, rotates, color: null | { name, rgb: "#rrggbb" } },
//!   background: "#rrggbb", responsive: bool,
//!   history: [frame id], canBack: bool,
//!   overlays: [{ id, name, x, y, width, height, background: null | "#rrggbbaa", closeOnOutside, position }],
//!   scroll: [x, y], scrollRange: [x, y],
//!   hotspots: [{ id, name, x, y, width, height, triggers: [trigger type] }],   // the active screen's, bottom-most first
//!   hovered: [id], pressed: bool, waiting: n, nextDue: null | seconds, clock, revision }
//! ```
//!
//! `Transition`:
//!
//! ```text
//! { seq,                      // counts up per transition
//!   kind: "navigate" | "back" | "overlayOpen" | "overlaySwap" | "overlayClose" | "changeTo" | "revert" | "scroll" | "restart",
//!   from, to,                 // the presented frame before and after
//!   overlay: null | id,       // the overlay opened, swapped in or closed
//!   transition: { type: instant|dissolve|smartAnimate|moveIn|moveOut|push|slideIn|slideOut, direction? },
//!   smartAnimate: bool, duration: seconds, durationMs, easing,      // easing as the file stores it
//!   follow: null | { x, y, length } }   // On drag: the unit vector the pointer moves along and the distance that completes it
//! ```
//!
//! ## `playerRender(params)`
//!
//! `{ scale?: 1 (0.01..8), format?: "png" | "rgba", progress?: 0..1, state?: "current" | "from" }`:
//! the screen as PNG bytes, or `rgba`: the picture's width and height (two u32, little-endian)
//! followed by its straight-alpha sRGB RGBA pixels, about `width × scale` × `height × scale` of
//! `state.screen` (read the size, never compute it). `progress` draws the last transition at that
//! point; `state: "from"` draws what showed before it. Pictures are capped at [`paint::MAX_SIDE`]
//! pixels a side and [`paint::MAX_PIXELS`] pixels, and a picture inside a transition is drawn at
//! a lower scale where it would pass [`paint::MAX_TRANSITION_PIXELS`].
//!
//! `inspect` (upstream's presenter `inspect_hover` and `inspect_panel`) answers the top-most
//! visible layer of the active screen (the top overlay, else the presented frame) under the screen
//! point `x`, `y` as the player shows it (scrolled, laid out for the device), else layer `id` when it
//! is still on the active screen, with its properties read from the player's copy (what the
//! prototype changed included), as the Inspect tab lists them:
//!
//! ```text
//! { id: null } or { id, name, bounds: Box, clip: Box,   // screen units; clip: the active screen's area
//!   sections: [{ title, rows: [{ label, value, color?, token? }] }] }
//! ```
//!
//! Not ported: upstream's presenter chrome (bar, options menu, device bodies, zoom and panning of a
//! prototype bigger than the window: the editor draws and scales the screen), scroll momentum and
//! rubber banding (the editor can feed `wheel` events as it coasts), widgets' plug-in callbacks
//! (plug-ins do not run in the kit), comments.

mod geometry;
mod paint;
mod smart;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use kition_design_color::Color;
use kition_design_doc::proto::{Action, Easing, KeyMods, Overflow, OverlayAnchor, OverlayPosition, Transition, Trigger};
use kition_design_doc::{Document, NodeId, NodeKind};
use kition_design_engine::proto_run;
use kition_design_geom::{Point, Rect};
use serde_json::{Value, json};

use crate::DesignDocument;
use geometry::{anchored_overlay_rect, apply_scroll, clamp_scroll, fit_viewport, page_settings, raise_fixed, scroll_range_in, viewport};

/// How far the pointer moves before a press is a drag (screen units).
const DRAG_START: f64 = 6.0;
/// The most hotspots the state lists.
const MAX_HOTSPOTS: usize = 2000;
/// The most timers one `tick` fires (each may arm the next).
const MAX_FIRES: usize = 32;
/// The most screens the history keeps.
const MAX_HISTORY: usize = 500;
/// The most overlays open at once.
const MAX_OVERLAYS: usize = 64;

/// A frame scrolling by Scroll to: its offsets before and after.
type ScrollMove = (NodeId, (f64, f64), (f64, f64));

/// Page → screen mapping for one placed frame (scale 1).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Place {
    page_min: (f64, f64),
    origin: (f64, f64),
}

impl Place {
    fn rect(&self, r: Rect) -> Rect {
        let (dx, dy) = (self.origin.0 - self.page_min.0, self.origin.1 - self.page_min.1);
        Rect::new(r.x0 + dx, r.y0 + dy, r.x1 + dx, r.y1 + dy)
    }
    fn page(&self, q: Point) -> Point {
        Point::new(self.page_min.0 + q.x - self.origin.0, self.page_min.1 + q.y - self.origin.1)
    }
}

/// An overlay placed on the screen.
#[derive(Clone, Debug)]
pub(crate) struct PlacedOverlay {
    id: NodeId,
    place: Place,
    rect: Rect,
    background: Option<Color>,
}

/// The screen as it shows: the display document (scrolled, sized to the device), its size, where
/// the presented frame and the overlays sit.
pub(crate) struct Screen {
    disp: Document,
    current: NodeId,
    size: (f64, f64),
    frame: Place,
    frame_size: (f64, f64),
    background: Color,
    overlays: Vec<PlacedOverlay>,
}

impl Screen {
    /// The active layer (the top overlay, else the presented frame) and where it sits.
    fn active(&self) -> (NodeId, Place) {
        self.overlays.last().map_or((self.current, self.frame), |o| (o.id, o.place))
    }

    fn bounds(&self) -> Rect {
        Rect::new(0.0, 0.0, self.size.0, self.size.1)
    }

    /// The active screen's hotspots (bottom-most first), in screen units.
    fn hotspots(&self) -> Vec<(NodeId, Rect)> {
        let (root, place) = self.active();
        let base = self.bounds();
        proto_run::hotspots(&self.disp, root)
            .into_iter()
            .map(|(id, b)| (id, place.rect(b).intersect(base)))
            .filter(|(_, r)| r.width() > 0.0 && r.height() > 0.0)
            .take(MAX_HOTSPOTS)
            .collect()
    }
}

/// Lays out `doc` (the player's document, unscrolled) for the screen.
fn screen_of(doc: &Document, scroll: &HashMap<NodeId, (f64, f64)>, current: NodeId, overlays: &[OverlayState]) -> Option<Screen> {
    let mut disp = apply_scroll(doc, scroll);
    if let Some(vp) = viewport(doc, current) {
        fit_viewport(&mut disp, current, vp);
    }
    let fb = disp.world_bounds(current)?;
    let settings = page_settings(&disp, current);
    let size = settings.device().map_or((fb.width(), fb.height()), |d| (d.width, d.height));
    if !(size.0.is_finite() && size.1.is_finite() && size.0 > 0.0 && size.1 > 0.0) {
        return None;
    }
    let frame = Place { page_min: (fb.x0, fb.y0), origin: ((size.0 - fb.width()) / 2.0, (size.1 - fb.height()) / 2.0) };
    let base = Rect::new(0.0, 0.0, size.0, size.1);
    let placed = overlays
        .iter()
        .filter_map(|o| {
            let ob = disp.world_bounds(o.id)?;
            let r = anchored_overlay_rect(o.position, o.anchor_to, base, (ob.width(), ob.height()), o.anchor.map(|a| frame.rect(a)), o.offset);
            Some(PlacedOverlay { id: o.id, place: Place { page_min: (ob.x0, ob.y0), origin: (r.x0, r.y0) }, rect: r, background: o.background })
        })
        .collect();
    Some(Screen { current, size, frame, frame_size: (fb.width(), fb.height()), background: settings.background, overlays: placed, disp })
}

/// A screen in the history (what Back returns to, and how).
#[derive(Clone, Debug)]
struct Visit {
    frame: NodeId,
    transition: Transition,
    dur: f64,
    easing: Easing,
}

/// An open overlay.
#[derive(Clone, Debug)]
pub(crate) struct OverlayState {
    id: NodeId,
    position: OverlayPosition,
    close_on_outside: bool,
    background: Option<Color>,
    /// The layer that opened it, in canvas units of the presented frame.
    anchor: Option<Rect>,
    offset: Option<[f64; 2]>,
    anchor_to: OverlayAnchor,
    /// When it opened (the player's clock): its After delay timers count from then.
    opened: f64,
    transition: Transition,
    dur: f64,
    easing: Easing,
}

/// A mouse trigger waiting out its delay.
#[derive(Clone, Copy, Debug)]
struct Waiting {
    node: NodeId,
    k: usize,
    rect: Option<Rect>,
    root: NodeId,
    due: f64,
}

#[derive(Clone)]
struct Snapshot {
    current: NodeId,
    design: Document,
    overlays: Vec<OverlayState>,
    history: Vec<Visit>,
    scroll: HashMap<NodeId, (f64, f64)>,
}

/// A "while hovering / while pressing" state that reverts when the pointer leaves or lifts.
struct Temp {
    rect: Rect,
    press: bool,
    snap: Snapshot,
    transition: Transition,
    dur: f64,
    easing: Easing,
}

/// What showed before a transition (the player's laid-out document, unscrolled).
#[derive(Clone)]
struct View {
    doc: Document,
    scroll: HashMap<NodeId, (f64, f64)>,
    current: NodeId,
    overlays: Vec<OverlayState>,
}

/// The last transition: what showed before it and how it animates.
struct Pending {
    seq: u64,
    kind: &'static str,
    from: View,
    to_frame: NodeId,
    overlay: Option<NodeId>,
    transition: Transition,
    dur: f64,
    easing: Easing,
    /// Scroll to: each frame's offsets before and after.
    scrolls: Vec<ScrollMove>,
    follow: Option<(f64, f64, f64)>,
}

/// The prototype player's state (one per document, while it plays).
pub struct Player {
    start: NodeId,
    current: NodeId,
    history: Vec<Visit>,
    overlays: Vec<OverlayState>,
    scroll: HashMap<NodeId, (f64, f64)>,
    source: Arc<Document>,
    original: Document,
    /// The document as designed, with what the prototype changed (variables, variants).
    design: Document,
    /// `design` laid out for the screen (responsive resize, interactive components, fixed layers on top).
    doc: Document,
    /// The editor's area, for responsive prototypes without a device.
    window: Option<(f64, f64)>,
    sections: HashMap<NodeId, NodeId>,
    clock: f64,
    arrived: f64,
    delays_fired: HashSet<(NodeId, usize)>,
    waiting: Vec<Waiting>,
    hovered: Vec<NodeId>,
    pointer: Option<Point>,
    press: Option<Point>,
    dragged: bool,
    drag_scroll: Option<(NodeId, (f64, f64))>,
    temp: Option<Temp>,
    pending: Option<Pending>,
    seq: u64,
    rev: u64,
    /// What the current call did (reset per call).
    links: Vec<String>,
    changed: bool,
    new_transition: bool,
}

fn finite(v: Option<f64>) -> Option<f64> {
    v.filter(|v| v.is_finite())
}

fn num(params: &Value, key: &str) -> Option<f64> {
    finite(params.get(key).and_then(Value::as_f64))
}

fn point(params: &Value) -> Result<Point, String> {
    match (num(params, "x"), num(params, "y")) {
        (Some(x), Some(y)) => Ok(Point::new(x, y)),
        _ => Err("invalid-params: `x` and `y` (screen units) are required".into()),
    }
}

fn size_param(v: Option<&Value>) -> Result<Option<(f64, f64)>, String> {
    match v {
        None | Some(Value::Null) => Ok(None),
        Some(v) => match (finite(v.get("width").and_then(Value::as_f64)), finite(v.get("height").and_then(Value::as_f64))) {
            (Some(w), Some(h)) if w >= 1.0 && h >= 1.0 => Ok(Some((w.min(100_000.0).round(), h.min(100_000.0).round()))),
            _ => Err("invalid-params: `window` is { width, height } (at least 1)".into()),
        },
    }
}

/// The first interaction on `id` whose trigger satisfies `want`.
fn find_trigger(doc: &Document, id: NodeId, want: impl Fn(&Trigger) -> bool) -> Option<usize> {
    doc.node(id)?.interactions.iter().position(|it| want(&it.trigger))
}

/// A DOM key name as key triggers name keys.
fn key_name(key: &str) -> String {
    match key {
        " " | "Spacebar" => "Space".into(),
        "Esc" => "Escape".into(),
        k if k.chars().count() == 1 => k.to_uppercase(),
        k => k.to_owned(),
    }
}

fn trigger_type(t: &Trigger) -> Value {
    serde_json::to_value(t).ok().and_then(|v| v.get("type").cloned()).unwrap_or(Value::Null)
}

impl Player {
    fn new(source: Arc<Document>, start: NodeId, window: Option<(f64, f64)>, now: f64) -> Self {
        let original = Document::clone(&source);
        let mut p = Self {
            start,
            current: start,
            history: Vec::new(),
            overlays: Vec::new(),
            scroll: HashMap::new(),
            design: original.clone(),
            doc: original.clone(),
            original,
            source,
            window,
            sections: HashMap::new(),
            clock: now,
            arrived: now,
            delays_fired: HashSet::new(),
            waiting: Vec::new(),
            hovered: Vec::new(),
            pointer: None,
            press: None,
            dragged: false,
            drag_scroll: None,
            temp: None,
            pending: None,
            seq: 0,
            rev: 0,
            links: Vec::new(),
            changed: false,
            new_transition: false,
        };
        p.relayout();
        p.arrive();
        p
    }

    /// The screen size a responsive prototype's frame takes (None: frames keep their size).
    fn fit(&self) -> Option<(f64, f64)> {
        let settings = page_settings(&self.design, self.current);
        if !settings.responsive {
            return None;
        }
        settings.device().map(|d| (d.width, d.height)).or(self.window)
    }

    /// Lays `design` out for the screen.
    fn relayout(&mut self) {
        let mut d = self.design.clone();
        if let Some(size) = self.fit() {
            kition_design_engine::cmd::proto::respond(&mut d, self.current, size);
        }
        proto_run::play_interactions(&mut d);
        raise_fixed(&mut d);
        self.doc = d;
        self.touch();
    }

    fn touch(&mut self) {
        self.rev += 1;
        self.changed = true;
    }

    /// After an action changed `design` (since `before`): derived layout settles, as after an edit.
    fn settle(&mut self, before: &Document) {
        kition_design_engine::normalize(&mut self.design, before);
        self.relayout();
    }

    /// Follows the editor's document (see the module documentation).
    fn sync(&mut self, doc: &Arc<Document>) {
        if Arc::ptr_eq(&self.source, doc) {
            return;
        }
        self.source = doc.clone();
        self.original = Document::clone(doc);
        self.design = self.original.clone();
        if !doc.contains(self.current) {
            // The start too gone: [`DesignDocument::player_follow`] starts again from the default.
            let start = if doc.contains(self.start) { self.start } else { self.current };
            self.restart(start, false);
            self.pending = None;
            self.touch();
            return;
        }
        self.relayout();
        self.history.retain(|v| doc.contains(v.frame));
        self.overlays.retain(|o| doc.contains(o.id));
        self.temp = None;
        self.pending = None;
        self.drag_scroll = None;
        self.scroll.retain(|f, _| doc.contains(*f));
        let clamped: Vec<(NodeId, (f64, f64))> = self.scroll.iter().map(|(f, s)| (*f, clamp_scroll(*s, self.range(&self.doc, *f)))).collect();
        self.scroll.extend(clamped);
        self.touch();
    }

    /// The frame playing is gone from the design (and so is the start).
    fn lost(&self) -> bool {
        !self.original.contains(self.current)
    }

    /// The start is gone from the design (Restart would go nowhere).
    fn lost_start(&self) -> bool {
        !self.original.contains(self.start)
    }

    fn screen(&self) -> Option<Screen> {
        screen_of(&self.doc, &self.scroll, self.current, &self.overlays)
    }

    /// How far `frame` scrolls: the presented frame within the device's screen, others within themselves.
    fn range(&self, doc: &Document, frame: NodeId) -> (f64, f64) {
        let vp = if frame == self.current { viewport(doc, frame) } else { None };
        scroll_range_in(doc, frame, vp)
    }

    fn view(&self) -> View {
        View { doc: self.doc.clone(), scroll: self.scroll.clone(), current: self.current, overlays: self.overlays.clone() }
    }

    /// Records a transition from the state as it is now; call before changing the state.
    fn begin(&mut self, kind: &'static str, transition: Transition, dur: f64, easing: Easing, overlay: Option<NodeId>) {
        self.seq += 1;
        self.pending = Some(Pending {
            seq: self.seq,
            kind,
            from: self.view(),
            to_frame: self.current,
            overlay,
            transition,
            dur: if dur.is_finite() { dur.max(0.0) } else { 0.0 },
            easing,
            scrolls: Vec::new(),
            follow: None,
        });
        self.new_transition = true;
    }

    /// The state changed after [`begin`](Self::begin): the transition ends on the current frame.
    fn end_transition(&mut self) {
        let current = self.current;
        if let Some(p) = self.pending.as_mut() {
            p.to_frame = current;
        }
    }

    fn arrive(&mut self) {
        proto_run::remember_sections(&self.doc, self.current, &mut self.sections);
        self.arrived = self.clock;
        self.delays_fired.clear();
        self.waiting.clear();
        self.hovered.clear();
        self.touch();
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            current: self.current,
            design: self.design.clone(),
            overlays: self.overlays.clone(),
            history: self.history.clone(),
            scroll: self.scroll.clone(),
        }
    }

    /// Shows `frame`; it starts at the top unless `preserve` keeps the scroll of the frame left.
    #[allow(clippy::too_many_arguments)]
    fn go(&mut self, kind: &'static str, frame: NodeId, transition: Transition, dur: f64, easing: Easing, remember: bool, preserve: bool) {
        self.begin(kind, transition, dur, easing, None);
        if remember {
            self.history.push(Visit { frame: self.current, transition, dur, easing });
            if self.history.len() > MAX_HISTORY {
                self.history.remove(0);
            }
        }
        let kept = self.scroll.get(&self.current).copied().filter(|_| preserve);
        self.overlays.clear();
        self.scroll.clear();
        self.drag_scroll = None;
        self.temp = None;
        self.current = frame;
        self.relayout();
        if let Some(s) = kept {
            let to = clamp_scroll(s, self.range(&self.doc, frame));
            self.scroll.insert(frame, to);
        }
        self.arrive();
        self.end_transition();
    }

    /// Starts over at `start` with the document as the editor has it.
    fn restart(&mut self, start: NodeId, mark: bool) {
        if mark {
            self.begin("restart", Transition::Instant, 0.0, Easing::Linear, None);
        }
        self.start = start;
        self.current = start;
        self.sections.clear();
        self.history.clear();
        self.overlays.clear();
        self.scroll.clear();
        self.drag_scroll = None;
        self.temp = None;
        self.design = self.original.clone();
        self.relayout();
        self.arrive();
        if mark {
            self.end_transition();
        }
    }

    /// Closes the top overlay; it animates out with its opening transition played backwards.
    fn close_overlay(&mut self) -> bool {
        let Some(o) = self.overlays.last().cloned() else { return false };
        self.begin("overlayClose", o.transition, o.dur, o.easing, Some(o.id));
        self.overlays.pop();
        self.hovered.clear();
        self.touch();
        true
    }

    /// Puts back the state a "while hovering / while pressing" interaction left.
    fn revert(&mut self) {
        let Some(t) = self.temp.take() else { return };
        self.begin("revert", t.transition.reversed(), t.dur, t.easing, None);
        self.current = t.snap.current;
        self.design = t.snap.design;
        self.relayout();
        self.overlays = t.snap.overlays;
        self.history = t.snap.history;
        self.scroll = t.snap.scroll;
        self.drag_scroll = None;
        self.hovered.clear();
        self.end_transition();
    }

    /// Screen rect `r` in canvas units of the presented frame.
    fn page_rect(&self, r: Option<Rect>) -> Option<Rect> {
        let r = r?;
        let s = self.screen()?;
        let (a, b) = (s.frame.page(Point::new(r.x0, r.y0)), s.frame.page(Point::new(r.x1, r.y1)));
        Some(Rect::new(a.x, a.y, b.x, b.y))
    }

    /// A mouse enter, leave, down or up met interaction `k` of `node`: its actions run now, or
    /// after its delay while `root` still shows.
    fn gesture(&mut self, node: NodeId, k: usize, rect: Option<Rect>, root: NodeId) -> bool {
        let delay = self.doc.node(node).and_then(|n| n.interactions.get(k)).map_or(0.0, |it| it.delay_secs());
        if delay <= 0.0 {
            return self.fire(node, k, rect);
        }
        if !self.waiting.iter().any(|w| (w.node, w.k) == (node, k)) {
            self.waiting.push(Waiting { node, k, rect, root, due: self.clock + delay });
        }
        false
    }

    /// Runs interaction `k` of `node` (`rect`: the layer's screen rect).
    fn fire(&mut self, node: NodeId, k: usize, rect: Option<Rect>) -> bool {
        let Some(it) = self.doc.node(node).and_then(|n| n.interactions.get(k)).cloned() else { return false };
        let dur = it.easing.seconds(it.duration_ms);
        let temp = match it.trigger {
            Trigger::OnHover => Some(false),
            Trigger::OnPress => Some(true),
            _ => None,
        };
        let snap = (temp.is_some() && self.temp.is_none()).then(|| self.snapshot());
        let mut did = false;
        for a in &it.actions {
            did |= self.perform(a, node, it.transition, dur, it.easing, rect, 0);
        }
        if did && let (Some(press), Some(snap), Some(rect)) = (temp, snap, rect) {
            self.temp = Some(Temp { rect, press, snap, transition: it.transition, dur, easing: it.easing });
        }
        did
    }

    /// One action; `depth` counts the conditionals it is inside.
    #[allow(clippy::too_many_arguments)]
    fn perform(&mut self, a: &Action, node: NodeId, tr: Transition, dur: f64, easing: Easing, rect: Option<Rect>, depth: usize) -> bool {
        match a {
            Action::Navigate { dest, preserve_scroll } => {
                let Some(top) = proto_run::navigate_target(&self.doc, *dest, &self.sections) else { return false };
                if !self.doc.contains(top) || (top == self.current && self.overlays.is_empty()) {
                    return false;
                }
                self.go("navigate", top, tr, dur, easing, true, *preserve_scroll);
                true
            }
            Action::Back => {
                if self.close_overlay() {
                    return true;
                }
                match self.history.pop() {
                    Some(v) => {
                        self.go("back", v.frame, v.transition.reversed(), v.dur, v.easing, false, false);
                        true
                    }
                    None => false,
                }
            }
            Action::ScrollTo { dest, offset } => {
                let moves = self.scroll_to_layer(*dest, *offset);
                if moves.is_empty() {
                    return false;
                }
                self.begin("scroll", tr, dur, easing, None);
                if let Some(p) = self.pending.as_mut() {
                    p.scrolls.clone_from(&moves);
                }
                self.scroll.extend(moves.iter().map(|(f, _, to)| (*f, *to)));
                self.touch();
                true
            }
            Action::OpenOverlay { dest, position, close_on_outside, background, offset, anchor } => {
                if !self.doc.contains(*dest) || self.overlays.len() >= MAX_OVERLAYS {
                    return false;
                }
                let at = self.page_rect(rect);
                let doc = &self.doc;
                self.delays_fired.retain(|(id, _)| !doc.is_ancestor(*dest, *id));
                self.begin("overlayOpen", tr, dur, easing, Some(*dest));
                self.overlays.push(OverlayState {
                    id: *dest,
                    position: *position,
                    close_on_outside: *close_on_outside,
                    background: *background,
                    anchor: at,
                    offset: *offset,
                    anchor_to: *anchor,
                    opened: self.clock,
                    transition: tr,
                    dur,
                    easing,
                });
                self.hovered.clear();
                self.touch();
                true
            }
            Action::SwapOverlay { dest } => {
                if !self.doc.contains(*dest) {
                    return false;
                }
                let at = self.page_rect(rect);
                let doc = &self.doc;
                self.delays_fired.retain(|(id, _)| !doc.is_ancestor(*dest, *id));
                let kind = if self.overlays.is_empty() { "overlayOpen" } else { "overlaySwap" };
                self.begin(kind, tr, dur, easing, Some(*dest));
                let clock = self.clock;
                match self.overlays.last_mut() {
                    Some(o) => {
                        o.id = *dest;
                        o.opened = clock;
                        o.transition = tr;
                        o.dur = dur;
                        o.easing = easing;
                    }
                    None => self.overlays.push(OverlayState {
                        id: *dest,
                        position: OverlayPosition::Center,
                        close_on_outside: true,
                        background: None,
                        anchor: at,
                        offset: None,
                        anchor_to: OverlayAnchor::Screen,
                        opened: clock,
                        transition: tr,
                        dur,
                        easing,
                    }),
                }
                self.hovered.clear();
                self.touch();
                true
            }
            Action::CloseOverlay => self.close_overlay(),
            Action::OpenUrl { url } => {
                if kition_design_engine::doc_urls::openable(url) && self.links.len() < 16 {
                    self.links.push(url.trim().to_owned());
                }
                true
            }
            Action::ChangeTo { dest } => {
                let Some(inst) = proto_run::change_to_target(&self.doc, node, *dest) else { return false };
                if !self.doc.contains(*dest) || !self.design.contains(inst) {
                    return false;
                }
                self.begin("changeTo", tr, dur, easing, None);
                let before = self.design.clone();
                if let Some(n) = self.design.node_mut(inst)
                    && let NodeKind::Instance { component, .. } = &mut n.kind
                {
                    *component = *dest;
                }
                kition_design_engine::components::sync_instance(&mut self.design, inst);
                self.settle(&before);
                self.end_transition();
                true
            }
            Action::SetVariable { variable, value, expr } => {
                let before = self.design.clone();
                let set = proto_run::set_variable(&mut self.design, node, variable, value, expr.as_ref());
                if set {
                    self.settle(&before);
                }
                set
            }
            Action::SetVariableMode { collection, mode } => {
                let before = self.design.clone();
                let set = proto_run::set_mode(&mut self.design, self.current, collection, mode);
                if set {
                    self.settle(&before);
                }
                set
            }
            Action::Conditional { blocks } => {
                if depth >= kition_design_doc::proto::MAX_ACTION_DEPTH {
                    return false;
                }
                let mut did = false;
                for inner in proto_run::branch(&self.doc, node, blocks) {
                    did |= self.perform(&inner, node, tr, dur, easing, rect, depth + 1);
                }
                did
            }
        }
    }

    /// What Scroll to `dest` moves: every scrolling frame around it, innermost first, brings the
    /// layer (then the frame holding it) to its top-left, the innermost `offset` short of it.
    fn scroll_to_layer(&self, dest: NodeId, offset: [f64; 2]) -> Vec<ScrollMove> {
        let doc = &self.doc;
        let mut out = Vec::new();
        let mut target = dest;
        let mut off = offset.map(|v| if v.is_finite() { v } else { 0.0 });
        for f in doc.ancestors(dest) {
            let (rx, ry) = self.range(doc, f);
            let Some(tn) = doc.node(target) else { break };
            if rx <= 0.0 && ry <= 0.0 {
                continue;
            }
            let rel = doc.world_transform(f).inverse() * doc.world_transform(target);
            let b = kition_design_geom::transform_rect_bbox(rel, tn.local_rect());
            let from = self.scroll.get(&f).copied().unwrap_or((0.0, 0.0));
            let to = clamp_scroll((if rx > 0.0 { b.x0 - off[0] } else { from.0 }, if ry > 0.0 { b.y0 - off[1] } else { from.1 }), (rx, ry));
            out.push((f, from, to));
            target = f;
            off = [0.0, 0.0];
        }
        out
    }

    /// The soonest After delay interaction of `root`'s subtree not fired yet, and the seconds until
    /// it is due (timers start when the screen arrives, an overlay's when it opens).
    fn next_delay(&self, doc: &Document, root: NodeId) -> Option<((NodeId, usize), f64)> {
        let since = self.overlays.last().filter(|o| o.id == root).map_or(self.arrived, |o| o.opened);
        let mut next: Option<((NodeId, usize), f64)> = None;
        for id in doc.subtree(root) {
            let Some(n) = doc.node(id) else { continue };
            for (k, it) in n.interactions.iter().enumerate() {
                if let Trigger::AfterDelay { ms } = it.trigger
                    && !self.delays_fired.contains(&(id, k))
                {
                    let due = since + f64::from(ms) / 1000.0 - self.clock;
                    if next.is_none_or(|(_, d)| due < d) {
                        next = Some(((id, k), due));
                    }
                }
            }
        }
        next
    }

    /// Moves the clock to `time` (never back).
    fn set_time(&mut self, time: Option<f64>) {
        if let Some(t) = time.filter(|t| t.is_finite() && *t > self.clock) {
            self.clock = t;
        }
    }

    /// Fires the delayed mouse triggers and After delay timers that are due.
    fn tick(&mut self) {
        for _ in 0..MAX_FIRES {
            let Some(screen) = self.screen() else { return };
            let root = screen.active().0;
            let (due, later): (Vec<Waiting>, Vec<Waiting>) = std::mem::take(&mut self.waiting).into_iter().partition(|w| w.due <= self.clock);
            self.waiting = later;
            self.waiting.retain(|w| w.root == root);
            let mut fired = false;
            for w in due.into_iter().filter(|w| w.root == root) {
                if self.fire(w.node, w.k, w.rect) {
                    fired = true;
                    break;
                }
            }
            if fired {
                continue;
            }
            match self.next_delay(&screen.disp, root) {
                Some(((id, k), due)) if due <= 0.0 => {
                    self.delays_fired.insert((id, k));
                    self.fire(id, k, None);
                }
                _ => return,
            }
        }
    }

    /// Seconds until the next timer is due (0: due now).
    fn next_due(&self, screen: Option<&Screen>) -> Option<f64> {
        let delay = screen.and_then(|s| self.next_delay(&s.disp, s.active().0)).map(|(_, d)| d);
        let waiting = self.waiting.iter().map(|w| w.due - self.clock).reduce(f64::min);
        [delay, waiting].into_iter().flatten().reduce(f64::min).map(|d| d.max(0.0))
    }

    /// Scrolling frames of the active screen under `q`, innermost first.
    fn scrollers_at(&self, screen: &Screen, q: Point) -> Vec<NodeId> {
        let (root, place) = screen.active();
        let mut out: Vec<NodeId> = screen
            .disp
            .subtree(root)
            .into_iter()
            .filter(|f| screen.disp.node(*f).is_some_and(|n| n.visible && (n.overflow != Overflow::None || *f == root)))
            .filter(|f| screen.disp.world_bounds(*f).is_some_and(|b| place.rect(b).contains(q)))
            .filter(|f| self.range(&self.doc, *f) != (0.0, 0.0))
            .collect();
        out.reverse();
        out
    }

    fn set_scroll(&mut self, f: NodeId, v: (f64, f64)) {
        if v.0.is_finite() && v.1.is_finite() && self.scroll.get(&f) != Some(&v) {
            self.scroll.insert(f, v);
            self.touch();
        }
    }

    /// One input event; true when the prototype used it.
    fn event(&mut self, params: &Value) -> Result<(bool, bool), String> {
        let kind = params.get("type").and_then(Value::as_str).ok_or("invalid-params: `type` is required")?;
        self.tick();
        match kind {
            "pointerMove" => Ok((self.pointer_move(Some(point(params)?)), false)),
            "pointerLeave" => Ok((self.pointer_move(None), false)),
            "pointerDown" => Ok((self.pointer_down(point(params)?), false)),
            "pointerUp" => Ok(self.pointer_up(point(params)?)),
            "pointerCancel" => Ok((self.pointer_cancel(), false)),
            "click" => {
                let q = point(params)?;
                self.pointer_move(Some(q));
                let down = self.pointer_down(q);
                let (up, missed) = self.pointer_up(q);
                Ok((down || up, missed))
            }
            "wheel" => {
                let q = point(params)?;
                let (dx, dy) = (num(params, "dx").unwrap_or(0.0), num(params, "dy").unwrap_or(0.0));
                Ok((self.wheel(q, dx, dy), false))
            }
            "key" => {
                let key = params.get("key").and_then(Value::as_str).filter(|k| !k.is_empty()).ok_or("invalid-params: `key` is required")?;
                let flag = |k: &str| params.get(k).and_then(Value::as_bool).unwrap_or(false);
                let mods = KeyMods { ctrl: flag("ctrl"), alt: flag("alt"), shift: flag("shift"), cmd: flag("meta") || flag("cmd") };
                Ok((self.key(key, mods), false))
            }
            other => Err(format!(
                "invalid-params: unknown event type {other:?} (pointerMove, pointerDown, pointerUp, pointerLeave, pointerCancel, click, wheel, key)"
            )),
        }
    }

    fn pointer_move(&mut self, q: Option<Point>) -> bool {
        self.pointer = q;
        let Some(screen) = self.screen() else { return false };
        let root = screen.active().0;
        if let Some(t) = &self.temp
            && !t.press
            && q.is_none_or(|q| !t.rect.contains(q))
        {
            self.revert();
            return true;
        }
        // A press moving past the drag distance: an On drag trigger its way, else the drag scrolls.
        if let (Some(start), Some(q)) = (self.press, q) {
            let (dx, dy) = (q.x - start.x, q.y - start.y);
            if !self.dragged && dx.hypot(dy) > DRAG_START {
                self.dragged = true;
                let hot = screen.hotspots();
                let doc = &screen.disp;
                let drags = |id: NodeId| {
                    doc.node(id)?.interactions.iter().position(|it| it.trigger == Trigger::OnDrag && it.transition.accepts_drag(dx as f32, dy as f32))
                };
                let found = hot.iter().rev().filter(|(_, r)| r.contains(start)).find_map(|(id, r)| drags(*id).map(|k| (*id, k, *r)));
                if let Some((id, k, r)) = found {
                    let seq = self.seq;
                    if self.fire(id, k, Some(r)) {
                        self.press = None;
                        let (w, h) = screen.size;
                        if let Some(p) = self.pending.as_mut().filter(|p| p.seq > seq) {
                            let (x, y) = p
                                .transition
                                .direction()
                                .map_or_else(|| if dx.abs() >= dy.abs() { (dx.signum(), 0.0) } else { (0.0, dy.signum()) }, smart::direction_vec);
                            p.follow = Some((x, y, if x != 0.0 { w } else { h }.max(1.0)));
                        }
                        return true;
                    }
                }
                let horizontal = dx.abs() >= dy.abs();
                let scrolls = self.scrollers_at(&screen, start);
                let on_axis = |r: (f64, f64)| if horizontal { r.0 > 0.0 } else { r.1 > 0.0 };
                let target = scrolls.iter().copied().find(|f| on_axis(self.range(&self.doc, *f))).or_else(|| scrolls.first().copied());
                if let Some(f) = target {
                    let from = clamp_scroll(self.scroll.get(&f).copied().unwrap_or((0.0, 0.0)), self.range(&self.doc, f));
                    self.drag_scroll = Some((f, from));
                }
            }
            if let Some((f, from)) = self.drag_scroll {
                let to = clamp_scroll((from.0 - dx, from.1 - dy), self.range(&self.doc, f));
                self.set_scroll(f, to);
                return true;
            }
        }
        // Enter and leave.
        let hot = screen.hotspots();
        let hits: Vec<(NodeId, Rect)> = hot.iter().rev().filter(|(_, r)| q.is_some_and(|q| r.contains(q))).copied().collect();
        let ids: Vec<NodeId> = hits.iter().map(|h| h.0).collect();
        let entered: Vec<(NodeId, Rect)> = hits.iter().filter(|(id, _)| !self.hovered.contains(id)).copied().collect();
        let left: Vec<NodeId> = self.hovered.iter().filter(|id| !ids.contains(id)).copied().collect();
        self.hovered = ids;
        let doc = &screen.disp;
        for id in left {
            if let Some(k) = find_trigger(doc, id, |t| *t == Trigger::MouseLeave)
                && self.gesture(id, k, None, root)
            {
                return true;
            }
        }
        for (id, r) in &entered {
            if let Some(k) = find_trigger(doc, *id, |t| *t == Trigger::MouseEnter)
                && self.gesture(*id, k, Some(*r), root)
            {
                return true;
            }
        }
        if self.temp.is_none()
            && let Some((id, k, r)) = hits.iter().find_map(|(id, r)| find_trigger(doc, *id, |t| *t == Trigger::OnHover).map(|k| (*id, k, *r)))
            && entered.iter().any(|e| e.0 == id)
            && self.fire(id, k, Some(r))
        {
            return true;
        }
        false
    }

    fn pick(screen: &Screen, q: Point, want: &dyn Fn(&Trigger) -> bool) -> Option<(NodeId, usize, Rect)> {
        screen
            .hotspots()
            .into_iter()
            .rev()
            .filter(|(_, r)| r.contains(q))
            .find_map(|(id, r)| find_trigger(&screen.disp, id, want).map(|k| (id, k, r)))
    }

    fn pointer_down(&mut self, q: Point) -> bool {
        self.pointer = Some(q);
        self.press = Some(q);
        self.dragged = false;
        self.drag_scroll = None;
        let Some(screen) = self.screen() else { return false };
        let root = screen.active().0;
        if let (Some(placed), Some(o)) = (screen.overlays.last(), self.overlays.last())
            && !placed.rect.contains(q)
        {
            if o.close_on_outside {
                self.close_overlay();
                self.press = None;
                self.hovered.clear();
                return true;
            }
            return false;
        }
        if let Some((id, k, r)) = Self::pick(&screen, q, &|t| *t == Trigger::MouseDown)
            && self.gesture(id, k, Some(r), root)
        {
            return true;
        }
        if let Some((id, k, r)) = Self::pick(&screen, q, &|t| *t == Trigger::OnPress)
            && self.fire(id, k, Some(r))
        {
            return true;
        }
        false
    }

    /// The press ends: (used, missed).
    fn pointer_up(&mut self, q: Point) -> (bool, bool) {
        self.pointer = Some(q);
        if self.temp.as_ref().is_some_and(|t| t.press) {
            self.revert();
        }
        let Some(_) = self.press.take() else { return (false, false) };
        let dragged = std::mem::take(&mut self.dragged);
        self.drag_scroll = None;
        let Some(screen) = self.screen() else { return (false, false) };
        let root = screen.active().0;
        if let Some((id, k, r)) = Self::pick(&screen, q, &|t| *t == Trigger::MouseUp)
            && self.gesture(id, k, Some(r), root)
        {
            return (true, false);
        }
        if dragged {
            return (true, false);
        }
        if let Some((id, k, r)) = Self::pick(&screen, q, &|t| *t == Trigger::OnClick)
            && self.fire(id, k, Some(r))
        {
            return (true, false);
        }
        (false, true)
    }

    /// The browser took the press away (a touch turned into a scroll, lost capture): the press
    /// and its drag end without a mouse up or a click, and a While pressing state reverts. True
    /// when a press was down.
    fn pointer_cancel(&mut self) -> bool {
        let pressing = self.temp.as_ref().is_some_and(|t| t.press);
        if pressing {
            self.revert();
        }
        let pressed = self.press.take().is_some();
        self.dragged = false;
        self.drag_scroll = None;
        pressing || pressed
    }

    fn wheel(&mut self, q: Point, dx: f64, dy: f64) -> bool {
        if dx == 0.0 && dy == 0.0 {
            return false;
        }
        let Some(screen) = self.screen() else { return false };
        for f in self.scrollers_at(&screen, q) {
            let cur = self.scroll.get(&f).copied().unwrap_or((0.0, 0.0));
            let next = clamp_scroll((cur.0 + dx, cur.1 + dy), self.range(&self.doc, f));
            if (next.0 - cur.0).abs() > 1e-6 || (next.1 - cur.1).abs() > 1e-6 {
                self.set_scroll(f, next);
                return true;
            }
        }
        false
    }

    fn key(&mut self, key: &str, mods: KeyMods) -> bool {
        let Some(screen) = self.screen() else { return false };
        let root = screen.active().0;
        let name = key_name(key);
        let doc = &screen.disp;
        let found = doc.subtree(root).into_iter().find_map(|id| find_trigger(doc, id, |t| t.fires_on_key(&name, mods)).map(|k| (id, k)));
        if let Some((id, k)) = found {
            return self.fire(id, k, None);
        }
        let plain = mods == KeyMods::default();
        if plain && name == "R" {
            self.restart(self.start, true);
            return true;
        }
        // The arrows (and Space or N forward) step through top-level frames when nothing uses them.
        let forward = match name.as_str() {
            "ArrowRight" => Some(true),
            "Space" | "N" if plain => Some(true),
            "ArrowLeft" => Some(false),
            _ => None,
        };
        if let Some(forward) = forward
            && self.overlays.is_empty()
        {
            let frames = self.doc.top_frames(self.doc.page_of(self.current).unwrap_or(self.current));
            if let Some(i) = frames.iter().position(|f| *f == self.current) {
                let next = if forward { frames.get(i + 1) } else { i.checked_sub(1).and_then(|j| frames.get(j)) };
                if let Some(n) = next.copied() {
                    self.go("navigate", n, Transition::Instant, 0.0, Easing::Linear, true, false);
                    return true;
                }
            }
        }
        false
    }

    /// The last transition as JSON (see the module documentation).
    fn transition_json(&self) -> Value {
        let Some(p) = &self.pending else { return Value::Null };
        json!({
            "seq": p.seq,
            "kind": p.kind,
            "from": p.from.current,
            "to": p.to_frame,
            "overlay": p.overlay,
            "transition": p.transition,
            "smartAnimate": p.transition == Transition::SmartAnimate,
            "duration": p.dur,
            "durationMs": (p.dur * 1000.0).round(),
            "easing": p.easing,
            "follow": p.follow.map(|(x, y, length)| json!({ "x": x, "y": y, "length": length })),
        })
    }

    /// The player's state as JSON (see the module documentation).
    fn state_json(&self) -> Value {
        let screen = self.screen();
        let doc = &self.doc;
        let settings = page_settings(doc, self.current);
        let flows = match doc.page_of(self.start).and_then(|pg| doc.node(pg)).map(|n| &n.kind) {
            Some(NodeKind::Page { flows, .. }) => flows.clone(),
            _ => Vec::new(),
        };
        let flow_json = |f: &kition_design_doc::proto::Flow| json!({ "start": f.start, "name": f.name, "description": f.description });
        let name_of = |id: NodeId| doc.node(id).map(|n| n.name.clone());
        let rect_json = |r: Rect| json!({ "x": r.x0, "y": r.y0, "width": r.width(), "height": r.height() });
        let device = settings.device().map(|d| {
            json!({
                "name": d.name,
                "width": d.width,
                "height": d.height,
                "landscape": settings.landscape(),
                "rotates": d.rotates,
                "color": settings.device_color().map(|c| json!({ "name": c.name, "rgb": format!("#{:02x}{:02x}{:02x}", c.rgb[0], c.rgb[1], c.rgb[2]) })),
            })
        });
        let (size, frame, overlays, hotspots) = match &screen {
            Some(s) => {
                let fr = Rect::new(s.frame.origin.0, s.frame.origin.1, s.frame.origin.0 + s.frame_size.0, s.frame.origin.1 + s.frame_size.1);
                let overlays: Vec<Value> = s
                    .overlays
                    .iter()
                    .zip(&self.overlays)
                    .map(|(placed, o)| {
                        json!({
                            "id": placed.id,
                            "name": name_of(placed.id),
                            "x": placed.rect.x0,
                            "y": placed.rect.y0,
                            "width": placed.rect.width(),
                            "height": placed.rect.height(),
                            "background": o.background.map(Color::to_css_hex),
                            "closeOnOutside": o.close_on_outside,
                            "position": o.position,
                        })
                    })
                    .collect();
                let hotspots: Vec<Value> = s
                    .hotspots()
                    .into_iter()
                    .map(|(id, r)| {
                        let triggers: Vec<Value> =
                            s.disp.node(id).map(|n| n.interactions.iter().map(|it| trigger_type(&it.trigger)).collect()).unwrap_or_default();
                        let mut v = rect_json(r);
                        v["id"] = json!(id);
                        v["name"] = json!(name_of(id));
                        v["triggers"] = Value::Array(triggers);
                        v
                    })
                    .collect();
                (json!({ "width": s.size.0, "height": s.size.1 }), rect_json(fr), overlays, hotspots)
            }
            None => (Value::Null, Value::Null, Vec::new(), Vec::new()),
        };
        let scroll = self.scroll.get(&self.current).copied().unwrap_or((0.0, 0.0));
        let range = self.range(doc, self.current);
        json!({
            "start": self.start,
            "current": self.current,
            "name": name_of(self.current),
            "flow": flows.iter().find(|f| f.start == self.start).map(flow_json),
            "flows": flows.iter().map(flow_json).collect::<Vec<_>>(),
            "screen": size,
            "frame": frame,
            "device": device,
            "background": settings.background.to_css_hex(),
            "responsive": settings.responsive,
            "history": self.history.iter().map(|v| v.frame).collect::<Vec<_>>(),
            "canBack": !self.history.is_empty() || !self.overlays.is_empty(),
            "overlays": overlays,
            "scroll": [scroll.0, scroll.1],
            "scrollRange": [range.0, range.1],
            "hotspots": hotspots,
            "hovered": self.hovered,
            "pressed": self.press.is_some(),
            "waiting": self.waiting.len(),
            "nextDue": self.next_due(screen.as_ref()),
            "clock": self.clock,
            "revision": self.rev,
        })
    }

    /// The answer of every command but `stop`.
    fn answer(&mut self, handled: bool, missed: bool) -> Value {
        let cursor = match (self.drag_scroll.is_some(), self.pointer) {
            (true, _) => "grabbing",
            (false, Some(q)) if self.screen().is_some_and(|s| s.hotspots().iter().any(|(_, r)| r.contains(q))) => "pointer",
            _ => "default",
        };
        let v = json!({
            "handled": handled,
            "changed": self.changed,
            "missed": missed,
            "cursor": cursor,
            "links": std::mem::take(&mut self.links),
            "transition": if self.new_transition { self.transition_json() } else { Value::Null },
            "state": self.state_json(),
        });
        self.changed = false;
        self.new_transition = false;
        v
    }

    /// The screen at `params` (see `playerRender`).
    /// `inspect`: the layer under a screen point (or `id`) on the active screen and its properties.
    fn inspect(&self, params: &Value) -> Result<Value, String> {
        use kition_design_engine::devinspect::{ColorFormat, properties_with};
        let fmt = match params.get("colorFormat") {
            None | Some(Value::Null) => ColorFormat::Hex,
            Some(v) => v.as_str().and_then(ColorFormat::from_id).ok_or("invalid-params: `colorFormat` is hex, rgb, css, hsl or hsb")?,
        };
        let Some(screen) = self.screen() else { return Ok(json!({ "id": null })) };
        let (root, place) = screen.active();
        let disp = &screen.disp;
        let clip = disp.world_bounds(root).map(|b| place.rect(b).intersect(screen.bounds())).unwrap_or_else(|| screen.bounds());
        let at = match (num(params, "x"), num(params, "y")) {
            (Some(x), Some(y)) => Some(Point::new(x, y)),
            _ => None,
        };
        // Upstream's `layer_at`: the last visible layer in paint order whose bounds hold the point.
        let hit = at.filter(|q| clip.contains(*q)).and_then(|q| {
            let mut hidden: HashSet<NodeId> = HashSet::new();
            let mut hit = None;
            for id in disp.subtree(root) {
                let Some(n) = disp.node(id) else { continue };
                if !n.visible || n.parent.is_some_and(|p| hidden.contains(&p)) {
                    hidden.insert(id);
                    continue;
                }
                if disp.world_bounds(id).is_some_and(|b| place.rect(b).contains(q)) {
                    hit = Some(id);
                }
            }
            hit
        });
        let on_screen = |id: NodeId| (id == root || disp.is_ancestor(root, id)) && self.doc.node(id).is_some();
        let Some(id) = hit.or_else(|| kition_design_engine::params::id(params, "id")).filter(|id| on_screen(*id)) else {
            return Ok(json!({ "id": null }));
        };
        let rect = |r: Rect| json!({ "x": r.x0, "y": r.y0, "width": r.width(), "height": r.height() });
        let bounds = disp.world_bounds(id).map(|b| place.rect(b));
        Ok(json!({
            "id": id,
            "name": self.doc.node(id).map(|n| n.name.clone()).unwrap_or_default(),
            "bounds": bounds.map(rect),
            "clip": rect(clip),
            "sections": serde_json::to_value(properties_with(&self.doc, id, fmt)).unwrap_or(Value::Null),
        }))
    }

    fn render(&self, params: &Value) -> Result<Vec<u8>, String> {
        let scale = match params.get("scale") {
            None | Some(Value::Null) => 1.0,
            Some(v) => finite(v.as_f64()).filter(|s| *s > 0.0).ok_or("invalid-params: `scale` must be a positive number")?.clamp(0.01, 8.0),
        };
        let rgba = match params.get("format").and_then(Value::as_str) {
            None | Some("png") => false,
            Some("rgba") => true,
            Some(other) => return Err(format!("invalid-params: `format` is png or rgba, not {other:?}")),
        };
        let from = match params.get("state").and_then(Value::as_str) {
            None | Some("current") => false,
            Some("from") => true,
            Some(other) => return Err(format!("invalid-params: `state` is current or from, not {other:?}")),
        };
        let progress = match params.get("progress") {
            None | Some(Value::Null) => None,
            Some(v) => Some(finite(v.as_f64()).ok_or("invalid-params: `progress` must be a number from 0 to 1")?.clamp(0.0, 1.0)),
        };
        let current = self.screen().ok_or("the frame playing draws nothing")?;
        let pending = self.pending.as_ref();
        let from_screen = |p: &Pending| screen_of(&p.from.doc, &p.from.scroll, p.from.current, &p.from.overlays);
        let mut pix = match (pending, from, progress) {
            (Some(p), true, _) => paint::compose(&from_screen(p).ok_or("what showed before draws nothing")?, scale, None)?,
            (None, true, _) => return Err("no transition: nothing showed before".into()),
            (Some(p), false, Some(t)) if t < 1.0 => {
                self.render_transition(p, &current, paint::fit_scale(current.size, scale, paint::MAX_TRANSITION_PIXELS), t)?
            }
            _ => paint::compose(&current, scale, None)?,
        };
        kition_design_render::to_srgb(&mut pix, current.disp.color_profile);
        if !rgba {
            return kition_design_render::encode_png(&pix);
        }
        // The size first: the editor reads it rather than rounding the screen's size itself.
        let (width, height) = (pix.width(), pix.height());
        let pixels = kition_design_render::to_rgba8(&pix);
        drop(pix);
        let mut out = Vec::with_capacity(8 + pixels.len());
        out.extend_from_slice(&width.to_le_bytes());
        out.extend_from_slice(&height.to_le_bytes());
        out.extend_from_slice(&pixels);
        Ok(out)
    }

    /// The last transition `p` at linear progress `t`.
    fn render_transition(&self, p: &Pending, current: &Screen, scale: f64, t: f64) -> Result<kition_design_render::Pixmap, String> {
        let e = p.easing.eval(t);
        let from = || screen_of(&p.from.doc, &p.from.scroll, p.from.current, &p.from.overlays).ok_or("what showed before draws nothing".to_owned());
        let moved = |screen: &Screen, at: f64| {
            let pl = smart::transition_frames(p.transition, at);
            screen.overlays.len().checked_sub(1).and_then(|i| screen.overlays.get(i).map(|o| (i, o))).map(|(i, o)| paint::Moved {
                index: i,
                dx: pl.to_off.0 * o.rect.width(),
                dy: pl.to_off.1 * o.rect.height(),
                alpha: pl.to_alpha,
            })
        };
        match p.kind {
            "overlayOpen" => paint::compose(current, scale, moved(current, e)),
            "overlayClose" => {
                let f = from()?;
                paint::compose(&f, scale, moved(&f, 1.0 - e))
            }
            "scroll" => {
                let mut scroll = self.scroll.clone();
                for (f, a, b) in &p.scrolls {
                    scroll.insert(*f, (smart::lerp(a.0, b.0, e), smart::lerp(a.1, b.1, e)));
                }
                let s = screen_of(&self.doc, &scroll, self.current, &self.overlays).ok_or("the frame playing draws nothing")?;
                paint::compose(&s, scale, None)
            }
            _ if matches!(p.transition, Transition::Instant) || p.dur <= 0.0 => paint::compose(current, scale, None),
            _ if p.transition == Transition::SmartAnimate => {
                let f = from()?;
                let disp = smart::smart_interpolate(&f.disp, f.current, &current.disp, current.current, e);
                let tween = Screen {
                    disp,
                    current: current.current,
                    size: current.size,
                    frame: current.frame,
                    frame_size: current.frame_size,
                    background: current.background,
                    overlays: current.overlays.clone(),
                };
                paint::compose(&tween, scale, None)
            }
            _ => {
                let f = from()?;
                let (a, b) = (paint::compose(&f, scale, None)?, paint::compose(current, scale, None)?);
                let (w, h) = paint::pixel_size(current.size, scale)?;
                let mut out = kition_design_render::Pixmap::new(w, h).ok_or_else(|| format!("cannot draw {w} x {h}"))?;
                paint::fill(&mut out, current.background, 1.0);
                let pl = smart::transition_frames(p.transition, e);
                let at = |off: (f64, f64)| ((off.0 * f64::from(w)).round() as i64, (off.1 * f64::from(h)).round() as i64);
                let (fa, ta) = (at(pl.from_off), at(pl.to_off));
                if pl.from_on_top {
                    paint::blit(&mut out, &b, ta.0, ta.1, pl.to_alpha);
                    paint::blit(&mut out, &a, fa.0, fa.1, pl.from_alpha);
                } else {
                    paint::blit(&mut out, &a, fa.0, fa.1, pl.from_alpha);
                    paint::blit(&mut out, &b, ta.0, ta.1, pl.to_alpha);
                }
                Ok(out)
            }
        }
    }
}

impl DesignDocument {
    /// The frame `start` (a frame, a layer in one, or a flow's start; default as `proto.present`)
    /// presents.
    fn player_start_frame(&self, start: Option<&Value>) -> Result<NodeId, String> {
        let params = match start {
            None | Some(Value::Null) => json!({}),
            Some(v) => json!({ "start": v }),
        };
        let answer = crate::query::read(self, "proto.present", &params)?;
        kition_design_engine::params::id(&answer, "start").ok_or_else(|| "no frame to present".to_owned())
    }

    /// Follows the document. When the frame playing and the start are gone, the player starts
    /// again from the design's default start (as `start` with none picks it), and with nothing
    /// left to present it stops; when only the start is gone, the default start is the start.
    fn player_follow(&mut self) -> Result<(), String> {
        let source = self.ds()?.doc.clone();
        let p = self.player.as_deref_mut().ok_or("not-playing: start the player first (player(\"start\"))")?;
        p.sync(&source);
        let (lost, lost_start) = (p.lost(), p.lost_start());
        if !lost && !lost_start {
            return Ok(());
        }
        match self.player_start_frame(None) {
            Ok(start) => {
                if let Some(p) = self.player.as_deref_mut() {
                    if lost {
                        p.restart(start, false);
                    } else {
                        p.start = start;
                    }
                }
                Ok(())
            }
            // Still on a frame: it is the start now.
            Err(_) if !lost => {
                if let Some(p) = self.player.as_deref_mut() {
                    p.start = p.current;
                }
                Ok(())
            }
            Err(_) => {
                self.player = None;
                Err("not-playing: the design has no frame left to present".into())
            }
        }
    }

    /// `player(command, params)`: the prototype player (see [`crate::player`]).
    pub fn player(&mut self, command: &str, params: &Value) -> Result<Value, String> {
        let params = if params.is_null() { json!({}) } else { params.clone() };
        if !params.is_object() {
            return Err("invalid-params: params must be an object".into());
        }
        let time = num(&params, "time");
        if command == "start" {
            let window = size_param(params.get("window"))?;
            let start = self.player_start_frame(params.get("start"))?;
            let source = self.ds()?.doc.clone();
            let mut p = Player::new(source, start, window, time.unwrap_or(0.0));
            p.tick();
            let v = p.answer(false, false);
            self.player = Some(Box::new(p));
            return Ok(v);
        }
        if command == "stop" {
            return Ok(json!({ "stopped": self.player.take().is_some() }));
        }
        let restart_frame = match command {
            "restart" if params.get("start").is_some_and(|v| !v.is_null()) => Some(self.player_start_frame(params.get("start"))?),
            _ => None,
        };
        self.player_follow()?;
        let p = self.player.as_deref_mut().ok_or("not-playing: start the player first (player(\"start\"))")?;
        p.set_time(time);
        if command == "inspect" {
            return p.inspect(&params);
        }
        let (handled, missed) = match command {
            "state" => (false, false),
            "tick" => {
                p.tick();
                (false, false)
            }
            "event" => p.event(&params)?,
            "restart" => {
                let start = restart_frame.unwrap_or(p.start);
                p.restart(start, true);
                (true, false)
            }
            "back" => {
                p.tick();
                (p.perform(&Action::Back, p.current, Transition::Instant, 0.0, Easing::Linear, None, 0), false)
            }
            "closeOverlay" => (p.close_overlay(), false),
            "scrollTo" => {
                let q = point(&params)?;
                let to = clamp_scroll((q.x, q.y), p.range(&p.doc, p.current));
                let current = p.current;
                p.set_scroll(current, to);
                (true, false)
            }
            "resize" => {
                let window = size_param(Some(&params))?;
                if window != p.window {
                    p.window = window;
                    p.relayout();
                    let current = p.current;
                    let range = p.range(&p.doc, current);
                    if let Some(s) = p.scroll.get(&current).copied() {
                        p.scroll.insert(current, clamp_scroll(s, range));
                    }
                }
                (false, false)
            }
            other => {
                return Err(format!(
                    "invalid-params: unknown player command {other:?} (start, stop, state, tick, event, restart, back, closeOverlay, scrollTo, resize, inspect)"
                ));
            }
        };
        Ok(p.answer(handled, missed))
    }

    /// `playerRender(params)`: the player's screen as PNG or RGBA bytes (see [`crate::player`]).
    pub fn player_render(&mut self, params: &Value) -> Result<Vec<u8>, String> {
        let params = if params.is_null() { json!({}) } else { params.clone() };
        if !params.is_object() {
            return Err("invalid-params: params must be an object".into());
        }
        self.player_follow()?;
        let p = self.player.as_deref_mut().ok_or("not-playing: start the player first (player(\"start\"))")?;
        p.render(&params)
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use kition_kit::wasm_bindgen::prelude::*;

    use crate::__kition_kit_export::WebDocument;

    #[wasm_bindgen(js_class = "DesignDocument", wasm_bindgen = kition_kit::wasm_bindgen)]
    impl WebDocument {
        /// The prototype player: `command` is start, stop, state, tick, event, restart, back,
        /// closeOverlay, scrollTo, resize or inspect; `params` a JSON object (an empty string is `{}`). The
        /// answer as JSON (see `player.rs`).
        pub fn player(&mut self, command: &str, params: &str) -> Result<String, JsError> {
            let params = kition_kit::parse_params(params).map_err(|e| JsError::new(&e))?;
            self.0.player(command, &params).map(|v| v.to_string()).map_err(|e| JsError::new(&e))
        }

        /// The player's screen as PNG (or RGBA) bytes; `params` is a JSON object (see `player.rs`).
        #[wasm_bindgen(js_name = playerRender)]
        pub fn player_render(&mut self, params: &str) -> Result<Vec<u8>, JsError> {
            let params = kition_kit::parse_params(params).map_err(|e| JsError::new(&e))?;
            self.0.player_render(&params).map_err(|e| JsError::new(&e))
        }
    }
}
