//! What each write rests on, from the independent reading of its handler (task card
//! C8-design-read, `tests/declarations_independent.rs`). Shared resolvers establish spatial picks,
//! references and dependency graphs; each family specifies the fields and positions its handler
//! consumes. One file per family of commands; the vocabulary is here.
//!
//! A premise holds an object's fields (its registers, `props.<key>`, derived `~bounds` and
//! `~hierarchy`) and its positions: `frame` (where and how large it is: a position field, held
//! only as a position) and `nodes`/`libraryStore` (its place in the layer tree). An object the
//! call names that no premise lists keeps every field and every position.

use std::collections::{BTreeMap, BTreeSet};

use kition_design_doc::{Document, NodeId};
use kition_kit::{Premise, Under};
use serde_json::{Value, json};

use crate::DesignDocument;

mod batch;
pub(crate) use batch::scratch;
mod edit;
mod layout;
pub(crate) mod resolve;
mod resources;
mod structure;
mod text;
mod vector;

/// The premises of one call, by object, as the guard reads them.
pub(crate) struct Rest<'a> {
    pub(crate) design: &'a DesignDocument,
    pub(crate) doc: &'a Document,
    pub(crate) command: &'a str,
    pub(crate) params: &'a Value,
    /// Object id (as the premise names it) → (fields, positions); `None` is every one.
    needs: BTreeMap<String, Need>,
    pub(crate) under: Under,
}

/// What one premise holds: the object's id, its fields and its positions (`None`: every one).
type Need = (Value, Option<BTreeSet<String>>, Option<BTreeSet<String>>);

/// The tree lists a layer's place is in.
const PLACE: [&str; 2] = ["nodes", "libraryStore"];

fn union(into: &mut Option<BTreeSet<String>>, more: Option<BTreeSet<String>>) {
    match (into.as_mut(), more) {
        (Some(known), Some(more)) => known.extend(more),
        _ => *into = None,
    }
}

/// A field list split as the guard reads it: `frame…` paths are positions, the rest fields;
/// `place` adds the layer's place in the tree.
fn split(fields: &[&str], place: bool) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut held = BTreeSet::new();
    let mut positions = BTreeSet::new();
    for field in fields {
        if *field == "frame.transform" {
            held.insert("~transform".to_owned());
        } else if matches!(*field, "frame.width" | "frame.height") {
            held.insert(field.replacen("frame.", "~", 1));
        } else if *field == "frame" || field.starts_with("frame.") {
            positions.insert("frame".to_owned());
        } else {
            held.insert((*field).to_owned());
        }
    }
    if place {
        positions.extend(PLACE.iter().map(|p| (*p).to_owned()));
    }
    (held, positions)
}

impl<'a> Rest<'a> {
    fn new(design: &'a DesignDocument, doc: &'a Document, command: &'a str, params: &'a Value, under: Under) -> Self {
        let mut rest = Self { design, doc, command, params, needs: BTreeMap::new(), under: Under::default() };
        for premise in &under.premises {
            rest.merge(
                premise.id.clone(),
                premise.fields.clone().map(|f| f.into_iter().collect()),
                premise.positions.clone().map(|p| p.into_iter().collect()),
            );
        }
        rest.under = Under { premises: Vec::new(), ..under };
        rest
    }

    fn merge(&mut self, id: Value, fields: Option<BTreeSet<String>>, positions: Option<BTreeSet<String>>) {
        let key = id.to_string();
        match self.needs.get_mut(&key) {
            Some((_, known, at)) => {
                union(known, fields);
                union(at, positions);
            }
            None => {
                self.needs.insert(key, (id, fields, positions));
            }
        }
    }

    /// The call reads `fields` of `id` (a layer, or a resource's key, or `"design"`), and its place in
    /// the tree when `place`: added to what it already holds.
    pub(crate) fn need(&mut self, id: impl Into<Value>, fields: &[&str], place: bool) {
        let (fields, positions) = split(fields, place);
        self.merge(id.into(), Some(fields), Some(positions));
    }

    /// The call reads every field of `id`, and its place when `place`.
    pub(crate) fn whole(&mut self, id: impl Into<Value>, place: bool) {
        let id = id.into();
        let layer = id.as_u64().is_some();
        // Every stored register excludes derived hierarchy when sibling order is immaterial.
        let fields =
            (!place && layer).then(|| kition_design_kdm::fields::NODE_REGISTERS.iter().copied().chain(["nodes"]).map(str::to_owned).collect());
        let positions = if place { None } else { Some(if layer { BTreeSet::from(["frame".to_owned()]) } else { BTreeSet::new() }) };
        self.merge(id, fields, positions);
    }

    /// What the call rests on of `id` is exactly this (what was held before is dropped): for a
    /// call the earlier declarations held more of than it reads.
    pub(crate) fn only(&mut self, id: impl Into<Value>, fields: &[&str], place: bool) {
        let id = id.into();
        let (fields, positions) = split(fields, place);
        self.needs.insert(id.to_string(), (id, Some(fields), Some(positions)));
    }

    /// Drops what the call was said to rest on of `id`: it reads nothing of it.
    pub(crate) fn drop(&mut self, id: impl Into<Value>) {
        let id = id.into();
        self.needs.remove(&id.to_string());
        self.under.objects.retain(|object| *object != id);
    }

    /// Fields of the design's root.
    pub(crate) fn root(&mut self, fields: &[&str]) {
        self.need("design", fields, false);
    }

    /// What the engine's clean-up after an edit of `node`'s size, visibility or place re-computes
    /// from: an auto layout parent's layout and the siblings it re-flows, a group's other
    /// children it refits to, up the ancestors that follow their children.
    pub(crate) fn reflow(&mut self, node: NodeId) {
        let doc = self.doc;
        let mut at = node;
        while let Some(parent) = doc.node(at).and_then(|n| n.parent) {
            let Some(p) = doc.node(parent) else { break };
            if p.is_page() || !(p.layout.is_some() || p.kind.bounds_follow_children()) {
                break;
            }
            self.need(parent.0, &["layout", "childLayout", "frame", "kind"], false);
            for sibling in p.children.iter().copied().filter(|c| *c != at) {
                self.need(sibling.0, &["frame", "visible", "childLayout"], true);
            }
            at = parent;
        }
    }

    /// The variables bound to `node`'s fields, their collections' modes and the aliases they
    /// follow, and the styles it is linked to: what the clean-up writes bound values from.
    pub(crate) fn bound(&mut self, node: NodeId) {
        let Some(n) = self.doc.node(node) else { return };
        let mut pending: Vec<String> = n.bound.values().cloned().collect();
        let styles: Vec<String> =
            [&n.styles.fill, &n.styles.stroke, &n.styles.effect, &n.styles.text, &n.styles.grid].into_iter().flatten().cloned().collect();
        for style in styles {
            self.need(json!(style), &["value", "props"], false);
        }
        let mut seen = BTreeSet::new();
        while let Some(key) = pending.pop() {
            if !seen.insert(key.clone()) {
                continue;
            }
            if let Some(variable) = self.doc.variables.get(&key) {
                self.need(json!(variable.id), &["type", "values", "props.extends", "props.overrides"], false);
                if let Some(collection) = self.doc.variables.collection(&variable.collection) {
                    self.need(json!(collection.id), &["modes"], false);
                }
                for value in variable.values.values() {
                    if let kition_design_doc::VariableValue::Alias(alias) = value {
                        pending.push(alias.clone());
                    }
                }
                pending.extend(variable.extends.iter().cloned());
            }
        }
    }

    /// The comment threads pinned to `node` (the clean-up re-pins them when it goes or moves).
    pub(crate) fn threads(&mut self, node: NodeId) {
        let ids: Vec<u64> = self.doc.comments.iter().filter(|c| c.anchor == Some(node)).map(|c| c.id).collect();
        for id in ids {
            self.need(id, &["anchor"], false);
        }
    }

    /// The layers the call acts on: `ids`, `id`, else the selection.
    pub(crate) fn targets(&self) -> Vec<NodeId> {
        node_list(self.params.get("ids"))
            .or_else(|| self.params.get("id").and_then(node_id).map(|id| vec![id]))
            .unwrap_or_else(|| self.design.session().selection().to_vec())
    }

    /// The layer a parameter names, if any.
    pub(crate) fn node(&self, key: &str) -> Option<NodeId> {
        self.params.get(key).and_then(node_id).filter(|id| self.doc.contains(*id))
    }

    /// The current page.
    pub(crate) fn page(&self) -> Option<NodeId> {
        self.design.ds().ok().map(|ds| ds.page)
    }

    fn finish(self) -> Under {
        let mut under = self.under;
        under.premises = self
            .needs
            .into_values()
            .map(|(id, fields, positions)| {
                let mut premise = Premise::on(id);
                premise.fields = fields.map(|f| f.into_iter().collect());
                premise.positions = positions.map(|p| p.into_iter().collect());
                premise
            })
            .collect();
        under
    }
}

pub(crate) fn node_id(value: &Value) -> Option<NodeId> {
    value.as_u64().or_else(|| value.as_str()?.parse().ok()).map(NodeId)
}

fn node_list(value: Option<&Value>) -> Option<Vec<NodeId>> {
    match value? {
        Value::Array(items) => Some(items.iter().filter_map(node_id).collect()),
        other => node_id(other).map(|id| vec![id]),
    }
}

/// `under` with what the reading found each write rests on.
pub(crate) fn under(design: &DesignDocument, command: &str, params: &Value) -> Result<Option<kition_kit::Under>, String> {
    if command == "command.batch" {
        return batch::under(design, params);
    }
    let earlier = resolve::inputs(design, command, params)?;
    let doc = &design.ds()?.doc;
    let mut rest = Rest::new(design, doc, command, params, earlier.unwrap_or_default());
    let family = command.split('.').next().unwrap_or_default();
    match family {
        "edit" | "object" | "command" | "data" | "page" | "file" | "branch" | "version" | "comment" | "board" | "tool" | "widget" => {
            edit::rest(&mut rest)
        }
        "arrange" | "align" | "distribute" | "transform" | "node" | "frame" | "shape" | "layout" | "section" | "guide" => layout::rest(&mut rest),
        "text" => text::rest(&mut rest),
        "vector" | "bool" | "shapeBuilder" => vector::rest(&mut rest),
        "component" | "instance" | "library" | "proto" | "dev" | "codeConnect" | "annotation" | "devResource" | "devMode" => {
            structure::rest(&mut rest)
        }
        _ => resources::rest(&mut rest),
    }
    let under = rest.finish();
    Ok((!under.premises.is_empty() || !under.objects.is_empty() || !under.within.is_empty() || under.covers.is_some()).then_some(under))
}
