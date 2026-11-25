//! What the layout family's writes rest on (see [`super`]).
//!
//! Every changing command ends in the engine's clean-up (`crates/engine/engine/src/derived.rs:15`
//! `normalize`): groups refit to their children (derived.rs:76-100) and auto layout frames re-flow
//! their children (derived.rs:102-128) up the ancestors of what changed. [`settle`] holds what that
//! reads; each command adds what its handler reads before it.

use kition_design_doc::{HConstraint, Node, NodeId, NodeKind, VConstraint};
use kition_kit::Within;
use serde_json::{Value, json};

use super::Rest;

/// What an auto layout re-flow reads of a frame (`crates/engine/layout/src/lib.rs:337`
/// `layout_frame`): its layout, its own sizing and min/max (`childLayout`) and its size; its
/// stroke when the layout counts it in (`insets`, layout/src/lib.rs:220).
fn flow_frame_fields(n: &Node) -> Vec<&'static str> {
    let mut fields = vec!["layout", "childLayout", "frame"];
    if n.layout.as_ref().is_some_and(|l| l.strokes_included) && n.has_visible_stroke() {
        fields.extend(["stroke", "strokes"]);
    }
    fields
}

/// Holds what a re-flow or refit of `parent` reads of `child` (where it is among the children
/// too): an auto layout frame each flowing child's box, visibility and sizing, and a text's
/// content it measures (`fit_hug_text`, layout/src/lib.rs:254); a group each child's box
/// (`fit_to_children`, doc/src/lib.rs:781), and visibility of masks (`box_children`).
fn flow_child(rest: &mut Rest<'_>, parent: &Node, child: NodeId) {
    let Some(c) = rest.doc.node(child) else { return };
    let mut fields = vec!["frame"];
    if parent.layout.is_some() {
        fields.extend(["visible", "childLayout"]);
        if c.kind.text().is_some() {
            fields.push("kind");
        }
    } else if c.is_mask {
        fields.extend(["visible", "props.is_mask"]);
    }
    rest.need(child.0, &fields, true);
}

/// Whether `n` re-computes from its children after an edit inside it: an auto layout frame
/// re-flows, a group or boolean refits (derived.rs:76, derived.rs:102).
fn follows(n: &Node) -> bool {
    !n.is_page() && (n.layout.is_some() || n.kind.bounds_follow_children())
}

/// `parent` re-flows or refits: its own inputs and every child's (derived.rs:91 `fit_to_children`,
/// derived.rs:121 `layout_frame`).
fn flows(rest: &mut Rest<'_>, parent: NodeId, except: Option<NodeId>) {
    let doc = rest.doc;
    let Some(p) = doc.node(parent) else { return };
    if !follows(p) {
        return;
    }
    if p.layout.is_some() {
        rest.need(parent.0, &flow_frame_fields(p), false);
    } else {
        // A group's box is its children's in page space: where it is matters.
        rest.need(parent.0, &["frame"], true);
    }
    for child in p.children.iter().copied().filter(|c| Some(*c) != except) {
        flow_child(rest, p, child);
    }
}

/// The clean-up after `node` changed size, place, visibility or structure: each ancestor that
/// follows its children re-flows or refits, up to the first that does not (derived.rs:76-128).
/// `node`'s place among the children counts in an auto layout flow.
pub(super) fn settle(rest: &mut Rest<'_>, node: NodeId) {
    let doc = rest.doc;
    let mut at = node;
    while let Some(parent) = doc.node(at).and_then(|n| n.parent) {
        let Some(p) = doc.node(parent) else { break };
        if !follows(p) {
            break;
        }
        rest.need(at.0, &[], true);
        flows(rest, parent, Some(at));
        at = parent;
    }
}

/// Where `node` is on its page: each ancestor's frame up to the page, which has no transform of
/// its own (`world_transform`, doc/src/lib.rs:693).
pub(super) fn world(rest: &mut Rest<'_>, node: NodeId) {
    for at in rest.doc.ancestors(node) {
        if rest.doc.node(at).is_some_and(|n| !n.is_page()) {
            rest.need(at.0, &["frame"], true);
        }
    }
}

/// Forgets what the earlier declarations held for `node`'s geometry (its ancestors, and the
/// layers below an ancestor that follows its children): the rules below hold it again.
pub(super) fn clear(rest: &mut Rest<'_>, node: NodeId) {
    let doc = rest.doc;
    rest.drop(node.0);
    for at in std::iter::once(node).chain(doc.ancestors(node)) {
        rest.drop(at.0);
        if doc.node(at).is_some_and(follows) {
            for below in doc.subtree(at) {
                rest.drop(below.0);
            }
        }
    }
}

/// `ids` without those inside another of them (the engine's `roots`).
fn roots(rest: &Rest<'_>, ids: &[NodeId]) -> Vec<NodeId> {
    ids.iter().copied().filter(|i| rest.doc.contains(*i) && !rest.doc.ancestors(*i).iter().any(|a| ids.contains(a))).collect()
}

/// Whether a layer's `kind` is read or written with its size: a vector's network and a text on a
/// path scale with it (`set_size`, layout/src/lib.rs:134), a text's resize mode changes
/// (cmd/arrange.rs:565), a group or section resizes its contents (cmd/props.rs:1244).
fn kind_with_size(n: &Node) -> bool {
    matches!(n.kind, NodeKind::Vector { .. } | NodeKind::Text(_) | NodeKind::Section { .. }) || n.kind.bounds_follow_children()
}

/// A binding on the layer's size the edit detaches (`detach_edited`, cmd/styles.rs:1002).
fn size_bound(n: &Node) -> bool {
    n.bound.keys().any(|k| {
        let k = k.to_ascii_lowercase();
        k.contains("width") || k.contains("height")
    })
}

/// What resizing `node` reads of its contents (`resize_node`, cmd/props.rs:1241): a section
/// keeps its children where they are, a group scales them (`scale_group_children`,
/// layout/src/lib.rs:1005), a frame moves them by their constraints (`apply_constraints`,
/// layout/src/lib.rs:920), recursing into those that change size.
fn contents(rest: &mut Rest<'_>, node: NodeId, depth: usize) {
    let doc = rest.doc;
    let Some(n) = doc.node(node) else { return };
    if depth > 64 {
        return;
    }
    if matches!(n.kind, NodeKind::Section { .. }) {
        for c in n.children.iter().copied() {
            rest.need(c.0, &["frame"], true);
        }
    } else if n.kind.bounds_follow_children() {
        for c in n.children.iter().copied() {
            let Some(cn) = doc.node(c) else { continue };
            let mut fields = vec!["frame"];
            if kind_with_size(cn) {
                fields.push("kind");
            }
            rest.need(c.0, &fields, true);
            if cn.kind.bounds_follow_children() || cn.kind.is_frame_like() {
                contents(rest, c, depth + 1);
            }
        }
    } else {
        let absolute_only = n.layout.is_some();
        for c in n.children.iter().copied() {
            let Some(cn) = doc.node(c) else { continue };
            let mut fields = vec!["constraints", "frame"];
            if absolute_only {
                fields.push("childLayout");
            }
            let resizes = matches!(cn.constraints.horizontal, HConstraint::LeftRight | HConstraint::Scale)
                || matches!(cn.constraints.vertical, VConstraint::TopBottom | VConstraint::Scale);
            if resizes && kind_with_size(cn) {
                fields.push("kind");
            }
            rest.need(c.0, &fields, true);
            if resizes && (cn.kind.is_frame_like() || cn.kind.bounds_follow_children()) && !(absolute_only && !cn.child_layout.absolute) {
                contents(rest, c, depth + 1);
            }
        }
    }
}

/// A frame of `node`'s size re-flows its own children (derived.rs:102: the edited layer's chain
/// starts at itself).
fn own_flow(rest: &mut Rest<'_>, node: NodeId) {
    let doc = rest.doc;
    if let Some(n) = doc.node(node).filter(|n| n.layout.is_some()) {
        for c in n.children.iter().copied() {
            flow_child(rest, n, c);
        }
    }
}

pub(super) fn rest(rest: &mut Rest<'_>) {
    match rest.command {
        "arrange.bringToFront" | "arrange.bringForward" | "arrange.sendBackward" | "arrange.sendToBack" => zorder(rest),
        "align.left"
        | "align.right"
        | "align.top"
        | "align.bottom"
        | "align.hcenter"
        | "align.vcenter"
        | "distribute.horizontal"
        | "distribute.vertical"
        | "arrange.tidy"
        | "arrange.packH"
        | "arrange.packV"
        | "arrange.spacing"
        | "arrange.reorder"
        | "transform.move"
        | "transform.moveTo"
        | "transform.rotate" => moved(rest),
        "transform.scale" => scaled(rest),
        "transform.resize" | "transform.nudgeSize" | "frame.swapOrientation" => resized(rest),
        "arrange.roundToPixels" => rounded(rest),
        "transform.nudge" => nudged(rest),
        "node.add" | "frame.add" => created(rest),
        command if command.starts_with("shape.") => created(rest),
        "node.set" => set(rest),
        "node.reparent" => reparented(rest),
        "node.reorder" => reordered(rest),
        "layout.add" | "layout.suggest" => layout_added(rest),
        "layout.remove" => layout_removed(rest),
        "layout.setTrack" => track(rest),
        "section.toggleContents" => sections(rest),
        command if command.starts_with("guide.") => guides(rest),
        _ => {}
    }
}

/// Layer order (cmd/arrange.rs:306 `zorder`): each layer moves among its siblings, to the top
/// (after the count of its parent's children), one place past the sibling next to it (when that
/// one is not moved too), or to the bottom. A parent that follows its children re-flows.
fn zorder(rest: &mut Rest<'_>) {
    let doc = rest.doc;
    let ids = roots(rest, &rest.targets());
    for &i in &ids {
        rest.only(i.0, &[], true);
    }
    for &i in &ids {
        let Some(parent) = doc.node(i).and_then(|n| n.parent) else { continue };
        let Some(p) = doc.node(parent) else { continue };
        let at = p.children.iter().position(|c| *c == i);
        let next = match rest.command {
            "arrange.bringToFront" => {
                rest.need(parent.0, &["~hierarchy"], false);
                None
            }
            "arrange.bringForward" => at.and_then(|k| p.children.get(k + 1)),
            "arrange.sendBackward" => at.and_then(|k| k.checked_sub(1)).and_then(|k| p.children.get(k)),
            _ => None,
        };
        if let Some(&sibling) = next.filter(|s| !ids.contains(s)) {
            rest.need(sibling.0, &[], true);
        }
        settle(rest, i);
    }
}

/// Moves in page space (cmd/arrange.rs:146 `move_world`, :168 `align`, :199 `distribute`, :227
/// `tidy`, :55 `transform.move`, :64 `moveTo`, :76 `rotate`; arrange.spacing/reorder through
/// `world_bounds`): each layer's box and where it is through its ancestors; one layer aligns to its
/// parent's box. The clean-up re-flows or refits what it is in.
fn moved(rest: &mut Rest<'_>) {
    let doc = rest.doc;
    let ids = roots(rest, &rest.targets());
    for &i in &ids {
        clear(rest, i);
    }
    for &i in &ids {
        let reflows = doc.node(i).and_then(|n| n.parent).and_then(|p| doc.node(p)).is_some_and(follows);
        rest.only(i.0, &[if rest.command == "transform.move" && !reflows { "frame.transform" } else { "frame" }], true);
    }
    for &i in &ids {
        world(rest, i);
        settle(rest, i);
    }
    if rest.command.starts_with("align.")
        && let [one] = ids.as_slice()
        && let Some(parent) = doc.node(*one).and_then(|n| n.parent).filter(|p| doc.node(*p).is_some_and(|n| !n.is_page()))
    {
        rest.need(parent.0, &["frame"], true);
        world(rest, parent);
    }
}

/// What `transform.scale` scales (cmd/arrange.rs:406,519): every layer of each subtree, its box,
/// paints, stroke, corners, effects, sizing, layout, grids and guides, and its kind (text sizes,
/// vector networks).
const SCALED: &[&str] =
    &["frame", "fills", "strokes", "stroke", "corners", "effects", "childLayout", "layout", "kind", "props.grids", "props.guides"];

fn scaled(rest: &mut Rest<'_>) {
    let doc = rest.doc;
    let ids = roots(rest, &rest.targets());
    for &i in &ids {
        clear(rest, i);
        for d in doc.subtree(i) {
            rest.drop(d.0);
        }
    }
    for &i in &ids {
        for d in doc.subtree(i) {
            rest.only(d.0, SCALED, true);
        }
    }
    for &i in &ids {
        world(rest, i);
        settle(rest, i);
    }
}

/// `resize_cmd` (cmd/arrange.rs:541; `nudgeSize` cmd/edit/select.rs:204 and `swapOrientation`
/// cmd/create.rs:90 go through it): the size and local transform (anchor), the sizing it makes
/// fixed, a text's resize mode, a size binding it detaches; the contents it resizes, its own
/// re-flow, and the clean-up of what it is in.
fn resized(rest: &mut Rest<'_>) {
    let doc = rest.doc;
    let ids: Vec<NodeId> = rest.targets().into_iter().filter(|i| doc.contains(*i)).collect();
    for &i in &ids {
        clear(rest, i);
        for d in doc.subtree(i) {
            rest.drop(d.0);
        }
    }
    for &i in &ids {
        let Some(n) = doc.node(i) else { continue };
        let mut fields = vec!["frame", "childLayout"];
        if kind_with_size(n) {
            fields.push("kind");
        }
        if size_bound(n) {
            fields.push("bindings.bound");
        }
        rest.only(i.0, &fields, false);
    }
    for &i in &ids {
        contents(rest, i, 0);
        own_flow(rest, i);
        settle(rest, i);
    }
}

/// `round_px` (cmd/arrange.rs:285): the local translation and size, rounded.
fn rounded(rest: &mut Rest<'_>) {
    let ids: Vec<NodeId> = rest.targets().into_iter().filter(|i| rest.doc.contains(*i)).collect();
    for &i in &ids {
        clear(rest, i);
    }
    for &i in &ids {
        rest.only(i.0, &["frame"], false);
    }
    for &i in &ids {
        settle(rest, i);
    }
}

/// `nudge` (cmd/edit/select.rs:165): a layer that flows in an auto layout frame
/// (`flowing_parent`, canvas/al_keys.rs:52: the parent's layout, its own visibility and
/// sizing) moves one place along the flow among the flowing children (`arrow_move`, :61);
/// another moves its local translation.
fn nudged(rest: &mut Rest<'_>) {
    let doc = rest.doc;
    let ids = roots(rest, &rest.targets());
    for &i in &ids {
        clear(rest, i);
    }
    for &i in &ids {
        let reflows = doc.node(i).and_then(|n| n.parent).and_then(|p| doc.node(p)).is_some_and(follows);
        rest.only(i.0, &[if reflows { "frame" } else { "frame.transform" }, "visible", "childLayout"], true);
    }
    for &i in &ids {
        let Some(parent) = doc.node(i).and_then(|n| n.parent) else { continue };
        let Some(p) = doc.node(parent) else { continue };
        if p.layout.is_some() && p.kind.is_frame_like() {
            rest.need(parent.0, &["~hierarchy"], false);
            flows(rest, parent, Some(i));
        } else if !p.is_page() {
            rest.need(parent.0, &["layout"], false);
        }
        settle(rest, i);
    }
}

/// `node.add` (cmd/create.rs:237; `frame.add` :342, `shape.*` :157): a new layer in
/// `parent` (else the current page), in a group's frame space (`frame_relative`, doc/src/lib.rs:568),
/// at `index` (what is there), then the parent re-flows or refits around it. A preset frame
/// without a place goes beside the right-most top-level layer (`free_spot`, cmd/create.rs:371).
pub(super) fn created(rest: &mut Rest<'_>) {
    let doc = rest.doc;
    let named = rest.node("parent");
    let Some(parent) = named.or_else(|| rest.page()) else { return };
    let Some(p) = doc.node(parent) else { return };
    if named.is_some() {
        clear(rest, parent);
        if p.kind.bounds_follow_children() {
            rest.only(parent.0, &["frame"], true);
        } else if p.layout.is_some() {
            rest.only(parent.0, &flow_frame_fields(p), false);
        } else {
            rest.only(parent.0, &[], false);
        }
    } else {
        rest.need(parent.0, &[], false);
    }
    if p.kind.bounds_follow_children() {
        for at in doc.ancestors(parent) {
            if doc.node(at).is_some_and(|n| n.kind.bounds_follow_children()) {
                rest.need(at.0, &["frame"], true);
            } else {
                break;
            }
        }
    }
    if let Some(index) = rest.params.get("index").and_then(Value::as_u64).and_then(|i| usize::try_from(i).ok()) {
        if let Some(&there) = p.children.get(index) {
            rest.need(there.0, &[], true);
            rest.under.objects.push(json!(there.0));
        }
        rest.under.within.push(Within { parent: json!(parent.0), field: "nodes".into(), from: index, to: index.saturating_add(1), time: None });
    }
    flows(rest, parent, None);
    if follows(p) {
        settle(rest, parent);
    }
    let preset = rest.command == "frame.add" && rest.params.get("preset").is_some_and(|v| !v.is_null());
    let placed = ["x", "y"].iter().any(|k| rest.params.get(k).is_some_and(Value::is_number));
    if preset && !placed && named.is_none() {
        rest.need(parent.0, &["~hierarchy"], false);
        for c in p.children.iter().copied() {
            rest.need(c.0, &["frame"], false);
        }
    }
}

/// Geometry keys of `node.set` (`props::apply`, cmd/props.rs:413-460).
const GEOMETRY: &[&str] = &["x", "y", "rotation", "width", "height", "w", "h"];
const SIZE: &[&str] = &["width", "height", "w", "h"];

/// `node.set` (cmd/edit/props.rs:92 → cmd/props.rs:413): x/y in the frame space of the groups it
/// is in; a size keeps the proportions when they are locked, fixes the sizing, resizes the
/// contents (`resize_node`) and re-flows its own children; then the clean-up of what it is in.
/// Only a call of geometry keys alone is re-declared here; the others keep the earlier ones.
fn set(rest: &mut Rest<'_>) {
    let doc = rest.doc;
    let props = rest.params.get("props").and_then(Value::as_object).or_else(|| rest.params.as_object());
    let Some(props) = props else { return };
    let keys: Vec<&str> = props.keys().map(String::as_str).filter(|k| !matches!(*k, "id" | "ids")).collect();
    let geometry = keys.iter().any(|k| GEOMETRY.contains(k));
    let sized = keys.iter().any(|k| SIZE.contains(k));
    let placed = keys.iter().any(|k| matches!(*k, "x" | "y" | "rotation"));
    if !geometry {
        return;
    }
    let ids: Vec<NodeId> = rest.targets().into_iter().filter(|i| doc.contains(*i)).collect();
    if !keys.iter().all(|k| GEOMETRY.contains(k)) {
        if sized {
            for &i in &ids {
                rest.need(i.0, &["props.constrain_proportions"], false);
            }
        }
        return;
    }
    for &i in &ids {
        clear(rest, i);
        if sized {
            for d in doc.subtree(i) {
                rest.drop(d.0);
            }
        }
    }
    for &i in &ids {
        let Some(n) = doc.node(i) else { continue };
        let reflows = n.parent.and_then(|parent| doc.node(parent)).is_some_and(follows);
        let mut fields = vec![if sized || reflows { "frame" } else { "frame.transform" }];
        if sized {
            fields.extend(["childLayout", "props.constrain_proportions"]);
            if n.layout.is_some() {
                fields.push("layout");
            }
            if kind_with_size(n) {
                fields.push("kind");
            }
            if size_bound(n) {
                fields.push("bindings.bound");
            }
        }
        rest.only(i.0, &fields, placed);
        // A layer inside an instance records what it overrides (the earlier declaration).
        if n.source.is_some() {
            rest.need(i.0, &["instance", "mirrors", "kind"], true);
        }
    }
    for &i in &ids {
        if placed {
            for at in doc.ancestors(i) {
                if doc.node(at).is_some_and(|n| n.kind.bounds_follow_children()) {
                    rest.need(at.0, &["frame"], true);
                } else {
                    break;
                }
            }
        }
        if sized {
            rest.need(i.0, &["~hierarchy"], false);
            contents(rest, i, 0);
            own_flow(rest, i);
        }
        settle(rest, i);
    }
}

/// `node.reparent` (cmd/edit/props.rs:239 → `Document::reparent`, doc/src/lib.rs:599): each layer
/// keeps its place on the page (its and its old ancestors' transforms, the new parent's), goes in
/// at `index` (the window the earlier declaration holds, and what is there now); both parents
/// re-flow or refit.
fn reparented(rest: &mut Rest<'_>) {
    let doc = rest.doc;
    let ids = roots(rest, &rest.targets());
    let parent = rest.node("parent");
    for &i in &ids {
        clear(rest, i);
    }
    if let Some(parent) = parent {
        clear(rest, parent);
    }
    for &i in &ids {
        rest.only(i.0, &["frame.transform"], true);
    }
    if let Some(parent) = parent {
        if doc.node(parent).is_some_and(|n| n.is_page()) {
            rest.only(parent.0, &[], false);
        } else {
            rest.only(parent.0, &["frame.transform"], true);
        }
    }
    for &i in &ids {
        world(rest, i);
        settle(rest, i);
    }
    let Some(parent) = parent else { return };
    if let Some(p) = doc.node(parent).filter(|p| follows(p)) {
        for &i in &ids {
            flow_child(rest, p, i);
        }
    }
    world(rest, parent);
    flows(rest, parent, None);
    if doc.node(parent).is_some_and(follows) {
        settle(rest, parent);
    }
    if let Some(index) = rest.params.get("index").and_then(Value::as_u64).and_then(|i| usize::try_from(i).ok()) {
        let staying: Vec<NodeId> = doc.node(parent).map(|n| n.children.iter().copied().filter(|c| !ids.contains(c)).collect()).unwrap_or_default();
        if let Some(&there) = staying.get(index) {
            rest.need(there.0, &[], true);
        }
    }
}

/// `node.reorder` (cmd/edit.rs:248 → `Document::reorder`, doc/src/lib.rs:683): the layer goes to
/// `index` among its siblings once it is out of the list: what is there (the last one past the
/// end); a parent that follows its children re-flows.
fn reordered(rest: &mut Rest<'_>) {
    let doc = rest.doc;
    let (Some(node), Some(index)) = (rest.node("id"), rest.params.get("index").and_then(Value::as_u64).and_then(|i| usize::try_from(i).ok())) else {
        return;
    };
    let others: Vec<NodeId> = doc
        .node(node)
        .and_then(|n| n.parent)
        .and_then(|p| doc.node(p))
        .map(|p| p.children.iter().copied().filter(|c| *c != node).collect())
        .unwrap_or_default();
    if let Some(&there) = others.get(index).or_else(|| others.last()) {
        rest.need(there.0, &[], true);
    }
    settle(rest, node);
}

/// `layout_add` (cmd/structure.rs:541): frames without auto layout get one inferred from their
/// children's boxes (`infer_layout`, :520: visible, sizing) and sorted along it; other layers are
/// first wrapped in a new frame at the topmost one's place (`wrap`, :161: their page boxes). Whether
/// every target is such a frame (kind, layout) decides which. The frame then re-flows, and what it
/// is in.
fn layout_added(rest: &mut Rest<'_>) {
    let doc = rest.doc;
    let ids: Vec<NodeId> = rest.targets().into_iter().filter(|i| doc.contains(*i)).collect();
    let frames = ids.iter().all(|i| doc.node(*i).is_some_and(|n| n.kind.is_frame_like() && n.layout.is_none()));
    for &i in &ids {
        clear(rest, i);
        for d in doc.subtree(i) {
            rest.drop(d.0);
        }
    }
    if frames {
        for &i in &ids {
            rest.only(i.0, &["kind", "layout", "frame", "childLayout", "~hierarchy"], false);
        }
        for &i in &ids {
            let Some(n) = doc.node(i) else { continue };
            for c in n.children.iter().copied() {
                let text = doc.node(c).is_some_and(|c| c.kind.text().is_some());
                rest.need(c.0, if text { &["frame", "visible", "childLayout", "kind"] } else { &["frame", "visible", "childLayout"] }, true);
            }
            settle(rest, i);
        }
    } else {
        for &i in &ids {
            rest.only(i.0, &["kind", "layout", "frame", "visible", "childLayout"], true);
        }
        for &i in &ids {
            world(rest, i);
            settle(rest, i);
        }
    }
}

/// `layout.remove` (cmd/structure.rs:71): the layout goes and sizing becomes coherent
/// (`coherent_sizing`, layout/src/lib.rs:176): the frame's against its parent's flow, its
/// children's against it, a text's against its resize mode; what it is in re-flows.
fn layout_removed(rest: &mut Rest<'_>) {
    let doc = rest.doc;
    let ids: Vec<NodeId> = rest.targets().into_iter().filter(|i| doc.contains(*i)).collect();
    for &i in &ids {
        clear(rest, i);
        for d in doc.subtree(i) {
            rest.drop(d.0);
        }
    }
    for &i in &ids {
        rest.only(i.0, &["layout", "childLayout", "~hierarchy"], false);
    }
    for &i in &ids {
        let Some(n) = doc.node(i) else { continue };
        if let Some(parent) = n.parent.filter(|p| doc.node(*p).is_some_and(|p| p.layout.is_some())) {
            rest.need(parent.0, &["layout", "childLayout"], false);
        }
        for c in n.children.iter().copied() {
            let text = doc.node(c).is_some_and(|c| c.kind.text().is_some());
            rest.need(c.0, if text { &["childLayout", "kind"] } else { &["childLayout"] }, true);
        }
        settle(rest, i);
    }
}

/// `layout.setTrack` (cmd/layout.rs:25): the grid's tracks; a fixed track without a value keeps
/// the size the track has now (`grid_geometry`: the children's boxes). The grid re-flows.
fn track(rest: &mut Rest<'_>) {
    let doc = rest.doc;
    let ids: Vec<NodeId> = rest.targets().into_iter().filter(|i| doc.contains(*i)).collect();
    let measured = rest.params.get("sizing").and_then(Value::as_str).is_some_and(|s| s.eq_ignore_ascii_case("fixed"))
        && rest.params.get("value").is_none_or(Value::is_null);
    for &i in &ids {
        clear(rest, i);
        for d in doc.subtree(i) {
            rest.drop(d.0);
        }
    }
    for &i in &ids {
        rest.only(i.0, if measured { &["layout", "frame", "~hierarchy"] } else { &["layout", "frame"] }, false);
    }
    for &i in &ids {
        own_flow(rest, i);
        settle(rest, i);
    }
}

/// `section.toggleContents` (cmd/section.rs:55): each section's `contents_hidden` (in its kind);
/// other layers are skipped.
fn sections(rest: &mut Rest<'_>) {
    let doc = rest.doc;
    for i in rest.targets() {
        let section = doc.node(i).is_some_and(|n| matches!(n.kind, NodeKind::Section { .. }));
        rest.only(i.0, if section { &["kind"] } else { &[] }, false);
    }
}

/// Ruler guides (cmd/page.rs:159-226): the owner's guide list, a page's in its kind, a frame's in
/// its own field; `frame`, else `page`, else the current page (`guide_owner`); `guide.remove`
/// without an index removes the selected guide of whichever owner it is on.
fn guides(rest: &mut Rest<'_>) {
    let doc = rest.doc;
    let owner = if rest.command == "guide.remove" && rest.params.get("index").is_none_or(Value::is_null) {
        rest.design.ds().ok().and_then(|ds| ds.selected_guide()).map(|(owner, _)| owner)
    } else {
        rest.node("frame").or_else(|| rest.node("page")).or_else(|| rest.page())
    };
    let Some(owner) = owner else { return };
    let field = if doc.node(owner).is_some_and(Node::is_page) { "kind" } else { "props.guides" };
    rest.only(owner.0, &[field], false);
}
