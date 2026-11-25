//! What the text family's writes rest on (see [`super`]).
//!
//! The text commands restyle or rewrite whole text layers, or the edited one's selected range
//! while a text is being edited (`style`/`para`/`set_list`, crates/engine/engine/src/cmd/text.rs:16,
//! :47, :77: `ids` is then ignored). The clean-up after them (derived.rs:15 `normalize`) makes the
//! sizing coherent, re-sizes auto-sizing text, keeps text style links, carries an edit inside a
//! main component to its instances (derived.rs:63 `sync_after_edit`) and re-flows what the text
//! is in.

use kition_design_doc::{NodeId, NodeKind, TextAutoResize};

use super::Rest;
use super::layout::{created, settle};

/// The commands that go through `style()` (cmd/text.rs:16) or otherwise detach the bindings an
/// edit changes (`bound_before`/`detach_edited`, cmd/styles.rs:994/1002): they read
/// `bindings.bound`; the paragraph commands (`para`, `set_list`) do not.
const DETACHES: &[&str] = &[
    "format.bold",
    "format.italic",
    "format.underline",
    "format.strikethrough",
    "text.link",
    "format.increaseFontSize",
    "format.decreaseFontSize",
    "text.weightUp",
    "text.weightDown",
    "text.lineHeightUp",
    "text.lineHeightDown",
    "text.trackingUp",
    "text.trackingDown",
    "text.caseOriginal",
    "text.caseUpper",
    "text.caseLower",
    "text.caseTitle",
    "text.caseSmallCaps",
    "text.case",
    "text.decoration",
    "text.setVariations",
    "text.set",
    "text.style",
    "text.fit",
];

/// Edit-mode keys that only move the caret through the laid-out text (canvas/textedit.rs:451-509).
const CARET_KEYS: &[&str] = &["Left", "Right", "Up", "Down", "Home", "End", "SelectAll"];

pub(super) fn rest(rest: &mut Rest<'_>) {
    match rest.command {
        "text.add" => created(rest),
        "text.onPath" => on_path(rest),
        "text.refit" => refit(rest),
        "text.edit.begin" => {
            if let Some(id) = rest.node("id") {
                // begin() clamps the caret to the text's character count.
                rest.only(id.0, &["kind"], false);
            }
        }
        "text.edit.key" => {
            let key = rest.params.get("key").and_then(|k| k.as_str()).unwrap_or_default();
            match edited(rest) {
                Some(id) if CARET_KEYS.contains(&key) => {
                    let mut fields = vec!["kind"];
                    if let Some(text) = rest.doc.node(id).and_then(|node| node.kind.text()).filter(|text| text.path.is_none()) {
                        fields.push("frame.width");
                        // key() discards layout_node's vertical offset. Only truncation uses
                        // box height in the layout whose caret positions it consumes.
                        if text.auto_resize == TextAutoResize::Truncate {
                            fields.push("frame.height");
                        }
                    }
                    rest.only(id.0, &fields, false);
                }
                // Escape ends the edit and removes a layer left empty (canvas/textedit.rs:110-157).
                Some(id) if key == "Escape" => {
                    rest.only(id.0, &["kind"], true);
                    settle(rest, id);
                }
                _ => texts(rest, false),
            }
        }
        command => texts(rest, DETACHES.contains(&command)),
    }
}

/// The text being edited, if any.
fn edited(rest: &Rest<'_>) -> Option<NodeId> {
    rest.design.ds().ok().and_then(|ds| ds.text_edit.as_ref().map(|edit| edit.id)).filter(|id| rest.doc.contains(*id))
}

/// The edited text, else every text in the targets' subtrees (cmd/text.rs:35, :56): which texts
/// are in a container is its children list. `ids` named while editing are not read.
fn texts(rest: &mut Rest<'_>, detaches: bool) {
    let doc = rest.doc;
    let (roots, ignored) = match edited(rest) {
        Some(id) => (vec![id], rest.targets()),
        None => (rest.targets(), Vec::new()),
    };
    for id in ignored.into_iter().filter(|i| !roots.contains(i) && doc.contains(*i)) {
        rest.only(id.0, &[], false);
    }
    let mut found = Vec::new();
    for root in roots.iter().copied().filter(|r| doc.contains(*r)) {
        for d in doc.subtree(root) {
            match doc.node(d) {
                Some(n) if n.kind.text().is_some() => found.push(d),
                Some(n) if !n.children.is_empty() || d == root => rest.only(d.0, &["~hierarchy"], false),
                _ => {}
            }
        }
    }
    held(rest, &found, &["kind", "frame.width", "frame.height", "childLayout", "bindings.styles"], detaches);
}

/// What each changed text rests on: its content and box (auto-size, derived.rs:49-58), its sizing
/// (`coherent_sizing`, derived.rs:43), its text style link (`sync_text_styles`, derived.rs:39), its
/// bindings when the edit detaches them; inside an instance the overrides it records, inside a
/// main the property it is linked to and each instance's copy that follows it
/// (`sync_after_edit`, derived.rs:63); and the re-flow of what it is in.
fn held(rest: &mut Rest<'_>, found: &[NodeId], base: &[&str], detaches: bool) {
    let doc = rest.doc;
    for &t in found {
        let Some(n) = doc.node(t) else { continue };
        let in_layout = n.parent.and_then(|parent| doc.node(parent)).is_some_and(|parent| parent.layout.is_some());
        let mut fields: Vec<&str> = base.iter().copied().filter(|field| in_layout || *field != "childLayout").collect();
        if !in_layout && base.contains(&"childLayout") {
            // coherent_sizing reads these even without a flow; min/max and grid limits
            // are consumed only by a parent's layout.
            fields.extend(["childLayout.sizing_h", "childLayout.sizing_v"]);
        }
        if detaches {
            fields.push("bindings.bound");
        }
        if n.source.is_some() {
            fields.extend(["instance.overrides", "mirrors"]);
        }
        if mains(rest, t).next().is_some() {
            fields.push("instance.prop_refs");
        }
        rest.only(t.0, &fields, false);
    }
    for &t in found {
        let main_ids: Vec<NodeId> = mains(rest, t).collect();
        for main in main_ids {
            rest.need(main.0, &["layout", "props"], false);
            let instances: Vec<NodeId> =
                doc.nodes().filter(|n| matches!(n.kind, NodeKind::Instance { component, .. } if component == main)).map(|n| n.id).collect();
            for instance in instances {
                rest.need(instance.0, &["kind", "instance.overrides"], false);
                for copy in doc.subtree(instance) {
                    if doc.node(copy).is_some_and(|c| c.source == Some(t)) {
                        rest.need(copy.0, &["kind", "instance.overrides", "mirrors", "frame"], false);
                    }
                }
            }
        }
        settle(rest, t);
    }
}

/// The main components `node` is in.
fn mains<'r>(rest: &'r Rest<'_>, node: NodeId) -> impl Iterator<Item = NodeId> + 'r {
    let doc = rest.doc;
    std::iter::once(node).chain(doc.ancestors(node)).filter(move |a| doc.node(*a).is_some_and(|n| matches!(n.kind, NodeKind::Component { .. })))
}

/// `text.refit` (fonts.rs:91): auto-sizing texts under `ids` (every text of the file without)
/// measured again from their content, their box written where it differs; changed boxes reflow
/// their parents. The sorted text membership of each scope is independent of sibling order.
fn refit(rest: &mut Rest<'_>) {
    let doc = rest.doc;
    let named = rest.params.get("ids").is_some_and(|v| !v.is_null());
    let earlier: Vec<_> = rest.needs.values().map(|(id, _, _)| id.clone()).chain(rest.under.objects.iter().cloned()).collect();
    for id in earlier {
        rest.drop(id);
    }
    let found: Vec<NodeId> = if named {
        let mut found = Vec::new();
        for root in rest.targets().into_iter().filter(|r| doc.contains(*r)) {
            if doc.node(root).is_some_and(|node| node.kind.text().is_none()) {
                rest.only(root.0, &["~textNodes"], false);
            }
            for d in doc.subtree(root) {
                if doc.node(d).is_some_and(|node| node.kind.text().is_some()) {
                    found.push(d);
                }
            }
        }
        found
    } else {
        for id in doc.ids().collect::<Vec<_>>() {
            rest.drop(id.0);
        }
        rest.only("design", &["~textNodes"], false);
        doc.nodes().filter(|n| n.kind.text().is_some()).map(|n| n.id).collect()
    };
    for &t in &found {
        rest.only(t.0, &["kind", "frame.width", "frame.height"], false);
    }
    for &t in &found {
        if doc.node(t).is_some_and(|node| {
            kition_design_text::auto_size(node)
                .is_some_and(|(width, height)| (width - node.width).abs() > 1e-6 || (height - node.height).abs() > 1e-6)
        }) {
            settle(rest, t);
        }
    }
}

/// `text.onPath` (cmd/text.rs:132-180): the text (else the first selected text) takes the path's
/// (else the first other selected layer's) place and box; the path's outline comes from its kind,
/// box and corners (`stroke_geometry`), its look (fills, effects, their styles and bindings) moves
/// to the text when shown; the path goes, and the comments pinned to it move. Both places re-flow.
fn on_path(rest: &mut Rest<'_>) {
    let doc = rest.doc;
    let selection = rest.design.session().selection().to_vec();
    let is_text = |id: &NodeId| doc.node(*id).is_some_and(|n| n.kind.text().is_some());
    let text = rest.node("text").or_else(|| selection.iter().copied().find(is_text));
    let path = rest.node("path").or_else(|| selection.iter().copied().find(|id| !is_text(id)));
    const LOOK: [&str; 6] = ["kind", "frame", "fills", "effects", "bindings.styles", "bindings.bound"];
    if let Some(path) = path {
        let vector = doc.node(path).is_some_and(|n| matches!(n.kind, NodeKind::Vector { .. }));
        let mut fields = LOOK.to_vec();
        if !vector {
            fields.push("corners");
        }
        rest.only(path.0, &fields, true);
    }
    if let Some(text) = text {
        rest.only(text.0, &LOOK, true);
    }
    if let Some(text) = text {
        settle(rest, text);
    }
    if let Some(path) = path {
        rest.threads(path);
        settle(rest, path);
    }
}
