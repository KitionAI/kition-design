//! What the resources family's writes rest on (see [`super`]): paints, effects, layout grids and
//! export settings of layers, styles, variables and their modes, the selection's colours, image
//! adjustments.
//!
//! Every edit ends in the engine's clean-up (`crates/engine/engine/src/derived.rs:15`
//! `normalize`): it writes bound values from their variables in the layer's modes
//! (`cmd/styles.rs:989` `apply_bindings_to`), unlinks a list that no longer shows its linked style
//! (`cmd/styles.rs:761` `sync_list_styles`), records overrides inside instances and carries a main
//! layer's edit to its copies (`assets/components/patch.rs:9` `sync_after_edit`), and re-flows an
//! auto layout parent when strokes change (`derived.rs:162` `affects_layout`). The writes here
//! rest on those inputs too.

use std::collections::BTreeSet;

use kition_design_doc::{Document, Node, NodeId, NodeKind, StyleKind, Variable, VariableValue};
use kition_design_engine::cmd::lists::{BoundList, list_binding};
use serde_json::{Value, json};

use super::Rest;

/// One of a layer's lists the family edits.
#[derive(Clone, Copy, PartialEq, Eq)]
enum List {
    Fills,
    Strokes,
    Effects,
    Grids,
    Exports,
}

impl List {
    /// The field the list is in.
    fn field(self) -> &'static str {
        match self {
            List::Fills => "fills",
            List::Strokes => "strokes",
            List::Effects => "effects",
            List::Grids => "props.grids",
            List::Exports => "props.exports",
        }
    }

    /// The list's items carry variable bindings and a style link.
    fn bound(self) -> Option<BoundList> {
        match self {
            List::Fills => Some(BoundList::Fills),
            List::Strokes => Some(BoundList::Strokes),
            List::Effects => Some(BoundList::Effects),
            List::Grids => Some(BoundList::Grids),
            List::Exports => None,
        }
    }

    /// The style the list of `n` is linked to.
    fn style(self, n: &Node) -> Option<&String> {
        match self {
            List::Fills => n.styles.fill.as_ref(),
            List::Strokes => n.styles.stroke.as_ref(),
            List::Effects => n.styles.effect.as_ref(),
            List::Grids => n.styles.grid.as_ref(),
            List::Exports => None,
        }
    }
}

/// Which item bindings of the edited list the write itself drops (the rest the clean-up writes
/// back from their variables).
#[derive(Clone, Copy)]
enum Dropped {
    None,
    Item(usize),
    All,
}

impl Dropped {
    fn keeps(self, index: usize) -> bool {
        match self {
            Dropped::None => true,
            Dropped::Item(i) => i != index,
            Dropped::All => false,
        }
    }
}

/// The premises as this family adjusts them: the first word on an object replaces what the
/// earlier declarations held of it, later ones add.
struct Hold<'r, 'a> {
    rest: &'r mut Rest<'a>,
    doc: &'a Document,
    touched: BTreeSet<String>,
    /// Objects a parameter names (by id or name): what is not read of them is held as there.
    named: BTreeSet<String>,
}

impl<'r, 'a> Hold<'r, 'a> {
    fn need(&mut self, id: impl Into<Value>, fields: &[&str], place: bool) {
        let id = id.into();
        if self.touched.insert(id.to_string()) {
            self.rest.only(id, fields, place);
        } else {
            self.rest.need(id, fields, place);
        }
    }

    /// Every field of `id`, and where it is when `place`.
    fn whole(&mut self, id: impl Into<Value>, place: bool) {
        let id = id.into();
        self.touched.insert(id.to_string());
        self.rest.drop(id.clone());
        self.rest.whole(id, place);
    }

    fn layer(&mut self, node: NodeId, fields: &[&str], place: bool) {
        self.need(node.0, fields, place);
    }

    /// A parameter value naming a resource: its key, and `name` held when it named it by name.
    fn resource(&mut self, given: Option<&str>, key: &str, fields: &[&str]) {
        self.named.insert(json!(key).to_string());
        let mut fields = fields.to_vec();
        if given.is_some_and(|g| g != key) {
            fields.push("name");
        }
        self.need(json!(key), &fields, false);
    }

    /// A variable and what resolving it reads: its collection's modes, the variables it aliases
    /// and inherits from (`kition_design_doc` `Variables::resolve`).
    fn chain(&mut self, key: &str) {
        let doc = self.doc;
        let mut pending = vec![key.to_owned()];
        let mut seen = BTreeSet::new();
        while let Some(key) = pending.pop() {
            let Some(variable) = doc.variables.get(&key) else { continue };
            if !seen.insert(variable.id.clone()) {
                continue;
            }
            let mut fields = vec!["collection", "values"];
            if variable.extends.is_some() {
                fields.push("props.extends");
            }
            self.need(json!(variable.id), &fields, false);
            if let Some(collection) = doc.variables.collection(&variable.collection) {
                self.need(json!(collection.id), &["modes"], false);
            }
            pending.extend(aliases(variable));
            pending.extend(variable.extends.iter().cloned());
        }
    }

    /// The modes `node` resolves variables in: its own, its ancestors' and an instance's main's
    /// (`kition_design_doc` `Document::modes_for`, `inherited_modes`).
    fn modes(&mut self, node: NodeId) {
        let doc = self.doc;
        self.layer(node, &["bindings.var_modes"], true);
        for at in std::iter::once(node).chain(doc.ancestors(node)) {
            if at != node {
                self.layer(at, &["bindings.var_modes"], false);
            }
            if let Some(NodeKind::Instance { component, .. }) = doc.node(at).map(|n| &n.kind) {
                self.layer(*component, &["bindings.var_modes"], false);
            }
        }
    }

    /// A variable bound on `node` the clean-up writes back (`cmd/styles.rs:989`).
    fn bound_var(&mut self, node: NodeId, key: &str) {
        self.chain(key);
        self.modes(node);
    }

    /// The style `sid` a write copies or compares with, as `node` sees it (`style_vars::kind_for`
    /// resolves its bound fields in the layer's modes).
    fn style(&mut self, sid: &str, given: Option<&str>, node: Option<NodeId>) {
        let doc = self.doc;
        let Some(style) = doc.style(sid) else { return };
        self.resource(given, &style.id, &["value", "props.bound"]);
        if let Some(node) = node {
            for key in style.bound.values() {
                self.bound_var(node, key);
            }
        }
    }

    /// The instance sync after an edit of `field` on `node` (`assets/components/patch.rs:9`): an
    /// override recorded against the main layer it mirrors, or the edit carried to its copies
    /// unless they override it.
    fn instance(&mut self, node: NodeId, field: &str) {
        let doc = self.doc;
        let Some(n) = doc.node(node) else { return };
        if let Some(source) = n.source {
            if field == "opacity" {
                // The override decision depends on the source value, not the source identity.
                self.layer(node, &["instance.overrides", "~sourceOpacity"], false);
            } else {
                self.layer(node, &["instance.overrides", "mirrors"], false);
                self.layer(source, &[field], false);
            }
        }
        let copies: Vec<NodeId> = doc.nodes().filter(|c| c.source == Some(node)).map(|c| c.id).collect();
        for copy in copies {
            self.layer(copy, &[field, "instance.overrides", "mirrors"], false);
        }
    }

    /// An edit of `list` on `node`: the list, its style link and bindings, the linked style when
    /// the write keeps the link (`cmd/styles.rs:761`), the variables bound to the items it keeps
    /// (`cmd/styles.rs:989`), the instance sync.
    fn list(&mut self, node: NodeId, list: List, keeps_link: bool, dropped: Dropped, extra: &[&str]) {
        let doc = self.doc;
        let mut fields = vec![list.field()];
        if list.bound().is_some() {
            fields.extend(["bindings.styles", "bindings.bound"]);
        }
        fields.extend(extra);
        self.layer(node, &fields, false);
        let Some(n) = doc.node(node) else { return };
        if keeps_link && let Some(sid) = list.style(n) {
            self.style(sid, None, Some(node));
        }
        if let Some(bl) = list.bound() {
            for (key, variable) in &n.bound {
                if let Some((l, i, _)) = list_binding(key)
                    && l == bl
                    && dropped.keeps(i)
                {
                    self.bound_var(node, variable);
                }
            }
        }
        self.instance(node, list.field());
    }

    /// The auto layout re-flow and group refit after `node`'s strokes changed
    /// (`derived.rs:162` `affects_layout`): each container's layout and size, the siblings it
    /// places.
    fn reflow(&mut self, node: NodeId) {
        let doc = self.doc;
        let mut at = node;
        while let Some(parent) = doc.node(at).and_then(|n| n.parent) {
            let Some(p) = doc.node(parent) else { break };
            if p.is_page() || !(p.layout.is_some() || p.kind.bounds_follow_children()) {
                break;
            }
            self.layer(parent, if p.layout.is_some() { &["layout", "frame", "childLayout"] } else { &["layout", "frame"] }, false);
            for sibling in p.children.iter().copied().filter(|c| *c != at) {
                self.layer(sibling, &["frame", "visible", "childLayout"], true);
            }
            at = parent;
        }
    }

    /// A layer inserted into `parent` (on top): the container it lands in and, when it lays out
    /// or wraps its children, the children it re-flows, up the containers that follow
    /// (`derived.rs:117`).
    fn insert(&mut self, parent: NodeId) {
        let doc = self.doc;
        let follows = |n: &Node| !n.is_page() && (n.layout.is_some() || n.kind.bounds_follow_children());
        if !doc.node(parent).is_some_and(follows) {
            self.layer(parent, &[], false);
            return;
        }
        let mut at = parent;
        let mut from: Option<NodeId> = None;
        while let Some(p) = doc.node(at).filter(|n| follows(n)) {
            self.layer(at, if p.layout.is_some() { &["layout", "frame", "childLayout"] } else { &["layout", "frame"] }, false);
            for child in p.children.iter().copied().filter(|c| Some(*c) != from) {
                self.layer(child, &["frame", "visible", "childLayout"], true);
            }
            from = Some(at);
            let Some(parent) = p.parent else { break };
            at = parent;
        }
    }

    /// A layer bound to `variable` through `key` whose value the clean-up writes again: the field,
    /// its bindings and modes, and what the variable resolves through.
    fn resolving(&mut self, node: NodeId, key: &str, variable: &str) {
        self.layer(node, &[register(key), "bindings.bound", "bindings.var_modes"], true);
        if register(key) == "frame" {
            self.reflow(node);
        }
        self.bound_var(node, variable);
    }
}

/// The variables a variable's values alias.
fn aliases(variable: &Variable) -> impl Iterator<Item = String> + '_ {
    variable.values.values().filter_map(|value| match value {
        VariableValue::Alias(alias) => Some(alias.clone()),
        _ => None,
    })
}

/// The register a binding key writes (`cmd/styles.rs:1150` `apply_bindings_matching`).
fn register(key: &str) -> &'static str {
    if let Some((list, _, _)) = list_binding(key) {
        return match list {
            BoundList::Fills => "fills",
            BoundList::Strokes => "strokes",
            BoundList::Effects => "effects",
            BoundList::Grids => "props.grids",
        };
    }
    match key {
        "width" | "height" => "frame",
        "opacity" => "opacity",
        "visible" => "visible",
        "strokeWeight" | "strokeTopWeight" | "strokeRightWeight" | "strokeBottomWeight" | "strokeLeftWeight" => "stroke",
        k if k.ends_with("Radius") => "corners",
        k if k.starts_with("padding") || k == "gap" || k == "itemSpacing" || k == "counterAxisSpacing" => "layout",
        k if k.starts_with("min") || k.starts_with("max") => "childLayout",
        _ => "kind",
    }
}

/// Whether resolving `variable` on `node` (in its modes) passes a variable and mode `at` takes:
/// `at(variable, mode, the layer chose the mode)` (`kition_design_doc` `Variables::resolve`).
fn resolves(doc: &Document, node: NodeId, variable: &str, at: &dyn Fn(&Variable, &str, bool) -> bool) -> bool {
    let modes = doc.modes_for(node);
    let mut key = variable.to_owned();
    for _ in 0..64 {
        let Some(v) = doc.variables.get(&key) else { return false };
        let Some(collection) = doc.variables.collection(&v.collection) else { return false };
        let chosen = modes.get(&collection.id);
        let Some(mode) = chosen.or_else(|| collection.default_mode()) else { return false };
        if at(v, mode, chosen.is_some()) {
            return true;
        }
        match v.values.get(mode) {
            Some(VariableValue::Alias(alias)) => key.clone_from(alias),
            _ => return false,
        }
    }
    false
}

/// Every binding of a layer under `within` (every layer when `None`): (layer, key, variable).
fn bindings(doc: &Document, within: Option<&BTreeSet<NodeId>>) -> Vec<(NodeId, String, String)> {
    doc.nodes()
        .filter(|n| within.is_none_or(|w| w.contains(&n.id)))
        .flat_map(|n| n.bound.iter().map(move |(k, v)| (n.id, k.clone(), v.clone())))
        .collect()
}

fn text<'p>(params: &'p Value, key: &str) -> Option<&'p str> {
    params.get(key).and_then(Value::as_str)
}

fn index(params: &Value, key: &str) -> Option<usize> {
    params.get(key).and_then(Value::as_f64).filter(|i| i.is_finite() && *i >= 0.0).map(|i| i as usize)
}

/// Every string and number in `params`, as premise keys.
fn mentions(value: &Value, out: &mut BTreeSet<String>) {
    match value {
        Value::String(s) => {
            out.insert(json!(s).to_string());
            if let Ok(n) = s.parse::<u64>() {
                out.insert(json!(n).to_string());
            }
        }
        Value::Number(_) => {
            out.insert(value.to_string());
        }
        Value::Array(items) => items.iter().for_each(|item| mentions(item, out)),
        Value::Object(fields) => fields.values().for_each(|item| mentions(item, out)),
        _ => {}
    }
}

pub(super) fn rest(rest: &mut Rest<'_>) {
    let doc = rest.doc;
    let command = rest.command;
    let params = rest.params;
    let targets = rest.targets();
    let objects = rest.under.objects.clone();
    let mut named = BTreeSet::new();
    mentions(params, &mut named);
    for t in &targets {
        named.insert(json!(t.0).to_string());
    }
    let mut h = Hold { rest, doc, touched: BTreeSet::new(), named };
    family(&mut h, command, params, &targets);
    // Direct resource names are already replayed by the typed name parameters. The mode-keyed
    // `values` map has no declared object path, so its aliases still need reference replay.
    let dynamic_values = command == "variable.add"
        && params
            .get("values")
            .and_then(Value::as_object)
            .into_iter()
            .flat_map(|values| values.values())
            .any(|value| value.get("alias").and_then(Value::as_str).is_some());
    let keep_covers = !command.starts_with("variable.") || dynamic_values;
    if !keep_covers {
        h.rest.under.covers = None;
    }
    // What the earlier declarations held and the handler does not read: a named object only as
    // being there, anything else not at all.
    let mut candidates: Vec<Value> = objects.clone();
    candidates.extend(targets.iter().map(|t| json!(t.0)));
    for key in ["source", "from", "parent", "target", "before"] {
        candidates.extend(params.get(key).and_then(super::node_id).map(|n| json!(n.0)));
    }
    candidates.extend(doc.styles.iter().map(|s| json!(s.id)));
    candidates.extend(doc.variables.variables.iter().map(|v| json!(v.id)));
    for c in &doc.variables.collections {
        candidates.push(json!(c.id));
        candidates.extend(c.modes.iter().map(|m| json!(m.id)));
    }
    for id in candidates {
        let key = id.to_string();
        if h.touched.contains(&key) {
            continue;
        }
        if h.named.contains(&key) || (h.rest.under.covers.is_some() && objects.contains(&id)) {
            h.need(id, &[], false);
        } else {
            h.rest.drop(id);
        }
    }
}

fn family(h: &mut Hold<'_, '_>, command: &str, params: &Value, targets: &[NodeId]) {
    let doc = h.doc;
    let list = |name: &str| match name {
        "fill" => List::Fills,
        "stroke" => List::Strokes,
        "effect" => List::Effects,
        "grid" => List::Grids,
        _ => List::Exports,
    };
    let (family, verb) = command.split_once('.').unwrap_or((command, ""));
    match (family, verb) {
        // cmd/edit/props.rs:257 `paint_list`: a paint pushed (a default one by layer type), or
        // the item at `index` (the top one) removed with its bindings.
        ("fill" | "stroke", "add" | "remove") => {
            let l = list(family);
            for &t in targets {
                let n = doc.node(t);
                let len = n.map_or(0, |n| if l == List::Fills { n.fills.len() } else { n.strokes.len() });
                let default = l == List::Fills && verb == "add" && params.get("paint").or_else(|| params.get("color")).is_none();
                let dropped = if verb == "remove" { Dropped::Item(index(params, "index").unwrap_or(len.saturating_sub(1))) } else { Dropped::None };
                h.list(t, l, true, dropped, if default { &["kind"] } else { &[] });
                if l == List::Strokes {
                    h.reflow(t);
                }
            }
        }
        // cmd/edit/props.rs:283 `paint_set`: the item at `index` (the top one; a default pushed
        // into an empty list) replaced, its bindings dropped where it changed, the link cleared.
        ("fill" | "stroke", "set") | ("fill", "pattern") => {
            let l = list(family);
            for &t in targets {
                let len = doc.node(t).map_or(0, |n| if l == List::Fills { n.fills.len() } else { n.strokes.len() });
                h.list(t, l, false, Dropped::Item(index(params, "index").unwrap_or(len.saturating_sub(1))), &[]);
                if l == List::Strokes && len == 0 {
                    h.reflow(t);
                }
            }
            // The pattern's source only has to be a layer outside the targets (props.rs:387).
            if let Some(source) = params.get("source").and_then(super::node_id) {
                h.layer(source, &[], false);
            }
        }
        // cmd/edit/props.rs:353 `paint_paste`: with `index` as `fill.set`, else as `fill.add`; a
        // stroke's copied settings replace the targets' `stroke`.
        ("fill" | "stroke", "paste") => {
            let l = list(family);
            let at = index(params, "index");
            for &t in targets {
                let dropped = at.map_or(Dropped::None, Dropped::Item);
                h.list(t, l, at.is_none(), dropped, if l == List::Strokes { &["stroke"] } else { &[] });
                if l == List::Strokes {
                    h.reflow(t);
                }
            }
        }
        // cmd/edit/props.rs:211: the width profile through `node.set` (the rest of `stroke` kept).
        ("stroke", "widthProfile") => {
            for &t in targets {
                h.layer(t, &["stroke"], false);
                h.instance(t, "stroke");
                h.reflow(t);
            }
        }
        // cmd/edit/props.rs:223: a black stroke pushed when a brush goes on a layer without one.
        ("stroke", "brush") => {
            let preset = params.get("preset").is_some_and(|p| !p.is_null());
            for &t in targets {
                if preset {
                    h.list(t, List::Strokes, true, Dropped::None, &["stroke"]);
                } else {
                    h.layer(t, &["stroke"], false);
                    h.instance(t, "stroke");
                }
                h.reflow(t);
            }
        }
        // cmd/edit.rs:297-322, cmd/edit/props.rs:440: effects pushed, removed with their
        // bindings, replaced (bindings kept: the clean-up writes them back).
        ("effect", "add" | "remove" | "set" | "paste") => {
            for &t in targets {
                let dropped = if verb == "remove" { index(params, "index").map_or(Dropped::None, Dropped::Item) } else { Dropped::None };
                h.list(t, List::Effects, true, dropped, &[]);
            }
        }
        // cmd/edit.rs:325-338: layout grids pushed or removed with their bindings.
        ("grid", "add" | "remove") => {
            for &t in targets {
                let dropped = if verb == "remove" { index(params, "index").map_or(Dropped::None, Dropped::Item) } else { Dropped::None };
                h.list(t, List::Grids, true, dropped, &[]);
            }
        }
        // cmd/edit.rs:340-383: export settings pushed (after the last one), edited, removed.
        ("export", _) => {
            for &t in targets {
                h.list(t, List::Exports, false, Dropped::None, &[]);
            }
        }
        // cmd/lists.rs:249 `move_item`: reordered with their bindings; the style link goes.
        ("fill" | "stroke" | "effect" | "grid", "move") => {
            for &t in targets {
                h.list(t, list(family), false, Dropped::None, &[]);
            }
        }
        ("image", _) => image(h, verb, params, targets),
        ("select", "colors" | "setPaint" | "applyColorStyle" | "bindColorVariable" | "detachColor" | "selectColor") => selection(h, verb, params),
        ("style", _) => style(h, verb, params, targets),
        ("variable", _) => variable(h, verb, params, targets),
        _ => {}
    }
}

/// `image.*` (cmd/create.rs:435, cmd/image.rs).
fn image(h: &mut Hold<'_, '_>, verb: &str, params: &Value, targets: &[NodeId]) {
    let strokes = matches!(text(params, "target"), Some("stroke" | "strokes"));
    let l = if strokes { List::Strokes } else { List::Fills };
    match verb {
        "place" => match params.get("target").and_then(super::node_id) {
            // create.rs:404/414: an image fill pushed (the others keep their bindings), or fill
            // `index` made the image (its bindings go); the link goes either way.
            Some(target) => h.list(target, List::Fills, false, index(params, "index").map_or(Dropped::None, Dropped::Item), &[]),
            // create.rs:479: a new layer on top of `parent` (the current page).
            None => {
                if let Some(parent) = params.get("parent").and_then(super::node_id).or_else(|| h.rest.page()) {
                    h.insert(parent);
                }
            }
        },
        // image.rs:84: every image paint of the list adjusted (a saved preset first: the earlier
        // declaration holds it by name), the link cleared.
        "adjust" => {
            for &t in targets {
                h.list(t, l, false, Dropped::None, &[]);
            }
        }
        // image.rs:138: given filters, or the first image paint of the targets'.
        "savePreset" if params.get("filters").is_none_or(Value::is_null) => {
            for &t in targets {
                h.layer(t, &[l.field()], false);
            }
        }
        _ => {}
    }
}

/// A `selection.*` target (selection_colors.rs): a layer's paint slot, or a text range's.
struct Slot {
    id: NodeId,
    list: List,
    index: usize,
    range: Option<usize>,
}

fn slots(params: &Value) -> Vec<Slot> {
    params
        .get("targets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|t| {
            Some(Slot {
                id: t.get("id").and_then(super::node_id)?,
                list: if t.get("fills").and_then(Value::as_bool).unwrap_or(true) { List::Fills } else { List::Strokes },
                index: index(t, "index").unwrap_or(0),
                range: t.get("range").and_then(Value::as_array).and_then(|r| r.first()).and_then(Value::as_f64).map(|s| s.max(0.0) as usize),
            })
        })
        .collect()
}

/// `selection.*` (crates/engine/engine/src/selection/selection_colors.rs).
fn selection(h: &mut Hold<'_, '_>, verb: &str, params: &Value) {
    let doc = h.doc;
    // A text range with fills of its own holds no binding or link (selection_colors.rs:230).
    let own = |slot: &Slot| {
        slot.range.is_some_and(|start| doc.node(slot.id).and_then(|n| n.kind.text()).is_some_and(|t| t.style_at(start).fills.is_some()))
    };
    for slot in slots(params) {
        h.named.insert(json!(slot.id.0).to_string());
        match verb {
            // :171: the slot's paint replaced (the list, or the range's chars from the paints they show).
            "setPaint" => match slot.range {
                Some(_) => h.layer(slot.id, &["fills", "kind"], false),
                None => h.list(slot.id, slot.list, false, Dropped::Item(slot.index), &[]),
            },
            // :204: the style's paints (in the layer's modes) on the list, or on the range.
            "applyColorStyle" => {
                let given = text(params, "style");
                let sid = given.and_then(|s| doc.style(s)).map(|s| s.id.clone());
                match slot.range {
                    Some(_) => h.layer(slot.id, &["kind"], false),
                    None => h.list(slot.id, slot.list, false, Dropped::All, &[]),
                }
                if let Some(sid) = sid {
                    h.style(&sid, given, Some(slot.id));
                }
            }
            // :282: the slot bound (the link cleared); the clean-up writes the value in the
            // layer's modes; a range with fills of its own takes the value.
            "bindColorVariable" => {
                let given = text(params, "variable");
                if let Some(v) = given.and_then(|v| doc.variables.get(v)) {
                    let key = v.id.clone();
                    h.resource(given, &key, &[]);
                    h.chain(&key);
                }
                if own(&slot) {
                    h.layer(slot.id, &["kind"], false);
                } else {
                    h.layer(slot.id, &[slot.list.field(), "bindings.styles", "bindings.bound"], false);
                }
                h.modes(slot.id);
            }
            // :322: the slot unbound (its alpha moved into the opacity), the link cleared.
            "detachColor" => {
                if own(&slot) {
                    h.layer(slot.id, &[], false);
                    continue;
                }
                let bound = doc.node(slot.id).is_some_and(|n| {
                    let bl = slot.list.bound();
                    n.bound.keys().any(|k| list_binding(k).is_some_and(|(l, i, f)| Some(l) == bl && i == slot.index && f == "color"))
                });
                let mut fields = vec!["bindings.styles", "bindings.bound"];
                if bound {
                    fields.push(slot.list.field());
                }
                h.layer(slot.id, &fields, false);
            }
            _ => {}
        }
    }
}

/// The list a style of `kind` sets through `stroke` (a paint style on strokes).
fn style_list(kind: &StyleKind, stroke: bool) -> Option<List> {
    match kind {
        StyleKind::Paint { .. } if stroke => Some(List::Strokes),
        StyleKind::Paint { .. } => Some(List::Fills),
        StyleKind::Effect { .. } => Some(List::Effects),
        StyleKind::Grid { .. } => Some(List::Grids),
        StyleKind::Text { .. } => None,
    }
}

/// The layers linked to style `sid` and the list (`None`: text chars) they link it for.
fn linked(doc: &Document, sid: &str) -> Vec<(NodeId, Option<List>)> {
    let mut out = Vec::new();
    for n in doc.nodes() {
        for l in [List::Fills, List::Strokes, List::Effects, List::Grids] {
            if l.style(n).is_some_and(|s| s == sid) {
                out.push((n.id, Some(l)));
            }
        }
        if n.styles.text.as_deref() == Some(sid) || n.kind.text().is_some_and(|t| t.links_text_style(sid)) {
            out.push((n.id, None));
        }
    }
    out
}

/// `style.*` (crates/engine/engine/src/cmd/styles.rs).
fn style(h: &mut Hold<'_, '_>, verb: &str, params: &Value, targets: &[NodeId]) {
    let doc = h.doc;
    let given = text(params, "style");
    let style = given.and_then(|s| doc.style(s));
    match verb {
        // :325: from `value`, else from `from` (the first selected layer), which is linked to it.
        "add" => {
            if params.get("value").is_some() {
                return;
            }
            let from = params.get("from").and_then(super::node_id).or_else(|| targets.first().copied());
            let Some(from) = from else { return };
            match text(params, "type").map(str::to_ascii_lowercase).as_deref() {
                Some("text") => h.layer(from, &["kind", "bindings.styles", "bindings.bound"], false),
                Some(t) => {
                    let l = match t {
                        "stroke" => List::Strokes,
                        "effect" => List::Effects,
                        "grid" => List::Grids,
                        _ => List::Fills,
                    };
                    h.list(from, l, false, Dropped::None, &[]);
                }
                None => {}
            }
        }
        // :434: the style's value (in each layer's modes) on the list, bindings it sets dropped;
        // a text style on chars.
        "apply" => {
            let Some(style) = style else { return };
            let stroke = matches!(text(params, "target"), Some("stroke"));
            for &t in targets {
                match style_list(&style.kind, stroke) {
                    Some(l) => {
                        h.list(t, l, false, Dropped::All, &[]);
                        let count = match &style.kind {
                            StyleKind::Paint { paints } => Some(paints.len()),
                            _ => None,
                        };
                        if l == List::Strokes && doc.node(t).is_some_and(|n| Some(n.strokes.len()) != count) {
                            h.reflow(t);
                        }
                    }
                    None => h.layer(t, &["kind", "bindings.styles", "bindings.bound"], false),
                }
                h.style(&style.id, given, Some(t));
            }
            if targets.is_empty() {
                h.style(&style.id, given, None);
            }
        }
        // :284: the link cleared; text: the chars' links (the whole layer's with no range).
        "detach" => {
            let range = params.get("start").is_some() || params.get("end").is_some();
            for &t in targets {
                match text(params, "which") {
                    Some("text") if range => h.layer(t, &["kind"], false),
                    Some("text") => h.layer(t, &["kind", "bindings.styles"], false),
                    _ => h.layer(t, &["bindings.styles"], false),
                }
            }
        }
        // :516: name, description, value (from a layer's list); a new value goes to every
        // linked layer (:672 `propagate_style`).
        "set" => {
            let Some(style) = style else { return };
            let mut fields = Vec::new();
            if params.get("name").is_some() {
                fields.push("name");
            }
            if params.get("description").is_some() {
                fields.push("props.description");
            }
            let value = params.get("value").is_some() || params.get("from").is_some();
            if value {
                fields.extend(["value", "props.bound"]);
            }
            h.resource(given, &style.id, &fields);
            if !value {
                return;
            }
            if params.get("value").is_none()
                && let Some(from) = params.get("from").and_then(super::node_id)
            {
                let field = style_list(&style.kind, false).map_or("kind", List::field);
                h.layer(from, &[field], false);
            }
            propagate(h, &style.id);
        }
        // :475: a field of the style bound to a variable; the style settles in default modes
        // (`style_vars::settle`) and each linked layer takes it in its own.
        "bind" => {
            let Some(style) = style else { return };
            h.resource(given, &style.id, &["value", "props.bound"]);
            let vgiven = text(params, "variable");
            if let Some(v) = vgiven.and_then(|v| doc.variables.get(v)) {
                let key = v.id.clone();
                h.resource(vgiven, &key, &[]);
                h.chain(&key);
            }
            for key in style.bound.values() {
                h.chain(key);
            }
            let varies = vgiven.is_some() || !style.bound.is_empty();
            for (node, l) in linked(doc, &style.id) {
                let field = l.map_or("kind", List::field);
                h.layer(node, &[field, "bindings.styles", "bindings.bound"], false);
                if varies {
                    h.modes(node);
                }
            }
        }
        // :589: a copy inserted after it (the name made unique).
        "duplicate" => {
            if let Some(style) = style {
                h.named.insert(json!(style.id).to_string());
                h.need(json!(style.id), &["name", "value", "props"], true);
            }
        }
        // :611: renamed into the folder; put back before `before` (its place in the list).
        "move" => {
            let folder = params.get("folder").is_some();
            for s in params.get("styles").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
                if let Some(style) = doc.style(s) {
                    h.resource(Some(s), &style.id, if folder { &["name"] } else { &[] });
                }
            }
            if let Some(before) = text(params, "before").and_then(|b| doc.style(b)) {
                h.named.insert(json!(before.id).to_string());
                h.need(json!(before.id), &[], true);
            }
        }
        // :649: the styles named in the folder (the earlier declaration covers the names).
        "renameFolder" => {
            let prefix = format!("{}/", text(params, "folder").unwrap_or("").trim().trim_matches('/'));
            for s in doc.styles.iter().filter(|s| s.name.starts_with(&prefix)) {
                h.need(json!(s.id), &["name"], false);
            }
        }
        // :72: removed; layers (and text chars) linked to it unlinked.
        "delete" => {
            let Some(style) = style else { return };
            h.resource(given, &style.id, &[]);
            for (node, l) in linked(doc, &style.id) {
                h.layer(node, if l.is_some() { &["bindings.styles"] } else { &["kind", "bindings.styles"] }, false);
            }
        }
        _ => {}
    }
}

/// A style's new value applied to every layer linked to it (styles.rs:672), in each layer's modes.
fn propagate(h: &mut Hold<'_, '_>, sid: &str) {
    let doc = h.doc;
    let Some(style) = doc.style(sid) else { return };
    for (node, l) in linked(doc, sid) {
        match l {
            Some(l) => h.layer(node, &[l.field(), "bindings.styles", "bindings.bound"], false),
            None => h.layer(node, &["kind", "bindings.styles"], false),
        }
        for key in style.bound.values() {
            h.bound_var(node, key);
        }
    }
}

/// `variable.*` (cmd/styles.rs, cmd/variables.rs, cmd/variables_io.rs).
fn variable(h: &mut Hold<'_, '_>, verb: &str, params: &Value, targets: &[NodeId]) {
    let doc = h.doc;
    let vgiven = text(params, "variable");
    let var = vgiven.and_then(|v| doc.variables.get(v));
    let cgiven = text(params, "collection");
    let coll = cgiven.and_then(|c| doc.variables.collection(c)).or_else(|| var.and_then(|v| doc.variables.collection(&v.collection)));
    let members = |cid: &str| -> Vec<String> { doc.variables.variables.iter().filter(|v| v.collection == cid).map(|v| v.id.clone()).collect() };
    match verb {
        // styles.rs:142: a mode appended; each variable of the collection takes the default's
        // (or `from`'s) value in it.
        "addMode" => {
            let Some(c) = coll else { return };
            h.resource(cgiven, &c.id, &["modes"]);
            for v in members(&c.id) {
                h.need(json!(v), &["collection", "values"], false);
            }
        }
        // styles.rs:899: values per mode of the collection; aliases name a variable of the type.
        "add" => {
            let Some(c) = coll else { return };
            h.resource(cgiven, &c.id, &["modes"]);
            let values = params.get("values").and_then(Value::as_object).into_iter().flat_map(|m| m.values()).chain(params.get("value"));
            for alias in values.filter_map(|v| v.get("alias").and_then(Value::as_str)) {
                if let Some(target) = doc.variables.get(alias) {
                    let key = target.id.clone();
                    h.resource(Some(alias), &key, &["type"]);
                }
            }
        }
        // styles.rs:959: the value in `mode` (the default); an alias is checked for its type and
        // loops along its target's aliases; the clean-up writes the value into every layer that
        // resolves it in that mode.
        "set" => {
            let Some(v) = var else { return };
            h.resource(vgiven, &v.id, &["type", "collection", "values"]);
            let Some(c) = coll else { return };
            h.need(json!(c.id), &["modes"], false);
            let mode = match text(params, "mode") {
                Some(m) => c.find_mode(m).map(|m| m.id.clone()),
                None => c.default_mode().cloned(),
            };
            if let Some(alias) = params.get("value").and_then(|x| x.get("alias")).and_then(Value::as_str)
                && let Some(target) = doc.variables.get(alias)
            {
                let key = target.id.clone();
                h.resource(Some(alias), &key, &["type", "values"]);
                let mut pending: Vec<String> = aliases(target).collect();
                let mut seen = BTreeSet::from([key]);
                while let Some(next) = pending.pop() {
                    let Some(t) = doc.variables.get(&next) else { continue };
                    if seen.insert(t.id.clone()) {
                        h.need(json!(t.id), &["values"], false);
                        pending.extend(aliases(t));
                    }
                }
            }
            let Some(mode) = mode else { return };
            let id = v.id.clone();
            follow(h, None, &|x, m, _| x.id == id && m == mode);
        }
        // styles.rs:181, variables.rs:270: aliases to it take its values, bound layers keep
        // theirs (alpha moved into the opacity).
        "delete" => {
            let Some(v) = var else { return };
            h.resource(vgiven, &v.id, &["collection", "values", "props.extends"]);
            forget(h, &BTreeSet::from([v.id.clone()]));
        }
        // styles.rs:788: the field bound (the paint's alpha moved); the clean-up writes the value
        // in the layer's modes; null unbinds.
        "bind" => {
            let Some(field) = text(params, "field") else { return };
            let reg = register(field);
            let list = list_binding(field).is_some();
            if let Some(v) = var {
                let key = v.id.clone();
                h.resource(vgiven, &key, &[]);
                h.chain(&key);
            }
            for &t in targets {
                if var.is_some() {
                    let mut fields = vec![reg, "bindings.bound", "bindings.var_modes"];
                    if list {
                        fields.push("bindings.styles");
                    }
                    h.layer(t, &fields, true);
                    h.modes(t);
                    h.instance(t, reg);
                    if reg == "frame" {
                        h.reflow(t);
                    }
                } else {
                    let color = list_binding(field).is_some_and(|(_, _, f)| f == "color");
                    h.layer(t, &if color { vec![reg, "bindings.bound"] } else { vec!["bindings.bound"] }, false);
                }
            }
        }
        // styles.rs:858: the collection's mode on the targets (the current page); the clean-up
        // writes bound values below them again.
        "setMode" => {
            let Some(c) = coll else { return };
            let by_name = text(params, "mode").is_some_and(|m| !c.modes.iter().any(|x| x.id == m));
            h.resource(cgiven, &c.id, if by_name { &["modes"] } else { &[] });
            let on: Vec<NodeId> = if targets.is_empty() { h.rest.page().into_iter().collect() } else { targets.to_vec() };
            let mut below = BTreeSet::new();
            for &t in &on {
                h.named.insert(json!(t.0).to_string());
                h.layer(t, &["bindings.var_modes"], false);
                below.extend(doc.subtree(t));
            }
            let cid = c.id.clone();
            follow(h, Some(&below), &|x, _, _| x.collection == cid);
        }
        // variables.rs:74: the fields given (uniqueness and scope checks only refuse).
        "update" => {
            let Some(v) = var else { return };
            let mut fields = Vec::new();
            for (param, field) in [
                ("name", "name"),
                ("description", "props.description"),
                ("scopes", "props.scopes"),
                ("hiddenFromPublishing", "props.hiddenFromPublishing"),
                ("codeSyntax", "props.codeSyntax"),
            ] {
                if params.get(param).is_some() {
                    fields.push(field);
                }
            }
            h.resource(vgiven, &v.id, &fields);
        }
        // variables.rs:145: a copy of all of it inserted after it.
        "duplicate" => {
            if let Some(v) = var {
                h.named.insert(json!(v.id).to_string());
                h.whole(json!(v.id), true);
            }
        }
        // variables.rs:186: the mode's name.
        "renameMode" => {
            if let Some(c) = coll {
                h.resource(cgiven, &c.id, &["modes"]);
            }
        }
        // variables.rs:200: the mode and its values go; layers on it go back to the default; the
        // clean-up writes again what layers resolved in it.
        "removeMode" => {
            let Some(c) = coll else { return };
            h.resource(cgiven, &c.id, &["modes"]);
            for v in members(&c.id) {
                h.need(json!(v), &["collection", "values"], false);
            }
            let Some(mode) = text(params, "mode").and_then(|m| c.find_mode(m)).map(|m| m.id.clone()) else { return };
            let on: Vec<NodeId> = doc.nodes().filter(|n| n.var_modes.get(&c.id) == Some(&mode)).map(|n| n.id).collect();
            for n in on {
                h.layer(n, &["bindings.var_modes"], false);
            }
            let cid = c.id.clone();
            follow(h, None, &|x, m, _| x.collection == cid && m == mode);
        }
        // variables.rs:221: the modes reordered; a new default re-resolves layers on Auto.
        "moveMode" => {
            let Some(c) = coll else { return };
            h.resource(cgiven, &c.id, &["modes"]);
            let Some(mode) = text(params, "mode").and_then(|m| c.find_mode(m)) else { return };
            let from = c.modes.iter().position(|m| m.id == mode.id).unwrap_or(0);
            let to = index(params, "to").unwrap_or(0).min(c.modes.len().saturating_sub(1));
            if from != to && (from == 0 || to == 0) {
                let cid = c.id.clone();
                follow(h, None, &|x, _, chosen| x.collection == cid && !chosen);
            }
        }
        // variables.rs:234: the name (uniqueness only refuses).
        "renameCollection" => {
            if let Some(c) = coll {
                h.resource(cgiven, &c.id, &["name"]);
            }
        }
        // variables.rs:251: its variables (by membership) forgotten, layers' modes for it cleared.
        "deleteCollection" => {
            let Some(c) = coll else { return };
            h.resource(cgiven, &c.id, &[]);
            let gone: BTreeSet<String> = members(&c.id).into_iter().collect();
            for v in &gone {
                h.need(json!(v), &["collection"], false);
            }
            forget(h, &gone);
            let on: Vec<NodeId> = doc.nodes().filter(|n| n.var_modes.contains_key(&c.id)).map(|n| n.id).collect();
            for n in on {
                h.layer(n, &["bindings.var_modes"], false);
            }
        }
        // variables.rs:300: overridden modes take the parent's values again.
        "resetOverride" => {
            let Some(v) = var else { return };
            h.resource(vgiven, &v.id, &["values", "props.extends", "props.overrides"]);
            if let Some(parent) = v.extends.as_deref().and_then(|p| doc.variables.get(p)) {
                h.need(json!(parent.id), &["values"], false);
            }
            let modes: BTreeSet<String> = match text(params, "mode") {
                Some(m) => coll.and_then(|c| c.find_mode(m)).map(|m| m.id.clone()).into_iter().collect(),
                None => v.overrides.iter().cloned().collect(),
            };
            let id = v.id.clone();
            follow(h, None, &|x, m, _| x.id == id && modes.contains(m));
        }
        // variables_io.rs:249: collections joined by name (local, not extended), modes by name,
        // variables by name and type; the clean-up writes the joined variables' layers again.
        "import" => import(h, params),
        _ => {}
    }
}

/// The layers (under `within`) whose bound values resolve through a variable and mode `at` takes:
/// what the clean-up writes again after the change (`cmd/styles.rs:989`).
fn follow(h: &mut Hold<'_, '_>, within: Option<&BTreeSet<NodeId>>, at: &dyn Fn(&Variable, &str, bool) -> bool) {
    let doc = h.doc;
    for (node, key, variable) in bindings(doc, within) {
        if resolves(doc, node, &variable, at) {
            h.resolving(node, &key, &variable);
        }
    }
}

/// Variables `gone` forgotten (variables.rs:270): aliases to them take their values, bound
/// layers keep theirs (a colour's alpha moved into the paint's opacity).
fn forget(h: &mut Hold<'_, '_>, gone: &BTreeSet<String>) {
    let doc = h.doc;
    for v in doc.variables.variables.iter().filter(|v| !gone.contains(&v.id)) {
        if aliases(v).any(|a| doc.variables.get(&a).is_some_and(|t| gone.contains(&t.id))) {
            h.need(json!(v.id), &["values"], false);
        }
    }
    for (node, key, variable) in bindings(doc, None) {
        if gone.contains(&variable) {
            let color = list_binding(&key).is_some_and(|(_, _, f)| f == "color");
            h.layer(node, &if color { vec![register(&key), "bindings.bound"] } else { vec!["bindings.bound"] }, false);
        }
    }
}

/// `variable.import` (variables_io.rs:249).
fn import(h: &mut Hold<'_, '_>, params: &Value) {
    let doc = h.doc;
    let given = match params.get("variables") {
        Some(Value::String(s)) => serde_json::from_str(s).ok(),
        other => other.cloned(),
    };
    let Some((src, _)) = given.as_ref().and_then(kition_design_format::rest::read_variables) else { return };
    // Matching is repeated after reload, including names that no longer match. The pinned SDK
    // cannot replay member-name selectors over object lists or scoped derived queries.
    h.need("design", &["~variableNames"], false);
    let mut joined = BTreeSet::new();
    for c in &src.collections {
        let Some(same) = doc
            .variables
            .collections
            .iter()
            .find(|x| x.name == c.name && x.extends.is_none() && !kition_design_engine::library::is_remote_collection(doc, &x.id))
        else {
            continue;
        };
        h.need(json!(same.id), &["name", "modes", "props.extends"], false);
        // A new mode copies every local variable's default, including variables absent from JSON.
        if c.modes.iter().any(|m| same.modes.iter().all(|local| local.name != m.name))
            && let Some(default) = same.default_mode()
        {
            let field = format!("values.{default}");
            for local in doc.variables.variables.iter().filter(|v| v.collection == same.id) {
                h.need(json!(local.id), &[field.as_str()], false);
            }
        }
        for v in src.variables.iter().filter(|v| v.collection == c.id) {
            if let Some(local) = doc.variables.variables.iter().find(|x| x.collection == same.id && x.name == v.name) {
                h.need(json!(local.id), &["name", "collection", "type", "values"], false);
                if local.kind == v.kind {
                    joined.insert(local.id.clone());
                }
            }
        }
    }
    if !joined.is_empty() {
        follow(h, None, &|x, _, _| joined.contains(&x.id));
    }
}
