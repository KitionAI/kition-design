//! What the edit family's writes rest on (see [`super`]): `edit.*`, `object.*`, `command.*`,
//! `data.*`, `page.*`, `file.*`, `branch.*`, `version.*`, `comment.*`, `board.*` and `tool.*`.
//!
//! Engine paths below are under `crates/engine/engine/src/`. After every edit the engine's
//! clean-up (`derived.rs:15` normalize) re-flows auto layout parents of what changed size, place
//! or visibility (`derived.rs:101`), refits groups (`derived.rs:73`), and moves the comment pins of
//! layers that left their page or were removed (`cmd/comment.rs:192`).

use std::collections::BTreeSet;

use kition_design_doc::{BoardItem, Document, NodeId, NodeKind, VoteScope};
use kition_kit::{Covers, Within};
use serde_json::{Value, json};

use super::{Rest, node_id};

/// What a pick of the container under a point reads of every layer (`cmd/comment.rs:602`
/// frame_at: visible containers, their world box, topmost first).
const PICKS: &[&str] = &["frame", "kind", "visible", "~hierarchy"];

/// The fields `object.convertToGroup` replaces or reads on the converted frame
/// (`cmd/structure.rs:265-283`).
const TO_GROUP: &[&str] = &[
    "kind",
    "layout",
    "fills",
    "strokes",
    "corners",
    "props.clip",
    "props.grids",
    "props.guides",
    "prototype.overflow",
    "bindings.styles",
    "bindings.bound",
    "frame",
    "~hierarchy",
];

/// The fields `object.convertToSection` replaces or reads (`cmd/structure.rs:288`).
const TO_SECTION: &[&str] =
    &["kind", "layout", "props.clip", "prototype.overflow", "props.grids", "props.guides", "bindings.styles", "bindings.bound"];

/// The fields `edit.pasteProperties` writes (`cmd/edit/clipboard.rs:315`, `cmd/props.rs:413`).
const PASTED: &[&str] = &["kind", "fills", "strokes", "stroke", "effects", "opacity", "props.blend", "bindings.styles", "bindings.bound"];

/// The colour-carrying fields a colour profile conversion rewrites on a layer
/// (`doc/src/colors.rs:53-124`).
const COLOURED: &[&str] = &["fills", "strokes", "effects", "props.grids", "prototype.interactions"];

pub(super) fn rest(rest: &mut Rest<'_>) {
    let command = rest.command;
    match command {
        "edit.undo" | "edit.redo" => history(rest, command == "edit.undo"),
        "edit.repeat" => repeat(rest),
        "edit.delete" => {
            let targets = rest.targets();
            removed(rest, &targets);
        }
        "edit.cut" => cut(rest),
        "edit.paste" | "edit.pasteDesignLayers" | "edit.pasteOver" | "edit.pasteReplace" => paste(rest),
        "edit.duplicate" => duplicate(rest),
        "edit.findReplace" => find_replace(rest),
        "edit.pasteProperties" => paste_properties(rest),
        "edit.pickColor" => tool_switch(rest),
        "tool.place" => {
            tool_place(rest);
            tool_switch(rest);
        }
        _ if command.starts_with("tool.") => tool_switch(rest),
        _ if command.starts_with("object.") => object(rest),
        _ if command.starts_with("page.") => page(rest),
        _ if command.starts_with("comment.") => comment(rest),
        _ if command.starts_with("file.") || command.starts_with("version.") || command.starts_with("branch.") => file(rest),
        _ if command.starts_with("board.") => board(rest),
        _ => {}
    }
    // Both sampled board handlers call board::require before doing any work. File
    // kind is persisted and can change in an independent native-file writer.
    if matches!(command, "board.label" | "tool.stamp") {
        rest.root(&["settings.kind"]);
    }
}

// ---- vocabulary of this family ----

fn is_page(doc: &Document, node: NodeId) -> bool {
    doc.node(node).is_some_and(|n| n.is_page())
}

/// Whether `node` is a main component's layer or holds one: an edit there is followed by every
/// instance (`derived.rs:63` sync_after_edit), so the component graph the earlier declarations
/// hold stays.
fn graph(doc: &Document, node: NodeId) -> bool {
    let main = |id: &NodeId| doc.node(*id).is_some_and(|n| matches!(n.kind, NodeKind::Component { .. } | NodeKind::ComponentSet { .. }));
    std::iter::once(node).chain(doc.ancestors(node)).any(|a| main(&a)) || doc.subtree(node).iter().any(main)
}

/// Forgets what the earlier declarations held (they held the targets whole and their component
/// graph, which these handlers do not read), unless a target is in a component graph. Answers
/// whether it did.
fn clear(rest: &mut Rest<'_>, targets: &[NodeId]) -> bool {
    if targets.iter().any(|t| graph(rest.doc, *t)) {
        drop_pages(rest);
        return false;
    }
    let ids: Vec<Value> = rest.needs.values().map(|(id, _, _)| id.clone()).collect();
    for id in ids {
        rest.drop(id);
    }
    true
}

/// Pages are held only as the parent a call adds to: their frame is not read (world transforms
/// start at the page, `doc/src/lib.rs:721`).
fn drop_pages(rest: &mut Rest<'_>) {
    for page in rest.doc.pages.clone() {
        rest.drop(page.0);
    }
}

/// What the call rests on of `id`: exactly `fields` when the earlier holds were cleared, else
/// added to them.
fn set(rest: &mut Rest<'_>, narrowed: bool, id: NodeId, fields: &[&str], place: bool) {
    if narrowed {
        rest.only(id.0, fields, place);
    } else {
        rest.need(id.0, fields, place);
    }
}

/// `fields` of `node` and where it is on its page: its place in the tree and each ancestor's
/// frame (`doc/src/lib.rs:721` world_bounds).
fn placed(rest: &mut Rest<'_>, node: NodeId, fields: &[&str]) {
    rest.need(node.0, fields, true);
    for ancestor in rest.doc.ancestors(node).into_iter().filter(|a| !is_page(rest.doc, *a)) {
        rest.need(ancestor.0, &["frame"], false);
    }
}

/// Whether `node`'s parent re-computes from it: an auto layout frame or a group.
fn in_flow(doc: &Document, node: NodeId) -> bool {
    doc.node(node)
        .and_then(|n| n.parent)
        .and_then(|p| doc.node(p))
        .is_some_and(|p| !p.is_page() && (p.layout.is_some() || p.kind.bounds_follow_children()))
}

/// What the clean-up re-computes when `node` changes size, visibility or place: an auto layout
/// parent's layout and the siblings it re-flows (`derived.rs:101`), a group's box from its
/// children (`derived.rs:73`), up the ancestors that follow their children.
fn flow(rest: &mut Rest<'_>, node: NodeId) {
    let doc = rest.doc;
    let mut at = node;
    while let Some(parent) = doc.node(at).and_then(|n| n.parent) {
        let Some(p) = doc.node(parent) else { break };
        if p.is_page() {
            break;
        }
        let siblings: &[&str] = if p.layout.is_some() {
            rest.need(parent.0, &["frame", "layout", "childLayout"], false);
            &["frame", "visible", "childLayout"]
        } else if p.kind.bounds_follow_children() {
            rest.need(parent.0, &["frame", "kind"], false);
            &["frame", "visible"]
        } else {
            break;
        };
        for sibling in p.children.iter().copied().filter(|c| *c != at) {
            rest.need(sibling.0, siblings, true);
        }
        at = parent;
    }
}

/// The comment threads pinned in `node`'s subtree: the clean-up re-pins them from the layer's
/// world transform when it is removed or leaves its page (`cmd/comment.rs:192-210`).
fn pins(rest: &mut Rest<'_>, node: NodeId) {
    let pinned: Vec<NodeId> = rest.doc.subtree(node).into_iter().filter(|n| rest.doc.comments.iter().any(|c| c.anchor == Some(*n))).collect();
    for layer in pinned {
        placed(rest, layer, &["frame.transform"]);
        rest.threads(layer);
    }
}

/// The layers a write removes: each must still be there (`cmd/edit.rs:456`); their parents
/// re-flow and their pins move.
fn removed(rest: &mut Rest<'_>, targets: &[NodeId]) {
    let narrowed = clear(rest, targets);
    for &target in targets {
        if is_page(rest.doc, target) {
            continue;
        }
        let place = in_flow(rest.doc, target);
        set(rest, narrowed, target, &[], place);
        flow(rest, target);
        pins(rest, target);
    }
}

/// Merges what another command rests on (a step of a batch, the repeated command).
fn absorb(rest: &mut Rest<'_>, command: &str, params: &Value) {
    let Ok(Some(under)) = super::under(rest.design, command, params) else { return };
    for premise in under.premises {
        rest.merge(premise.id, premise.fields.map(|f| f.into_iter().collect()), premise.positions.map(|p| p.into_iter().collect()));
    }
    rest.under.objects.extend(under.objects);
    rest.under.within.extend(under.within);
    if rest.under.covers.is_none() {
        rest.under.covers = under.covers;
    }
}

/// The edited layers of a vector edit in progress.
fn edited(rest: &Rest<'_>) -> Vec<NodeId> {
    rest.design
        .ds()
        .ok()
        .and_then(|ds| ds.vector_edit.as_ref())
        .map(|edit| std::iter::once(edit.id).chain(edit.others.iter().map(|part| part.id)).collect())
        .unwrap_or_default()
}

/// Whether the call names its layers (`ids`/`id`), not the selection.
fn named(params: &Value) -> bool {
    ["ids", "id"].iter().any(|key| params.get(*key).is_some_and(|v| !v.is_null()))
}

// ---- edit.* and command.* ----

/// Undo and redo put back what the step changed (`session/undo.rs:124`): the registers of the
/// layers it changed (KDM undoes only that step), the resources it changed, and the history.
fn history(rest: &mut Rest<'_>, undo: bool) {
    let Ok(ds) = rest.design.ds() else { return };
    rest.root(&["~history"]);
    let scope = rest.design.session().undo_scope();
    let stack = if undo { &ds.history.undo } else { &ds.history.redo };
    let Some(entry) = stack.iter().rev().find(|e| e.scope == scope).or_else(|| stack.last()) else { return };
    let (now, then) = (&*ds.doc, &*entry.doc);
    let mut layers: BTreeSet<NodeId> = kition_design_engine::changed_nodes(now, then).into_iter().collect();
    layers.extend(then.ids().filter(|id| !now.contains(*id)));
    for layer in layers {
        match (now.node(layer), then.node(layer)) {
            (Some(a), Some(b)) => {
                let (Ok(fa), Ok(fb)) = (kition_design_kdm::fields::node_fields(a, None), kition_design_kdm::fields::node_fields(b, None)) else {
                    rest.whole(layer.0, true);
                    continue;
                };
                let differ: Vec<&str> = fa.iter().zip(fb.iter()).filter(|(x, y)| x != y).map(|(x, _)| x.0).collect();
                let moved = a.parent != b.parent || now.index_in_parent(layer) != then.index_in_parent(layer);
                rest.need(layer.0, &differ, moved);
            }
            _ => rest.need(layer.0, &[], true),
        }
    }
    for style in now.styles.iter().filter(|s| !then.styles.contains(s)).chain(then.styles.iter().filter(|s| !now.styles.contains(s))) {
        rest.whole(json!(style.id), false);
    }
    let (vn, vt) = (&now.variables, &then.variables);
    for variable in vn.variables.iter().filter(|v| !vt.variables.contains(v)).chain(vt.variables.iter().filter(|v| !vn.variables.contains(v))) {
        rest.whole(json!(variable.id), false);
    }
    for collection in
        vn.collections.iter().filter(|c| !vt.collections.contains(c)).chain(vt.collections.iter().filter(|c| !vn.collections.contains(c)))
    {
        rest.whole(json!(collection.id), false);
    }
}

/// `edit.repeat` runs the last command again with its stored parameters (`cmd/edit.rs:402`): it
/// rests on what that command rests on.
fn repeat(rest: &mut Rest<'_>) {
    if let Some((command, params)) = rest.design.session().last_command.clone()
        && command != "edit.repeat"
    {
        absorb(rest, &command, &params);
    }
}

/// A cut copies the whole subtrees in page space (`cmd/edit/clipboard.rs:142`), then deletes; the
/// copy stays on the clipboard also for an agent's bracketed call
/// (`bindings/wasm/src/context.rs:243-250`).
fn cut(rest: &mut Rest<'_>) {
    let targets = rest.targets();
    drop_resources(rest);
    drop_pages(rest);
    for &target in &targets {
        for layer in rest.doc.subtree(target).into_iter().skip(1) {
            rest.whole(layer.0, false);
        }
        rest.whole(target.0, true);
        placed(rest, target, &[]);
        flow(rest, target);
        pins(rest, target);
    }
}

/// Styles and variables: a copy or a removal carries their keys, not their values.
fn drop_resources(rest: &mut Rest<'_>) {
    let ids: Vec<Value> = rest.needs.values().map(|(id, _, _)| id.clone()).filter(|id| id.as_str().is_some_and(|s| s != "design")).collect();
    for id in ids {
        rest.drop(id);
    }
}

/// Paste (`cmd/edit/clipboard.rs:151-288`): into the named parent, the one selected frame, or the
/// page; centred on the parent's world box when the copy is not inside it; an auto layout parent
/// re-flows. A main component copied in this file becomes an instance of its current subtree; a
/// paste from another file adopts its library records.
fn paste(rest: &mut Rest<'_>) {
    let design = rest.design;
    let Ok(ds) = design.ds() else { return };
    let doc = rest.doc;
    let clip = &design.session().clipboard;
    let designs = rest.command == "edit.pasteDesignLayers";
    let copy_local = !designs && clip.source_doc == ds.uid;
    // The mains the copy becomes instances of or follows (`cmd/edit/clipboard.rs:172-180`).
    let instanced = clip.nodes.iter().flat_map(|(_, nodes)| nodes).filter_map(|n| match &n.kind {
        NodeKind::Instance { component, .. } => Some(*component),
        _ => None,
    });
    let mains: Vec<NodeId> = clip.copied_components.iter().copied().chain(instanced).collect();
    // A removed main is still named (its subtree is then empty).
    let keep: BTreeSet<NodeId> = mains.iter().flat_map(|m| std::iter::once(*m).chain(doc.subtree(*m))).collect();
    // Earlier holds of the targets' parents and their world places: rebuilt below.
    let layers: Vec<NodeId> = rest.needs.values().filter_map(|(id, _, _)| id.as_u64().map(NodeId)).filter(|n| !keep.contains(n)).collect();
    for layer in layers {
        rest.drop(layer.0);
    }
    // Only a foreign clipboard adopts library records. Local copies resolve their named
    // mains/styles/variables above; unrelated library links do not affect their result.
    if copy_local || designs || clip.source.is_none() {
        rest.drop("design");
        if !clip.images.is_empty() {
            rest.root(&["images"]);
        }
    }
    // The mains are read whole; their order inside the main is the main's own field.
    for layer in &keep {
        rest.drop(layer.0);
        rest.whole(layer.0, false);
    }
    let selection = design.session().selection().to_vec();
    match rest.command {
        "edit.pasteOver" => match selection.first() {
            Some(&first) => {
                placed(rest, first, &["frame"]);
                if let Some(parent) = doc.node(first).and_then(|n| n.parent) {
                    into(rest, parent);
                }
            }
            None => into(rest, ds.page),
        },
        "edit.pasteReplace" => {
            for target in rest.targets() {
                placed(rest, target, &["frame"]);
                pins(rest, target);
                if let Some(parent) = doc.node(target).and_then(|n| n.parent) {
                    into(rest, parent);
                }
            }
        }
        _ => {
            let target = rest.node("parent").or_else(|| {
                selection.first().copied().filter(|i| {
                    selection.len() == 1
                        && doc.node(*i).is_some_and(|n| n.kind.is_frame_like() && !matches!(n.kind, NodeKind::Instance { .. }) && !n.is_board_atom())
                        && !(copy_local && clip.copied_components.iter().any(|root| doc.is_ancestor(*root, *i)))
                })
            });
            for selected in selection.iter().filter(|s| Some(**s) != target && !keep.contains(*s)) {
                rest.only(selected.0, &[], false);
            }
            into(rest, target.unwrap_or(ds.page));
        }
    }
}

/// What a paste into `parent` reads of it: its kind and world box, and its layout and children
/// when it is an auto layout frame (`cmd/edit/clipboard.rs:212-233`).
fn into(rest: &mut Rest<'_>, parent: NodeId) {
    let doc = rest.doc;
    if is_page(doc, parent) {
        rest.only(parent.0, &[], false);
        return;
    }
    let Some(p) = doc.node(parent) else { return };
    if p.layout.is_some() {
        placed(rest, parent, &["kind", "frame", "layout", "childLayout"]);
        for child in p.children.clone() {
            rest.need(child.0, &["frame", "visible", "childLayout"], true);
        }
    } else {
        placed(rest, parent, &["kind", "frame"]);
    }
}

/// Duplicate copies the whole subtree at index + 1 (`cmd/edit.rs:506`); a main's copy is an
/// instance of it. Its parent re-flows; nothing else of the ancestors is read.
fn duplicate(rest: &mut Rest<'_>) {
    let doc = rest.doc;
    let targets = rest.targets();
    drop_pages(rest);
    drop_resources(rest);
    for &target in &targets {
        for ancestor in doc.ancestors(target) {
            if !targets.contains(&ancestor) {
                rest.drop(ancestor.0);
            }
        }
    }
    for &target in &targets {
        unplace_descendants(rest, target);
        flow(rest, target);
    }
}

/// A subtree read whole: the root's children lists already say where each layer is.
fn unplace_descendants(rest: &mut Rest<'_>, root: NodeId) {
    for layer in rest.doc.subtree(root).into_iter().skip(1) {
        let whole = rest.needs.get(&json!(layer.0).to_string()).is_some_and(|(_, fields, _)| fields.is_none());
        if whole {
            rest.drop(layer.0);
            rest.whole(layer.0, false);
        }
    }
}

/// Find and replace scans every layer of the current page (`cmd/edit/props.rs:30-63`): its kind
/// (is it text) and characters, or its name; a changed text re-sizes and its parent re-flows.
fn find_replace(rest: &mut Rest<'_>) {
    let Some(page) = rest.page() else { return };
    let doc = rest.doc;
    let names = rest.params.get("layerNames").and_then(Value::as_bool) == Some(true);
    let find = rest.params.get("find").and_then(Value::as_str).unwrap_or_default().to_lowercase();
    let layers = doc.subtree(page);
    for layer in &layers {
        rest.drop(layer.0);
    }
    rest.need(page.0, &["~hierarchy"], false);
    for &layer in layers.iter().skip(1) {
        let Some(n) = doc.node(layer) else { continue };
        let text = n.kind.text();
        if names {
            rest.need(layer.0, &["name", "kind"], true);
        } else if text.is_some() {
            rest.need(layer.0, &["kind"], true);
        }
        if !find.is_empty() && text.is_some_and(|t| t.characters.to_lowercase().contains(&find)) {
            flow(rest, layer);
        }
    }
}

/// Paste properties writes the copied paints, effects, opacity and blend (corners and text
/// style when the layer takes them) and drops style links the new values left
/// (`cmd/edit/clipboard.rs:315`, `cmd/styles.rs:761`).
fn paste_properties(rest: &mut Rest<'_>) {
    let targets = rest.targets();
    let narrowed = clear(rest, &targets);
    for &target in &targets {
        let Some(n) = rest.doc.node(target) else { continue };
        let mut fields = PASTED.to_vec();
        if n.kind.has_corner_radius() {
            fields.push("corners");
        }
        set(rest, narrowed, target, &fields, false);
        styled(rest, target, &["fill", "stroke", "effect"]);
    }
}

/// The values of the styles `node` links for `kinds` (fill, stroke, effect): the clean-up keeps
/// a link only while the layer's value is the style's (`cmd/styles.rs:761`).
fn styled(rest: &mut Rest<'_>, node: NodeId, kinds: &[&str]) {
    let Some(n) = rest.doc.node(node) else { return };
    let styles: Vec<String> = kinds
        .iter()
        .filter_map(|kind| match *kind {
            "fill" => n.styles.fill.clone(),
            "stroke" => n.styles.stroke.clone(),
            _ => n.styles.effect.clone(),
        })
        .collect();
    if !styles.is_empty() {
        rest.need(node.0, &["bindings.styles"], false);
    }
    for style in styles {
        rest.need(json!(style), &["value"], false);
    }
}

// ---- tool.* ----

/// A tool switch ends a vector edit for tools other than Move, Scale and Hand: it reads each
/// edited layer's network to remove an emptied one (`canvas/vectoredit.rs:550`).
fn tool_switch(rest: &mut Rest<'_>) {
    use kition_design_engine::ToolId;
    let tool = match rest.command {
        "tool.set" | "tool.place" => rest.params.get("tool").and_then(Value::as_str).and_then(ToolId::from_id),
        "edit.pickColor" => Some(ToolId::Eyedropper),
        command => command.strip_prefix("tool.").and_then(ToolId::from_id),
    };
    if tool.is_some_and(|tool| !matches!(tool, ToolId::Move | ToolId::Scale | ToolId::Hand)) {
        super::vector::cleanup(rest, &edited(rest));
    }
}

/// `tool.place` draws at the click box's top-left: the parent is the frame there
/// (`cmd/create.rs:107` frame_at: visible, unlocked, kind, world box), the new layer goes in its
/// space and an auto layout parent re-flows; a frame or section takes the parent's children it
/// encloses (`canvas/tools/pen.rs:38`). The pick itself stays covered.
fn tool_place(rest: &mut Rest<'_>) {
    use kition_design_engine::ToolId;
    let design = rest.design;
    let Ok(ds) = design.ds() else { return };
    let doc = rest.doc;
    let tool = rest.params.get("tool").and_then(Value::as_str).and_then(ToolId::from_id).unwrap_or(design.session().tool.tool);
    if !tool.draws_shape() {
        return;
    }
    let (w, h) = tool.click_size();
    let (x, y) = rest.params.get("x").and_then(Value::as_f64).zip(rest.params.get("y").and_then(Value::as_f64)).unwrap_or((ds.view.cx, ds.view.cy));
    let start = kition_design_geom::Point::new(x - w / 2.0, y - h / 2.0);
    let (mut x0, mut y0, mut x1, mut y1) = (start.x, start.y, start.x + w, start.y + h);
    if ds.view.snap_pixel {
        (x0, y0, x1, y1) = (x0.round(), y0.round(), x1.round(), y1.round());
    }
    let area = kition_design_geom::Rect::new(x0, y0, x1, y1);
    let picked = kition_design_engine::cmd::create::frame_at(doc, ds.page, start, &[]);
    let parent =
        picked.filter(|id| tool != ToolId::Section || doc.node(*id).is_some_and(|n| matches!(n.kind, NodeKind::Section { .. }))).unwrap_or(ds.page);
    let pen = design.session().tool.pen.as_ref().and_then(|pen| pen.id);
    let held: Vec<Value> =
        rest.needs.values().map(|(id, _, _)| id.clone()).filter(|id| id.as_u64().is_some_and(|n| Some(NodeId(n)) != pen)).collect();
    for id in held {
        rest.drop(id);
    }
    rest.under.objects.push(json!(parent.0));
    if is_page(doc, parent) {
        rest.only(parent.0, &[], false);
    } else {
        rest.need(parent.0, &["frame", "layout", "visible", "locked", "kind"], false);
        for ancestor in doc.ancestors(parent).into_iter().filter(|a| !is_page(doc, *a)) {
            rest.need(ancestor.0, &["frame"], false);
        }
        if doc.node(parent).is_some_and(|p| p.layout.is_some()) {
            for child in doc.node(parent).map(|p| p.children.clone()).unwrap_or_default() {
                rest.need(child.0, &["frame", "visible", "childLayout"], true);
            }
        }
    }
    if matches!(tool, ToolId::Frame | ToolId::Section) {
        let enclosed = kition_design_engine::tools::enclosed_layers(doc, parent, area, tool == ToolId::Section, &[]);
        if !is_page(doc, parent) {
            for child in doc.node(parent).map(|p| p.children.clone()).unwrap_or_default() {
                rest.need(child.0, &["visible", "locked", "kind", "frame"], false);
            }
        }
        for layer in enclosed {
            rest.under.objects.push(json!(layer.0));
            placed(rest, layer, &["visible", "locked", "kind", "frame"]);
        }
    }
}

// ---- object.* ----

fn object(rest: &mut Rest<'_>) {
    let command = rest.command;
    let doc = rest.doc;
    let vector = !named(rest.params) && !edited(rest).is_empty();
    let targets = if vector { edited(rest) } else { rest.targets() };
    match command {
        "object.flipH" | "object.flipV" | "object.rotate180" | "object.rotateLeft" | "object.rotateRight" => {
            let narrowed = clear(rest, &targets);
            for &target in &targets {
                if vector {
                    // The selected points of the edited network (`canvas/vectoredit/transform.rs:165`).
                    set(rest, narrowed, target, &["kind", "frame"], true);
                } else {
                    // About the centre of the targets' world box (`cmd/arrange.rs:361,382`).
                    if narrowed {
                        rest.only(target.0, &[], false);
                    }
                    placed(rest, target, &["frame"]);
                    flow(rest, target);
                }
            }
        }
        "object.frameSelection" | "object.group" | "object.wrapInSection" => wrap(rest, &targets, false),
        "object.mask" if targets.len() > 1 => wrap(rest, &targets, true),
        "object.mask" | "object.releaseMask" => {
            // Toggles or clears the mask flag (`cmd/structure.rs:57,358`).
            let narrowed = clear(rest, &targets);
            for &target in &targets {
                set(rest, narrowed, target, &["props.is_mask"], false);
            }
        }
        "object.ungroup" | "object.removeFrame" => {
            // Children go to the container's parent at its index, world places kept, its opacity
            // multiplied in (`cmd/structure.rs:324`).
            let narrowed = clear(rest, &targets);
            for &target in &targets {
                let Some(n) = doc.node(target) else { continue };
                if !n.kind.is_container() {
                    set(rest, narrowed, target, &["kind"], false);
                    continue;
                }
                if narrowed {
                    rest.only(target.0, &[], false);
                }
                placed(rest, target, &["kind", "opacity", "frame.transform", "~hierarchy"]);
                let children: &[&str] =
                    if n.layout.is_some() { &["frame.transform", "props.bool_op", "childLayout"] } else { &["frame.transform", "props.bool_op"] };
                for child in n.children.clone() {
                    rest.need(child.0, children, true);
                    if n.opacity < 1.0 {
                        rest.need(child.0, &["opacity"], false);
                    }
                }
                flow(rest, target);
            }
        }
        "object.resizeToFit" => {
            // An auto layout frame hugs (`cmd/structure.rs:723`); a plain frame takes its visible
            // children's box and moves them back (`cmd/structure.rs:731-751`).
            let narrowed = clear(rest, &targets);
            for &target in &targets {
                let Some(n) = doc.node(target) else { continue };
                let (own, children): (&[&str], &[&str]) = if n.layout.is_some() {
                    (&["kind", "layout", "childLayout", "frame", "~hierarchy"], &["frame", "visible", "childLayout"])
                } else {
                    (&["kind", "layout", "frame", "~hierarchy"], &["frame", "visible"])
                };
                set(rest, narrowed, target, own, false);
                for child in n.children.clone() {
                    rest.need(child.0, children, true);
                }
                flow(rest, target);
            }
        }
        "object.setThumbnail" => {
            // Clears the marker from every layer's plugin data, then sets it (`cmd/structure.rs:91`).
            let narrowed = clear(rest, &targets);
            if let Some(&target) = targets.first() {
                set(rest, narrowed, target, &["props.plugin_data"], false);
            }
            rest.root(&["nodes"]);
        }
        "object.rasterize" => {
            // Renders everything that paints the layer, puts the image at its index + 1 in its
            // parent's space, removes it (`cmd/structure.rs:607`).
            let narrowed = clear(rest, &targets);
            for &target in &targets {
                if narrowed {
                    for layer in doc.subtree(target).into_iter().skip(1) {
                        rest.whole(layer.0, false);
                    }
                }
                rest.whole(target.0, true);
                placed(rest, target, &[]);
                flow(rest, target);
                pins(rest, target);
            }
        }
        "object.toggleVisible" => {
            // Reads whether all are visible unless told, replaces visible (`cmd/edit/select.rs:225`).
            let narrowed = clear(rest, &targets);
            for &target in &targets {
                let place = in_flow(doc, target);
                set(rest, narrowed, target, &["visible"], place);
                flow(rest, target);
            }
        }
        "object.hideOthers" => hide_others(rest, &targets),
        "object.removeFill" | "object.removeStroke" | "object.swapFillStroke" => {
            // Replaces the paint lists (`cmd/edit.rs:213-225`); the clean-up drops a style link
            // whose value the layer left (`cmd/styles.rs:761`).
            let narrowed = clear(rest, &targets);
            let (fields, kinds): (&[&str], &[&str]) = match command {
                "object.removeFill" => (&["fills"], &["fill"]),
                "object.removeStroke" => (&["strokes"], &["stroke"]),
                _ => (&["fills", "strokes"], &["fill", "stroke"]),
            };
            for &target in &targets {
                set(rest, narrowed, target, fields, false);
                styled(rest, target, kinds);
            }
        }
        "object.rename" if rest.params.get("name").is_none_or(Value::is_null) => {
            // Only asks the editor for an inline rename (`cmd/edit.rs:185`).
            let narrowed = clear(rest, &targets);
            for &target in &targets {
                set(rest, narrowed, target, &[], false);
            }
        }
        "object.batchRename" if rest.params.get("name").is_none_or(Value::is_null) => {
            // Find and replace in the current names: no numbering by stacking order
            // (`selection/rename.rs:105`).
            for &target in &targets {
                rest.only(target.0, &["name"], false);
            }
        }
        "object.moveToPage" => {
            // To the top of the page with the world transform kept; pins follow
            // (`cmd/structure.rs:590`, `cmd/comment.rs:197`).
            let narrowed = clear(rest, &targets);
            for &target in &targets {
                if narrowed {
                    rest.only(target.0, &[], false);
                }
                placed(rest, target, &["frame.transform"]);
                flow(rest, target);
                pins(rest, target);
            }
            if let Some(page) = rest.node("page") {
                rest.only(page.0, &[], false);
            }
        }
        "object.opacityKeys" => {
            // Replaces opacity and drops an opacity binding it overrode (`cmd/styles.rs:994`).
            let narrowed = clear(rest, &targets);
            for &target in &targets {
                set(rest, narrowed, target, &["opacity", "bindings.bound"], false);
            }
        }
        "object.convertToGroup" => {
            // `cmd/structure.rs:226,265-283`: the frame's own fields cleared, its children no
            // longer absolute, the box fitted to them.
            let narrowed = clear(rest, &targets);
            for &target in &targets {
                set(rest, narrowed, target, TO_GROUP, false);
                for child in doc.node(target).map(|n| n.children.clone()).unwrap_or_default() {
                    rest.need(child.0, &["frame", "childLayout"], true);
                }
            }
        }
        "object.convertToFrame" => {
            // A group loses its paints; a section keeps them but must hold no section
            // (`cmd/structure.rs:235-263`).
            let narrowed = clear(rest, &targets);
            for &target in &targets {
                let Some(n) = doc.node(target) else { continue };
                if matches!(n.kind, NodeKind::Section { .. }) {
                    set(rest, narrowed, target, &["kind", "props.clip", "~hierarchy"], false);
                    for child in n.children.clone() {
                        rest.need(child.0, &["kind"], false);
                    }
                } else {
                    set(rest, narrowed, target, &["kind", "fills", "strokes", "props.clip"], false);
                }
            }
        }
        "object.convertToSection" => {
            // `cmd/structure.rs:288`: frame-only fields cleared, children no longer absolute.
            let narrowed = clear(rest, &targets);
            for &target in &targets {
                set(rest, narrowed, target, TO_SECTION, false);
                for child in doc.node(target).map(|n| n.children.clone()).unwrap_or_default() {
                    rest.need(child.0, &["childLayout"], false);
                }
            }
        }
        _ => {}
    }
}

/// Group, frame selection, wrap in section and mask of several (`cmd/structure.rs:161,216,366`):
/// a new container around the targets' world boxes above the topmost one, the targets moved in
/// with their world places kept; a mask group's bottom layer becomes the mask.
fn wrap(rest: &mut Rest<'_>, targets: &[NodeId], mask: bool) {
    let doc = rest.doc;
    let narrowed = clear(rest, targets);
    let bottom = targets.iter().copied().min_by_key(|t| doc.index_in_parent(*t).unwrap_or(usize::MAX));
    for &target in targets {
        if narrowed {
            rest.only(target.0, &[], false);
        }
        let fields: &[&str] = if mask && Some(target) == bottom { &["frame", "props.is_mask"] } else { &["frame"] };
        placed(rest, target, fields);
        if rest.command == "object.group" {
            // cmd/structure.rs:174 refuses sections even when their box is unchanged.
            rest.need(target.0, &["kind"], false);
        }
        flow(rest, target);
    }
}

/// Hides every top-level layer of the current page but those holding the selection
/// (`cmd/edit/select.rs:248`).
fn hide_others(rest: &mut Rest<'_>, selection: &[NodeId]) {
    let Some(page) = rest.page() else { return };
    let doc = rest.doc;
    let narrowed = clear(rest, selection);
    let tops: BTreeSet<NodeId> = selection
        .iter()
        .filter_map(|s| std::iter::once(*s).chain(doc.ancestors(*s)).find(|a| doc.node(*a).and_then(|n| n.parent) == Some(page)))
        .collect();
    rest.need(page.0, &["~hierarchy"], false);
    for &selected in selection {
        set(rest, narrowed, selected, &[], tops.contains(&selected));
    }
    for child in doc.node(page).map(|p| p.children.clone()).unwrap_or_default() {
        if tops.contains(&child) {
            rest.need(child.0, &[], true);
        } else {
            rest.need(child.0, &["visible"], true);
        }
    }
}

// ---- page.* ----

/// The page `id` names, else the current page (`cmd/page.rs:13-34`).
fn named_page(rest: &Rest<'_>) -> Option<NodeId> {
    rest.node("id").or_else(|| rest.page())
}

fn page(rest: &mut Rest<'_>) {
    let Some(page) = named_page(rest) else { return };
    let doc = rest.doc;
    match rest.command {
        // A new page takes the current page's background (`cmd/page.rs:38-58`).
        "page.add" => rest.need(page.0, &["kind"], false),
        // `cmd/page.rs:82-88`, `cmd/page.rs:140-147`.
        "page.rename" => rest.only(page.0, &["name"], false),
        "page.setBackground" => rest.only(page.0, &["kind"], false),
        // The page and everything on it go (`cmd/page.rs:89-94`).
        "page.delete" => {
            clear(rest, &[]);
            rest.only(page.0, &[], false);
        }
        // Copies the name, the page's settings and every layer whole, inserted right after it
        // (`cmd/page.rs:95-127`).
        "page.duplicate" => {
            clear(rest, &[]);
            rest.only(page.0, &["name", "kind"], true);
            for layer in doc.subtree(page).into_iter().skip(1) {
                rest.whole(layer.0, true);
            }
        }
        // Its place among the pages is replaced; the window holds what is at the new place
        // (`cmd/page.rs:128-139`).
        "page.move" => {
            rest.only(page.0, &[], true);
        }
        _ => {}
    }
}

// ---- comment.* ----

/// The innermost visible frame or section under `(x, y)` on `page`, topmost first
/// (`cmd/comment.rs:602` frame_at).
fn frame_at(doc: &Document, page: NodeId, x: f64, y: f64) -> Option<NodeId> {
    let point = kition_design_geom::Point::new(x, y);
    let mut found = None;
    let mut stack = vec![page];
    while let Some(parent) = stack.pop() {
        let Some(n) = doc.node(parent) else { continue };
        for child in n.children.iter().rev() {
            let Some(c) = doc.node(*child) else { continue };
            if !c.visible || !c.kind.is_container() {
                continue;
            }
            let local = doc.world_transform(*child).inverse() * point;
            if local.x >= 0.0 && local.y >= 0.0 && local.x <= c.width && local.y <= c.height {
                if c.kind.is_frame_like() || matches!(c.kind, NodeKind::Section { .. }) {
                    found = Some(*child);
                }
                stack.push(*child);
                break;
            }
        }
    }
    found
}

/// Where a pinned comment lands (`cmd/comment.rs:285-301`): pinned to the frame under the point
/// (`anchor: true`, the default when moving) or to the layer named (on that page only), in its
/// local space.
fn pin(rest: &mut Rest<'_>, page: NodeId, by_default: bool) {
    let doc = rest.doc;
    let anchor = rest.params.get("anchor").cloned().unwrap_or(Value::Bool(by_default));
    let layer = match &anchor {
        Value::Bool(true) => {
            let (Some(x), Some(y)) = (rest.params.get("x").and_then(Value::as_f64), rest.params.get("y").and_then(Value::as_f64)) else { return };
            rest.under.covers = Some(Covers { fields: PICKS.iter().map(|f| (*f).to_owned()).collect() });
            let found = frame_at(doc, page, x, y);
            if let Some(found) = found {
                rest.under.objects.push(json!(found.0));
                placed(rest, found, &["frame", "visible", "kind"]);
            }
            return;
        }
        other => node_id(other).filter(|n| doc.page_of(*n) == Some(page)),
    };
    if let Some(layer) = layer {
        placed(rest, layer, &["frame"]);
    }
}

fn comment(rest: &mut Rest<'_>) {
    let doc = rest.doc;
    let thread = rest.params.get("id").and_then(Value::as_u64).and_then(|id| doc.comments.iter().find(|c| c.id == id));
    let reply = rest.params.get("reply").is_some_and(|r| !r.is_null());
    let (id, page) = (thread.map(|c| c.id), thread.map(|c| c.page));
    let thread_reads = |rest: &mut Rest<'_>, fields: &[&str]| {
        if let Some(id) = id {
            rest.only(id, fields, false);
        }
    };
    match rest.command {
        // `cmd/comment.rs:377-404`: a thread on the page, pinned when asked.
        "comment.add" => {
            let Some(page) = rest.node("page").or_else(|| rest.page()) else { return };
            rest.only(page.0, &[], false);
            if rest.params.get("anchor").is_some_and(|a| !a.is_null() && *a != Value::Bool(false)) {
                pin(rest, page, false);
            }
        }
        // Appends a message, reopens the thread, marks who read it (`cmd/comment.rs:406-416`).
        "comment.reply" => thread_reads(rest, &["anchor", "resolved"]),
        // The message's text or reactions (`cmd/comment.rs:417-459`).
        "comment.edit" | "comment.react" => thread_reads(rest, &["comments"]),
        "comment.resolve" => thread_reads(rest, &["resolved"]),
        "comment.delete" => thread_reads(rest, if reply { &["comments"] } else { &[] }),
        // To a point on the thread's page, pinned by default (`cmd/comment.rs:467-485`).
        "comment.move" => {
            thread_reads(rest, &["anchor"]);
            if let Some(page) = page {
                pin(rest, page, true);
            }
        }
        _ => {}
    }
}

// ---- file.*, version.*, branch.* ----

fn file(rest: &mut Rest<'_>) {
    let doc = rest.doc;
    match rest.command {
        // `cmd/file/open.rs:115-181`: an image or an SVG goes on top of the current page (an image
        // stored by its hash); only a design file merges pages and resources.
        "file.import" | "file.placeImage" => {
            let name = rest.params.get("name").and_then(Value::as_str).unwrap_or("import.png").to_ascii_lowercase();
            let extension = name.rsplit_once('.').map(|(_, e)| e.to_owned()).unwrap_or_default();
            let design = rest.command == "file.import" && matches!(extension.as_str(), "kdesign" | "json" | "fig" | "sketch");
            if design {
                return;
            }
            rest.drop("design");
            if let Some(page) = rest.node("parent").or_else(|| rest.page()) {
                rest.need(page.0, &[], false);
            }
            if extension != "svg" || rest.command == "file.placeImage" {
                use base64::Engine as _;
                let data = rest.params.get("data").and_then(Value::as_str).unwrap_or_default();
                let data = data.split_once(',').map_or(data, |(_, d)| d);
                if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(data.trim()) {
                    let hash = kition_design_format::image_hash(&bytes);
                    if doc.images.contains_key(&hash) {
                        rest.need(json!(hash), &[], false);
                    }
                }
            }
        }
        // A snapshot of the whole design (`cmd/file/version.rs:115-131`, `:14-28`).
        "file.saveVersion" | "version.autosave" | "branch.create" => rest.whole("design", false),
        // Replaces the document's name (`cmd/file.rs:176-180`).
        "file.rename" => rest.root(&["settings.name"]),
        // The whole design is replaced by the version's (`cmd/file/version.rs:133-171`); a
        // three-way merge reads the whole main design (`sharing/branches.rs`).
        "version.restore" | "branch.merge" | "branch.updateFromMain" => {
            clear(rest, &[]);
            rest.only(
                "design",
                &["settings", "nodes", "libraryStore", "styles", "collections", "variables", "libraries", "libraryCopies", "images"],
                false,
            );
            rest.root(if rest.command == "version.restore" { &["~history", "~snapshots"] } else { &["~branches"] });
            if rest.command != "version.restore" {
                rest.root(&["comments"]);
            }
        }
        // The version at the index (`cmd/file.rs:241-267`).
        "version.rename" => rest.only("design", &["~history"], false),
        "version.delete" => rest.only("design", &["~versionIds"], false),
        "file.colorProfile" => colour_profile(rest),
        _ => {}
    }
}

/// `file.colorProfile` (`cmd/file/save.rs:122-132`, `doc/src/colors.rs:53-124`): converting
/// rewrites every colour of the design; without converting only the profile changes.
fn colour_profile(rest: &mut Rest<'_>) {
    let doc = rest.doc;
    let Some(profile) = rest.params.get("profile").filter(|p| !p.is_null()) else { return };
    let _ = profile;
    if rest.params.get("convert").and_then(Value::as_bool) == Some(false) {
        rest.root(&["settings.color_profile"]);
        return;
    }
    rest.root(&["settings.color_profile", "settings.document_colors"]);
    for page in doc.pages.clone() {
        rest.need(page.0, &["kind"], false);
    }
    for node in doc.nodes().filter(|n| !n.is_page()) {
        if node.kind.text().is_some() {
            rest.need(node.id.0, &[COLOURED, &["kind"]].concat(), false);
        } else {
            rest.need(node.id.0, COLOURED, false);
        }
    }
    for style in &doc.styles {
        rest.need(json!(style.id), &["value"], false);
    }
    for variable in &doc.variables.variables {
        rest.need(json!(variable.id), &["values"], false);
    }
}

// ---- board.* ----

/// The connector a layer is, or whose label it is.
fn connector_of(doc: &Document, node: NodeId) -> NodeId {
    match doc.node(node).and_then(|n| n.board.as_ref()) {
        Some(BoardItem::Label { connector }) => *connector,
        _ => node,
    }
}

/// The layers a connector's ends are attached to.
fn ends(doc: &Document, connector: NodeId) -> Vec<NodeId> {
    match doc.node(connector).and_then(|n| n.board.as_ref()) {
        Some(BoardItem::Connector(c)) => [c.from.node(), c.to.node()].into_iter().flatten().collect(),
        _ => Vec::new(),
    }
}

/// A connector's route from its ends' world boxes (`boards/board/connectors.rs:36`,
/// `derived.rs:191-220` settle_routes).
fn routed(rest: &mut Rest<'_>, layers: impl IntoIterator<Item = NodeId>) {
    for layer in layers {
        rest.under.objects.push(json!(layer.0));
        rest.need(layer.0, &["frame"], false);
        for ancestor in rest.doc.ancestors(layer).into_iter().filter(|a| !is_page(rest.doc, *a)) {
            rest.need(ancestor.0, &["frame"], false);
        }
    }
}

/// The first layer inside `node` (a shape's outline, `boards/board/edit.rs:77,162`): held by its
/// place.
fn first_child(rest: &mut Rest<'_>, node: NodeId) {
    rest.under.within.push(Within { parent: json!(node.0), field: "nodes".into(), from: 0, to: 1, time: None });
}

fn board(rest: &mut Rest<'_>) {
    let doc = rest.doc;
    let targets = rest.targets();
    match rest.command {
        // A new item in the named parent or on the page (`boards/board/items.rs:95,210`); an auto
        // layout parent re-flows it.
        "board.sticky" | "board.shape" => {
            if let Some(parent) = rest.node("parent") {
                rest.only(parent.0, &[], false);
                if let Some(p) = doc.node(parent).filter(|p| p.layout.is_some()) {
                    rest.need(parent.0, &["frame", "layout", "childLayout"], false);
                    for child in p.children.clone() {
                        rest.need(child.0, &["frame", "visible", "childLayout"], true);
                    }
                }
            }
        }
        // Beside the sticky, in its parent, with its colour (`boards/board/items.rs:133`).
        "board.nextSticky" => {
            if let Some(&sticky) = targets.first() {
                rest.only(sticky.0, &["props.board", "fills", "frame"], true);
            }
        }
        "board.connector" => {
            clear(rest, &[]);
            let layers: Vec<NodeId> = ["from", "to"].iter().filter_map(|key| rest.node(key)).collect();
            routed(rest, layers);
        }
        // One end moves; the route is laid from the kept end and the target
        // (`boards/board/connectors.rs:87`).
        "board.reattach" => {
            clear(rest, &[]);
            let end = rest.params.get("end").and_then(Value::as_str).unwrap_or("to");
            for &target in &targets {
                let connector = connector_of(doc, target);
                rest.only(connector.0, &["props.board", "kind", "frame"], false);
                let kept = match doc.node(connector).and_then(|n| n.board.as_ref()) {
                    Some(BoardItem::Connector(c)) => {
                        if end == "from" {
                            c.to.node()
                        } else {
                            c.from.node()
                        }
                    }
                    _ => None,
                };
                let layers: Vec<NodeId> = kept.into_iter().chain(rest.node("target")).collect();
                routed(rest, layers);
            }
        }
        // The label among the connector's parent's children; its text replaced, or the label
        // removed (`boards/board/edit.rs:13,51`).
        "board.label" => {
            let empty = rest.params.get("text").and_then(Value::as_str).is_none_or(str::is_empty);
            for &target in &targets {
                let connector = connector_of(doc, target);
                rest.only(connector.0, &["props.board", "~hierarchy"], true);
                let siblings = doc.node(connector).and_then(|n| n.parent).and_then(|p| doc.node(p)).map(|p| p.children.clone()).unwrap_or_default();
                let label = siblings.into_iter().find(|s| doc.node(*s).is_some_and(|n| n.board == Some(BoardItem::Label { connector })));
                let Some(label) = label else { continue };
                if empty {
                    rest.only(label.0, &["props.board", "~hierarchy"], true);
                    continue;
                }
                rest.only(label.0, &["props.board", "layout", "~hierarchy"], true);
                let text = doc.node(label).and_then(|n| n.children.iter().copied().find(|c| doc.node(*c).is_some_and(|k| k.kind.text().is_some())));
                if let Some(text) = text {
                    rest.only(text.0, &["kind", "~hierarchy"], true);
                }
            }
        }
        // By item: a sticky's fills, a shape's outline (its first layer), a line's strokes
        // (`boards/board/edit.rs:66`).
        "board.setColor" => {
            for &target in &targets {
                match doc.node(target).and_then(|n| n.board.as_ref()) {
                    Some(BoardItem::Sticky { .. }) => rest.only(target.0, &["props.board", "fills"], false),
                    Some(BoardItem::Shape { .. }) => {
                        rest.only(target.0, &["props.board"], false);
                        first_child(rest, target);
                    }
                    Some(BoardItem::Connector(_) | BoardItem::Ink) => rest.only(target.0, &["props.board", "strokes"], false),
                    _ => rest.only(target.0, &["props.board"], false),
                }
            }
        }
        // A connector's routing and caps, re-routed from its ends; a shape's outline (its first
        // layer), padding and name; a text's size (`boards/board/edit.rs:117-181`).
        "board.setStyle" => {
            for &target in &targets {
                let target = connector_of(doc, target);
                match doc.node(target).and_then(|n| n.board.as_ref()) {
                    Some(BoardItem::Connector(_)) => {
                        rest.only(target.0, &["props.board", "stroke", "kind", "frame"], false);
                        let layers = ends(doc, target);
                        routed(rest, layers);
                    }
                    Some(BoardItem::Shape { .. }) => {
                        rest.only(target.0, &["name", "props.board", "layout", "frame", "~hierarchy"], false);
                        if rest.params.get("shape").is_some_and(|s| !s.is_null()) {
                            first_child(rest, target);
                        }
                        if rest.params.get("fontSize").is_some_and(|s| !s.is_null()) {
                            let text = doc
                                .node(target)
                                .and_then(|n| n.children.iter().copied().find(|c| doc.node(*c).is_some_and(|k| k.kind.text().is_some())));
                            if let Some(text) = text {
                                rest.need(text.0, &["kind"], false);
                            }
                        }
                    }
                    _ => rest.only(target.0, &["props.board"], false),
                }
            }
        }
        // `cmd/facilitate.rs:181-235`: a start reads and replaces the vote and ballots; a ballot
        // reads the running vote's scope and the voter's ballot; ending touches the vote only.
        "board.vote" => {
            let action = rest.params.get("action").and_then(Value::as_str).unwrap_or_default();
            match action {
                "end" => rest.only("design", &["settings.vote"], false),
                "start" => {
                    rest.only("design", &["settings.vote", "settings.votes"], false);
                    let on_selection = rest.params.get("on").and_then(Value::as_str) == Some("selection") || rest.params.get("ids").is_some();
                    if on_selection {
                        for &target in &targets {
                            rest.only(target.0, &[], true);
                        }
                    }
                }
                "cast" | "retract" => {
                    rest.only("design", &["settings.vote", "settings.votes"], false);
                    let stickies = matches!(doc.vote.as_ref().map(|v| &v.scope), Some(VoteScope::Stickies));
                    if let Some(target) = rest.node("id") {
                        rest.only(target.0, if stickies { &["props.board"] } else { &[] }, true);
                    }
                }
                _ => {}
            }
        }
        "board.tableEdit" => table_edit(rest, &targets),
        _ => {}
    }
}

/// `boards/board_table.rs:216`: the table `id` is in (each layer up from it checked), its rows
/// (laid-out children) and column lines; a row removed by index, each row restyled, the column
/// lines remade from the first row's cell count and the table's size.
fn table_edit(rest: &mut Rest<'_>, targets: &[NodeId]) {
    let doc = rest.doc;
    let Some(&target) = targets.first() else { return };
    let path: Vec<NodeId> = std::iter::once(target).chain(doc.ancestors(target)).collect();
    let Some(table) = path.iter().copied().find(|a| doc.node(*a).is_some_and(|n| n.board == Some(BoardItem::Table))) else { return };
    let Some(t) = doc.node(table) else { return };
    let subtree = doc.subtree(table);
    for layer in &subtree {
        rest.drop(layer.0);
    }
    rest.only(table.0, &["props.board", "frame"], false);
    for &layer in path.iter().take_while(|a| **a != table) {
        rest.need(layer.0, &["props.board", "~hierarchy"], true);
    }
    let rows: Vec<NodeId> = t.children.iter().copied().filter(|c| doc.node(*c).is_some_and(|n| !n.child_layout.absolute)).collect();
    let removed = (rest.params.get("action").and_then(Value::as_str) == Some("removeRow"))
        .then(|| {
            let index = rest.params.get("index").and_then(Value::as_u64).and_then(|i| usize::try_from(i).ok());
            index.map_or_else(|| rows.last().copied(), |i| rows.get(i).copied())
        })
        .flatten();
    for &child in &t.children {
        if !rows.contains(&child) {
            rest.need(child.0, &["childLayout"], true);
        } else if Some(child) == removed {
            rest.need(child.0, &["childLayout", "~hierarchy"], true);
        } else {
            rest.need(child.0, &["childLayout", "fills", "strokes", "~hierarchy"], true);
        }
    }
    // Removing a column resolves cells among the row's flowing children; their contents do
    // not decide which cell is removed.
    let action = rest.params.get("action").and_then(Value::as_str).unwrap_or_default();
    let index = rest.params.get("index").and_then(Value::as_u64).and_then(|i| usize::try_from(i).ok());
    if action == "removeColumn" {
        for row in rows {
            let cells: Vec<_> = doc
                .node(row)
                .map(|r| r.children.iter().copied().filter(|c| doc.node(*c).is_some_and(|n| !n.child_layout.absolute)).collect())
                .unwrap_or_default();
            for cell in &cells {
                rest.need(cell.0, &["childLayout"], true);
            }
            if let Some(cell) = index.map_or_else(|| cells.last().copied(), |i| cells.get(i).copied()) {
                rest.need(cell.0, &[], true);
            }
        }
    }
}
