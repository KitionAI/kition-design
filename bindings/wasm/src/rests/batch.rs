//! Batch premises follow execution order, including the editor state and step references that
//! earlier steps produce. Replay owns cloned documents, no replica, no live connection and no
//! file access. Spatial covers remain conservative: the batch has no covered-place replay.

use std::collections::BTreeSet;
use std::sync::Arc;

use kition_design_doc::{Document, NodeId, NodeKind};
use kition_design_engine::{BatchScope, BatchStep, Session};
use kition_kit::{Covers, Under, Within};
use serde_json::{Value, json};

use super::Rest;
use crate::{DesignDocument, engine_error, no_files};

/// Bounds the work needed before a guarded batch can begin, including nested batches.
const MAX_STEPS: usize = 4096;

/// Source of a step result whose fields can feed later parameters. Each nested batch keeps
/// its own result array, matching the engine's separate reference scopes.
enum Source {
    Node { doc: Arc<Document>, id: NodeId },
    Styles { doc: Arc<Document>, kind: Option<String> },
    Document(Arc<Document>),
    Batch(Vec<Source>),
    Other,
}

pub(super) fn under(design: &DesignDocument, params: &Value) -> Result<Option<Under>, String> {
    crate::agent::screen_batch(params)?;
    let doc = &design.ds()?.doc;
    let mut scratch = scratch(design)?;
    let mut collected = Rest::new(design, doc, "command.batch", params, Under::default());
    let initial = identities(&scratch.session);
    let mut created = BTreeSet::new();
    let mut remaining = MAX_STEPS;
    replay(&mut scratch, params, &mut collected, &initial, &mut created, &mut remaining)?;
    let mut under = collected.finish();
    // Only identities actually allocated during replay are exempt. A missing object named by
    // a source/default still has to reach the guard as a deletion premise.
    under.premises.retain(|premise| !created.contains(&premise.id.to_string()));
    under.objects.retain(|object| !created.contains(&object.to_string()));
    under.within.retain(|window| !created.contains(&window.parent.to_string()));
    Ok((!under.premises.is_empty() || !under.objects.is_empty() || !under.within.is_empty() || under.covers.is_some()).then_some(under))
}

pub(crate) fn scratch(design: &DesignDocument) -> Result<DesignDocument, String> {
    let mut session = Session::empty();
    session.docs = design.session.docs.clone();
    session.active = design.session.active;
    if let Some(state) = design.session.editor_state() {
        session.restore_editor_state(&state).map_err(engine_error)?;
    }
    session.libraries = design.session.libraries.clone();
    session.set_origin(Some(design.session.origin()));
    Ok(DesignDocument {
        session,
        zoom: design.zoom,
        held: design.held,
        interaction: design.interaction,
        replay: Vec::new(),
        viewport: design.viewport,
        player: None,
        schema: design.schema.clone(),
        libraries: design.libraries.clone(),
        kept: crate::context::Kept::default(),
    })
}

fn steps(params: &Value) -> Result<Vec<BatchStep>, String> {
    let commands =
        params.get("commands").and_then(Value::as_array).ok_or_else(|| "invalid-params: command.batch: `commands` must be an array".to_owned())?;
    commands
        .iter()
        .enumerate()
        .map(|(index, step)| {
            let command =
                step.get("command").and_then(Value::as_str).ok_or_else(|| format!("invalid-params: command.batch: step {index} has no `command`"))?;
            Ok(BatchStep {
                command: command.to_owned(),
                params: step.get("params").cloned().unwrap_or_else(|| json!({})),
                name: kition_design_engine::batch::step_name(index, step).map_err(engine_error)?,
            })
        })
        .collect()
}

fn replay(
    design: &mut DesignDocument,
    params: &Value,
    collected: &mut Rest<'_>,
    initial: &BTreeSet<String>,
    created: &mut BTreeSet<String>,
    remaining: &mut usize,
) -> Result<(Value, Source), String> {
    let steps = steps(params)?;
    if steps.len() > *remaining {
        return Err(format!("invalid-params: command.batch has more than {MAX_STEPS} steps"));
    }
    *remaining -= steps.len();
    let mut scope = BatchScope::new(&steps).map_err(engine_error)?;
    let label = params.get("label").and_then(Value::as_str).unwrap_or("Batch");
    // Keep the engine's grouping depth and history behavior while permitting each leaf's
    // premise calculation to read the complete scratch DesignDocument.
    let mut session = std::mem::replace(&mut design.session, Session::empty());
    let result = session.grouped(label, |session| {
        std::mem::swap(session, &mut design.session);
        let result = replay_steps(design, &steps, &mut scope, collected, initial, created, remaining);
        std::mem::swap(session, &mut design.session);
        result.map_err(|error| kition_design_engine::bad("command.batch", error))
    });
    design.session = session;
    let sources = result.map_err(engine_error)?;
    Ok((Value::Array(scope.into_results()), Source::Batch(sources)))
}

fn replay_steps(
    design: &mut DesignDocument,
    steps: &[BatchStep],
    scope: &mut BatchScope,
    collected: &mut Rest<'_>,
    initial: &BTreeSet<String>,
    created: &mut BTreeSet<String>,
    remaining: &mut usize,
) -> Result<Vec<Source>, String> {
    let mut sources = Vec::with_capacity(steps.len());
    for step in steps {
        let (result, source) = if step.command == "command.batch" {
            replay(design, &step.params, collected, initial, created, remaining)?
        } else {
            let params = scope.resolve(&step.params).map_err(engine_error)?;
            referenced_inputs(&step.params, steps, &sources, collected);
            if step.command.starts_with("widget.") {
                return Err("invalid-params: command.batch cannot replay widget callbacks for its guard".into());
            }
            if let Some(under) = super::under(design, &step.command, &params)? {
                absorb(collected, under);
            }
            copied_inputs(design, &step.command, &params, collected)?;
            let result = no_files(|| design.session.execute(&step.command, &params)).map_err(engine_error)?;
            let source = if step.command == "node.get" {
                match params.get("id").and_then(super::node_id) {
                    Some(id) => Source::Node { doc: design.ds()?.doc.clone(), id },
                    None => Source::Other,
                }
            } else if step.command == "style.list" {
                Source::Styles { doc: design.ds()?.doc.clone(), kind: params.get("type").and_then(Value::as_str).map(str::to_owned) }
            } else if kition_design_engine::find_command(&step.command).is_some_and(|spec| !spec.history) {
                // Comparison gap: other non-history result schemas have no field provenance
                // table yet. Hold their document only if a later reference consumes the result.
                Source::Document(design.ds()?.doc.clone())
            } else {
                Source::Other
            };
            created.extend(identities(&design.session).difference(initial).cloned());
            (result, source)
        };
        scope.push(result);
        sources.push(source);
    }
    Ok(sources)
}

/// Called after the engine resolver validates the reference grammar and depth. Keep the raw
/// reference's source path, rather than guessing provenance from its resolved scalar value.
fn referenced_inputs(params: &Value, steps: &[BatchStep], sources: &[Source], collected: &mut Rest<'_>) {
    let mut pending = vec![params];
    while let Some(value) = pending.pop() {
        match value {
            Value::Object(object) if object.contains_key("$step") || object.contains_key("$ref") => {
                let index = if object.contains_key("$step") {
                    object.get("$step").and_then(Value::as_u64).and_then(|index| usize::try_from(index).ok())
                } else {
                    object.get("$ref").and_then(Value::as_str).and_then(|name| steps.iter().position(|step| step.name.as_deref() == Some(name)))
                };
                if let Some(source) = index.and_then(|index| sources.get(index)) {
                    let field = object.get("field").and_then(Value::as_str).unwrap_or("id");
                    source_input(source, field.strip_prefix('/').unwrap_or(field), collected);
                }
            }
            Value::Object(object) => pending.extend(object.values()),
            Value::Array(array) => pending.extend(array),
            _ => {}
        }
    }
}

fn source_input(source: &Source, path: &str, collected: &mut Rest<'_>) {
    match source {
        Source::Batch(sources) => {
            let (index, tail) = path.split_once('/').map_or((path, None), |(index, tail)| (index, Some(tail)));
            if let Some(source) = index.parse::<usize>().ok().and_then(|index| sources.get(index)) {
                match tail {
                    Some(tail) => source_input(source, tail, collected),
                    None => source_whole(source, collected),
                }
            }
        }
        Source::Node { doc, id } => node_input(doc, *id, path, collected),
        Source::Styles { doc, kind } => style_input(doc, kind.as_deref(), path, collected),
        Source::Document(doc) => document_inputs(doc, collected),
        Source::Other => {}
    }
}

fn source_whole(source: &Source, collected: &mut Rest<'_>) {
    match source {
        Source::Node { doc, id } => whole_query(doc, *id, collected),
        Source::Styles { doc, kind } => styles_inputs(doc, kind.as_deref(), collected),
        Source::Document(doc) => document_inputs(doc, collected),
        Source::Batch(sources) => {
            for source in sources {
                source_whole(source, collected);
            }
        }
        Source::Other => {}
    }
}

fn node_input(doc: &Document, id: NodeId, path: &str, collected: &mut Rest<'_>) {
    let (field, tail) = path.split_once('/').map_or((path, None), |(field, tail)| (field, Some(tail)));
    if field == "children" {
        if let Some((index, tail)) = tail.map(|tail| tail.split_once('/').map_or((tail, None), |(index, tail)| (index, Some(tail))))
            && let Some(index) = index.parse::<usize>().ok()
        {
            collected.need(id.0, &[], false);
            collected.under.within.push(Within { parent: json!(id.0), field: "nodes".into(), from: 0, to: index.saturating_add(1), time: None });
            if let Some(child) = doc.node(id).and_then(|node| node.children.get(index)) {
                collected.need(child.0, &[], true);
                match tail {
                    Some(tail) => node_input(doc, *child, tail, collected),
                    None => whole_query(doc, *child, collected),
                }
            }
        } else {
            whole_query(doc, id, collected);
        }
        return;
    }
    let fields: &[&str] = match field {
        "id" => &[],
        "width" => &["frame.width"],
        "height" => &["frame.height"],
        "name" => &["name"],
        "type" => &["kind.type"],
        "rotation" => &["frame.transform"],
        "visible" => &["visible"],
        "locked" => &["locked"],
        "opacity" => &["opacity"],
        "blend" => &["props.blend"],
        "fills" => &["fills"],
        "strokes" => &["strokes"],
        "effects" => &["effects"],
        "layout" => &["layout"],
        "strokeWeight" => &["strokes", "stroke.weight"],
        "strokeAlign" => &["strokes", "stroke.align"],
        "strokeSides" => &["strokes", "stroke.sides"],
        "dashes" => &["strokes", "stroke.dashes"],
        "strokeJoin" => &["strokes", "stroke.join"],
        "miterAngle" => &["strokes", "stroke.join", "stroke.miter_limit"],
        "widthProfile" => &["strokes", "stroke.width_profile"],
        "brush" => &["strokes", "stroke.brush"],
        "cornerRadius" => &["corners.radii"],
        "cornerSmoothing" => &["corners"],
        "absolute" | "minWidth" | "maxWidth" | "minHeight" | "maxHeight" => &["childLayout"],
        "clip" => &["kind.type", "props.clip"],
        "devStatus" => &["dev.dev_status"],
        "devChanged" => &["dev.dev_changed"],
        "devNote" => &["dev.dev_note"],
        "annotations" => &["dev.annotations"],
        "codeLinks" => &["dev.code_links"],
        "measurements" => &["dev.measurements"],
        "sectionContentsHidden" => &["kind.contents_hidden"],
        "isMask" => &["props.is_mask"],
        "maskType" => &["props.is_mask", "props.mask_type"],
        "grids" => &["props.grids"],
        "exports" => &["props.exports"],
        "board" => &["props.board"],
        "interactions" => &["prototype.interactions"],
        "styles" => &["bindings.styles"],
        "boundVariables" => &["bindings.bound", "fills", "strokes", "effects"],
        "overrides" => &["instance.overrides"],
        "pluginData" => &["props.plugin_data"],
        "characters" => &["text"],
        "fontFamily" => &["kind.style.font_family", "text"],
        "fontStyle" => &["kind.style.font_style", "text"],
        "fontSize" => &["kind.style.font_size", "text"],
        "fontVariations" => &["kind.style.variations", "text"],
        "lineHeight" => &["kind.style.line_height", "text"],
        "letterSpacing" => &["kind.style.letter_spacing", "text"],
        "textAlign" => &["kind.align_h"],
        "textAutoResize" => &["kind.auto_resize"],
        "maxLines" => &["kind.max_lines"],
        "textPath" => &["kind.path"],
        "textDecoration" => &["kind.style.decoration", "text"],
        "textDecorationStyle" | "textDecorationThickness" | "textDecorationOffset" | "textDecorationSkipInk" | "textDecorationColor" => {
            &["kind.style.decoration_look", "text"]
        }
        "listStyle" | "lineTypes" | "lineIndentations" | "listStartOffsets" => &["kind.list", "text"],
        "listSpacing" => &["kind.list_spacing"],
        "hangingPunctuation" => &["kind.hanging_punctuation"],
        "hangingList" => &["kind.hanging_list"],
        "leadingTrim" => &["kind.leading_trim"],
        "styledRuns" => &["kind.style", "text"],
        "textStyleRanges" => &["kind.style", "text"],
        "arc" => &["kind.start", "kind.sweep", "kind.inner"],
        "pointCount" => &["kind.count"],
        "innerRadius" => &["kind.inner_ratio"],
        "booleanOperation" => &["kind.op"],
        "mainComponent" => &["kind.component"],
        "componentProperties" | "componentPropertyDefinitions" => &["kind.props"],
        "description" => &["kind.description"],
        "vertices" => &["kind.network.vertices"],
        "segments" => &["kind.network.segments"],
        "background" => &["kind.background"],
        "flows" => &["kind.flows"],
        "childCount" => &["nodes"],
        "sizing" | "constraints" => {
            collected.need(id.0, &["kind.type", "layout", "childLayout", "constraints", "~hierarchy.parent"], false);
            if let Some(parent) = doc.node(id).and_then(|node| node.parent) {
                collected.need(parent.0, &["kind.type", "layout"], false);
            }
            return;
        }
        "gridCell" => {
            grid_input(doc, id, collected);
            return;
        }
        "variableModes" => {
            modes_input(doc, id, tail, collected);
            return;
        }
        "devResources" => {
            resources_input(doc, id, collected);
            return;
        }
        "slot" => {
            slot_input(doc, id, collected);
            return;
        }
        "operandOperation" => {
            collected.need(id.0, &["props.bool_op", "~hierarchy.parent"], false);
            if let Some(parent) = doc.node(id).and_then(|node| node.parent) {
                collected.need(parent.0, &["kind.type"], false);
            }
            return;
        }
        "x" | "y" => {
            relative_query(doc, id, collected);
            return;
        }
        "absoluteBounds" => {
            world_query(doc, id, collected);
            return;
        }
        "mainComponentName" => {
            collected.need(id.0, &["kind.component"], false);
            if let Some(kition_design_doc::Node { kind: NodeKind::Instance { component, .. }, .. }) = doc.node(id) {
                collected.need(component.0, &["name"], false);
            }
            return;
        }
        _ => {
            whole_query(doc, id, collected);
            return;
        }
    };
    collected.need(id.0, fields, false);
}

fn relative_query(doc: &Document, id: NodeId, collected: &mut Rest<'_>) {
    collected.need(id.0, &["frame.transform", "~hierarchy.parent"], false);
    for id in doc.ancestors(id) {
        collected.need(id.0, &["kind.type"], false);
        if !doc.node(id).is_some_and(|node| node.kind.bounds_follow_children()) {
            break;
        }
        collected.need(id.0, &["frame.transform", "~hierarchy.parent"], false);
    }
}

fn world_query(doc: &Document, id: NodeId, collected: &mut Rest<'_>) {
    collected.need(id.0, &["frame", "~hierarchy.parent"], false);
    for ancestor in doc.ancestors(id) {
        if !doc.node(ancestor).is_some_and(|node| node.is_page()) {
            collected.need(ancestor.0, &["frame.transform", "~hierarchy.parent"], false);
        }
    }
}

fn whole_query(doc: &Document, id: NodeId, collected: &mut Rest<'_>) {
    collected.whole(id.0, false);
    for node in doc.subtree(id) {
        collected.whole(node.0, false);
        grid_input(doc, node, collected);
        modes_input(doc, node, None, collected);
        resources_input(doc, node, collected);
        slot_input(doc, node, collected);
        if let Some(kition_design_doc::Node { kind: NodeKind::Instance { component, .. }, .. }) = doc.node(node) {
            collected.need(component.0, &["name"], false);
        }
    }
    world_query(doc, id, collected);
}

/// The returned cell indices come from grid placement, not the track geometry. Placement
/// reads every sibling's visibility, anchors and spans, and the parent's column count/order.
fn grid_input(doc: &Document, id: NodeId, collected: &mut Rest<'_>) {
    collected.need(id.0, &["childLayout", "~hierarchy.parent"], false);
    let Some(parent) = doc.node(id).and_then(|node| node.parent) else { return };
    collected.need(parent.0, &["layout", "nodes"], false);
    if let Some(parent) = doc.node(parent) {
        for child in &parent.children {
            collected.need(child.0, &["visible", "childLayout"], true);
        }
    }
}

fn modes_input(doc: &Document, id: NodeId, path: Option<&str>, collected: &mut Rest<'_>) {
    collected.need(id.0, &["bindings.var_modes"], false);
    let Some(node) = doc.node(id) else { return };
    let name = path.and_then(|path| path.split('/').next());
    for (key, mode) in &node.var_modes {
        let Some(collection) = doc.variables.collection(key) else {
            collected.need(json!(key), &["name", "modes"], false);
            continue;
        };
        if name.is_some_and(|name| name != collection.name) {
            continue;
        }
        collected.need(json!(collection.id), &["name", "modes"], false);
        if let Some(mode) = collection.find_mode(mode) {
            collected.need(json!(mode.id), &["name"], false);
        }
    }
}

fn resources_input(doc: &Document, id: NodeId, collected: &mut Rest<'_>) {
    collected.need(id.0, &["dev.dev_resources", "kind.component", "mirrors", "~hierarchy.parent"], false);
    // A slot override stops instance ancestry in components::scope.
    for ancestor in doc.ancestors(id) {
        collected.need(ancestor.0, &["kind.type", "instance.overrides", "~hierarchy.parent"], false);
        if doc.node(ancestor).is_some_and(|node| node.overrides.contains("slot") || matches!(node.kind, NodeKind::Instance { .. })) {
            break;
        }
    }
    if kition_design_engine::components::inside_instance(doc, id) {
        return;
    }
    if let Some(kition_design_doc::Node { kind: NodeKind::Instance { component, .. }, .. }) = doc.node(id) {
        collected.need(component.0, &["dev.dev_resources", "~hierarchy.parent"], false);
        if let Some(parent) = doc.node(*component).and_then(|node| node.parent) {
            collected.need(parent.0, &["kind.type"], false);
            if doc.node(parent).is_some_and(|node| matches!(node.kind, NodeKind::ComponentSet { .. })) {
                collected.need(parent.0, &["dev.dev_resources"], false);
            }
        }
    }
}

fn slot_input(doc: &Document, id: NodeId, collected: &mut Rest<'_>) {
    let mut at = id;
    let mut seen = BTreeSet::new();
    while seen.insert(at) {
        collected.need(at.0, &["kind.type", "instance.prop_refs", "mirrors"], false);
        let Some(node) = doc.node(at).filter(|node| matches!(node.kind, NodeKind::Frame)) else { return };
        if node.prop_refs.contains_key("slot") {
            for owner in std::iter::once(at).chain(doc.ancestors(at)) {
                collected.need(owner.0, &["kind.type", "~hierarchy.parent"], false);
                if doc.node(owner).is_some_and(|node| matches!(node.kind, NodeKind::Component { .. })) {
                    collected.need(owner.0, &["kind.props"], false);
                    break;
                }
            }
            return;
        }
        let Some(source) = node.source else { return };
        at = source;
    }
}

fn style_input(doc: &Document, kind: Option<&str>, path: &str, collected: &mut Rest<'_>) {
    let (index, tail) = path.split_once('/').map_or((path, None), |(index, tail)| (index, Some(tail)));
    // Comparison gap: filtered style-list membership is conservative for edits of another type.
    collected.root(&["styles"]);
    for style in &doc.styles {
        if kind.is_some() {
            collected.need(json!(style.id), &["value.type"], false);
        }
    }
    let Some(style) = index
        .parse::<usize>()
        .ok()
        .and_then(|index| doc.styles.iter().filter(|style| kind.is_none_or(|kind| style.kind.type_name() == kind)).nth(index))
    else {
        styles_inputs(doc, kind, collected);
        return;
    };
    let fields: &[&str] = match tail.and_then(|tail| tail.split('/').next()) {
        Some("id") => &[],
        Some("name") => &["name"],
        Some("description") => &["props.description"],
        Some("bound") => &["props.bound"],
        Some("type") => &["value.type"],
        Some(_) => &["value", "props"],
        None => {
            collected.whole(json!(style.id), false);
            return;
        }
    };
    collected.need(json!(style.id), fields, false);
}

fn styles_inputs(doc: &Document, kind: Option<&str>, collected: &mut Rest<'_>) {
    collected.root(&["styles"]);
    for style in &doc.styles {
        collected.need(json!(style.id), &["value.type"], false);
        if kind.is_none_or(|kind| style.kind.type_name() == kind) {
            collected.whole(json!(style.id), false);
        }
    }
}

/// Comparison gap: result fields of non-history commands other than node.get/style.list are
/// conservatively dependent on the whole design. This closes wrong writes while their result
/// schemas still lack independent field-by-field provenance readings.
fn document_inputs(doc: &Document, collected: &mut Rest<'_>) {
    collected.whole("design", true);
    for node in doc.ids() {
        collected.whole(node.0, true);
    }
    for thread in &doc.comments {
        collected.whole(thread.id, false);
    }
    for style in &doc.styles {
        collected.whole(json!(style.id), false);
    }
    for variable in &doc.variables.variables {
        collected.whole(json!(variable.id), false);
    }
    for collection in &doc.variables.collections {
        collected.whole(json!(collection.id), false);
        for mode in &collection.modes {
            collected.whole(json!(mode.id), false);
        }
    }
    for hash in doc.images.keys() {
        collected.whole(json!(hash), false);
    }
    for library in &doc.libraries.enabled {
        collected.whole(json!(library.id), false);
    }
}

fn absorb(rest: &mut Rest<'_>, under: Under) {
    for premise in under.premises {
        rest.merge(
            premise.id,
            premise.fields.map(|fields| fields.into_iter().collect()),
            premise.positions.map(|positions| positions.into_iter().collect()),
        );
    }
    rest.under.objects.extend(under.objects);
    rest.under.within.extend(under.within);
    if let Some(covers) = under.covers {
        let known = rest.under.covers.get_or_insert_with(Covers::default);
        known.fields.extend(covers.fields);
        known.fields.sort();
        known.fields.dedup();
    }
}

/// A copy in the batch freezes file data for a later paste; unlike a pre-existing clipboard it
/// is therefore a dependency of this call.
fn copied_inputs(design: &DesignDocument, command: &str, params: &Value, collected: &mut Rest<'_>) -> Result<(), String> {
    if !matches!(command, "edit.copy" | "edit.copyProperties" | "fill.copy" | "stroke.copy" | "effect.copy") {
        return Ok(());
    }
    let doc = &design.ds()?.doc;
    let source = Rest::new(design, doc, command, params, Under::default());
    let targets = source.targets();
    if command == "edit.copy" {
        for target in targets {
            for node in doc.subtree(target) {
                collected.whole(node.0, false);
            }
            collected.need(target.0, &[], true);
            for ancestor in doc.ancestors(target) {
                if !doc.node(ancestor).is_some_and(|node| node.is_page()) {
                    collected.need(ancestor.0, &["frame.transform"], false);
                }
            }
        }
    } else if let Some(target) = targets.first() {
        let fields: &[&str] = match command {
            "fill.copy" => &["fills"],
            "stroke.copy" => &["strokes"],
            "effect.copy" => &["effects"],
            _ => &["kind", "fills", "strokes", "stroke", "effects", "opacity", "props.blend", "corners"],
        };
        collected.need(target.0, fields, false);
    }
    Ok(())
}

fn identities(session: &Session) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for ds in &session.docs {
        document_ids(&ds.doc, &mut ids);
    }
    ids
}

fn document_ids(doc: &Document, ids: &mut BTreeSet<String>) {
    ids.extend(doc.ids().map(|NodeId(id)| json!(id).to_string()));
    ids.extend(doc.comments.iter().map(|thread| json!(thread.id).to_string()));
    ids.extend(doc.styles.iter().map(|style| json!(style.id).to_string()));
    ids.extend(doc.variables.variables.iter().map(|variable| json!(variable.id).to_string()));
    for collection in &doc.variables.collections {
        ids.insert(json!(collection.id).to_string());
        ids.extend(collection.modes.iter().map(|mode| json!(mode.id).to_string()));
    }
    ids.extend(doc.images.keys().map(|hash| json!(hash).to_string()));
    ids.extend(doc.libraries.enabled.iter().map(|library| json!(library.id).to_string()));
}
