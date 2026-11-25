//! Shared spatial, reference, and dependency resolution for the per-family premise tables.

use std::collections::{BTreeMap, BTreeSet};

use kition_design_doc::proto::Expr;
use kition_design_doc::{Document, NodeId, NodeKind, PropDef};
use kition_kit::{Premise, Under, Within};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::DesignDocument;

const COVERS: &[&str] =
    &["frame", "kind", "visible", "locked", "props", "fills", "strokes", "stroke", "corners", "layout", "childLayout", "~bounds", "~hierarchy"];
const REFERENCES: &[&str] = &["name", "type", "collection"];

fn tool_place_area(
    design: &DesignDocument,
    params: &Value,
) -> Option<(kition_design_engine::ToolId, kition_design_geom::Point, kition_design_geom::Rect)> {
    use kition_design_engine::ToolId;
    use kition_design_geom::{Point, Rect};
    let ds = design.ds().ok()?;
    let tool = params.get("tool").and_then(Value::as_str).and_then(ToolId::from_id).unwrap_or(design.session().tool.tool);
    if !tool.draws_shape() {
        return None;
    }
    // Session::place sends pointer down at the click-sized shape's top-left corner.
    let (w, h) = tool.click_size();
    let (x, y) = params.get("x").and_then(Value::as_f64).zip(params.get("y").and_then(Value::as_f64)).unwrap_or((ds.view.cx, ds.view.cy));
    let start = Point::new(x - w / 2.0, y - h / 2.0);
    let (mut x0, mut y0, mut x1, mut y1) = (start.x, start.y, start.x + w, start.y + h);
    if ds.view.snap_pixel {
        (x0, y0, x1, y1) = (x0.round(), y0.round(), x1.round(), y1.round());
    }
    Some((tool, start, Rect::new(x0, y0, x1, y1)))
}

fn tool_place_inputs(design: &DesignDocument, doc: &Document, params: &Value) -> Option<Vec<NodeId>> {
    use kition_design_engine::ToolId;
    let ds = design.ds().ok()?;
    let (tool, start, area) = tool_place_area(design, params)?;
    let picked = kition_design_engine::cmd::create::frame_at(doc, ds.page, start, &[]);
    let parent =
        picked.filter(|id| tool != ToolId::Section || doc.node(*id).is_some_and(|n| matches!(n.kind, NodeKind::Section { .. }))).unwrap_or(ds.page);
    let mut inputs = vec![parent];
    if matches!(tool, ToolId::Frame | ToolId::Section) {
        inputs.extend(kition_design_engine::tools::enclosed_layers(doc, parent, area, tool == ToolId::Section, &[]));
    }
    Some(inputs)
}

pub(super) fn point_target(design: &DesignDocument, doc: &Document, command: &str, params: &Value) -> Option<(Option<NodeId>, NodeId)> {
    let view = &design.ds().ok()?.view;
    let point = kition_design_geom::Point::new(
        params.get("x").and_then(Value::as_f64).or_else(|| (command == "library.place").then_some(view.cx))?,
        params.get("y").and_then(Value::as_f64).or_else(|| (command == "library.place").then_some(view.cy))?,
    );
    if !point.x.is_finite() || !point.y.is_finite() {
        return None;
    }
    let page = design.ds().ok()?.page;
    if matches!(command, "library.swapAt" | "library.place") {
        let link = kition_design_engine::library::find_link(doc, params.get("library")?.as_str()?)?;
        let lib = crate::libraries::read(design, link).ok()?;
        let key = kition_design_engine::library::component_key(&lib, params.get("component")?)?;
        let mut imported = doc.clone();
        let map = kition_design_engine::library::import(&mut imported, &lib, &[key], false).ok()?;
        let local = *map.get(&key)?;
        if command == "library.place" {
            return kition_design_engine::drop::instance_place_slot(&imported, page, point, local)
                .map(|(source, slot)| (doc.contains(source).then_some(source), slot.parent));
        }
        return kition_design_engine::drop::instance_swap_target(&imported, page, point, local)
            .map(|(source, target)| (doc.contains(source).then_some(source), target));
    }
    let component = params.get("component").and_then(id)?;
    if command == "instance.place" {
        return kition_design_engine::drop::instance_place_slot(doc, page, point, component).map(|(source, slot)| (Some(source), slot.parent));
    }
    kition_design_engine::drop::instance_swap_target(doc, page, point, component).map(|(source, target)| (Some(source), target))
}

fn id(value: &Value) -> Option<NodeId> {
    value.as_u64().or_else(|| value.as_str()?.parse().ok()).map(NodeId)
}

#[derive(Default)]
struct Holds(BTreeMap<NodeId, (BTreeSet<String>, BTreeSet<String>)>);

impl Holds {
    fn fields(&mut self, node: NodeId, fields: &[&str], positioned: bool) {
        let (held, positions) = self.0.entry(node).or_default();
        held.extend(fields.iter().map(|s| (*s).to_owned()));
        if positioned {
            positions.extend(["nodes".into(), "libraryStore".into()]);
            // SDK positions currently match an exact path or its root, not intermediate paths.
            if fields.iter().any(|field| field.starts_with("frame")) {
                positions.insert("frame".into());
            }
        }
    }

    fn world(&mut self, doc: &Document, node: NodeId) {
        for node in std::iter::once(node).chain(doc.ancestors(node)) {
            self.fields(node, &["frame.transform"], true);
        }
    }

    fn geometry(&mut self, doc: &Document, node: NodeId) {
        self.world(doc, node);
        self.fields(node, &["frame", "kind", "constraints", "childLayout", "layout"], true);
        // Constraints, wrapping groups, and auto layout read other descendants' geometry.
        for ancestor in std::iter::once(node).chain(doc.ancestors(node)) {
            if doc.node(ancestor).is_some_and(|n| n.layout.is_some() || n.kind.bounds_follow_children()) {
                for child in doc.subtree(ancestor) {
                    self.fields(child, &["frame", "kind", "visible", "layout", "childLayout", "constraints", "stroke", "strokes"], true);
                }
            }
        }
    }

    fn resize(&mut self, doc: &Document, node: NodeId) {
        self.geometry(doc, node);
        for child in doc.subtree(node) {
            self.fields(child, &["frame", "kind", "constraints", "layout", "childLayout", "visible", "stroke", "strokes", "~hierarchy"], true);
        }
    }

    fn finish(self) -> Vec<Premise> {
        self.0.into_iter().map(|(node, (fields, positions))| Premise::on(json!(node.0)).fields(fields).positions(positions)).collect()
    }
}

fn targets(doc: &DesignDocument, params: &Value) -> Vec<NodeId> {
    params
        .get("ids")
        .and_then(Value::as_array)
        .map(|ids| ids.iter().filter_map(id).collect())
        .or_else(|| params.get("id").and_then(id).map(|id| vec![id]))
        .unwrap_or_else(|| doc.session().selection().to_vec())
}

fn property(key: &str) -> Option<&'static [&'static str]> {
    Some(match key {
        "name" => &["name"],
        "locked" => &["locked"],
        "visible" => &["visible"],
        "opacity" => &["opacity", "~boundVariables"],
        "blend" | "blendMode" => &["props.blend"],
        "fills" | "fill" => &["fills", "~boundVariables", "~styleLinks"],
        "strokes" | "stroke" => &["strokes", "bindings"],
        "effects" => &["effects", "bindings"],
        "x" | "y" | "rotation" | "width" | "height" | "w" | "h" => &["frame", "kind", "constraints", "childLayout"],
        "clip" | "clipsContent" => &["props.clip"],
        "isMask" => &["props.is_mask"],
        "maskType" => &["props.mask_type"],
        "constrainProportions" => &["props.constrain_proportions"],
        "exports" | "exportSettings" => &["props.exports"],
        "grids" | "layoutGrids" => &["props.grids", "bindings"],
        "interactions" | "overflow" | "scrollPosition" => &["prototype"],
        "fillStyle" => &["fills", "bindings"],
        "strokeStyle" => &["strokes", "bindings"],
        "effectStyle" => &["effects", "bindings"],
        "gridStyle" => &["props.grids", "bindings"],
        "textStyle" => &["kind", "bindings", "frame"],
        "boundVariables" | "variableModes" => &["bindings"],
        "cornerRadius" | "cornerSmoothing" | "topLeftRadius" | "topRightRadius" | "bottomLeftRadius" | "bottomRightRadius" => {
            &["corners", "bindings"]
        }
        _ => return None,
    })
}

/// Paths inside values whose grammar resolves another object (not every integer in a value).
fn expression_refs(doc: &Document, expression: &Expr, objects: &mut Vec<Value>, depth: usize) {
    use kition_design_doc::VariableValue;
    if depth > kition_design_doc::proto::MAX_EXPR_DEPTH {
        return;
    }
    match expression {
        Expr::Value(VariableValue::Alias(name)) => {
            if let Some(variable) = doc.variables.get(name) {
                objects.push(json!(variable.id));
            }
        }
        Expr::Mode { collection } => {
            if let Some(collection) = doc.variables.collection(collection) {
                objects.push(json!(collection.id));
            }
        }
        Expr::InMode { variable, mode } => {
            if let Some(variable) = doc.variables.get(variable) {
                objects.push(json!(variable.id));
                if let Some(mode) = doc.variables.collection(&variable.collection).and_then(|c| c.find_mode(mode)) {
                    objects.push(json!(mode.id));
                }
            }
        }
        Expr::Op { args, .. } => {
            for arg in args {
                expression_refs(doc, arg, objects, depth + 1);
            }
        }
        _ => {}
    }
}

fn referenced(doc: &Document, value: &Value, objects: &mut Vec<Value>) {
    match value {
        Value::Object(map) => {
            if let Some(text) = map.get("expression").and_then(Value::as_str)
                && let Ok(expression) = Expr::parse(text, &doc.variables)
            {
                expression_refs(doc, &expression, objects, 0);
            }
            for field in ["condition", "expr"] {
                if let Some(expression) = map.get(field).and_then(|v| serde_json::from_value::<Expr>(v.clone()).ok()) {
                    expression_refs(doc, &expression, objects, 0);
                }
            }
            if map.get("type").and_then(Value::as_str) == Some("setVariableMode")
                && let Some(collection) = map.get("collection").and_then(Value::as_str).and_then(|c| doc.variables.collection(c))
            {
                objects.push(json!(collection.id));
                if let Some(mode) = map.get("mode").and_then(Value::as_str).and_then(|m| collection.find_mode(m)) {
                    objects.push(json!(mode.id));
                }
            }
            for (key, value) in map {
                if key == "alias" {
                    if let Some(variable) = value.as_str().and_then(|v| doc.variables.get(v)) {
                        objects.push(json!(variable.id));
                    }
                } else if key == "dest" && value.is_u64() {
                    objects.push(value.clone());
                } else if key == "variable" {
                    if let Some(variable) = value.as_str().and_then(|v| doc.variables.get(v)) {
                        objects.push(json!(variable.id));
                    }
                } else if matches!(
                    key.as_str(),
                    "props" | "value" | "values" | "action" | "actions" | "interaction" | "interactions" | "blocks" | "condition" | "expr"
                ) {
                    referenced(doc, value, objects);
                }
            }
        }
        Value::Array(values) => values.iter().for_each(|value| referenced(doc, value, objects)),
        _ => {}
    }
}

struct LiteralMode<'a> {
    collection: &'a str,
    mode: &'a str,
}

/// These actions store a collection and mode id, without evaluating any variable values.
/// Other reference grammars retain the general dependency traversal.
fn literal_modes(params: &Value) -> Option<Vec<LiteralMode<'_>>> {
    fn visit<'a>(value: &'a Value, found: &mut Vec<LiteralMode<'a>>, depth: usize) -> bool {
        if depth > 64 {
            return false;
        }
        match value {
            Value::Object(fields) => {
                if fields.get("type").and_then(Value::as_str) == Some("setVariableMode") {
                    let (Some(collection), Some(mode)) =
                        (fields.get("collection").and_then(Value::as_str), fields.get("mode").and_then(Value::as_str))
                    else {
                        return false;
                    };
                    found.push(LiteralMode { collection, mode });
                    fields.iter().filter(|(key, _)| !matches!(key.as_str(), "type" | "collection" | "mode")).all(|(key, value)| {
                        !matches!(key.as_str(), "expression" | "expr" | "condition" | "variable" | "alias") && visit(value, found, depth + 1)
                    })
                } else {
                    fields.iter().all(|(key, value)| {
                        !matches!(key.as_str(), "expression" | "expr" | "condition" | "variable" | "alias" | "collection" | "mode")
                            && visit(value, found, depth + 1)
                    })
                }
            }
            Value::Array(values) => values.iter().all(|value| visit(value, found, depth + 1)),
            _ => true,
        }
    }
    let mut found = Vec::new();
    (visit(params, &mut found, 0) && !found.is_empty()).then_some(found)
}

pub(super) fn narrow_literal_modes(rest: &mut super::Rest<'_>) {
    let Some(actions) = literal_modes(rest.params) else { return };
    let doc = rest.doc;
    let previous: Vec<Value> = rest
        .needs
        .values()
        .filter(|(id, _, _)| {
            id.as_str().is_some_and(|key| {
                doc.variables.collection(key).is_some()
                    || doc.variables.collections.iter().any(|collection| collection.modes.iter().any(|m| m.id == key))
            })
        })
        .map(|(id, _, _)| id.clone())
        .collect();
    for id in previous {
        rest.drop(id);
    }
    for action in actions {
        let Some(collection) = doc.variables.collection(action.collection) else { continue };
        let named = collection.id != action.collection;
        rest.need(json!(collection.id), if named { &["name"] } else { &[] }, false);
        rest.under.objects.push(json!(collection.id));
        if named && doc.variables.collections.iter().filter(|c| c.name == action.collection).count() > 1 {
            rest.root(&["collections"]);
        }
        if let Some(mode) = collection.find_mode(action.mode) {
            let named = mode.id != action.mode;
            rest.need(json!(mode.id), if named { &["name"] } else { &[] }, false);
            // A mode must remain in this collection. The SDK's list-position hold also covers
            // its index, although an index alone matters only for duplicate mode names.
            if let Some((_, _, Some(positions))) = rest.needs.get_mut(&json!(mode.id).to_string()) {
                positions.insert("modes".into());
            }
            rest.under.objects.push(json!(mode.id));
            if named && collection.modes.iter().filter(|m| m.name == action.mode).count() > 1 {
                rest.need(json!(collection.id), &["modes"], false);
            }
        }
    }
    rest.under.objects.sort_by_key(Value::to_string);
    rest.under.objects.dedup();
}

fn variable_inputs(doc: &Document, command: &str, params: &Value, under: &mut Under) {
    let variable = params.get("variable").and_then(Value::as_str).and_then(|key| doc.variables.get(key));
    if matches!(command, "variable.bind" | "variable.set" | "variable.resetOverride") {
        under.objects.extend(variable.map(|v| json!(v.id)));
    }
    let collection = params
        .get("collection")
        .and_then(Value::as_str)
        .or_else(|| variable.map(|v| v.collection.as_str()))
        .and_then(|key| doc.variables.collection(key));
    if let Some(collection) = collection {
        under.premises.push(Premise::on(collection.id.clone()));
        for mode in &collection.modes {
            under.premises.push(Premise::on(mode.id.clone()));
        }
        if matches!(command, "variable.addMode" | "variable.removeMode" | "variable.deleteCollection") {
            for v in doc.variables.variables.iter().filter(|v| v.collection == collection.id) {
                under.premises.push(Premise::on(v.id.clone()));
            }
        }
        if command == "variable.add" {
            for key in params.get("values").and_then(Value::as_object).into_iter().flat_map(|m| m.keys()) {
                if let Some(mode) = collection.find_mode(key) {
                    under.objects.push(json!(mode.id));
                }
            }
        }
    }
}

/// Definitions used by bound values can alias other definitions. Visit each stored resource
/// once, including its default-mode order; ids in unrelated JSON values are never guessed.
fn stored_inputs(doc: &Document, under: &mut Under, binding: bool) {
    let mut pending: Vec<_> = under
        .objects
        .iter()
        .chain(under.premises.iter().filter(|p| p.fields.is_none()).map(|p| &p.id))
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    let mut seen = BTreeSet::new();
    while let Some(key) = pending.pop() {
        if !seen.insert(key.clone()) {
            continue;
        }
        if let Some(variable) = doc.variables.get(&key) {
            let need = Premise::on(variable.id.clone());
            under.premises.push(if binding {
                need.fields(["type", "collection", "values", "props.extends"]).positions(Vec::<String>::new())
            } else {
                need
            });
            pending.push(variable.collection.clone());
            pending.extend(variable.extends.iter().cloned());
            for value in variable.values.values() {
                if let kition_design_doc::VariableValue::Alias(alias) = value {
                    pending.push(alias.clone());
                }
            }
        }
        if let Some(collection) = doc.variables.collection(&key) {
            under.premises.push(Premise::on(collection.id.clone()).positions(Vec::<String>::new()));
            pending.extend(collection.extends.iter().cloned());
            under.premises.extend(collection.modes.iter().map(|mode| Premise::on(mode.id.clone()).positions(Vec::<String>::new())));
        }
        if let Some(style) = doc.style(&key) {
            under.premises.push(Premise::on(style.id.clone()));
            pending.extend(style.bound.values().cloned());
        }
    }
}

// The SDK replaces repeated premises within one step. Union them here, with an unbounded
// premise retaining its meaning, so a later geometric hold cannot erase a consumed subtree.
fn merge_premises(premises: &mut Vec<Premise>) {
    let mut merged: BTreeMap<String, Premise> = BTreeMap::new();
    fn union(left: &mut Option<Vec<String>>, right: Option<Vec<String>>) {
        match (left.as_mut(), right) {
            (Some(left), Some(right)) => {
                left.extend(right);
                left.sort();
                left.dedup();
            }
            _ => *left = None,
        }
    }
    for premise in premises.drain(..) {
        if let Some(held) = merged.get_mut(&premise.id.to_string()) {
            union(&mut held.fields, premise.fields);
            union(&mut held.positions, premise.positions);
        } else {
            merged.insert(premise.id.to_string(), premise);
        }
    }
    premises.extend(merged.into_values());
}

/// Component operations read the owner, variant definitions and nested mains during rebuilds.
/// Walk only their dependency graph, bounded by the number of document nodes.
fn component_inputs(doc: &Document, roots: impl IntoIterator<Item = NodeId>, under: &mut Under) {
    let mut pending: Vec<_> = roots.into_iter().collect();
    let mut seen = BTreeSet::new();
    while let Some(root) = pending.pop() {
        if !seen.insert(root) {
            continue;
        }
        let root_need =
            || Premise::on(json!(root.0)).fields(kition_design_kdm::fields::NODE_FIELDS.iter().copied().chain(["nodes"])).positions(["frame"]);
        under.premises.push(root_need());
        for node in doc.subtree(root) {
            let need = Premise::on(json!(node.0));
            under.premises.push(if node == root { root_need() } else { need });
            let Some(n) = doc.node(node) else { continue };
            under.objects.extend(n.bound.values().cloned().map(Value::String));
            under.objects.extend(
                [&n.styles.fill, &n.styles.stroke, &n.styles.effect, &n.styles.text, &n.styles.grid]
                    .into_iter()
                    .flatten()
                    .cloned()
                    .map(Value::String),
            );
            if let NodeKind::Instance { component, props } = &n.kind {
                pending.push(*component);
                if let Some(NodeKind::Component { props: defs, .. }) = doc.node(*component).map(|n| &n.kind) {
                    for (name, value) in props {
                        if matches!(defs.get(name), Some(PropDef::InstanceSwap { .. })) {
                            pending.extend(id(value));
                        }
                    }
                }
            }
            if let Some(source) = n.source {
                pending.push(source);
            }
            if let Some(set) = kition_design_engine::components::variant_set(doc, node) {
                pending.push(set);
            }
            if let NodeKind::Component { props, .. } = &n.kind {
                for def in props.values() {
                    if let PropDef::InstanceSwap { default, .. } = def {
                        pending.push(*default);
                    }
                }
            }
        }
    }
}

fn component_references(doc: &Document, command: &str, params: &Value, ids: &[NodeId], under: &mut Under) {
    let mut roots: Vec<_> = params.get("component").and_then(id).into_iter().collect();
    roots.extend(params.get("layer").and_then(id));
    for node in ids {
        roots.push(*node);
        roots.extend(kition_design_engine::components::owning_component(doc, *node));
        roots.extend(kition_design_engine::components::owning_instance(doc, *node));
    }
    if matches!(command, "component.addProperty" | "component.editProperty" | "component.makeSlot") {
        let name = params.get("name").and_then(Value::as_str).unwrap_or("");
        let def = roots.iter().filter_map(|i| doc.node(*i)).find_map(|n| match &n.kind {
            NodeKind::Component { props, .. } => props.get(name),
            _ => None,
        });
        let kind = params.get("type").and_then(Value::as_str).unwrap_or("").to_ascii_lowercase();
        let swap = matches!(kind.as_str(), "instanceswap" | "instance_swap" | "instance") || matches!(def, Some(PropDef::InstanceSwap { .. }));
        if swap {
            roots.extend(params.get("default").and_then(id));
        }
        if swap || kind == "slot" || command == "component.makeSlot" || matches!(def, Some(PropDef::Slot { .. })) {
            for value in params.get("preferred").and_then(Value::as_array).into_iter().flatten().take(500) {
                if let Some(node) = id(value) {
                    under.premises.push(Premise::on(json!(node.0)).fields(["kind"]).positions(Vec::<String>::new()));
                }
            }
        }
    }
    if command == "instance.setProperties" {
        for node in ids {
            let Some(NodeKind::Instance { component, .. }) = doc.node(*node).map(|n| &n.kind) else { continue };
            let Some(NodeKind::Component { props, .. }) = doc.node(*component).map(|n| &n.kind) else { continue };
            for (name, value) in params.get("properties").and_then(Value::as_object).into_iter().flatten() {
                if matches!(props.get(name), Some(PropDef::InstanceSwap { .. })) {
                    roots.extend(id(value));
                }
            }
        }
    }
    // A removed reference must still be named: subtree() of a missing node is empty.
    for node in &roots {
        under
            .premises
            .push(Premise::on(json!(node.0)).fields(kition_design_kdm::fields::NODE_FIELDS.iter().copied().chain(["nodes"])).positions(["frame"]));
    }
    if matches!(command, "component.editProperty" | "component.deleteProperty") {
        let components: BTreeSet<_> = roots.iter().flat_map(|root| doc.subtree(*root)).collect();
        for node in doc.nodes() {
            if matches!(&node.kind, NodeKind::Instance { component, .. } if components.contains(component)) {
                under.premises.push(Premise::on(json!(node.id.0)).fields(["kind.props", "bindings"]).positions(Vec::<String>::new()));
            }
        }
    }
    component_inputs(doc, roots, under);
}

// Pruning is a query over references, including users created since observation. The root
// owns both trees and resource lists; its derived fact records that query's complete inputs.
fn library_usage(doc: &Document) -> Value {
    json!({
        "nodes": doc.nodes().filter_map(|n| {
            let instance = matches!(n.kind, NodeKind::Instance { .. });
            let text_styles = kition_design_engine::library::style_links(n).collect::<Vec<_>>();
            let linked = [&n.styles.fill, &n.styles.stroke, &n.styles.effect, &n.styles.text, &n.styles.grid].iter().any(|v| v.is_some());
            (instance || linked || !text_styles.is_empty() || !n.bound.is_empty() || !n.var_modes.is_empty() || !n.interactions.is_empty())
                .then(|| json!({"id":n.id,"ancestors":doc.ancestors(n.id),
                    "component": if let NodeKind::Instance { component, .. } = &n.kind { Some(component) } else { None },
                    "styles":n.styles,"textStyles":text_styles,"bound":n.bound,"modes":n.var_modes,"actions":n.interactions}))
        }).collect::<Vec<_>>(),
        "variables": {
            "collections": doc.variables.collections.iter().map(|c| json!({"id":c.id,"extends":c.extends})).collect::<Vec<_>>(),
            "variables": doc.variables.variables.iter().map(|v| {
                let aliases: BTreeSet<&str> = v.values.values().filter_map(|value| match value {
                    kition_design_doc::VariableValue::Alias(target) => Some(target.as_str()),
                    _ => None,
                }).collect();
                json!({"id":v.id,"name":v.name,"collection":v.collection,"extends":v.extends,"aliases":aliases})
            }).collect::<Vec<_>>()
        }
    })
}

fn library_local_inputs(doc: &Document, under: &mut Under) {
    under.premises.push(Premise::on("design").fields(["libraryCopies", "libraries", "~libraryUsage"]).positions(Vec::<String>::new()));
    let roots = doc
        .libraries
        .components
        .iter()
        .map(|r| r.root)
        .chain(doc.nodes().filter_map(|n| matches!(n.kind, NodeKind::Instance { .. }).then_some(n.id)));
    component_inputs(doc, roots, under);
    under.premises.extend(doc.styles.iter().map(|s| Premise::on(s.id.clone())));
    under.premises.extend(doc.variables.collections.iter().map(|c| Premise::on(c.id.clone())));
    under.premises.extend(doc.variables.variables.iter().map(|v| Premise::on(v.id.clone())));
    for node in doc.nodes().filter(|n| !n.bound.is_empty() || n.styles != Default::default()) {
        under.premises.push(Premise::on(json!(node.id.0)).fields(["bindings", "kind"]).positions(Vec::<String>::new()));
    }
}

fn traversed_inputs(
    design: &DesignDocument,
    command: &str,
    params: &Value,
    ids: &[NodeId],
    holds: &mut Holds,
    under: &mut Under,
) -> Result<(), String> {
    let ds = design.ds()?;
    let doc = &ds.doc;
    if command == "edit.findReplace" {
        for node in doc.subtree(ds.page) {
            holds.fields(node, &["~hierarchy"], true);
            if doc.node(node).is_some_and(|n| n.kind.text().is_some()) {
                holds.fields(node, &["kind", "frame", "bindings", "instance"], false);
            }
            if params.get("layerNames").and_then(Value::as_bool) == Some(true) {
                holds.fields(node, &["name"], false);
            }
        }
    }
    if command.starts_with("text.") && !matches!(command, "text.add" | "text.onPath") {
        let global_refit = command == "text.refit" && params.get("ids").is_none_or(Value::is_null);
        let roots = if global_refit { doc.pages.clone() } else { ds.text_edit.as_ref().map_or_else(|| ids.to_vec(), |edit| vec![edit.id]) };
        if global_refit {
            for node in doc.nodes() {
                holds.fields(node.id, &["~hierarchy"], false);
            }
        }
        for node in roots.into_iter().flat_map(|root| doc.subtree(root)).filter(|node| doc.node(*node).is_some_and(|n| n.kind.text().is_some())) {
            holds.fields(node, &["kind", "bindings", "instance", "mirrors", "childLayout", "frame"], true);
        }
    }
    if matches!(command, "layout.add" | "layout.remove" | "layout.suggest" | "layout.setTrack") {
        for node in ids {
            holds.resize(doc, *node);
        }
    }
    if command.starts_with("bool.")
        || matches!(
            command,
            "vector.flatten"
                | "vector.outlineStroke"
                | "object.rasterize"
                | "object.resizeToFit"
                | "arrange.group"
                | "object.frameSelection"
                | "object.wrapInSection"
                | "arrange.ungroup"
                | "object.removeFrame"
                | "object.convertToFrame"
                | "object.convertToGroup"
                | "object.convertToSection"
                | "edit.duplicate"
                | "edit.delete"
                | "edit.cut"
                | "page.duplicate"
                | "page.delete"
        )
    {
        let roots = if command.starts_with("page.") { vec![params.get("id").and_then(id).unwrap_or(ds.page)] } else { ids.to_vec() };
        for root in roots {
            holds.world(doc, root);
            component_inputs(doc, [root], under);
        }
    }
    if command == "library.publish" {
        // Publishing consumes local assets and their dependency graph, not arbitrary canvas layers.
        let assets = kition_design_engine::library::publishable(doc);
        under.premises.push(Premise::on("design").fields(["~assets", "~published"]).positions(Vec::<String>::new()));
        component_inputs(doc, assets.components, under);
        for style in &doc.styles {
            under.premises.push(Premise::on(style.id.clone()));
        }
        for collection in &doc.variables.collections {
            under.premises.push(Premise::on(collection.id.clone()));
        }
        for variable in &doc.variables.variables {
            under.premises.push(Premise::on(variable.id.clone()));
        }
    }
    if command.starts_with("proto.flow.") || command == "proto.settings" {
        holds.fields(ds.page, &[if command == "proto.settings" { "kind.prototype" } else { "kind.flows" }], false);
    }
    Ok(())
}

/// Every stored field of the root that a whole-file read depends on (publication excluded).
const FILE_INPUTS: [&str; 9] = ["settings", "nodes", "libraryStore", "styles", "collections", "variables", "libraries", "libraryCopies", "images"];

fn whole(doc: &Document, roots: impl IntoIterator<Item = NodeId>, under: &mut Under) {
    for root in roots {
        // A removed root must still be named: subtree() of a missing node is empty.
        under.premises.push(Premise::on(json!(root.0)));
        under.premises.extend(doc.subtree(root).into_iter().map(|node| Premise::on(json!(node.0))));
    }
}

/// C8-design review 2 item 3: families whose handlers read beyond the objects they name.
fn audited_inputs(
    design: &DesignDocument,
    command: &str,
    params: &Value,
    ids: &[NodeId],
    holds: &mut Holds,
    under: &mut Under,
) -> Result<(), String> {
    let ds = design.ds()?;
    let doc = &ds.doc;
    let design_fields = |under: &mut Under, fields: &[&str]| {
        under.premises.push(Premise::on("design").fields(fields.iter().copied()).positions(Vec::<String>::new()));
    };
    match command {
        // Paste resolves the session clipboard against this file: copied mains become instances,
        // instances follow their mains, styles/variables/images are resolved by id, and a paste
        // from another file adopts its library links and copies.
        "edit.paste" | "edit.pasteDesignLayers" | "edit.pasteOver" | "edit.pasteReplace" => {
            let clip = &design.session().clipboard;
            let mut mains: Vec<NodeId> = clip.copied_components.iter().copied().collect();
            for node in clip.nodes.iter().flat_map(|(_, nodes)| nodes) {
                if let NodeKind::Instance { component, .. } = &node.kind {
                    mains.push(*component);
                }
                under.objects.extend(node.bound.values().cloned().map(Value::String));
                under.objects.extend(kition_design_engine::library::style_links(node).cloned().map(Value::String));
            }
            whole(doc, mains.iter().copied(), under);
            component_inputs(doc, mains.into_iter().filter(|main| doc.contains(*main)), under);
            design_fields(under, &["libraries", "libraryCopies", "libraryStore", "images"]);
            let parents: Vec<_> =
                params.get("parent").and_then(id).into_iter().chain(ids.iter().filter_map(|node| doc.node(*node)?.parent)).collect();
            for parent in parents {
                holds.world(doc, parent);
                holds.fields(parent, &["frame", "kind", "layout", "~hierarchy"], true);
            }
        }
        // A repeated duplicate steps by the current offset between the last sources and copies.
        "edit.duplicate" if params.get("dx").is_none() && params.get("dy").is_none() => {
            if let Some((sources, copies)) = ds.last_duplicate() {
                for &node in sources.iter().chain(copies) {
                    holds.world(doc, node);
                    holds.fields(node, &["frame", "kind", "childLayout"], true);
                }
            }
        }
        // Imported pages keep resources whose ids exist and add the rest; library records are
        // merged and links re-added.
        "file.import" => design_fields(under, &["styles", "collections", "variables", "libraries", "libraryCopies", "libraryStore"]),
        // Collections, modes and variables are matched by name; new modes copy default values.
        "variable.import" => design_fields(under, &["collections", "variables", "libraries"]),
        // Copies the whole source subtree and finds layers by name in it.
        "data.fill" => {
            let roots: Vec<_> = params.get("id").and_then(id).into_iter().collect();
            whole(doc, roots.iter().copied(), under);
            component_inputs(doc, roots, under);
        }
        // Resolving accepts every change in the subtree since the frame was marked.
        "dev.resolveChange" => whole(doc, ids.iter().copied(), under),
        "node.set" if params.get("props").and_then(|p| p.get("devStatus")).is_some() || params.get("devStatus").is_some() => {
            whole(doc, ids.iter().copied(), under);
        }
        // Links live on the component set or main, and omitted fields keep that owner's link.
        "codeConnect.set" | "codeConnect.remove" => {
            let owners: Vec<_> = ids.iter().filter_map(|node| kition_design_engine::code_connect::owner(doc, *node)).collect();
            whole(doc, owners.iter().copied(), under);
            component_inputs(doc, owners, under);
        }
        // Components are matched by name across the file.
        "codeConnect.import" => {
            design_fields(under, &["nodes"]);
            let components = doc.nodes().filter(|n| matches!(n.kind, NodeKind::Component { .. } | NodeKind::ComponentSet { .. })).map(|n| n.id);
            whole(doc, components.collect::<Vec<_>>(), under);
        }
        // Without `to` a measurement goes to the layer's parent; indexes count in `from`'s list.
        "devMode.addMeasurement" | "devMode.removeMeasurement" => {
            let from = params.get("from").and_then(id).or_else(|| design.session().selection().first().copied());
            if let Some(from) = from {
                under.premises.push(Premise::on(json!(from.0)));
                if command == "devMode.addMeasurement"
                    && params.get("to").is_none_or(Value::is_null)
                    && let Some(parent) = doc.node(from).and_then(|n| n.parent)
                {
                    under.premises.push(Premise::on(json!(parent.0)));
                    holds.fields(from, &["~hierarchy"], true);
                }
            }
        }
        // Rows and columns are addressed by index in the table that `id` is in.
        "board.tableEdit" => {
            let table = params.get("id").and_then(id).and_then(|node| {
                std::iter::once(node)
                    .chain(doc.ancestors(node))
                    .find(|a| doc.node(*a).is_some_and(|n| n.board == Some(kition_design_doc::BoardItem::Table)))
            });
            whole(doc, table, under);
        }
        "board.vote" => {
            let fields = super::vote_fields(design, doc, params);
            design_fields(under, &fields.iter().map(String::as_str).collect::<Vec<_>>());
        }
        "board.timer" => design_fields(under, &["settings.timer"]),
        "devResource.add" | "devResource.edit" | "devResource.remove" => {
            if let Some(&node) = ids.first() {
                holds.fields(node, &["dev.dev_resources"], false);
                for ancestor in std::iter::once(node).chain(doc.ancestors(node)) {
                    holds.fields(ancestor, &["kind.type", "~hierarchy.parent"], false);
                }
            }
        }
        // Widget callbacks run plugin code that may read and write any part of the file.
        "widget.click" | "widget.setText" | "widget.menu" | "widget.insert" => {
            let widget = params.get("id").and_then(id).and_then(|node| kition_design_engine::widgets::widget_of(doc, node));
            whole(doc, widget, under);
            design_fields(under, &FILE_INPUTS);
        }
        // Conversion reads each shape's outline, including all boolean operands. Begin keeps
        // only layer ids in session state; subsequent commands hold those layers again.
        "vector.edit.begin" => {
            whole(doc, ids.iter().copied(), under);
            for &node in ids {
                holds.world(doc, node);
            }
        }
        // Edit mode addresses vertices, segments, handles, regions and width points by index in
        // the edited layers, which come from the session, not from a parameter.
        _ if (command.starts_with("vector.edit.") && command != "vector.edit.begin")
            || matches!(command, "vector.deleteHeal" | "vector.join" | "vector.smoothJoin") =>
        {
            if let Some(edit) = ds.vector_edit.as_ref() {
                let edited: Vec<_> = std::iter::once(edit.id).chain(edit.others.iter().map(|part| part.id)).collect();
                for &node in &edited {
                    holds.world(doc, node);
                    if let Some(parent) = doc.node(node).and_then(|n| n.parent) {
                        holds.fields(parent, &["~hierarchy"], true);
                    }
                }
                whole(doc, edited, under);
            }
        }
        // Regions come from the arrangement of every eligible shape in page space.
        "shapeBuilder.merge" | "shapeBuilder.remove" => {
            for &node in ids {
                holds.world(doc, node);
                if let Some(parent) = doc.node(node).and_then(|n| n.parent) {
                    holds.fields(parent, &["frame", "layout", "~hierarchy"], true);
                }
            }
            whole(doc, ids.iter().copied(), under);
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn inputs(design: &DesignDocument, command: &str, params: &Value) -> Result<Option<Under>, String> {
    let doc = &design.ds()?.doc;
    if command == "variable.update"
        && let Some(variable) = params.get("variable").and_then(Value::as_str).and_then(|key| doc.variables.get(key))
    {
        let mut fields = Vec::new();
        for key in ["name", "description", "scopes", "hiddenFromPublishing", "codeSyntax"] {
            if params.get(key).is_some() {
                fields.push(if key == "name" { key.into() } else { format!("props.{key}") });
            }
        }
        // Validation of metadata also reads the variable type and inheritance status.
        fields.extend(["type".into(), "props.extends".into()]);
        return Ok(Some(Under {
            premises: vec![Premise::on(variable.id.clone()).fields(fields).positions(Vec::<String>::new())],
            ..Under::default()
        }));
    }
    let ids = if crate::acts::targets_any(command).is_some() || crate::acts::reads_any(command).contains(&kition_kit::EditorState::Selection) {
        targets(design, params)
    } else {
        Vec::new()
    };
    let mut holds = Holds::default();
    let mut under = Under::default();
    if crate::commands::switches_tool(command) {
        use kition_design_engine::ToolId;
        let tool = match command {
            "tool.set" | "tool.place" => params.get("tool").and_then(Value::as_str).and_then(ToolId::from_id),
            "edit.pickColor" => Some(ToolId::Eyedropper),
            _ => command.strip_prefix("tool.").and_then(ToolId::from_id),
        };
        if let Some(tool) = tool {
            if !matches!(tool, ToolId::Move | ToolId::Scale | ToolId::Hand)
                && let Some(edit) = design.ds()?.vector_edit.as_ref()
            {
                whole(doc, std::iter::once(edit.id).chain(edit.others.iter().map(|part| part.id)), &mut under);
            }
            if tool != ToolId::Pen {
                whole(doc, design.session().tool.pen.as_ref().and_then(|pen| pen.id), &mut under);
            }
        }
    }
    if command.starts_with("component.") || command.starts_with("instance.") {
        component_references(doc, command, params, &ids, &mut under);
    }
    traversed_inputs(design, command, params, &ids, &mut holds, &mut under)?;
    audited_inputs(design, command, params, &ids, &mut holds, &mut under)?;
    if matches!(command, "library.acceptUpdates" | "library.swap" | "library.remove") {
        library_local_inputs(doc, &mut under);
    }
    match command {
        "tool.place" => {
            for node in tool_place_inputs(design, doc, params).unwrap_or_default() {
                under.objects.push(json!(node.0));
                holds.geometry(doc, node);
                holds.fields(node, &["visible", "locked", "~hierarchy"], true);
            }
            under.covers = Some(kition_kit::Covers {
                fields: ["frame", "kind", "visible", "locked", "props.board", "instance", "mirrors", "~bounds", "~hierarchy"]
                    .iter()
                    .map(|field| (*field).into())
                    .collect(),
            });
        }
        "node.set" => {
            let props = params.get("props").and_then(Value::as_object).or_else(|| params.as_object());
            if let Some(props) = props {
                let keys: Vec<_> = props.keys().filter(|key| !matches!(key.as_str(), "id" | "ids")).collect();
                // A partial dictionary would suppress the SDK's conservative default on unknown keys.
                if keys.iter().all(|key| property(key).is_some()) {
                    for &node in &ids {
                        for key in &keys {
                            holds.fields(node, property(key).unwrap_or(&[]), false);
                        }
                        if keys.iter().any(|key| key.as_str() != "name") && doc.node(node).is_some_and(|n| n.source.is_some()) {
                            holds.fields(node, &["instance", "mirrors", "kind"], true);
                        }
                        if keys.iter().any(|key| {
                            matches!(key.as_str(), "x" | "y" | "rotation" | "width" | "height" | "w" | "h" | "visible" | "strokes" | "stroke")
                        }) {
                            holds.geometry(doc, node);
                        }
                        if keys.iter().any(|key| matches!(key.as_str(), "width" | "height" | "w" | "h")) {
                            holds.resize(doc, node);
                        }
                    }
                }
                // Prototype action grammars and variable aliases carry references inside values.
                if let Some(interactions) = props.get("interactions") {
                    referenced(doc, interactions, &mut under.objects);
                }
                for key in ["boundVariables", "variableModes"] {
                    if let Some(map) = props.get(key).and_then(Value::as_object) {
                        if key == "variableModes" {
                            under.objects.extend(map.keys().cloned().map(Value::String));
                        }
                        under.objects.extend(map.values().filter(|v| v.is_string()).cloned());
                    }
                }
            }
        }
        "object.rename" | "object.batchRename" => {
            for &node in &ids {
                holds.fields(node, &["name"], command == "object.batchRename");
            }
        }
        "object.toggleLock" => {
            for &node in &ids {
                holds.fields(node, &["locked"], false);
            }
        }
        "transform.move" => {
            for &node in &ids {
                holds.world(doc, node);
                if std::iter::once(node)
                    .chain(doc.ancestors(node))
                    .any(|i| doc.node(i).is_some_and(|n| n.layout.is_some() || n.kind.bounds_follow_children()))
                {
                    holds.geometry(doc, node);
                }
            }
        }
        "transform.moveTo"
        | "transform.rotate"
        | "transform.resize"
        | "transform.scale"
        | "align.left"
        | "align.right"
        | "align.top"
        | "align.bottom"
        | "align.hcenter"
        | "align.vcenter"
        | "distribute.horizontal"
        | "distribute.vertical"
        | "arrange.tidy"
        | "arrange.spacing"
        | "arrange.packH"
        | "arrange.packV" => {
            for &node in &ids {
                holds.geometry(doc, node);
                if command == "transform.scale" || command == "transform.resize" {
                    // Scaling styles and resizing constrained children read the entire subtree.
                    under.objects.extend(doc.subtree(node).into_iter().map(|i| json!(i.0)));
                    if command == "transform.scale" {
                        holds.fields(node, kition_design_kdm::fields::NODE_FIELDS, true);
                    }
                }
            }
        }
        "fill.set" | "stroke.set" | "effect.set" | "fill.remove" | "stroke.remove" | "effect.remove" => {
            let field = command.split('.').next().unwrap_or("");
            let field = match field {
                "fill" => "fills",
                "stroke" => "strokes",
                _ => "effects",
            };
            for &node in &ids {
                let n = doc.node(node);
                let len = n.map_or(0, |n| match field {
                    "fills" => n.fills.len(),
                    "strokes" => n.strokes.len(),
                    _ => n.effects.len(),
                });
                let index = params.get("index").and_then(Value::as_u64).and_then(|i| usize::try_from(i).ok()).unwrap_or(len.saturating_sub(1));
                let path = format!("{field}.{index}");
                holds.fields(node, &[&path, "bindings"], false);
                if command.ends_with("remove") {
                    holds.fields(node, &[field], false);
                }
            }
        }
        "node.reparent" => {
            for &node in &ids {
                holds.world(doc, node);
            }
            if let Some(parent) = params.get("parent").and_then(id) {
                holds.geometry(doc, parent);
                if let Some(index) = params.get("index").and_then(Value::as_u64).and_then(|i| usize::try_from(i).ok()) {
                    for child in doc
                        .node(parent)
                        .into_iter()
                        .flat_map(|n| n.children.iter())
                        .filter(|child| !ids.contains(child))
                        .take(index.saturating_add(1))
                    {
                        under.objects.push(json!(child.0));
                        holds.fields(*child, &[], false);
                    }
                    under.within.push(Within { parent: json!(parent.0), field: "nodes".into(), from: 0, to: index.saturating_add(1), time: None });
                }
            }
        }
        "node.reorder" => {
            if let (Some(node), Some(index)) =
                (params.get("id").and_then(id), params.get("index").and_then(Value::as_u64).and_then(|i| usize::try_from(i).ok()))
            {
                holds.fields(node, &[], true);
                if let Some(parent) = doc.node(node).and_then(|n| n.parent) {
                    let current = doc.index_in_parent(node).unwrap_or(0);
                    under.within.push(Within {
                        parent: json!(parent.0),
                        field: "nodes".into(),
                        from: current.min(index),
                        to: current.max(index).saturating_add(1),
                        time: None,
                    });
                }
            }
        }
        "page.move" => {
            let node = params.get("id").and_then(id).unwrap_or(design.ds()?.page);
            if let Some(index) = params.get("index").and_then(Value::as_f64).filter(|n| n.is_finite()) {
                let index = (index.max(0.0) as usize).min(doc.pages.len().saturating_sub(1));
                let current = doc.pages.iter().position(|page| *page == node).unwrap_or(0);
                let from = current.min(index);
                let to = current.max(index).saturating_add(1);
                for page in doc.pages.iter().take(to).skip(from) {
                    holds.fields(*page, &[], true);
                    under.objects.push(json!(page.0));
                }
                under.within.push(Within { parent: json!("design"), field: "nodes".into(), from, to, time: None });
            }
        }
        "instance.add" | "instance.swap" | "instance.slotInsert" => {
            if let Some(component) = params.get("component").and_then(id) {
                under.objects.extend(doc.subtree(component).into_iter().map(|i| json!(i.0)));
            }
            for &node in &ids {
                under.objects.extend(doc.subtree(node).into_iter().map(|i| json!(i.0)));
            }
        }
        "instance.swapAt" | "instance.place" | "library.swapAt" | "library.place" => {
            let Some((component, target)) = point_target(design, doc, command, params) else {
                return Ok(Some(Under {
                    covers: Some(kition_kit::Covers { fields: COVERS.iter().map(|s| (*s).into()).collect() }),
                    ..Under::default()
                }));
            };
            under.objects.push(json!(target.0));
            component_inputs(doc, component, &mut under);
            for node in component.into_iter().flat_map(|node| doc.subtree(node)).chain(if matches!(command, "instance.place" | "library.place") {
                vec![target]
            } else {
                doc.subtree(target)
            }) {
                let need = Premise::on(json!(node.0));
                under.premises.push(if node == target || Some(node) == component {
                    need.fields(kition_design_kdm::fields::NODE_FIELDS.iter().copied().chain(["~bounds"])).positions(["frame"])
                } else {
                    need
                });
            }
            for node in doc.ancestors(target) {
                holds.world(doc, node);
            }
            if matches!(command, "instance.place" | "library.place") {
                holds.geometry(doc, target);
            }
            under.covers = Some(kition_kit::Covers { fields: COVERS.iter().map(|s| (*s).into()).collect() });
        }
        "board.connector" | "board.reattach" => {
            for key in ["from", "to", "target"] {
                if let Some(node) = params.get(key).and_then(id) {
                    under.objects.push(json!(node.0));
                    holds.geometry(doc, node);
                }
            }
            for &node in &ids {
                let connector_id = match doc.node(node).and_then(|n| n.board.as_ref()) {
                    Some(kition_design_doc::BoardItem::Label { connector }) => *connector,
                    _ => node,
                };
                if let Some(kition_design_doc::BoardItem::Connector(connector)) = doc.node(connector_id).and_then(|n| n.board.as_ref()) {
                    holds.fields(connector_id, &["props.board"], false);
                    for end in [&connector.from, &connector.to] {
                        if let Some(node) = end.node() {
                            under.objects.push(json!(node.0));
                            holds.geometry(doc, node);
                        }
                    }
                }
            }
        }
        "style.set" => {
            if let Some(style) = params.get("style").and_then(Value::as_str).and_then(|s| doc.style(s)) {
                let mut fields = Vec::new();
                if params.get("name").is_some() {
                    fields.push("name");
                }
                if params.get("description").is_some() {
                    fields.push("props.description");
                }
                if params.get("value").is_some() || params.get("from").is_some() {
                    fields.extend(["value", "props"]);
                }
                under.premises.push(Premise::on(style.id.clone()).fields(fields).positions(Vec::<String>::new()));
            }
        }
        "variable.bind" => {
            if let Some(field) = params.get("field").and_then(Value::as_str).and_then(property) {
                for &node in &ids {
                    holds.fields(node, field, false);
                    // Binding replaces only this field's binding; styles are independent.
                    if let Some((held, _)) = holds.0.get_mut(&node) {
                        held.remove("bindings");
                        held.insert("bindings.var_modes".into());
                        if let Some(field) = params.get("field").and_then(Value::as_str) {
                            held.insert(format!("bindings.bound.{field}"));
                        }
                    }
                }
            }
        }
        "style.renameFolder" => {
            let prefix = format!("{}/", params.get("folder").and_then(Value::as_str).unwrap_or("").trim().trim_matches('/'));
            for style in doc.styles.iter().filter(|s| s.name.starts_with(&prefix) && !kition_design_engine::library::is_remote_style(doc, &s.id)) {
                under.objects.push(json!(style.id));
                under.premises.push(Premise::on(style.id.clone()).fields(["name"]).positions(Vec::<String>::new()));
            }
            under.covers = Some(kition_kit::Covers { fields: vec!["name".into()] });
        }
        "proto.conditional.addBlock" => {
            if !params.get("else").and_then(kition_design_engine::params::bool_of).unwrap_or(false)
                && ["expression", "condition"].iter().all(|key| params.get(key).is_none_or(Value::is_null))
            {
                under.premises.push(Premise::on("design").fields(["~defaultCondition"]).positions(Vec::<String>::new()));
            }
        }
        "version.restore" => {
            // The restored version's content (the snapshot its checkpoint carries)
            // replaces the whole design: every node, resource and setting it overwrites.
            component_inputs(doc, doc.pages.iter().copied(), &mut under);
            under.premises.push(
                Premise::on("design")
                    .fields(["settings", "nodes", "libraryStore", "styles", "collections", "variables", "libraries", "libraryCopies", "images"])
                    .positions(Vec::<String>::new()),
            );
        }
        "annotation.addCategory" | "annotation.editCategory" | "annotation.removeCategory" => {
            let fields = if command == "annotation.editCategory" && doc.annotation_categories.is_some() {
                let key = params.get("id").cloned().unwrap_or(Value::Null);
                let mut fields = vec![format!("settings.annotation_categories[id={key}]"), format!("settings.annotation_categories[name={key}]")];
                if let Some(name) = params.get("name") {
                    fields.push(format!("settings.annotation_categories[name={name}]"));
                }
                fields
            } else {
                vec!["settings.annotation_categories".into()]
            };
            under.premises.push(Premise::on("design").fields(fields).positions(Vec::<String>::new()));
        }
        "annotation.add" | "annotation.edit" => {
            if let Some(key) = params.get("category").filter(|key| key.as_str().is_some_and(|s| !s.trim().is_empty())) {
                under.premises.push(
                    Premise::on("design")
                        .fields([format!("settings.annotation_categories[id={key}]"), format!("settings.annotation_categories[name={key}]")])
                        .positions(Vec::<String>::new()),
                );
            }
        }
        "image.adjust" | "image.savePreset" | "image.deletePreset" => {
            let key = if command == "image.adjust" {
                params.get("preset").cloned()
            } else {
                params.get("name").and_then(Value::as_str).map(|name| {
                    if command == "image.savePreset" {
                        json!(name.trim().chars().filter(|c| !c.is_control()).take(100).collect::<String>())
                    } else {
                        json!(name)
                    }
                })
            };
            if let Some(key) = key {
                under.premises.push(Premise::on("design").fields([format!("settings.image_presets[name={key}]")]).positions(Vec::<String>::new()));
                // Appending checks MAX_PRESETS (cmd/image.rs); replacing an existing
                // record does not. Other records' contents remain independent.
                if command == "image.savePreset" && !doc.image_presets.iter().any(|preset| Some(preset.name.as_str()) == key.as_str()) {
                    under.premises.push(Premise::on("design").fields(["~imagePresetCount"]).positions(Vec::<String>::new()));
                }
            }
        }
        _ => {}
    }
    if command.starts_with("variable.") {
        variable_inputs(doc, command, params, &mut under);
    }
    if command.starts_with("proto.")
        || command.starts_with("variable.")
        || command == "node.set" && params.get("props").is_some_and(|p| p.get("interactions").is_some())
    {
        referenced(doc, params, &mut under.objects);
        if command == "variable.add" {
            for value in params.get("values").and_then(Value::as_object).into_iter().flat_map(|values| values.values()) {
                referenced(doc, value, &mut under.objects);
            }
        }
        under.covers = Some(kition_kit::Covers { fields: REFERENCES.iter().map(|s| (*s).into()).collect() });
    }
    under.premises.extend(holds.finish());
    let stored = under.premises.len();
    if !command.starts_with("proto.") || literal_modes(params).is_none() {
        stored_inputs(doc, &mut under, command == "variable.bind");
    }
    // `covered_references` answers the definitions a reference place reached, alias chains
    // included; name the same set here, or any covered ask (an unrelated layer's rename) finds
    // them "no longer under the place" and refuses the write as relocated.
    if under.covers.as_ref().is_some_and(|c| c.fields.iter().map(String::as_str).eq(REFERENCES.iter().copied())) {
        let reached: Vec<_> = under.premises.get(stored..).unwrap_or_default().iter().map(|p| p.id.clone()).collect();
        under.objects.extend(reached);
    }
    under.objects.sort_by_key(Value::to_string);
    under.objects.dedup();
    for object in &under.objects {
        if !under.premises.iter().any(|p| p.id == *object) {
            under.premises.push(Premise::on(object.clone()));
        }
    }
    merge_premises(&mut under.premises);
    Ok((!under.premises.is_empty() || !under.objects.is_empty() || !under.within.is_empty() || under.covers.is_some()).then_some(under))
}

/// Puts back what a layer's paint or effect list held at the basis into `fields` (each list the
/// array of its items' values, as `node_from` takes it), from one `before` entry: the whole list
/// (`fills`: the items, each shown as its object's fields or its value), one item (`fills.1`:
/// its object's fields; `null` when it was not there) or a field inside an item's value
/// (`fills.1.paint.color.r`). `None` when `key` names no item list; `Some(false)` when the entry
/// cannot be put back.
fn item_before(fields: &mut BTreeMap<&str, Value>, key: &str, value: &Value) -> Option<bool> {
    let (list, _, value_field) = kition_design_kdm::items::LISTS.iter().find(|(list, _, _)| key == *list || key.starts_with(&format!("{list}.")))?;
    let of_item = |item: &Value| item.get(*value_field).cloned().unwrap_or_else(|| item.clone());
    let rest = key.strip_prefix(*list).and_then(|r| r.strip_prefix('.'));
    let Some(rest) = rest else {
        // The guard gives the list as it is now (each item its object's id) with the edits since
        // the basis written over it: an item's edited fields (`{paint: {color: {r}}}`) go over its
        // value now, `null` is an item made since (it was not there), an id an item as it is.
        let Some(items) = value.as_array() else { return Some(false) };
        let Some(Value::Array(now)) = fields.get(*list).cloned() else { return Some(false) };
        let mut then = Vec::with_capacity(items.len());
        for (index, item) in items.iter().enumerate() {
            let current = now.get(index).cloned();
            match (item, current) {
                (Value::Null, _) => {}
                (Value::String(_), Some(current)) => then.push(current),
                (Value::Object(_), current) => {
                    let mut value = current.unwrap_or(Value::Null);
                    if let Some(patch) = item.get(*value_field) {
                        merge_over(&mut value, patch);
                    }
                    then.push(value);
                }
                _ => return Some(false),
            }
        }
        fields.insert(*list, Value::Array(then));
        return Some(true);
    };
    let mut parts = rest.split('.');
    let Some(index) = parts.next().and_then(|i| i.parse::<usize>().ok()) else { return Some(false) };
    let Some(Value::Array(items)) = fields.get_mut(*list) else { return Some(false) };
    match parts.next() {
        None => {
            if value.is_null() {
                if index < items.len() {
                    items.remove(index);
                }
            } else if index < items.len() {
                items[index] = of_item(value);
            } else if index == items.len() {
                items.push(of_item(value));
            } else {
                return Some(false);
            }
            Some(true)
        }
        // An item's bindings do not decide what a point picks.
        Some("bound") => Some(true),
        Some(field) if field == *value_field => {
            let Some(mut at) = items.get_mut(index) else { return Some(false) };
            for part in parts {
                let next = match at {
                    Value::Object(map) => map.get_mut(part),
                    Value::Array(list) => part.parse::<usize>().ok().and_then(|i| list.get_mut(i)),
                    _ => None,
                };
                let Some(next) = next else { return Some(false) };
                at = next;
            }
            *at = value.clone();
            Some(true)
        }
        Some(_) => Some(false),
    }
}

/// Writes `patch` over `value`: objects field by field, anything else whole.
fn merge_over(value: &mut Value, patch: &Value) {
    match (value, patch) {
        (Value::Object(into), Value::Object(fields)) => {
            for (key, field) in fields {
                merge_over(into.entry(key.clone()).or_insert(Value::Null), field);
            }
        }
        (value, patch) => *value = patch.clone(),
    }
}

/// Replay the engine picker with all known prior registers installed together. Tree moves and
/// deleted nodes lack enough hierarchy in CoveredAsk; refuse those instead of guessing.
pub(crate) fn covered(
    design: &DesignDocument,
    command: &str,
    params: &Value,
    asked: &[kition_kit::CoveredAsk],
) -> Result<Option<kition_kit::CoveredAt>, String> {
    if command == "style.renameFolder" {
        let prefix = format!("{}/", params.get("folder").and_then(Value::as_str).unwrap_or("").trim().trim_matches('/'));
        return Ok(Some(kition_kit::CoveredAt {
            covered: asked
                .iter()
                .map(|ask| {
                    let was = ask.before.get("name").and_then(Value::as_str);
                    kition_kit::Covered {
                        id: ask.id.clone(),
                        covered: if ask.created { Some(false) } else { was.map(|n| n.starts_with(&prefix)) },
                        ..Default::default()
                    }
                })
                .collect(),
            under: Vec::new(),
        }));
    }
    if command.starts_with("proto.") || command.starts_with("variable.") || command == "node.set" {
        return covered_references(design, command, params, asked);
    }
    if !matches!(command, "instance.swapAt" | "instance.place" | "library.swapAt" | "library.place" | "tool.place") {
        return Ok(None);
    }
    let mut before = (*design.ds()?.doc).clone();
    for ask in asked {
        if id(&ask.id).is_none() {
            continue;
        }
        if !ask.unknown.is_empty() || (!ask.created && ask.before.is_empty()) {
            return Ok(None);
        }
        if ask.deleted {
            continue;
        }
        let Some(node) = id(&ask.id) else {
            return Ok(None);
        };
        if ask.created {
            if before.node(node).is_some_and(|n| !n.children.is_empty()) {
                return Ok(None);
            }
            if before.contains(node) {
                before.remove(node).map_err(crate::engine_error)?;
            }
            continue;
        }
        let Some(current) = before.node(node) else {
            return Ok(None);
        };
        let mut fields: BTreeMap<&str, Value> =
            kition_design_kdm::fields::node_fields(current, None).map_err(crate::engine_error)?.into_iter().collect();
        // The item lists as `node_from` takes them (each the array of its values), the layer's own.
        for (list, _, _) in kition_design_kdm::items::LISTS {
            let values: Vec<Value> =
                kition_design_kdm::items::items(current, list).map_err(crate::engine_error)?.into_iter().map(|item| item.value).collect();
            fields.insert(list, Value::Array(values));
        }
        for (key, value) in &ask.before {
            // Derived facts and the reserved prior list position (`$at`) are put back below.
            if key.starts_with('~') || key == "$at" {
                continue;
            }
            // A text's characters never decide what a point picks; a paint list's items are put
            // back by value (an item's object shows as its fields: `paint` or `effect`).
            if key == "text" {
                continue;
            }
            if let Some(restored) = item_before(&mut fields, key, value) {
                if !restored {
                    return Ok(None);
                }
                continue;
            }
            if !kition_design_kdm::fields::NODE_REGISTERS.contains(&key.as_str()) {
                return Ok(None);
            }
            fields.insert(key, value.clone());
        }
        let Ok((mut restored, _)) = kition_design_kdm::fields::node_from(node, &fields, current.parent, current.children.clone()) else {
            return Ok(None);
        };
        // What the registers do not hold: the bindings of the items' fields and the text.
        for (key, variable) in &current.bound {
            if kition_design_kdm::items::item_key(current, key).is_some() {
                restored.bound.entry(key.clone()).or_insert_with(|| variable.clone());
            }
        }
        if let (NodeKind::Text(was), NodeKind::Text(now)) = (&current.kind, &mut restored.kind) {
            now.characters.clone_from(&was.characters);
            now.runs.clone_from(&was.runs);
            now.paragraphs.clone_from(&was.paragraphs);
        }
        let transform = restored.transform;
        *before.get_mut(node).map_err(crate::engine_error)? = restored;
        if let Some(parent) = ask.before.get("~hierarchy").and_then(|h| h.get("parent")).and_then(id) {
            if before.node(node).and_then(|n| n.parent) != Some(parent) {
                if before.reparent(node, parent, None).is_err() {
                    return Ok(None);
                }
                before.get_mut(node).map_err(crate::engine_error)?.transform = transform;
            }
            if let Some(index) = ask.before["~hierarchy"]["index"].as_u64().and_then(|i| usize::try_from(i).ok()) {
                before.reorder(node, index).map_err(crate::engine_error)?;
            }
        }
        // Where a layer others moved was in its layer list at the basis (the SDK's reserved
        // `$at`: `{parent, field, index}`): stacking order decides what a point picks. A layer
        // list other than a layer's children (a page, the design's own) cannot be put back here.
        if let Some(at) = ask.before.get("$at") {
            let (Some(parent), Some(index)) = (at.get("parent").and_then(id), at.get("index").and_then(Value::as_u64)) else {
                return Ok(None);
            };
            let Ok(index) = usize::try_from(index) else {
                return Ok(None);
            };
            if !before.contains(parent) || before.pages.contains(&node) {
                return Ok(None);
            }
            if before.node(node).and_then(|n| n.parent) != Some(parent) {
                if before.reparent(node, parent, None).is_err() {
                    return Ok(None);
                }
                before.get_mut(node).map_err(crate::engine_error)?.transform = transform;
            }
            before.reorder(node, index).map_err(crate::engine_error)?;
        }
    }
    // A deleted leaf outside the point cannot have affected the pick. For a deleted subtree,
    // overlapping bounds, or incomplete history the journal cannot prove this: return unknown.
    // (A layer's paint and effect objects go with it: only layers are asked about here.)
    for ask in asked.iter().filter(|ask| ask.deleted && id(&ask.id).is_some()) {
        let hierarchy = &ask.before["~hierarchy"];
        if hierarchy["children"].as_u64() != Some(0) {
            return Ok(None);
        }
        let Some(parent) = hierarchy.get("parent").and_then(id) else {
            return Ok(None);
        };
        let Some(bounds) = ask.before.get("~bounds").and_then(Value::as_array).filter(|a| a.len() == 4) else {
            return Ok(None);
        };
        let coordinates: Option<Vec<_>> = bounds.iter().map(Value::as_f64).collect();
        let Some(coordinates) = coordinates else {
            return Ok(None);
        };
        let transform = ask.before.get("frame").and_then(|f| f.get("transform")).cloned();
        let Some(local) = transform.and_then(|v| serde_json::from_value::<kition_design_geom::Affine>(v).ok()) else {
            return Ok(None);
        };
        let rect = kition_design_geom::Rect::new(coordinates[0], coordinates[1], coordinates[2], coordinates[3]);
        let world = before.world_transform(parent) * local;
        let view = &design.ds()?.view;
        let mut at = kition_design_geom::Point::new(
            params.get("x").and_then(Value::as_f64).unwrap_or(view.cx),
            params.get("y").and_then(Value::as_f64).unwrap_or(view.cy),
        );
        if command == "tool.place"
            && let Some((tool, start, area)) = tool_place_area(design, params)
        {
            at = start;
            let bounds = kition_design_geom::transform_rect_bbox(world, rect);
            if matches!(tool, kition_design_engine::ToolId::Frame | kition_design_engine::ToolId::Section)
                && area.contains(bounds.origin())
                && area.contains(kition_design_geom::Point::new(bounds.x1, bounds.y1))
            {
                return Ok(None);
            }
        }
        if !world.determinant().is_finite() || world.determinant().abs() < 1e-12 || rect.contains(world.inverse() * at) {
            return Ok(None);
        }
    }
    let picked = if command == "tool.place" {
        tool_place_inputs(design, &before, params).unwrap_or_default()
    } else {
        point_target(design, &before, command, params).map(|(_, target)| target).into_iter().collect()
    };
    Ok(Some(kition_kit::CoveredAt {
        under: picked.iter().map(|node| json!(node.0)).collect(),
        covered: asked
            .iter()
            .map(|ask| kition_kit::Covered {
                id: ask.id.clone(),
                covered: Some(id(&ask.id).is_some_and(|node| picked.iter().any(|target| node == *target || before.is_ancestor(node, *target)))),
                ..Default::default()
            })
            .collect(),
    }))
}

/// Replay dynamic expression names with the prior resource names installed together. Deleted
/// definitions have no complete prior type/value in this SDK journal, so their replay is unknown.
fn covered_references(
    design: &DesignDocument,
    command: &str,
    params: &Value,
    asked: &[kition_kit::CoveredAsk],
) -> Result<Option<kition_kit::CoveredAt>, String> {
    let mut before = (*design.ds()?.doc).clone();
    for ask in asked {
        if id(&ask.id).is_some() || ask.id == "design" {
            continue;
        }
        let Some(key) = ask.id.as_str() else { return Ok(None) };
        if ask.deleted || !ask.unknown.is_empty() {
            return Ok(None);
        }
        if ask.created {
            before.variables.variables.retain(|v| v.id != key);
            before.variables.collections.retain(|c| c.id != key);
            for c in &mut before.variables.collections {
                c.modes.retain(|m| m.id != key);
            }
            continue;
        }
        if let Some(v) = before.variables.variables.iter_mut().find(|v| v.id == key) {
            if let Some(name) = ask.before.get("name").and_then(Value::as_str) {
                v.name = name.into();
            }
            if let Some(collection) = ask.before.get("collection").and_then(Value::as_str) {
                v.collection = collection.into();
            }
            if let Some(kind) = ask.before.get("type") {
                v.kind = serde_json::from_value(kind.clone()).map_err(crate::engine_error)?;
            }
        }
        for c in &mut before.variables.collections {
            if c.id == key
                && let Some(name) = ask.before.get("name").and_then(Value::as_str)
            {
                c.name = name.into();
            }
            if let Some(mode) = c.modes.iter_mut().find(|m| m.id == key)
                && let Some(name) = ask.before.get("name").and_then(Value::as_str)
            {
                mode.name = name.into();
            }
        }
    }
    let mut objects = Vec::new();
    referenced(&before, params, &mut objects);
    if let Some(values) = params.get("values").and_then(Value::as_object) {
        for value in values.values() {
            referenced(&before, value, &mut objects);
        }
    }
    if let Some(variable) = params.get("variable").and_then(Value::as_str).and_then(|key| before.variables.get(key)) {
        objects.push(json!(variable.id));
    }
    let mut inputs = Under { objects, ..Under::default() };
    if !command.starts_with("proto.") || literal_modes(params).is_none() {
        stored_inputs(&before, &mut inputs, false);
    }
    let mut objects = inputs.objects;
    objects.extend(inputs.premises.into_iter().map(|p| p.id));
    objects.sort_by_key(Value::to_string);
    objects.dedup();
    Ok(Some(kition_kit::CoveredAt {
        covered: asked
            .iter()
            .map(|ask| kition_kit::Covered { id: ask.id.clone(), covered: Some(objects.contains(&ask.id)), ..Default::default() })
            .collect(),
        under: objects,
    }))
}

fn snapshot_fingerprint(doc: &Document) -> Result<String, String> {
    let mut digest = Sha256::new();
    digest.update(serde_json::to_vec(doc).map_err(crate::engine_error)?);
    for (key, image) in &doc.images {
        digest.update(serde_json::to_vec(&(key, image.width, image.height, image.bytes.len())).map_err(crate::engine_error)?);
        digest.update(image.bytes.as_slice());
    }
    Ok(format!("{:x}", digest.finalize()))
}

pub(super) fn name_key(name: &str) -> String {
    format!("{:x}", Sha256::digest(name.as_bytes()))
}

pub(crate) fn derived(design: &DesignDocument, ids: &[Value], names: &[String]) -> Result<Vec<kition_kit::DerivedValues>, String> {
    let ds = design.ds()?;
    let doc = &ds.doc;
    let wanted: Vec<_> = if ids.is_empty() { doc.ids().collect() } else { ids.iter().filter_map(id).collect() };
    let mut out: Vec<_> = wanted
        .into_iter()
        .filter_map(|node| {
            let n = doc.node(node)?;
            let bounds = n.own_paint_bounds(kition_design_geom::Affine::IDENTITY);
            let mut values = serde_json::Map::new();
            if names.is_empty() || names.iter().any(|s| s == "bounds") {
                values.insert("bounds".into(), json!([bounds.x0, bounds.y0, bounds.x1, bounds.y1]));
            }
            if names.is_empty() || names.iter().any(|s| s == "width") {
                values.insert("width".into(), json!(n.width));
            }
            if names.is_empty() || names.iter().any(|s| s == "height") {
                values.insert("height".into(), json!(n.height));
            }
            if names.is_empty() || names.iter().any(|s| s == "transform") {
                values.insert("transform".into(), json!(n.transform));
            }
            if names.is_empty() || names.iter().any(|s| s == "hierarchy") {
                values
                    .insert("hierarchy".into(), json!({"parent":n.parent.map(|p|p.0),"index":doc.index_in_parent(node),"children":n.children.len()}));
            }
            if names.is_empty() || names.iter().any(|s| s == "sourceOpacity") {
                values.insert(
                    "sourceOpacity".into(),
                    json!(n.source.and_then(|id| doc.node(id)).map(|s| json!({"opacity":s.opacity,"bound":s.bound.get("opacity")}))),
                );
            }
            if names.is_empty() || names.iter().any(|s| s == "boundVariables") {
                values.insert("boundVariables".into(), json!(n.bound));
            }
            if names.is_empty() || names.iter().any(|s| s == "styleLinks") {
                values.insert("styleLinks".into(), json!(n.styles));
            }
            if names.is_empty() || names.iter().any(|s| s == "textNodes") {
                let mut text: Vec<u64> =
                    doc.subtree(node).into_iter().filter(|id| doc.node(*id).is_some_and(|n| n.kind.text().is_some())).map(|id| id.0).collect();
                text.sort_unstable();
                values.insert("textNodes".into(), json!(text));
            }
            if names.is_empty() || names.iter().any(|s| s == "children") {
                values.insert("children".into(), json!(n.children));
            }
            Some(kition_kit::DerivedValues { id: json!(node.0), values })
        })
        .collect();
    if ids.is_empty() || ids.iter().any(|id| id == "design") {
        let mut values = serde_json::Map::new();
        if names.is_empty() || names.iter().any(|n| n == "textNodes") {
            values.insert("textNodes".into(), json!(doc.nodes().filter(|n| n.kind.text().is_some()).map(|n| n.id).collect::<Vec<_>>()));
        }
        if names.is_empty() || names.iter().any(|n| n == "variableNames") {
            values.insert("variableNames".into(), json!({
                "collections": doc.variables.collections.iter().map(|c| json!({"id":c.id,"name":c.name,"extends":c.extends,"remote":kition_design_engine::library::is_remote_collection(doc, &c.id)})).collect::<Vec<_>>(),
                "variables": doc.variables.variables.iter().map(|v| json!({"id":v.id,"collection":v.collection,"name":v.name,"type":v.kind})).collect::<Vec<_>>()
            }));
        }
        if names.is_empty() || names.iter().any(|n| n == "codeOwners") {
            let mut owners: BTreeMap<String, Vec<u64>> = BTreeMap::new();
            for n in doc.nodes().filter(|n| kition_design_engine::code_connect::owner(doc, n.id) == Some(n.id)) {
                owners.entry(name_key(&n.name)).or_default().push(n.id.0);
            }
            for ids in owners.values_mut() {
                ids.sort_unstable();
            }
            values.insert("codeOwners".into(), json!(owners));
        }
        if names.is_empty() || names.iter().any(|n| n == "imagePresetCount") {
            values.insert("imagePresetCount".into(), json!(doc.image_presets.len()));
        }
        if names.is_empty() || names.iter().any(|n| n == "defaultCondition") {
            values.insert(
                "defaultCondition".into(),
                serde_json::to_value(kition_design_engine::cmd::proto_actions::default_condition(&doc.variables)).map_err(crate::engine_error)?,
            );
        }
        if names.is_empty() || names.iter().any(|n| n == "assets") {
            let assets = kition_design_engine::library::publishable(doc);
            let variables: Vec<_> = doc.variables.variables.iter().filter(|v| assets.collections.contains(&v.collection)).map(|v| &v.id).collect();
            values.insert(
                "assets".into(),
                json!({"components":assets.components,"styles":assets.styles,"collections":assets.collections,"variables":variables}),
            );
        }
        if names.is_empty() || names.iter().any(|n| n == "libraryUsage") {
            values.insert("libraryUsage".into(), library_usage(doc));
        }
        if names.is_empty() || names.iter().any(|n| n == "published") {
            let published = doc
                .published
                .as_ref()
                .map(|p| -> Result<Value, String> { Ok(json!({"manifest":p.manifest,"content":snapshot_fingerprint(&p.doc)?})) })
                .transpose()?;
            values.insert("published".into(), json!(published));
        }
        if !values.is_empty() {
            out.push(kition_kit::DerivedValues { id: json!("design"), values });
        }
    }
    Ok(out)
}

pub(crate) fn carried(design: &DesignDocument, ids: &[Value], changes: &[Value]) -> Result<Vec<kition_kit::Carried>, String> {
    let doc = &design.ds()?.doc;
    Ok(ids
        .iter()
        .filter_map(|value| {
            let node = id(value)?;
            let moved = changes.iter().any(|change| change.get("id") == Some(value) && change.get("from").is_some());
            let carries = !moved
                && changes.iter().any(|change| {
                    change.get("id") == Some(value)
                        && (change["fields"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(Value::as_str)
                            .any(|field| field == "frame.transform" || field.starts_with("frame.transform.")))
                });
            let count = doc.subtree(node).len().saturating_sub(1) as u64;
            (carries && count > 0)
                .then(|| kition_kit::Carried { id: value.clone(), carries: kition_kit::Carries { count, what: "world location".into() } })
        })
        .collect())
}
