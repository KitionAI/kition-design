//! What [`KitDocument::check`](kition_kit::KitDocument::check) reports on a design (KHP A21, A38),
//! from what the engine computes: broken references (main components, images, prototype
//! destinations and flows, variables, styles), text that does not fit its box or reads badly on its
//! backdrop, fonts drawn with a substitute (A39), layers that never show, and design-system hygiene
//! (unused styles, colours a variable already holds, positions off the pixel grid).
//!
//! Every code is in [`CATALOGUE`] (shipped as `agent/checks.json`, one conformance fixture per
//! code). Layers are named by `target.path` `layer:<id>` (the ids the engine's commands take in
//! `ids`) and `target.page` (counted from 1); styles by `style:<id>`. Each diagnostic's `fix` is a
//! command `<prefix>_run` takes as it is. Only the pages are checked: the library store's copies are
//! checked in the library, and the layers inside an instance only for what an override changes
//! (text). A code found more than [`PER_CODE`] times is listed that often, and `design.truncated`
//! counts the rest.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use kition_design_color::{Color, Paint, PaintKind};
use kition_design_doc::proto::{self, Action};
use kition_design_doc::{Document, Node, NodeId, NodeKind, Sizing, TextAutoResize, VariableType, VariableValue};
use kition_kit::diagnostics::{Diagnostic, Severity, Target};
use serde_json::{Value, json};

/// At most this many diagnostics of one code; the rest are counted in one `design.truncated`.
pub const PER_CODE: usize = 100;

/// One code the check reports: its severity and a one-sentence summary (`agent/checks.json`).
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub code: &'static str,
    pub severity: Severity,
    pub summary: &'static str,
}

const fn entry(code: &'static str, severity: Severity, summary: &'static str) -> Entry {
    Entry { code, severity, summary }
}

/// Every code the check reports, errors first (KHP A38).
pub const CATALOGUE: &[Entry] = &[
    entry("design.missing-component", Severity::Error, "An instance's main component is not in the file, so it cannot follow its main."),
    entry("design.missing-image", Severity::Error, "A fill or stroke shows an image the file does not hold, so it draws nothing."),
    entry(
        "design.broken-interaction",
        Severity::Error,
        "A prototype interaction goes to a layer, or sets a variable or collection, that the file no longer has.",
    ),
    entry("design.broken-flow", Severity::Error, "A prototype flow starts at a frame that is not on its page."),
    entry("design.text-overflow", Severity::Warning, "A fixed-size text layer's text runs past the bottom of its box."),
    entry(
        "design.low-contrast",
        Severity::Warning,
        "Text contrasts with the solid colour behind it less than WCAG AA asks (4.5:1, or 3:1 for large text).",
    ),
    entry("design.missing-variable", Severity::Warning, "A layer is bound to a variable the file does not have, so the value no longer follows it."),
    entry("design.missing-style", Severity::Warning, "A layer uses a style the file does not have, so it no longer follows it."),
    entry("design.outside-clip", Severity::Warning, "A visible layer lies wholly outside the frame that clips it, so it never shows."),
    entry("design.empty-component", Severity::Warning, "A main component has no layers and paints nothing, so its instances show nothing."),
    entry("design.font-substituted", Severity::Info, "Text names a font this kit does not have, so it is drawn (and previewed) with a substitute."),
    entry("design.text-ellipsis", Severity::Info, "A truncating text layer cuts its text off with an ellipsis."),
    entry("design.unbound-color", Severity::Info, "A solid colour equals a colour variable's value but is not bound to it."),
    entry("design.unused-style", Severity::Info, "A local style is used by no layer."),
    entry("design.off-pixel", Severity::Info, "A layer's position or size is not on whole pixels, so its edges render blurred."),
    entry(
        "design.absolute-in-auto-layout",
        Severity::Info,
        "A layer in an auto layout frame ignores the layout (absolute position), so the layout does not move it.",
    ),
    entry("design.empty-text", Severity::Info, "A text layer has no text."),
    entry("design.empty-frame", Severity::Info, "A frame has no layers and paints nothing."),
    entry("design.truncated", Severity::Info, "A code was found more often than the check lists; the rest are counted."),
];

/// WCAG AA: normal text needs 4.5:1, large text (24 px, or 18.66 px bold) 3:1.
const AA_NORMAL: f32 = 4.5;
const AA_LARGE: f32 = 3.0;

/// Colours closer than this per channel are the same colour (half a step of 8 bits).
const SAME_COLOR: f32 = 0.5 / 255.0;

/// How far off a whole pixel a position or size may be before it counts.
const OFF_PIXEL: f64 = 0.01;

fn on(n: &Node, page: u32) -> Target {
    Target { path: Some(format!("layer:{}", n.id)), page: Some(page), ..Target::default() }
}

fn label(n: &Node) -> String {
    if n.name.trim().is_empty() { format!("Layer {}", n.id) } else { format!("\"{}\"", n.name) }
}

fn hex(c: Color) -> String {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02X}{:02X}{:02X}", byte(c.r), byte(c.g), byte(c.b))
}

#[derive(Default)]
struct Found {
    list: Vec<Diagnostic>,
    counts: BTreeMap<String, usize>,
}

impl Found {
    fn push(&mut self, diagnostic: Diagnostic) {
        let count = self.counts.entry(diagnostic.code.clone()).or_default();
        *count += 1;
        if *count <= PER_CODE {
            self.list.push(diagnostic);
        }
    }

    fn finish(mut self) -> Vec<Diagnostic> {
        for (code, count) in &self.counts {
            if *count > PER_CODE {
                self.list.push(Diagnostic::new(
                    "design.truncated",
                    Severity::Info,
                    format!("{} more `{code}` diagnostics are not listed.", count - PER_CODE),
                ));
            }
        }
        // Stable: errors first, then in the order found (pages, then layers top-down).
        self.list.sort_by_key(|d| d.severity);
        self.list
    }
}

/// Everything worth fixing in the design, errors first.
pub fn check(doc: &Document) -> Vec<Diagnostic> {
    check_libraries(doc, &BTreeMap::new())
}

/// [`check`], with the enabled libraries whose files can't be read (library id → its name and
/// why): their components' instances can't follow their mains (`design.missing-component`).
pub fn check_libraries(doc: &Document, unreadable: &BTreeMap<String, (String, String)>) -> Vec<Diagnostic> {
    run(doc, &mut |family| kition_design_text::fontdb::has_family(family), unreadable)
}

/// [`check`] with `has_font` saying whether a family can be drawn here.
pub fn check_with(doc: &Document, has_font: &mut dyn FnMut(&str) -> bool) -> Vec<Diagnostic> {
    run(doc, has_font, &BTreeMap::new())
}

fn run(doc: &Document, has_font: &mut dyn FnMut(&str) -> bool, unreadable: &BTreeMap<String, (String, String)>) -> Vec<Diagnostic> {
    let mut checker = Checker::new(doc, has_font, unreadable);
    for (index, page) in doc.pages.iter().enumerate() {
        let number = u32::try_from(index + 1).unwrap_or(u32::MAX);
        checker.page(*page, number);
    }
    checker.unused_styles();
    checker.found.finish()
}

struct Checker<'a> {
    doc: &'a Document,
    has_font: &'a mut dyn FnMut(&str) -> bool,
    /// Enabled libraries whose files can't be read: id → (name, why).
    unreadable: &'a BTreeMap<String, (String, String)>,
    found: Found,
    /// Families already asked about.
    fonts: BTreeMap<String, bool>,
    /// Style ids some layer uses.
    used_styles: BTreeSet<String>,
    /// Local colour variables' default values, for `design.unbound-color`.
    colors: Vec<(String, String, Color)>,
    /// The style ids the file has (a layer's reference is an id: styles follow edits by id).
    style_ids: HashSet<&'a str>,
    /// The main components some instance uses, made when first asked.
    used_mains: Option<HashSet<NodeId>>,
    /// Parents' children as [`backdrop`] looks under text layers.
    under: HashMap<NodeId, Under>,
}

impl<'a> Checker<'a> {
    fn new(doc: &'a Document, has_font: &'a mut dyn FnMut(&str) -> bool, unreadable: &'a BTreeMap<String, (String, String)>) -> Self {
        let library: BTreeSet<&str> = doc.libraries.variables.iter().map(|record| record.local.as_str()).collect();
        let colors = doc
            .variables
            .variables
            .iter()
            .filter(|v| v.kind == VariableType::Color && !library.contains(v.id.as_str()))
            .filter_map(|v| match doc.variables.resolve(&v.id, &BTreeMap::new()) {
                Some(VariableValue::Color(color)) => Some((v.id.clone(), v.name.clone(), color)),
                _ => None,
            })
            .collect();
        let style_ids = doc.styles.iter().map(|style| style.id.as_str()).collect();
        Self {
            doc,
            has_font,
            unreadable,
            found: Found::default(),
            fonts: BTreeMap::new(),
            used_styles: BTreeSet::new(),
            colors,
            style_ids,
            used_mains: None,
            under: HashMap::new(),
        }
    }

    /// The library (its name, and why its file can't be read) that main component `component`
    /// was copied from, when that library's file can't be read now.
    fn unreadable_library(&self, component: NodeId) -> Option<(&'a str, &'a str)> {
        let library = kition_design_engine::library::library_of(self.doc, component)?;
        self.unreadable.get(library).map(|(name, why)| (name.as_str(), why.as_str()))
    }

    fn page(&mut self, page: NodeId, number: u32) {
        let Some(p) = self.doc.node(page) else { return };
        if let NodeKind::Page { flows, .. } = &p.kind {
            for flow in flows {
                if self.doc.page_of(flow.start) != Some(page) {
                    let name = if flow.name.trim().is_empty() { "A flow".to_owned() } else { format!("The flow \"{}\"", flow.name) };
                    self.found.push(
                        Diagnostic::new(
                            "design.broken-flow",
                            Severity::Error,
                            format!("{name} on page {number} starts at layer {}, which is not on that page.", flow.start),
                        )
                        .on(Target { page: Some(number), ..Target::default() })
                        .fix("proto.flow.remove", Some(json!({ "start": flow.start }))),
                    );
                }
            }
        }
        let mut stack: Vec<(NodeId, bool)> = p.children.iter().rev().map(|c| (*c, false)).collect();
        while let Some((id, in_instance)) = stack.pop() {
            let Some(n) = self.doc.node(id) else { continue };
            let descend = self.node(n, number, in_instance);
            if descend {
                let inner = in_instance || matches!(n.kind, NodeKind::Instance { .. });
                stack.extend(n.children.iter().rev().map(|c| (*c, inner)));
            }
        }
    }

    /// Checks one layer; answers whether to check its children.
    fn node(&mut self, n: &Node, page: u32, in_instance: bool) -> bool {
        self.note_styles(n);
        if let NodeKind::Text(_) = &n.kind {
            self.text(n, page);
        }
        if in_instance {
            return true;
        }
        let doc = self.doc;
        let unread = match &n.kind {
            NodeKind::Instance { component, .. } => self.unreadable_library(*component),
            _ => None,
        };
        if let Some((library, why)) = unread {
            self.found.push(
                Diagnostic::new(
                    "design.missing-component",
                    Severity::Error,
                    format!("{} is an instance of a component from the library \"{library}\", whose file can't be read ({why}), so it cannot follow its main.", label(n)),
                )
                .on(on(n, page))
                .fix("component.detach", Some(json!({ "ids": [n.id] }))),
            );
            return true;
        }
        match &n.kind {
            NodeKind::Instance { component, .. } if doc.node(*component).is_none() => self.found.push(
                Diagnostic::new(
                    "design.missing-component",
                    Severity::Error,
                    format!("{} is an instance of a main component this file no longer has.", label(n)),
                )
                .on(on(n, page))
                .fix("component.detach", Some(json!({ "ids": [n.id] }))),
            ),
            NodeKind::Text(t) if t.characters.trim().is_empty() => self.found.push(
                Diagnostic::new("design.empty-text", Severity::Info, format!("{} is a text layer without text.", label(n)))
                    .on(on(n, page))
                    .fix("edit.delete", Some(json!({ "ids": [n.id] }))),
            ),
            NodeKind::Frame if n.children.is_empty() && paints_nothing(n) => self.found.push(
                Diagnostic::new("design.empty-frame", Severity::Info, format!("{} is an empty frame that paints nothing.", label(n)))
                    .on(on(n, page))
                    .fix("edit.delete", Some(json!({ "ids": [n.id] }))),
            ),
            NodeKind::Component { .. } if n.children.is_empty() && paints_nothing(n) => {
                let used = self
                    .used_mains
                    .get_or_insert_with(|| {
                        doc.nodes()
                            .filter_map(|other| match other.kind {
                                NodeKind::Instance { component, .. } => Some(component),
                                _ => None,
                            })
                            .collect()
                    })
                    .contains(&n.id);
                let diagnostic = Diagnostic::new(
                    "design.empty-component",
                    Severity::Warning,
                    format!("{} is a main component with no layers that paints nothing, so its instances show nothing.", label(n)),
                )
                .on(on(n, page));
                // Deleting a main its instances use would leave them without one.
                self.found.push(if used { diagnostic } else { diagnostic.fix("edit.delete", Some(json!({ "ids": [n.id] }))) });
            }
            _ => {}
        }
        self.images(n, page);
        self.interactions(n, page);
        self.bindings(n, page);
        self.unbound_color(n, page);
        self.placement(n, page);
        !self.outside_clip(n, page)
    }

    fn images(&mut self, n: &Node, page: u32) {
        for (list, command, which) in [(&n.fills, "fill.remove", "fill"), (&n.strokes, "stroke.remove", "stroke")] {
            for (index, paint) in list.iter().enumerate() {
                if let PaintKind::Image(image) = &paint.kind
                    && !image.image.is_empty()
                    && !self.doc.images.contains_key(&image.image)
                {
                    self.found.push(
                        Diagnostic::new(
                            "design.missing-image",
                            Severity::Error,
                            format!("{}'s {which} {} shows an image the file does not hold, so it draws nothing.", label(n), index + 1),
                        )
                        .on(on(n, page))
                        .fix(command, Some(json!({ "ids": [n.id], "index": index }))),
                    );
                    return;
                }
            }
        }
    }

    /// The first broken action of each interaction.
    fn interactions(&mut self, n: &Node, page: u32) {
        let doc = self.doc;
        for (index, interaction) in n.interactions.iter().enumerate() {
            let broken = proto::walk(&interaction.actions).find_map(|action| match action {
                Action::Navigate { dest, .. }
                | Action::ScrollTo { dest, .. }
                | Action::OpenOverlay { dest, .. }
                | Action::SwapOverlay { dest }
                | Action::ChangeTo { dest }
                    if doc.node(*dest).is_none() =>
                {
                    Some(format!("goes to layer {dest}, which the file no longer has"))
                }
                Action::SetVariable { variable, .. } if doc.variables.get(variable).is_none() => {
                    Some(format!("sets the variable {variable:?}, which the file does not have"))
                }
                Action::SetVariableMode { collection, .. } if doc.variables.collection(collection).is_none() => {
                    Some(format!("switches the collection {collection:?}, which the file does not have"))
                }
                _ => None,
            });
            if let Some(why) = broken {
                self.found.push(
                    Diagnostic::new("design.broken-interaction", Severity::Error, format!("Interaction {} of {} {why}.", index + 1, label(n)))
                        .on(on(n, page))
                        .fix("proto.interaction.remove", Some(json!({ "id": n.id, "index": index }))),
                );
            }
        }
    }

    /// Variables and styles the layer names that the file does not have.
    fn bindings(&mut self, n: &Node, page: u32) {
        let doc = self.doc;
        if let Some((field, variable)) = n.bound.iter().find(|(_, variable)| doc.variables.get(variable).is_none()) {
            self.found.push(
                Diagnostic::new(
                    "design.missing-variable",
                    Severity::Warning,
                    format!("{}'s {field} is bound to the variable {variable:?}, which the file does not have.", label(n)),
                )
                .on(on(n, page))
                .fix("variable.bind", Some(json!({ "ids": [n.id], "field": field, "variable": null }))),
            );
        }
        let styles = [
            ("fill", &n.styles.fill),
            ("stroke", &n.styles.stroke),
            ("effect", &n.styles.effect),
            ("grid", &n.styles.grid),
            ("text", &n.styles.text),
        ];
        let style_ids = &self.style_ids;
        if let Some((which, Some(style))) = styles.into_iter().find(|(_, style)| style.as_ref().is_some_and(|id| !style_ids.contains(id.as_str()))) {
            self.found.push(
                Diagnostic::new(
                    "design.missing-style",
                    Severity::Warning,
                    format!("{} uses the {which} style {style:?}, which the file does not have.", label(n)),
                )
                .on(on(n, page))
                .fix("style.detach", Some(json!({ "ids": [n.id], "which": which }))),
            );
        }
    }

    fn note_styles(&mut self, n: &Node) {
        let refs = [&n.styles.fill, &n.styles.stroke, &n.styles.effect, &n.styles.grid, &n.styles.text];
        self.used_styles.extend(refs.into_iter().flatten().cloned());
        if let NodeKind::Text(t) = &n.kind {
            self.used_styles.extend(t.spans().into_iter().filter_map(|(_, _, style)| style.style_id.clone()));
            self.used_styles.extend(t.style.style_id.clone());
        }
    }

    /// A solid fill or stroke that equals a local colour variable's value without being bound.
    fn unbound_color(&mut self, n: &Node, page: u32) {
        if self.colors.is_empty() || matches!(n.kind, NodeKind::Text(_)) {
            return;
        }
        for (list, key, style) in [(&n.fills, "fills", &n.styles.fill), (&n.strokes, "strokes", &n.styles.stroke)] {
            if style.is_some() {
                continue;
            }
            for (index, paint) in list.iter().enumerate() {
                let PaintKind::Solid { color } = paint.kind else { continue };
                let field = format!("{key}/{index}/color");
                // Bound under any key of the slot (`fill` names the first fill's colour too).
                if !paint.visible || kition_design_engine::cmd::lists::paint_binding_key(n, key == "fills", index).is_some() {
                    continue;
                }
                let Some((id, name, _)) = self.colors.iter().find(|(_, _, value)| same_color(*value, color)) else { continue };
                let message = format!(
                    "{}'s {} is {}, the value of the colour variable {name:?}, but is not bound to it.",
                    label(n),
                    &key[..key.len() - 1],
                    hex(color)
                );
                let fix = json!({ "ids": [n.id], "field": field, "variable": id });
                self.found.push(Diagnostic::new("design.unbound-color", Severity::Info, message).on(on(n, page)).fix("variable.bind", Some(fix)));
                return;
            }
        }
    }

    /// Off-pixel positions and sizes, and absolute layers in auto layout frames.
    fn placement(&mut self, n: &Node, page: u32) {
        let doc = self.doc;
        let parent = n.parent.and_then(|p| doc.node(p));
        let in_layout = parent.is_some_and(|p| p.layout.is_some() && !p.is_page());
        if in_layout && n.child_layout.absolute {
            self.found.push(
                Diagnostic::new(
                    "design.absolute-in-auto-layout",
                    Severity::Info,
                    format!("{} ignores its auto layout frame's layout (absolute position).", label(n)),
                )
                .on(on(n, page))
                .fix("node.set", Some(json!({ "id": n.id, "props": { "absolute": false } }))),
            );
        }
        let gridded = matches!(
            n.kind,
            NodeKind::Frame
                | NodeKind::Section { .. }
                | NodeKind::Rectangle
                | NodeKind::Ellipse { .. }
                | NodeKind::Text(_)
                | NodeKind::Component { .. }
                | NodeKind::ComponentSet { .. }
                | NodeKind::Instance { .. }
        );
        if !gridded || (in_layout && !n.child_layout.absolute) {
            return;
        }
        let c = n.transform.as_coeffs();
        let unrotated = c[1].abs() < 1e-9 && c[2].abs() < 1e-9 && c[0] > 0.0 && c[3] > 0.0;
        let off = |v: f64| v.is_finite() && (v - v.round()).abs() > OFF_PIXEL;
        // A size the layer does not choose (an auto-sizing text box, a frame that hugs its
        // content) is whatever its content needs.
        let fixed = |sizing: Sizing| sizing == Sizing::Fixed;
        let size_off = match &n.kind {
            NodeKind::Text(t) => !matches!(t.auto_resize, TextAutoResize::WidthAndHeight | TextAutoResize::Height) && (off(n.width) || off(n.height)),
            _ => (fixed(n.child_layout.sizing_h) && off(n.width)) || (fixed(n.child_layout.sizing_v) && off(n.height)),
        };
        if unrotated && (off(c[4]) || off(c[5]) || size_off) {
            let at = |v: f64| format!("{}", (v * 100.0).round() / 100.0);
            self.found.push(
                Diagnostic::new(
                    "design.off-pixel",
                    Severity::Info,
                    format!("{} is at {}, {} and {} x {}, not on whole pixels.", label(n), at(c[4]), at(c[5]), at(n.width), at(n.height)),
                )
                .on(on(n, page))
                .fix("arrange.roundToPixels", Some(json!({ "ids": [n.id] }))),
            );
        }
    }

    /// A visible layer wholly outside the frame that clips it (a frame that does not scroll).
    /// Answers true when it reported one (its children are hidden with it).
    fn outside_clip(&mut self, n: &Node, page: u32) -> bool {
        let doc = self.doc;
        if !n.visible || n.is_mask {
            return false;
        }
        let Some(parent) = n.parent.and_then(|p| doc.node(p)) else { return false };
        if parent.is_page() || !(parent.clip && parent.kind.is_frame_like()) || parent.overflow != proto::Overflow::None {
            return false;
        }
        let (Some(clip), Some(shown)) = (doc.world_bounds(parent.id), doc.world_render_bounds(n.id).or_else(|| doc.world_bounds(n.id))) else {
            return false;
        };
        let overlap = clip.intersect(shown);
        if overlap.width() > 0.0 && overlap.height() > 0.0 {
            return false;
        }
        self.found.push(
            Diagnostic::new(
                "design.outside-clip",
                Severity::Warning,
                format!("{} lies wholly outside {}, which clips its content, so it never shows.", label(n), label(parent)),
            )
            .on(on(n, page))
            .fix("edit.delete", Some(json!({ "ids": [n.id] }))),
        );
        true
    }

    fn text(&mut self, n: &Node, page: u32) {
        let NodeKind::Text(t) = &n.kind else { return };
        let mut missing: Vec<String> = Vec::new();
        for (_, _, style) in t.spans() {
            let family = style.font_family.trim();
            if family.is_empty() || missing.iter().any(|m| m == family) {
                continue;
            }
            let has_font = &mut self.has_font;
            let known = *self.fonts.entry(family.to_lowercase()).or_insert_with(|| has_font(family));
            if !known {
                missing.push(family.to_owned());
            }
        }
        if !missing.is_empty() {
            let names = missing.iter().map(|family| format!("\"{family}\"")).collect::<Vec<_>>().join(", ");
            self.found.push(
                Diagnostic::new(
                    "design.font-substituted",
                    Severity::Info,
                    format!("{} names {names}, which this kit does not have, so it is drawn with a substitute.", label(n)),
                )
                .on(on(n, page)),
            );
        }
        if t.characters.trim().is_empty() || t.path.is_some() || !n.visible {
            return;
        }
        match t.auto_resize {
            TextAutoResize::None => {
                let laid = kition_design_text::layout_cached(t, Some(n.width), Some(n.height));
                let by = laid.height - n.height;
                if by > 0.5 {
                    self.found.push(
                        Diagnostic::new(
                            "design.text-overflow",
                            Severity::Warning,
                            format!("{}'s text is {:.0} px taller than its {:.0} px box, so it runs past the bottom.", label(n), by, n.height),
                        )
                        .on(on(n, page))
                        .fix("node.set", Some(json!({ "id": n.id, "props": { "textAutoResize": "height" } }))),
                    );
                }
            }
            TextAutoResize::Truncate => {
                if kition_design_text::layout_cached(t, Some(n.width), Some(n.height)).truncated {
                    self.found.push(
                        Diagnostic::new("design.text-ellipsis", Severity::Info, format!("{} cuts its text off with an ellipsis.", label(n)))
                            .on(on(n, page))
                            .fix("node.set", Some(json!({ "id": n.id, "props": { "textAutoResize": "height" } }))),
                    );
                }
            }
            TextAutoResize::WidthAndHeight | TextAutoResize::Height => {}
        }
        self.contrast(n, t, page);
    }

    /// Text on a known solid backdrop with less than WCAG AA contrast.
    fn contrast(&mut self, n: &Node, t: &kition_design_doc::TextData, page: u32) {
        let Some(backdrop) = backdrop(self.doc, n, &mut self.under) else { return };
        let mut worst: Option<(f32, f32, Color)> = None;
        for (start, end, style) in t.spans() {
            let shown: String = t.characters.chars().skip(start).take(end.saturating_sub(start)).collect();
            if shown.trim().is_empty() {
                continue;
            }
            let fills = style.fills.as_deref().unwrap_or(&n.fills);
            let Some(color) = top_solid(fills) else { return };
            let color = over(color, backdrop);
            let bold = style.font_style.to_lowercase().contains("bold") || style.variations.get("wght").is_some_and(|w| *w >= 700.0);
            let large = style.font_size >= 24.0 || (bold && style.font_size >= 18.66);
            let needed = if large { AA_LARGE } else { AA_NORMAL };
            let ratio = color.contrast(backdrop);
            if ratio < needed && worst.is_none_or(|(r, _, _)| ratio < r) {
                worst = Some((ratio, needed, color));
            }
        }
        let Some((ratio, needed, color)) = worst else { return };
        let better = if Color::BLACK.contrast(backdrop) >= Color::WHITE.contrast(backdrop) { Color::BLACK } else { Color::WHITE };
        self.found.push(
            Diagnostic::new(
                "design.low-contrast",
                Severity::Warning,
                format!(
                    "{}'s text ({}) on {} contrasts {:.2}:1, under the {:.1}:1 WCAG AA asks for.",
                    label(n),
                    hex(color),
                    hex(backdrop),
                    ratio,
                    needed
                ),
            )
            .on(on(n, page))
            .fix("node.set", Some(json!({ "id": n.id, "props": { "fills": [hex(better)] } }))),
        );
    }

    /// Local styles no layer uses (library copies are the library's).
    fn unused_styles(&mut self) {
        let library: BTreeSet<&str> = self.doc.libraries.styles.iter().map(|record| record.local.as_str()).collect();
        for style in &self.doc.styles {
            if self.used_styles.contains(&style.id) || library.contains(style.id.as_str()) {
                continue;
            }
            let name = if style.name.trim().is_empty() { style.id.clone() } else { style.name.clone() };
            self.found.push(
                Diagnostic::new("design.unused-style", Severity::Info, format!("The style {name:?} is used by no layer."))
                    .on(Target { path: Some(format!("style:{}", style.id)), ..Target::default() })
                    .fix("style.delete", Some(json!({ "style": style.id }))),
            );
        }
    }
}

/// Paints nothing of its own: no visible fill, no stroke, no effect.
fn paints_nothing(n: &Node) -> bool {
    n.fills.iter().all(|p| !p.visible) && n.strokes.iter().all(|p| !p.visible) && n.effects.is_empty()
}

fn same_color(a: Color, b: Color) -> bool {
    (a.r - b.r).abs() <= SAME_COLOR && (a.g - b.g).abs() <= SAME_COLOR && (a.b - b.b).abs() <= SAME_COLOR && (a.a - b.a).abs() <= SAME_COLOR
}

/// The colour a paint list shows when its top visible paint is solid (with its opacity as alpha),
/// else none: a gradient, image or pattern has no one colour.
fn top_solid(paints: &[Paint]) -> Option<Color> {
    let top = paints.iter().rev().find(|p| p.visible && p.opacity > 0.0)?;
    match top.kind {
        PaintKind::Solid { color } => Some(Color { a: (color.a * top.opacity).clamp(0.0, 1.0), ..color }),
        _ => None,
    }
}

/// `top` composited over an opaque `under`.
fn over(top: Color, under: Color) -> Color {
    let a = top.a.clamp(0.0, 1.0);
    let mix = |t: f32, u: f32| t * a + u * (1.0 - a);
    Color { r: mix(top.r, under.r), g: mix(top.g, under.g), b: mix(top.b, under.b), a: 1.0 }
}

/// An opaque solid fill, if that is what `n` shows behind its children.
fn opaque_fill(n: &Node) -> Option<Option<Color>> {
    let top = n.fills.iter().rev().find(|p| p.visible && p.opacity > 0.0);
    match top.map(|p| (&p.kind, p)) {
        None => Some(None),
        Some((PaintKind::Solid { color }, p)) if color.a * p.opacity >= 0.99 => Some(Some(*color)),
        // Translucent, a gradient, an image or a pattern: no single colour.
        Some(_) => None,
    }
}

/// The opaque solid colour right behind text layer `n`, when the check can tell: the nearest
/// layer below it (a sibling covering it, or an ancestor) with an opaque solid fill, else the page
/// background. None when something else may show behind it (an image, a gradient, a translucent
/// fill, a layer partly over it, a faded or blurred ancestor).
/// `under` keeps each parent's [`Under`] once made.
fn backdrop(doc: &Document, n: &Node, under: &mut HashMap<NodeId, Under>) -> Option<Color> {
    let text = doc.world_bounds(n.id)?;
    let mut child = n;
    for _ in 0..1000 {
        let parent = doc.node(child.parent?)?;
        if parent.is_page() {
            return match &parent.kind {
                NodeKind::Page { background, .. } => Some(Color { a: 1.0, ..*background }),
                _ => None,
            };
        }
        let layers = under.entry(parent.id).or_insert_with(|| Under::new(doc, parent));
        if let Some((sibling, bounds)) = layers.nearest_below(child.id, text) {
            let sibling = doc.node(sibling)?;
            let covers = bounds.x0 <= text.x0 && bounds.y0 <= text.y0 && bounds.x1 >= text.x1 && bounds.y1 >= text.y1;
            let solid =
                sibling.opacity >= 0.99 && sibling.effects.is_empty() && covers && matches!(sibling.kind, NodeKind::Rectangle | NodeKind::Frame);
            return match (solid, opaque_fill(sibling)) {
                (true, Some(Some(color))) => Some(color),
                _ => None,
            };
        }
        if parent.opacity < 0.99 || !parent.effects.is_empty() {
            return None;
        }
        match opaque_fill(parent)? {
            Some(color) => return Some(color),
            None => child = parent,
        }
    }
    None
}

type Rect = kition_design_geom::Rect;

/// Cells per side of [`Under`]'s grid.
const UNDER_GRID: usize = 64;

/// One parent's visible, unmasked children with what they paint over, bucketed on a grid over
/// their bounds, so a text layer finds the nearest sibling under it without walking every one.
struct Under {
    children: Vec<NodeId>,
    index: HashMap<NodeId, usize>,
    bounds: Vec<Option<Rect>>,
    /// The grid: its corner and cell size, and each cell's children (indexes, ascending).
    origin: (f64, f64),
    cell: (f64, f64),
    cells: HashMap<(usize, usize), Vec<usize>>,
    /// Children too large for the grid (or with bounds it cannot place), checked everywhere.
    wide: Vec<usize>,
}

impl Under {
    fn new(doc: &Document, parent: &Node) -> Self {
        let children = parent.children.clone();
        let index = children.iter().enumerate().map(|(i, c)| (*c, i)).collect();
        let bounds: Vec<Option<Rect>> = children
            .iter()
            .map(|c| {
                let n = doc.node(*c).filter(|n| n.visible && !n.is_mask)?;
                doc.world_render_bounds(n.id).or_else(|| doc.world_bounds(n.id))
            })
            .collect();
        let finite = |r: &Rect| [r.x0, r.y0, r.x1, r.y1].iter().all(|v| v.is_finite());
        let union = bounds.iter().flatten().filter(|r| finite(r)).copied().reduce(|a, b| a.union(b));
        let (origin, cell) = union.map_or(((0.0, 0.0), (1.0, 1.0)), |u| {
            let side = |v: f64| (v / UNDER_GRID as f64).max(1.0);
            ((u.x0, u.y0), (side(u.width()), side(u.height())))
        });
        let mut under = Self { children, index, bounds, origin, cell, cells: HashMap::new(), wide: Vec::new() };
        for i in 0..under.bounds.len() {
            let Some(r) = under.bounds.get(i).copied().flatten() else { continue };
            match under
                .span(r)
                .filter(|(xs, ys)| finite(&r) && (xs.end() - xs.start() + 1) * (ys.end() - ys.start() + 1) <= UNDER_GRID * UNDER_GRID / 16)
            {
                Some((xs, ys)) => {
                    for x in xs {
                        for y in ys.clone() {
                            under.cells.entry((x, y)).or_default().push(i);
                        }
                    }
                }
                None => under.wide.push(i),
            }
        }
        under
    }

    /// The grid cells `r` touches.
    fn span(&self, r: Rect) -> Option<(std::ops::RangeInclusive<usize>, std::ops::RangeInclusive<usize>)> {
        let at = |v: f64, origin: f64, cell: f64| ((v - origin) / cell).floor().clamp(0.0, (UNDER_GRID - 1) as f64) as usize;
        if ![r.x0, r.y0, r.x1, r.y1].iter().all(|v| v.is_finite()) {
            return None;
        }
        Some((
            at(r.x0, self.origin.0, self.cell.0)..=at(r.x1, self.origin.0, self.cell.0),
            at(r.y0, self.origin.1, self.cell.1)..=at(r.y1, self.origin.1, self.cell.1),
        ))
    }

    /// The nearest child below `child` (stacked under it) whose bounds overlap `text`, with them.
    fn nearest_below(&self, child: NodeId, text: Rect) -> Option<(NodeId, Rect)> {
        let at = self.index.get(&child).copied().unwrap_or(self.children.len());
        let overlaps = |i: usize| {
            self.bounds.get(i).copied().flatten().filter(|b| {
                let overlap = b.intersect(text);
                overlap.width() > 0.0 && overlap.height() > 0.0
            })
        };
        let mut best: Option<usize> = None;
        let mut scan = |list: &[usize]| {
            let end = list.partition_point(|i| *i < at);
            for &i in list.get(..end).unwrap_or_default().iter().rev() {
                if best.is_some_and(|b| i <= b) {
                    break;
                }
                if overlaps(i).is_some() {
                    best = Some(i);
                    break;
                }
            }
        };
        scan(&self.wide);
        match self.span(text) {
            Some((xs, ys)) => {
                for x in xs {
                    for y in ys.clone() {
                        if let Some(list) = self.cells.get(&(x, y)) {
                            scan(list);
                        }
                    }
                }
            }
            // A text layer the grid cannot place: every child, as they stack.
            None => scan(&(0..self.children.len()).collect::<Vec<_>>()),
        }
        let i = best?;
        Some((*self.children.get(i)?, overlaps(i)?))
    }
}

/// The catalogue as `agent/checks.json` holds it: code, severity, summary, and its fixture.
pub fn catalogue_json() -> Value {
    Value::Array(
        CATALOGUE
            .iter()
            .map(|entry| {
                json!({
                    "code": entry.code,
                    "severity": entry.severity,
                    "summary": entry.summary,
                    "fixture": format!("check-{}.kdesign", entry.code.trim_start_matches("design.")),
                })
            })
            .collect(),
    )
}
