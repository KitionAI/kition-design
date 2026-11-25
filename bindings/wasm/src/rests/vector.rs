//! What the vector family's writes rest on (see [`super`]).
//!
//! These handlers read the named layers' geometry and look, never their bindings: a flattened,
//! outlined, combined or edited layer takes the values its fields hold, so the variables and
//! styles the earlier declarations reached through the layers' bindings are not held here. A
//! page has no frame of its own (it never moves), so a layer's place on the page is only its
//! own frame and its non-page ancestors' frames.

use std::collections::BTreeSet;

use kition_design_doc::{NodeId, NodeKind};
use kition_design_engine::cmd::edit::{roots, z_sorted};
use kition_design_engine::shapebuilder::Part;
use serde_json::Value;

use super::Rest;

/// The look a flattened or merged layer passes to the new vector (`cmd/structure.rs:429-445`,
/// `canvas/shapebuilder.rs:286-300`).
const LOOK: [&str; 7] = ["name", "fills", "strokes", "stroke", "effects", "opacity", "props.blend"];

pub(super) fn rest(rest: &mut Rest<'_>) {
    let command = rest.command;
    let edited = edited(rest);
    match command {
        "vector.flatten" | "bool.flatten" => {
            let targets = rest.targets();
            flatten(rest, &targets);
        }
        "vector.join" | "vector.smoothJoin" | "vector.deleteHeal" if edited.is_some() => {
            let edited = edited.unwrap_or_default();
            edit_mode(rest, &edited, &["kind", "frame"], true);
        }
        "vector.join" | "vector.smoothJoin" => {
            let targets = rest.targets();
            if targets.len() >= 2 {
                // Two or more layers are flattened into one (`cmd/structure.rs:706`).
                flatten(rest, &targets);
            } else {
                // One open vector is closed: its network only (`cmd/structure.rs:689-703`).
                only_layers(rest, &targets, &["kind"], false);
            }
        }
        "vector.simplify" | "vector.reverse" | "vector.fillRule" | "vector.reverseLoop" => {
            // The network is read and replaced, the box kept (`cmd/structure.rs:105,634,802,855`).
            let targets = rest.targets();
            only_layers(rest, &targets, &["kind"], false);
        }
        "vector.offset" => {
            // The outline (kind, size, corners) grown into a network; the box moves and resizes,
            // so an auto layout parent or group re-flows (`cmd/structure.rs:654`, `derived.rs:79-130`).
            let targets = rest.targets();
            reset(rest);
            for node in targets {
                rest.only(node.0, &["kind", "frame", "corners"], false);
                rest.reflow(node);
            }
        }
        "vector.outlineStroke" => {
            let targets = rest.targets();
            outline_stroke(rest, &targets);
        }
        "bool.union" | "bool.subtract" | "bool.intersect" | "bool.exclude" => {
            let targets = rest.targets();
            boolean(rest, &targets, command == "bool.subtract");
        }
        "vector.add" => create(rest),
        "vector.edit.begin" => {
            let targets = rest.targets();
            begin(rest, &targets);
        }
        "shapeBuilder.merge" | "shapeBuilder.remove" => {
            let targets = rest.targets();
            shape_builder(rest, &targets, command == "shapeBuilder.merge");
        }
        _ if command.starts_with("vector.edit.") => {
            if let Some(edited) = edited {
                edit_command(rest, &edited);
            }
        }
        _ => {}
    }
}

/// The layers being edited in vector edit mode (the active one first), if any.
fn edited(rest: &Rest<'_>) -> Option<Vec<NodeId>> {
    let ds = rest.design.ds().ok()?;
    let edit = ds.vector_edit.as_ref()?;
    Some(std::iter::once(edit.id).chain(edit.others.iter().map(|part| part.id)).collect())
}

/// Forget what the earlier declarations held: the rules below say all these handlers read.
fn reset(rest: &mut Rest<'_>) {
    let ids: Vec<Value> = rest.needs.values().map(|(id, _, _)| id.clone()).chain(rest.under.objects.iter().cloned()).collect();
    for id in ids {
        rest.drop(id);
    }
}

/// Exactly `fields` of each of `layers`.
fn only_layers(rest: &mut Rest<'_>, layers: &[NodeId], fields: &[&str], place: bool) {
    reset(rest);
    for node in layers {
        rest.only(node.0, fields, place);
    }
}

/// Whether a layer's outline follows its corner radii (`doc/src/geometry.rs:8-24`).
fn has_corners(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Rectangle
            | NodeKind::Frame
            | NodeKind::Component { .. }
            | NodeKind::ComponentSet { .. }
            | NodeKind::Instance { .. }
            | NodeKind::Section { .. }
            | NodeKind::Polygon { .. }
            | NodeKind::Star { .. }
    )
}

/// What a layer's outline is drawn from: its kind (the network, the text), its size and, for
/// rounded shapes, its corners (`render/src/draw.rs:300-335`, `doc/src/geometry.rs:8-36`).
fn outline(rest: &mut Rest<'_>, node: NodeId) {
    let corners = rest.doc.node(node).is_some_and(|n| has_corners(&n.kind));
    rest.need(node.0, &["kind", "frame"], false);
    if corners {
        rest.need(node.0, &["corners"], false);
    }
}

/// Where `node` is on its page: its own frame and its non-page ancestors' frames (a page never
/// moves).
fn world(rest: &mut Rest<'_>, node: NodeId) {
    let doc = rest.doc;
    for at in std::iter::once(node).chain(doc.ancestors(node)) {
        if doc.node(at).is_some_and(|n| !n.is_page()) {
            // The transform by name too, as the earlier declarations hold it (`place.rs` world).
            rest.merge(serde_json::json!(at.0), Some(["frame.transform".to_owned()].into()), Some(["frame".to_owned()].into()));
        }
    }
}

/// A new layer goes into `parent` at an index next to `top`: the parent's frame (the layer's
/// place in it goes through its world transform) unless it is a page, and what an auto layout
/// parent or a group re-flows (`cmd/structure.rs:163-176`, `derived.rs:79-130`).
fn into_parent_of(rest: &mut Rest<'_>, top: NodeId) {
    let Some(parent) = rest.doc.node(top).and_then(|n| n.parent) else { return };
    if rest.doc.node(parent).is_some_and(|n| !n.is_page()) {
        world(rest, parent);
        rest.need(parent.0, &["layout"], false);
    }
    rest.reflow(top);
}

/// `vector.flatten` (also `bool.flatten`, and `vector.join` of two or more layers): every root
/// target's outline in its parent's space and its place (removed), a group's visible children's
/// outlines, the bottom layer's look (the new vector's), the top layer's parent and index (where
/// it goes), the comment threads pinned to removed layers (`cmd/structure.rs:383-451`,
/// `cmd/comment.rs:192`).
fn flatten(rest: &mut Rest<'_>, targets: &[NodeId]) {
    let doc = rest.doc;
    let ids = z_sorted(doc, &roots(doc, targets));
    reset(rest);
    for node in &ids {
        outline(rest, *node);
        rest.need(node.0, &["~hierarchy"], true);
        world(rest, *node);
        for layer in doc.subtree(*node) {
            rest.threads(layer);
            if layer != *node {
                // Groups and frames flatten to their visible layers' outlines (`render/src/draw.rs:338`).
                outline(rest, layer);
                rest.need(layer.0, &["visible"], false);
            }
        }
    }
    if let Some(bottom) = ids.first() {
        rest.need(bottom.0, &LOOK, false);
    }
    if let Some(top) = ids.last() {
        into_parent_of(rest, *top);
    }
}

/// `vector.outlineStroke`: each layer with a visible stroke; its stroke area (kind, size,
/// corners, strokes, stroke) filled with its strokes and their style. A closed layer with a
/// visible fill keeps its fill and is grouped with the outline (its fills read); otherwise the
/// outline stands in for it (how it is drawn and placed) and it is removed
/// (`cmd/structure.rs:455-521`).
fn outline_stroke(rest: &mut Rest<'_>, targets: &[NodeId]) {
    let doc = rest.doc;
    reset(rest);
    for &node in targets {
        let Some(n) = doc.node(node) else { continue };
        outline(rest, node);
        rest.need(node.0, &["strokes", "stroke", "name", "bindings.styles", "~hierarchy"], true);
        if !n.has_visible_stroke() {
            continue;
        }
        let closed = kition_design_doc::geometry::is_closed(n);
        if closed {
            rest.need(node.0, &["fills"], false);
        }
        if !(closed && n.fills.iter().any(|f| f.visible)) {
            rest.need(node.0, &["opacity", "props.blend", "effects", "constraints", "childLayout", "visible", "locked"], false);
            rest.threads(node);
        } else {
            world(rest, node);
        }
        into_parent_of(rest, node);
    }
}

/// `bool.*`: one boolean group named alone gets the operation (its kind, its default name, its
/// operands' own operations); operands all inside boolean groups get their own operation (their
/// group's kind); otherwise the layers are wrapped in a new boolean group at the top one's
/// place, sized from their world bounds and looking like the top one (the bottom one for
/// subtract) (`canvas/booleans.rs:96-130`, `cmd/structure.rs:161-221`).
fn boolean(rest: &mut Rest<'_>, targets: &[NodeId], subtract: bool) {
    let doc = rest.doc;
    reset(rest);
    let is_group = |id: NodeId| doc.node(id).is_some_and(|n| matches!(n.kind, NodeKind::BooleanOp { .. }));
    let group_of = |id: NodeId| doc.node(id).and_then(|n| n.parent).filter(|p| is_group(*p));
    if let [one] = targets
        && is_group(*one)
    {
        rest.only(one.0, &["kind", "name"], false);
        for child in doc.node(*one).map(|n| n.children.clone()).unwrap_or_default() {
            rest.only(child.0, &["props.bool_op"], false);
        }
        return;
    }
    if targets.iter().all(|id| group_of(*id).is_some()) {
        for &node in targets {
            rest.only(node.0, &["props.bool_op", "~hierarchy"], true);
            if let Some(group) = group_of(node) {
                rest.need(group.0, &["kind"], false);
            }
        }
        return;
    }
    let ids = z_sorted(doc, &roots(doc, targets));
    for &node in &ids {
        rest.need(node.0, &["frame", "props.bool_op", "~hierarchy"], true);
        world(rest, node);
    }
    if let Some(source) = if subtract { ids.first() } else { ids.last() } {
        rest.need(source.0, &["fills", "strokes", "stroke", "effects"], false);
    }
    if let Some(top) = ids.last() {
        into_parent_of(rest, *top);
    }
}

/// `vector.add`: on the current page, or in `parent` (re-flowed when it has auto layout, its
/// transform chain when it is a group) (`cmd/create.rs:283-332`, `derived.rs:104`).
fn create(rest: &mut Rest<'_>) {
    reset(rest);
    let Some(parent) = rest.node("parent").or_else(|| rest.page()) else { return };
    rest.need(parent.0, &[], false);
    let doc = rest.doc;
    let Some(p) = doc.node(parent) else { return };
    if p.is_page() {
        return;
    }
    if p.kind.bounds_follow_children() {
        world(rest, parent);
    }
    if p.layout.is_some() {
        rest.need(parent.0, &["layout", "frame", "childLayout"], false);
        for child in p.children.iter().copied() {
            rest.need(child.0, &["frame", "visible", "childLayout"], true);
        }
    }
    // A parent that grows re-flows its own auto layout parent or refits its group.
    rest.reflow(parent);
}

/// `vector.edit.begin`: a vector only enters edit mode (its kind decides); any other shape is
/// converted from its outline into a network, its corners reset and its box refitted
/// (`canvas/vectoredit.rs:237,507`).
fn begin(rest: &mut Rest<'_>, targets: &[NodeId]) {
    let doc = rest.doc;
    reset(rest);
    for &node in targets {
        let Some(n) = doc.node(node) else { continue };
        if matches!(n.kind, NodeKind::Vector { .. }) {
            rest.only(node.0, &["kind"], false);
            continue;
        }
        outline(rest, node);
        // A boolean group (or any layer with layers) is converted from its operands' outlines,
        // which the conversion consumes (`canvas/vectoredit.rs:237`).
        for layer in doc.subtree(node).into_iter().filter(|layer| *layer != node) {
            rest.whole(layer.0, true);
        }
        if matches!(n.kind, NodeKind::Line) {
            rest.need(node.0, &["stroke"], false);
        }
        rest.reflow(node);
    }
}

/// Vector edit mode addresses points, segments, handles, regions and width points by index in
/// the edited layers' networks (their kind). Edits that move the network refit the box (frame)
/// and re-flow the parent; page-space parameters go through the world transform
/// (`canvas/vectoredit.rs:779-1221`, `canvas/vectoredit/transform.rs:209-237`).
fn edit_mode(rest: &mut Rest<'_>, edited: &[NodeId], fields: &[&str], refit: bool) {
    reset(rest);
    for &node in edited {
        rest.only(node.0, fields, false);
        if refit {
            world(rest, node);
            rest.reflow(node);
        }
    }
}

fn edit_command(rest: &mut Rest<'_>, edited: &[NodeId]) {
    let command = rest.command;
    let cut_across = command == "vector.edit.cut" && rest.params.get("from").is_some_and(|v| !v.is_null());
    match command {
        // The part that comes apart goes to a full copy of the layer above it
        // (`canvas/vectoredit.rs:1186`: duplicate_into).
        _ if cut_across => {
            reset(rest);
            for &node in edited {
                rest.whole(node.0, true);
                world(rest, node);
                rest.reflow(node);
            }
        }
        // The stroke's width profile is read and replaced (`canvas/vectoredit/width.rs:187`).
        "vector.edit.widthPoint" => edit_mode(rest, edited, &["stroke"], false),
        // Edits of the network that never refit the box (`canvas/vectoredit.rs:968,998,1024-1046,
        // 1070,1126`); `end` reads whether a layer lost every segment and then removes it (its
        // pinned comments re-anchored, its parent re-flowed) (`canvas/vectoredit.rs:550-560`).
        "vector.edit.mirroring"
        | "vector.edit.addVertex"
        | "vector.edit.cornerRadius"
        | "vector.edit.strokeCap"
        | "vector.edit.strokeJoin"
        | "vector.edit.connect"
        | "vector.edit.cut" => edit_mode(rest, edited, &["kind"], false),
        "vector.edit.end" => {
            edit_mode(rest, edited, &["kind"], false);
            cleanup(rest, edited);
        }
        // Painting a region fills it with the layer's fills, or a default when it has no visible
        // fill (`canvas/vectoredit.rs:1221`).
        "vector.edit.paint" => {
            edit_mode(rest, edited, &["kind", "frame"], true);
            let doc = rest.doc;
            for &node in edited {
                if doc.node(node).is_some_and(|n| !n.fills.iter().any(|f| f.visible)) {
                    rest.need(node.0, &["fills"], false);
                }
            }
        }
        _ => edit_mode(rest, edited, &["kind", "frame"], true),
    }
}

/// Ending an edit only tests the network for emptiness. Removal also re-flows its parent and
/// re-anchors comments; the removed layer's appearance and sibling order are immaterial.
pub(super) fn cleanup(rest: &mut Rest<'_>, edited: &[NodeId]) {
    for &node in edited {
        rest.only(node.0, &["kind"], false);
        let empty = rest.doc.node(node).is_some_and(|n| matches!(&n.kind, NodeKind::Vector { network } if network.segments.is_empty()));
        if empty {
            if rest.doc.comments.iter().any(|c| c.anchor == Some(node)) {
                rest.need(node.0, &["frame"], false);
                world(rest, node);
                rest.threads(node);
            }
            rest.reflow(node);
        }
    }
}

/// `shapeBuilder.*`: the arrangement of the named layers' shapes (groups expanded; visible,
/// unlocked shapes outside instances and boolean groups): each shape's fill area (kind, size,
/// corners) and stroke area (strokes, stroke) in page space. Picked regions are taken from the
/// shapes they cover (reshaped or removed, comment pins re-anchored, strokes outlined); a merge
/// makes a vector styled like the top shape of the first region, at its parent and index (an
/// auto layout parent keeps it absolute). The region a point picks is one of these shapes'
/// regions: no other layer under the point is read (`canvas/shapebuilder.rs:87-130,200-360`).
fn shape_builder(rest: &mut Rest<'_>, targets: &[NodeId], merge: bool) {
    let doc = rest.doc;
    let shapes = kition_design_engine::shapebuilder::eligible(doc, targets);
    let styled = styled_parts(rest, targets, merge);
    reset(rest);
    let mut groups: Vec<_> = roots(doc, targets).into_iter().map(|node| (node, 0)).collect();
    while let Some((node, depth)) = groups.pop() {
        let Some(n) = doc.node(node).filter(|n| matches!(n.kind, NodeKind::Group)) else { continue };
        rest.need(node.0, &["kind"], true);
        if depth < 64 {
            // eligible() expands these exact children. Equal-bounds replacements still
            // change the arrangement and its first picked source.
            rest.need(node.0, &["~children"], false);
            groups.extend(n.children.iter().copied().map(|child| (child, depth + 1)));
        }
    }
    for &node in &shapes {
        outline(rest, node);
        rest.need(node.0, &["visible", "locked", "strokes", "stroke", "~hierarchy"], true);
        world(rest, node);
        for ancestor in doc.ancestors(node) {
            if doc.node(ancestor).is_some_and(|n| !n.is_page()) {
                rest.need(ancestor.0, &["kind"], false);
            }
        }
        rest.threads(node);
        if let Some(parent) = doc.node(node).and_then(|n| n.parent).filter(|p| doc.node(*p).is_some_and(|n| !n.is_page())) {
            rest.need(parent.0, &["layout"], false);
        }
        rest.reflow(node);
    }
    match styled {
        Some(parts) => {
            for part in parts {
                let fields: Vec<_> = LOOK.iter().copied().filter(|field| !part.stroke || *field != "fills").collect();
                rest.need(part.id.0, &fields, false);
            }
        }
        None => {
            // Invalid geometry cannot be planned precisely; keep every possible source's look.
            for node in shapes {
                rest.need(node.0, &LOOK, false);
            }
        }
    }
}

/// The merge uses the top part of its first picked region. Every touched stroke with unpicked
/// regions also produces an outlined remainder, styled from that stroke's source, for merge
/// and remove alike. Use the engine's arrangement for both picks and remainder membership.
fn styled_parts(rest: &Rest<'_>, targets: &[NodeId], merge: bool) -> Option<BTreeSet<Part>> {
    use kition_design_engine::params::{f64_of, get};

    let mut scratch = kition_design_engine::Session::empty();
    scratch.docs.push(rest.design.ds().ok()?.clone());
    let arrangement = crate::no_files(|| kition_design_engine::shapebuilder::arrangement(&mut scratch, targets)).ok()?;
    let mut picked = Vec::new();
    let mut add = |index| {
        if index < arrangement.regions.len() && !picked.contains(&index) {
            picked.push(index);
        }
    };
    if let Some(Value::Array(indices)) = get(rest.params, "regions") {
        for index in indices.iter().filter_map(f64_of).filter(|index| *index >= 0.0 && index.fract() == 0.0 && *index < 1e9) {
            add(index as usize);
        }
    }
    if let Some(Value::Array(points)) = get(rest.params, "points") {
        for point in points.iter().take(10_000) {
            if let (Some(x), Some(y)) = (point.get(0).and_then(f64_of), point.get(1).and_then(f64_of))
                && let Some(index) = arrangement.region_at(kition_design_geom::Point::new(x, y))
            {
                add(index);
            }
        }
    }
    let touched: BTreeSet<Part> =
        picked.iter().filter_map(|index| arrangement.regions.get(*index)).flat_map(|region| region.parts.iter().copied()).collect();
    let touched_shapes: BTreeSet<NodeId> = touched.iter().map(|part| part.id).collect();
    let whole_shape = touched_shapes.len() == 1
        && arrangement
            .regions
            .iter()
            .enumerate()
            .all(|(index, region)| picked.contains(&index) || !region.sources.iter().any(|source| touched_shapes.contains(source)));
    let mut styled = BTreeSet::new();
    if merge && whole_shape {
        return Some(styled);
    }
    if merge && let Some(part) = picked.first().and_then(|index| arrangement.regions.get(*index)).and_then(|region| region.parts.first()) {
        styled.insert(*part);
    }
    for part in touched.iter().filter(|part| part.stroke) {
        if arrangement.regions.iter().enumerate().any(|(index, region)| !picked.contains(&index) && region.parts.contains(part)) {
            styled.insert(*part);
        }
    }
    Some(styled)
}
