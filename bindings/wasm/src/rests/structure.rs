//! What the structure family's writes rest on (see [`super`]): components, instances and
//! libraries, prototypes, and the Dev Mode records (code links, annotations, resources,
//! measurements).
//!
//! The engine's clean-up after an edit (`crates/engine/engine/src/derived.rs:15` normalize)
//! rebuilds every instance of an edited main from the main's whole subtree, keeping each copied
//! layer's overrides found through its `mirrors` (its source) and its place among the copies
//! (`assets/components/patch.rs:9`, `instances.rs:288`): a write that edits a main, or an
//! instance's properties, rests on those copies' `instance`/`mirrors` and places, and the
//! rebuilt instance on its main's whole subtree.

use std::collections::BTreeSet;

use kition_design_doc::{NodeId, NodeKind};
use kition_design_engine::components;
use serde_json::{Value, json};

use super::Rest;

pub(super) fn rest(rest: &mut Rest<'_>) {
    let command = rest.command;
    let family = command.split('.').next().unwrap_or_default();
    match family {
        "component" | "instance" => component(rest),
        "library" => library(rest),
        "proto" => proto(rest),
        _ => dev(rest),
    }
}

/// Forget what the earlier declarations held of objects: the rules below say all the handler
/// reads. Places told by asking the kit (`covers`) and windows (`within`) are kept.
fn reset(rest: &mut Rest<'_>) {
    let ids: Vec<Value> = rest.needs.values().map(|(id, _, _)| id.clone()).chain(rest.under.objects.iter().cloned()).collect();
    for id in ids {
        rest.drop(id);
    }
}

/// Forget what was held of layers only (styles, variables, collections, links and the root
/// stay).
fn reset_layers(rest: &mut Rest<'_>) {
    let ids: Vec<Value> = rest.needs.values().map(|(id, _, _)| id.clone()).filter(Value::is_u64).collect();
    for id in ids {
        rest.drop(id);
    }
}

fn is_page(rest: &Rest<'_>, node: NodeId) -> bool {
    rest.doc.node(node).is_some_and(|n| n.is_page())
}

/// Where `node` is on its page: its own frame and its non-page ancestors' (a page never moves).
fn world(rest: &mut Rest<'_>, node: NodeId) {
    let doc = rest.doc;
    for at in std::iter::once(node).chain(doc.ancestors(node)) {
        if !is_page(rest, at) {
            rest.need(at.0, &["frame"], false);
        }
    }
}

/// A new container goes into `top`'s parent next to it: the parent's world transform (unless a
/// page) and what an auto layout parent or a group re-flows (`cmd/structure.rs:163-221`,
/// `derived.rs:79-130`).
fn into_parent_of(rest: &mut Rest<'_>, top: NodeId) {
    if let Some(parent) = rest.doc.node(top).and_then(|n| n.parent).filter(|p| !is_page(rest, *p)) {
        world(rest, parent);
    }
    rest.reflow(top);
}

/// A main an instance copies: the main's every field, its layers' every field and places, the
/// variables and styles they are bound to, and the mains of instances nested in it
/// (`assets/components/instances.rs:11,288`). `place`: where the main itself is matters too.
fn main_graph(rest: &mut Rest<'_>, main: NodeId, place: bool) {
    let doc = rest.doc;
    let mut pending = vec![(main, place)];
    let mut seen = BTreeSet::new();
    while let Some((root, place)) = pending.pop() {
        if !seen.insert(root) {
            continue;
        }
        rest.whole(root.0, place);
        if let Some(NodeKind::Component { props, .. }) = doc.node(root).map(|n| &n.kind) {
            for def in props.values() {
                if let kition_design_doc::PropDef::InstanceSwap { default, .. } = def {
                    pending.push((*default, false));
                }
            }
        }
        for layer in doc.subtree(root) {
            if layer != root {
                rest.whole(layer.0, true);
            }
            rest.bound(layer);
            if let Some(NodeKind::Instance { component, .. }) = doc.node(layer).map(|n| &n.kind)
                && layer != root
            {
                pending.push((*component, false));
            }
        }
    }
}

/// An instance rebuilt from its main: its kind (main, property values) and `instance` record,
/// each copied layer's overrides and source (`instance`, `mirrors`) and place
/// (`assets/components/instances.rs:288`, `sync.rs`).
fn copies(rest: &mut Rest<'_>, instance: NodeId, fields: &[&str]) {
    rest.need(instance.0, fields, false);
    for layer in rest.doc.subtree(instance) {
        if layer != instance {
            rest.need(layer.0, &["instance", "mirrors"], true);
        }
    }
}

/// Every instance of the mains in `mains` (rebuilt when a main changes): see [`copies`].
fn instances_of(rest: &mut Rest<'_>, mains: &[NodeId], fields: &[&str]) {
    for main in mains {
        for instance in components::instances_of(rest.doc, *main) {
            copies(rest, instance, fields);
        }
    }
}

/// The main an instance shows.
fn main_of(rest: &Rest<'_>, instance: NodeId) -> Option<NodeId> {
    match rest.doc.node(instance).map(|n| &n.kind) {
        Some(NodeKind::Instance { component, .. }) => Some(*component),
        _ => None,
    }
}

/// The registers an override group stands for (`assets/components/instances.rs:110` copy_group);
/// `None`: every field.
fn group_fields(group: &str) -> Option<&'static [&'static str]> {
    Some(match group {
        "fills" => &["fills", "bindings"],
        "strokes" => &["strokes", "stroke", "bindings"],
        "effects" => &["effects", "bindings"],
        "opacity" => &["opacity", "props.blend"],
        "visible" => &["visible"],
        "name" => &["name"],
        "corners" => &["corners"],
        "characters" => &["kind"],
        "textStyle" => &["kind", "bindings"],
        "padding" | "gap" | "alignment" => &["layout"],
        "grids" => &["props.grids", "bindings"],
        "exports" => &["props.exports"],
        "interactions" => &["prototype"],
        "size" => &["frame", "kind"],
        _ => return None,
    })
}

fn need_group(rest: &mut Rest<'_>, node: NodeId, group: &str, place: bool) {
    match group_fields(group) {
        Some(fields) => rest.need(node.0, fields, place),
        None => rest.whole(node.0, place),
    }
}

/// A component used as a new instance's main: a set gives its default variant, chosen by the
/// variants' places (top-left) and named by the set (`cmd/component.rs:84-98`,
/// `assets/components/variants.rs:5`).
fn component_param(rest: &mut Rest<'_>, component: NodeId) -> Option<NodeId> {
    let doc = rest.doc;
    if doc.node(component).is_some_and(|n| matches!(n.kind, NodeKind::ComponentSet { .. })) {
        rest.need(component.0, &["kind"], false);
        // A new instance is named after the set (`assets/components.rs:263`).
        if rest.command == "instance.add" {
            rest.need(component.0, &["name"], false);
        }
        for variant in doc.node(component).map(|n| n.children.clone()).unwrap_or_default() {
            rest.need(variant.0, &["kind", "frame"], true);
        }
        let main = components::default_variant(doc, component)?;
        main_graph(rest, main, true);
        return Some(main);
    }
    main_graph(rest, component, false);
    Some(component)
}

/// What owns the properties `node` names: the set when it is or holds the node, else the
/// owning component (`cmd/component/props.rs:214-232`).
fn owner(rest: &Rest<'_>, node: NodeId) -> Option<NodeId> {
    let doc = rest.doc;
    if doc.node(node).is_some_and(|n| matches!(n.kind, NodeKind::ComponentSet { .. })) {
        return Some(node);
    }
    let component = components::owning_component(doc, node)?;
    Some(components::variant_set(doc, component).unwrap_or(component))
}

/// The components an owner stands for (a set's variants).
fn owner_components(rest: &Rest<'_>, owner: NodeId) -> Vec<NodeId> {
    match rest.doc.node(owner) {
        Some(n) if matches!(n.kind, NodeKind::ComponentSet { .. }) => n.children.clone(),
        Some(_) => vec![owner],
        None => Vec::new(),
    }
}

/// An owner's definitions: a component's kind and its place (its parent decides whether it is a
/// variant); a set's kind and its variants' names (variant options) and kinds (their
/// definitions) (`cmd/component/props.rs:214-555`).
fn owner_defs(rest: &mut Rest<'_>, owner: NodeId) {
    let doc = rest.doc;
    if doc.node(owner).is_some_and(|n| matches!(n.kind, NodeKind::ComponentSet { .. })) {
        rest.need(owner.0, &["kind"], false);
        for variant in owner_components(rest, owner) {
            rest.need(variant.0, &["name", "kind"], true);
        }
    } else {
        rest.need(owner.0, &["kind"], true);
    }
}

fn component(rest: &mut Rest<'_>) {
    let doc = rest.doc;
    let command = rest.command;
    let targets = rest.targets();
    match command {
        // A frame becomes a component (its kind replaced); a group too, its fills cleared, clip
        // off, blend pass-through; other layers are wrapped in a new frame at their world bounds,
        // at the top one's parent and index, named after a single layer
        // (`cmd/component.rs:60-69,305-343`, `cmd/structure.rs:161-221`).
        "component.add" | "component.addMultiple" => {
            reset(rest);
            let mut wrapped = Vec::new();
            for &node in &targets {
                match doc.node(node).map(|n| &n.kind) {
                    Some(NodeKind::Frame) => rest.only(node.0, &["kind"], false),
                    Some(NodeKind::Group) => rest.only(node.0, &["kind", "fills", "props.clip", "props.blend"], false),
                    Some(_) => {
                        rest.only(node.0, &["name", "kind", "frame"], true);
                        world(rest, node);
                        wrapped.push(node);
                    }
                    None => {}
                }
            }
            let tops: Vec<NodeId> = if command == "component.addMultiple" {
                wrapped
            } else {
                kition_design_engine::cmd::edit::z_sorted(doc, &wrapped).last().copied().into_iter().collect()
            };
            for top in tops {
                into_parent_of(rest, top);
            }
        }
        // The mains are wrapped in a new set at the top one's place, sized from their world
        // bounds, named after the first; names without `=` are rewritten (`variants.rs:4-92`).
        "component.combineVariants" => {
            reset(rest);
            let ids = kition_design_engine::cmd::edit::z_sorted(doc, &targets);
            for &node in &ids {
                rest.only(node.0, &["name", "kind", "frame"], true);
                world(rest, node);
            }
            if let Some(top) = ids.last() {
                into_parent_of(rest, *top);
            }
        }
        // The instance's and its layers' overrides cleared, its properties reset to the main's
        // defaults and their bindings dropped; the clean-up rebuilds it from the main
        // (`instance.rs:52-88`).
        "component.resetInstance" => {
            reset(rest);
            for &node in &targets {
                let Some(instance) = components::owning_instance(doc, node) else { continue };
                copies(rest, instance, &["kind", "instance", "bindings"]);
                if let Some(main) = main_of(rest, instance) {
                    main_graph(rest, main, false);
                }
            }
        }
        "component.resetOverride" => {
            reset(rest);
            let group = rest.params.get("group").and_then(Value::as_str).map(str::to_owned);
            for &node in &targets {
                let n = doc.node(node);
                let top = n.is_some_and(|n| matches!(n.kind, NodeKind::Instance { .. })) && !components::inside_instance(doc, node);
                match (&group, top) {
                    // One group of a layer: its main layer's value copied back (`instance.rs:90-125`).
                    (Some(group), _) => {
                        rest.need(node.0, &["instance", "mirrors"], true);
                        if let Some(source) = n.and_then(|n| n.source) {
                            need_group(rest, source, group, false);
                        }
                    }
                    // The top instance: every override cleared, rebuilt from the main.
                    (None, true) => {
                        let changes = n.is_some_and(|n| {
                            !n.overrides.is_empty()
                                || match &n.kind {
                                    NodeKind::Instance { component, props } => {
                                        doc.node(*component).is_some_and(|main| *props != components::default_props(main))
                                    }
                                    _ => false,
                                }
                        });
                        if changes {
                            copies(rest, node, &["kind", "instance", "mirrors"]);
                        } else {
                            rest.need(node.0, &["kind", "instance", "mirrors"], false);
                        }
                        if let Some(main) = main_of(rest, node) {
                            main_graph(rest, main, false);
                        }
                    }
                    (None, false) => {
                        rest.need(node.0, &["instance", "mirrors"], true);
                        if let Some(source) = n.and_then(|n| n.source) {
                            rest.whole(source.0, false);
                        }
                    }
                }
            }
        }
        // The instance becomes a frame: kind, source, overrides, property references and
        // bindings replaced, its layers unlinked, the main's variable modes kept as its own
        // (`instance.rs:4-50`, `guard.rs:22`).
        "component.detach" => {
            reset(rest);
            for &node in &targets {
                let Some(main) = main_of(rest, node) else {
                    rest.only(node.0, &["kind"], false);
                    continue;
                };
                rest.need(node.0, &["kind", "mirrors", "instance", "bindings"], false);
                for layer in doc.subtree(node) {
                    if layer != node {
                        rest.need(layer.0, &["mirrors", "instance"], true);
                    }
                }
                rest.need(main.0, &["bindings"], false);
            }
        }
        // The outermost instance's overridden groups are copied onto their sources in the main;
        // the clean-up rebuilds every instance of the main (`instance.rs:229-270`).
        "component.pushChanges" => {
            reset(rest);
            let node = rest.node("id").or_else(|| targets.first().copied());
            let Some(instance) = node.and_then(|n| components::root_instance(doc, n)) else { return };
            rest.need(instance.0, &["kind", "instance"], false);
            for layer in doc.subtree(instance) {
                let Some(n) = doc.node(layer) else { continue };
                if layer != instance {
                    rest.need(layer.0, &["instance", "mirrors"], true);
                }
                for group in &n.overrides {
                    need_group(rest, layer, group, layer != instance);
                    if let Some(source) = n.source {
                        need_group(rest, source, group, true);
                    }
                }
            }
            if let Some(main) = main_of(rest, instance) {
                rest.need(main.0, &[], false);
                for other in components::instances_of(doc, main).into_iter().filter(|i| *i != instance) {
                    copies(rest, other, &["kind", "instance"]);
                }
            }
        }
        // The instance (its main gone) is replaced by its whole subtree pasted as the main, at
        // its parent and index (`instance.rs:304-357`).
        "component.restoreMain" => {
            reset(rest);
            for &node in &targets {
                for layer in doc.subtree(node) {
                    rest.whole(layer.0, true);
                }
            }
        }
        // The asset root (outermost component or set among the layer and its ancestors) gets or
        // loses a leading `.` (`cmd/component.rs:27-49`).
        "component.hideWhenPublishing" => {
            reset(rest);
            for &node in &targets {
                rest.need(node.0, &[], true);
                let chain: Vec<NodeId> = std::iter::once(node).chain(doc.ancestors(node)).collect();
                let root = chain
                    .iter()
                    .copied()
                    .rfind(|a| doc.node(*a).is_some_and(|n| matches!(n.kind, NodeKind::Component { .. } | NodeKind::ComponentSet { .. })));
                for at in chain {
                    if Some(at) == root {
                        rest.need(at.0, &["name", "kind"], true);
                        break;
                    }
                    if !is_page(rest, at) && at != node {
                        rest.need(at.0, &["kind"], true);
                    }
                }
            }
        }
        // A set: its last variant duplicated whole, named from the variants' names, placed below
        // the lowest; the set grows to fit. A lone main is first wrapped in a new set
        // (`variants.rs:111-175`).
        "component.addVariant" => {
            reset(rest);
            for &node in &targets {
                let Some(n) = doc.node(node) else { continue };
                if matches!(n.kind, NodeKind::ComponentSet { .. }) {
                    rest.need(node.0, &["kind", "frame"], false);
                    let base = n.children.last().copied();
                    for &variant in &n.children {
                        rest.need(variant.0, &["name", "kind", "frame"], true);
                    }
                    if let Some(base) = base {
                        for layer in doc.subtree(base) {
                            rest.whole(layer.0, true);
                        }
                    }
                } else {
                    for layer in doc.subtree(node) {
                        rest.whole(layer.0, true);
                    }
                    world(rest, node);
                    into_parent_of(rest, node);
                }
            }
        }
        // A new instance copies the main whole, beside the main's world bounds unless placed;
        // on the current page or in `parent` (`cmd/component.rs:84-98`).
        "instance.add" => {
            reset(rest);
            let Some(component) = rest.node("component") else { return };
            let main = component_param(rest, component);
            let placed = rest.params.get("x").is_some_and(|v| !v.is_null()) && rest.params.get("y").is_some_and(|v| !v.is_null());
            if !placed && let Some(main) = main {
                world(rest, main);
            }
            match rest.node("parent") {
                Some(parent) => {
                    rest.need(parent.0, &[], false);
                    let p = doc.node(parent);
                    if p.is_some_and(|p| p.layout.is_some()) {
                        rest.need(parent.0, &["layout", "frame"], false);
                        for child in p.map(|p| p.children.clone()).unwrap_or_default() {
                            rest.need(child.0, &["frame", "visible", "childLayout"], true);
                        }
                    }
                    if p.is_some_and(|p| p.kind.bounds_follow_children()) {
                        world(rest, parent);
                    }
                    rest.reflow(parent);
                }
                None => {
                    if let Some(page) = rest.page() {
                        rest.need(page.0, &[], false);
                    }
                }
            }
        }
        // The point names the instance; a sibling's insertion or deletion does not change
        // that target. Spatial replay keeps the pick, without holding its sibling index.
        "instance.swapAt" | "instance.place" => point(rest),
        // Exposed only in a main, outside instances: its ancestors up to the owning component
        // decide (`cmd/component.rs:106-125`).
        "component.exposeInstance" => {
            reset(rest);
            for &node in &targets {
                rest.only(node.0, &["kind", "props.exposed"], true);
                ancestors_to_component(rest, node);
            }
        }
        // A frame inside a main becomes a slot: its name names the property, the definition goes
        // into the owner's components, their instances get it (`instance.rs:154-170`,
        // `props.rs:214-295`).
        "component.makeSlot" => {
            reset(rest);
            for &node in &targets {
                rest.only(node.0, &["name", "kind", "instance"], true);
                ancestors_to_component(rest, node);
                if let Some(owner) = owner(rest, node) {
                    let comps = owner_components(rest, owner);
                    instances_of(rest, &comps, &["kind"]);
                }
            }
        }
        // The slot's definition removed from the owner, every reference to it in the owner's
        // layers, and its instances' values (`instance.rs:172-182`, `props.rs:522-555`).
        "component.removeSlot" => {
            reset(rest);
            for &node in &targets {
                rest.only(node.0, &["instance"], true);
                ancestors_to_component(rest, node);
                if let Some(owner) = owner(rest, node) {
                    let comps = owner_components(rest, owner);
                    for comp in &comps {
                        for layer in doc.subtree(*comp) {
                            if layer != *comp {
                                rest.need(layer.0, &["instance"], true);
                            }
                        }
                    }
                    instances_of(rest, &comps, &["kind", "bindings"]);
                }
            }
        }
        // The slot named in the instance (through its copies' sources to the main's slot frame
        // and the owner's definitions); insert copies a main whole into it, reset gives it back
        // the main's default content (`instance.rs:196-227`).
        "instance.slotInsert" | "instance.resetSlot" => {
            reset(rest);
            let slot = rest.params.get("slot").and_then(Value::as_str).map(str::to_owned);
            for &node in &targets {
                let Some(instance) = components::owning_instance(doc, node) else { continue };
                rest.need(instance.0, &["kind"], false);
                let found = components::slots_of(doc, instance)
                    .into_iter()
                    .find(|(id, name)| *id == node || slot.as_deref().is_some_and(|slot| slot == name.as_str()));
                let Some((copy, _)) = found else { continue };
                rest.need(copy.0, &["instance", "mirrors", "~hierarchy"], true);
                if let Some(source) = doc.node(copy).and_then(|n| n.source) {
                    rest.need(source.0, &["kind", "instance"], false);
                    if let Some(owner) = components::owning_component(doc, source) {
                        rest.need(owner.0, &["kind"], false);
                    }
                    if command == "instance.resetSlot" {
                        for child in doc.node(source).map(|n| n.children.clone()).unwrap_or_default() {
                            for layer in doc.subtree(child) {
                                rest.whole(layer.0, true);
                            }
                        }
                    }
                }
            }
            if command == "instance.slotInsert"
                && let Some(component) = rest.node("component")
            {
                component_param(rest, component);
            }
        }
        // The main replaced; property values of the same names kept; the clean-up rebuilds from
        // the new main, matching old layers by parent, source and name (`instance.rs:359-387`).
        "instance.swap" => {
            reset(rest);
            for &node in &targets {
                // sync_instance_depth preserves explicit modes, then resolves the new
                // main's bindings in them, for id-addressed swaps as for point swaps.
                rest.need(node.0, &["kind", "instance", "bindings.var_modes"], false);
                for layer in doc.subtree(node) {
                    if layer != node {
                        rest.need(layer.0, &["name", "mirrors", "instance"], true);
                    }
                }
            }
            if let Some(component) = rest.node("component") {
                component_param(rest, component);
            }
        }
        // Property values written into the instance's kind (a variant switch replaces its main);
        // the clean-up rebuilds it from the (new) main (`props.rs:92-146`).
        "instance.setProperties" => {
            reset(rest);
            for &node in &targets {
                let Some(main) = main_of(rest, node) else { continue };
                rest.need(node.0, &["kind"], false);
                for layer in doc.subtree(node) {
                    if layer != node {
                        rest.need(layer.0, &["instance", "mirrors"], true);
                    }
                }
                match components::variant_set(doc, main) {
                    Some(set) => {
                        rest.need(node.0, &["instance"], false);
                        rest.need(set.0, &["kind"], false);
                        rest.need(main.0, &["name", "kind"], true);
                        let wanted = variant_switch(rest, main);
                        match wanted.and_then(|wanted| components::find_variant(doc, set, &wanted)) {
                            Some(next) if next != main => main_graph(rest, next, true),
                            _ => main_graph(rest, main, true),
                        }
                    }
                    None => {
                        main_graph(rest, main, false);
                        rest.need(main.0, &["kind"], false);
                    }
                }
                // Instance-swap values name other mains.
                let definitions = match doc.node(main).map(|main| &main.kind) {
                    Some(NodeKind::Component { props, .. }) => Some(props),
                    _ => None,
                };
                for (name, value) in rest.params.get("properties").and_then(Value::as_object).into_iter().flatten() {
                    if definitions.and_then(|defs| defs.get(name)).is_some_and(|def| matches!(def, kition_design_doc::PropDef::InstanceSwap { .. }))
                        && let Some(other) = super::node_id(value).filter(|id| *id != node)
                    {
                        main_graph(rest, other, false);
                    }
                }
            }
        }
        "component.addProperty" | "component.editProperty" | "component.deleteProperty" => properties(rest, &targets),
        "component.bindProperty" => {
            reset(rest);
            let layers = rest.node("layer").map(|l| vec![l]).unwrap_or_else(|| targets.clone());
            let unbind = rest.params.get("property").and_then(Value::as_str).is_none_or(str::is_empty);
            for layer in layers {
                rest.need(layer.0, &["kind", "instance"], true);
                if !unbind {
                    if let Some(component) = components::owning_component(doc, layer) {
                        rest.need(component.0, &["kind"], false);
                    }
                    rest.need(layer.0, &["visible"], false);
                }
                // Copies of the layer in instances follow it.
                for copy in doc.nodes().filter(|n| n.source == Some(layer)).map(|n| n.id).collect::<Vec<_>>() {
                    rest.need(copy.0, &["instance", "mirrors"], true);
                }
            }
        }
        _ => {}
    }
}

/// The variant a property switch asks for: the main's own name pairs with the given values.
fn variant_switch(rest: &Rest<'_>, main: NodeId) -> Option<std::collections::BTreeMap<String, String>> {
    let mut wanted = components::variant_props(&rest.doc.node(main)?.name);
    for (name, value) in rest.params.get("properties")?.as_object()? {
        if let (Some(slot), Some(value)) = (wanted.get_mut(name), value.as_str()) {
            *slot = value.to_owned();
        }
    }
    Some(wanted)
}

/// A layer's ancestors up to its owning component (their kinds and places decide whether it is
/// in a main or in an instance).
fn ancestors_to_component(rest: &mut Rest<'_>, node: NodeId) {
    let doc = rest.doc;
    for at in doc.ancestors(node) {
        if is_page(rest, at) {
            break;
        }
        rest.need(at.0, &["kind"], true);
        if doc.node(at).is_some_and(|n| matches!(n.kind, NodeKind::Component { .. })) {
            break;
        }
    }
}

/// `component.addProperty` / `editProperty` / `deleteProperty` (`props.rs:214-555`): the owner's
/// definitions; a bound layer (its visibility, characters or main give the default); a rename
/// or a removal rewrites the owner's layers' references and the instances' values and bindings;
/// every instance of the owner's components is rebuilt.
fn properties(rest: &mut Rest<'_>, targets: &[NodeId]) {
    let doc = rest.doc;
    let command = rest.command;
    reset(rest);
    let node = rest.node("id").or_else(|| targets.first().copied());
    let Some(node) = node else { return };
    let Some(owner) = owner(rest, node) else { return };
    owner_defs(rest, owner);
    let comps = owner_components(rest, owner);
    let set = doc.node(owner).is_some_and(|n| matches!(n.kind, NodeKind::ComponentSet { .. }));
    let layer = rest.node("layer").or_else(|| (node != owner && !comps.contains(&node)).then_some(node));
    let swap = rest
        .params
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|kind| matches!(kind.to_ascii_lowercase().as_str(), "instanceswap" | "instance_swap" | "instance"))
        || comps.iter().filter_map(|comp| doc.node(*comp)).any(|n| {
            let NodeKind::Component { props, .. } = &n.kind else { return false };
            rest.params
                .get("name")
                .and_then(Value::as_str)
                .and_then(|name| props.get(name))
                .is_some_and(|def| matches!(def, kition_design_doc::PropDef::InstanceSwap { .. }))
        });
    if swap && let Some(default) = rest.params.get("default").and_then(super::node_id) {
        rest.need(default.0, &["kind"], false);
    }
    for preferred in rest.params.get("preferred").and_then(Value::as_array).into_iter().flatten().filter_map(super::node_id) {
        rest.need(preferred.0, &["kind"], false);
    }
    match command {
        "component.addProperty" => {
            if let Some(layer) = layer {
                rest.need(layer.0, &["visible", "kind", "instance"], true);
            }
            let variant = rest.params.get("type").and_then(Value::as_str).is_some_and(|t| t.eq_ignore_ascii_case("variant"));
            if variant && !set {
                // A lone main is first wrapped in a new set.
                for comp in &comps {
                    rest.need(comp.0, &["name", "kind", "frame"], true);
                    world(rest, *comp);
                    into_parent_of(rest, *comp);
                }
            }
            if !set {
                instances_of(rest, &comps, &["kind"]);
            }
        }
        "component.editProperty" => {
            if rest.params.get("newName").is_some_and(|v| !v.is_null()) && !set {
                for comp in &comps {
                    for layer in doc.subtree(*comp) {
                        if layer != *comp {
                            rest.need(layer.0, &["instance"], true);
                        }
                    }
                }
                for comp in &comps {
                    for instance in components::instances_of(doc, *comp) {
                        rest.need(instance.0, &["kind", "bindings"], false);
                    }
                }
            }
            if set && rest.params.get("default").is_some_and(|v| !v.is_null()) {
                // A new default variant swaps two variants' places; the set refits.
                rest.need(owner.0, &["frame"], false);
                for comp in &comps {
                    rest.need(comp.0, &["frame"], false);
                }
            }
        }
        _ => {
            if !set {
                for comp in &comps {
                    for layer in doc.subtree(*comp) {
                        if layer != *comp {
                            rest.need(layer.0, &["kind", "instance"], true);
                        }
                    }
                }
                instances_of(rest, &comps, &["kind", "bindings"]);
            }
        }
    }
}

/// The library link a `library` parameter names (by id, name or path), and the file it lends.
fn link(rest: &Rest<'_>, key: &str) -> Option<kition_design_doc::LibraryLink> {
    kition_design_engine::library::find_link(rest.doc, rest.params.get(key)?.as_str()?).cloned()
}

/// The local copy of the library component `component` names, when the document has one.
fn local_copy(rest: &Rest<'_>, link: &kition_design_doc::LibraryLink) -> Option<NodeId> {
    let lib = crate::libraries::read(rest.design, link).ok()?;
    let key = kition_design_engine::library::component_key(&lib, rest.params.get("component")?)?;
    rest.doc.libraries.components.iter().find(|r| r.library == link.id && r.key == key).map(|r| r.root)
}

fn library(rest: &mut Rest<'_>) {
    let doc = rest.doc;
    let command = rest.command;
    let targets = rest.targets();
    // The link: found by id, name or path; its file read by its path (`cmd/library.rs:187`,
    // `library.rs:727`, `libraries.rs:119`).
    if let Some(link) = link(rest, "library") {
        rest.need(json!(link.id), &["name", "path"], false);
    }
    match command {
        // Every local component and set (whole), local style, collection and variable is
        // snapshotted; names decide what is offered; the previous publication gives versions
        // (`cmd/library.rs:336-344`, `assets/library.rs:205-443`).
        "library.publish" => {
            reset(rest);
            rest.only("design", &["settings.name", "~assets", "~published"], false);
            let include: Vec<NodeId> =
                rest.params.get("include").and_then(Value::as_array).map(|ids| ids.iter().filter_map(super::node_id).collect()).unwrap_or_default();
            let assets = kition_design_engine::library::publishable(doc).components;
            if include.is_empty() {
                for asset in assets {
                    main_graph(rest, asset, true);
                }
                let resources: Vec<String> = doc
                    .styles
                    .iter()
                    .map(|s| s.id.clone())
                    .chain(doc.variables.collections.iter().map(|c| c.id.clone()))
                    .chain(doc.variables.variables.iter().map(|v| v.id.clone()))
                    .collect();
                for id in resources {
                    rest.whole(json!(id), false);
                }
            } else {
                // Only what is included, and what it uses.
                reset_layers(rest);
                let resources: Vec<Value> =
                    rest.needs.values().map(|(id, _, _)| id.clone()).filter(|id| id.is_string() && *id != json!("design")).collect();
                for id in resources {
                    rest.drop(id);
                }
                rest.only("design", &["settings.name", "~published"], false);
                for asset in include {
                    for layer in doc.subtree(asset) {
                        rest.whole(layer.0, true);
                        rest.bound(layer);
                    }
                }
            }
        }
        // The file at the path (its manifest), this file's own publication (refused when the
        // same), and the link with the file's id (path and name replaced) (`cmd/library.rs:516-552`).
        "library.add" => {
            rest.root(&["libraries", "~published"]);
            for link in doc.libraries.enabled.clone() {
                rest.need(json!(link.id), &["name", "path"], false);
            }
        }
        // Pruning reads every layer's mains, style links, bindings, modes and actions (the
        // root's layers and resources and their derived usage); a swap or an update rebuilds
        // the instances of the library's copies (`cmd/library.rs:449-641`, `library.rs:1122-1258`,
        // `library_swap.rs:45`).
        "library.swap" | "library.remove" | "library.acceptUpdates" => {
            reset(rest);
            let mut links: Vec<kition_design_doc::LibraryLink> = Vec::new();
            match command {
                "library.swap" => links.extend(link(rest, "from")),
                "library.remove" => links.extend(link(rest, "id")),
                _ => match rest.params.get("ids").and_then(Value::as_array) {
                    Some(ids) => links
                        .extend(ids.iter().filter_map(Value::as_str).filter_map(|key| kition_design_engine::library::find_link(doc, key)).cloned()),
                    None => links.extend(doc.libraries.enabled.iter().cloned()),
                },
            }
            for link in &links {
                rest.need(json!(link.id), if command == "library.acceptUpdates" { &["name", "path", "version"] } else { &["name"] }, false);
            }
            let mut fields = vec!["libraryCopies", "~libraryUsage"];
            if command == "library.swap" || rest.params.get("ids").is_none() && command == "library.acceptUpdates" {
                fields.push("libraries");
            }
            rest.only("design", &fields, false);
            // Layers: the copies of the links' components and the instances showing them.
            reset_layers(rest);
            let ids: BTreeSet<&str> = links.iter().map(|l| l.id.as_str()).collect();
            let roots: Vec<NodeId> = doc.libraries.components.iter().filter(|r| ids.contains(r.library.as_str())).map(|r| r.root).collect();
            for &root in &roots {
                if command == "library.remove" {
                    rest.need(root.0, &[], false);
                } else {
                    // An update re-imports the copy in place; a swap only reads it.
                    main_graph(rest, root, command == "library.acceptUpdates");
                }
                for instance in components::instances_of(doc, root) {
                    match command {
                        "library.remove" => rest.need(instance.0, &["kind", "bindings", "prototype"], false),
                        "library.swap" => {
                            rest.need(instance.0, &["kind", "instance", "name", "mirrors"], false);
                            for layer in doc.subtree(instance) {
                                if layer != instance {
                                    rest.need(layer.0, &["name", "mirrors", "instance"], true);
                                }
                            }
                        }
                        _ => copies(rest, instance, &["kind", "instance"]),
                    }
                }
            }
        }
        "library.place" | "library.swapAt" => {
            point(rest);
        }
        // An existing copy is answered as is; otherwise the file's snapshot is copied in
        // (`cmd/library.rs:111-122`).
        "library.import" => {
            rest.root(&["libraryCopies"]);
            if let Some(link) = link(rest, "library")
                && let Some(copy) = local_copy(rest, &link)
            {
                rest.need(copy.0, &[], false);
            }
        }
        // The file's style copied (with its record), then applied: the layers' paints of that
        // kind and their style link replaced, the bindings it replaces dropped
        // (`cmd/library.rs:655-674`, `cmd/styles.rs:379,434`).
        "library.applyStyle" => {
            rest.root(&["styles", "libraryCopies"]);
            let field = match rest.params.get("target").and_then(Value::as_str).unwrap_or("fill") {
                "stroke" => "strokes",
                "effect" => "effects",
                "text" => "kind",
                "grid" => "props.grids",
                _ => "fills",
            };
            reset_layers(rest);
            for node in targets {
                rest.only(node.0, &[field, "bindings"], false);
            }
        }
        // Variables and their collections copied with their records (`library.rs:826-907`); a
        // binding checks the field takes it (the layer's kind and auto layout)
        // (`cmd/styles.rs:788`).
        "library.importVariable" | "library.extendCollection" => {
            rest.root(&["libraryCopies", "collections", "variables"]);
        }
        "library.bindVariable" => {
            rest.root(&["libraryCopies", "collections", "variables"]);
            reset_layers(rest);
            for node in targets {
                rest.only(node.0, &["kind", "layout", "bindings"], false);
            }
        }
        // A local variable's value in a mode (default: its collection's default mode) becomes
        // an alias of the copy (`cmd/library.rs:228-241`, `cmd/styles.rs:959`).
        "library.aliasVariable" => {
            rest.root(&["libraryCopies", "collections", "variables"]);
            let variable = rest.params.get("variable").and_then(Value::as_str).and_then(|key| doc.variables.get(key)).cloned();
            if let Some(variable) = variable {
                rest.only(json!(variable.id), &["type", "collection", "values"], false);
                if let Some(collection) = doc.variables.collection(&variable.collection) {
                    rest.need(json!(collection.id), &["modes"], false);
                }
            }
        }
        // The copied collection's mode set on the layers (the selection, else the current page);
        // their bindings re-resolve (`cmd/styles.rs:858-891`).
        "library.setVariableMode" => {
            rest.root(&["libraryCopies", "collections"]);
            reset_layers(rest);
            let layers = if targets.is_empty() { rest.page().into_iter().collect() } else { targets };
            for node in layers {
                rest.only(node.0, &["bindings"], false);
            }
        }
        _ => {}
    }
}

/// A point resolves a source main and a destination. Placement copies the main into that
/// parent's space; swapping preserves the old instance's properties and copied overrides.
fn point(rest: &mut Rest<'_>) {
    let doc = rest.doc;
    let command = rest.command;
    let picked = super::resolve::point_target(rest.design, doc, command, rest.params);
    reset(rest);
    let placing = command.ends_with(".place");
    if let Some(covers) = &mut rest.under.covers {
        let fields: &[&str] = if placing {
            &["frame", "kind", "visible", "locked", "layout", "childLayout", "instance", "mirrors", "~bounds", "~hierarchy"]
        } else {
            &[
                "frame",
                "kind",
                "visible",
                "locked",
                "props.clip",
                "props.is_mask",
                "fills",
                "strokes",
                "stroke",
                "corners",
                "layout",
                "~bounds",
                "~hierarchy",
            ]
        };
        covers.fields = fields.iter().map(|field| (*field).into()).collect();
    }
    if command.starts_with("library.") {
        rest.root(&["libraryCopies"]);
        if let Some(link) = link(rest, "library") {
            rest.need(json!(link.id), &["name", "path"], false);
        }
    }
    let Some((source, target)) = picked else { return };
    if let Some(source) = source {
        main_graph(rest, source, command.starts_with("library."));
        if let Some(input) = rest.node("component")
            && doc.node(input).is_some_and(|n| matches!(n.kind, NodeKind::ComponentSet { .. }))
        {
            component_param(rest, input);
        }
        if !placing && let Some(set) = components::variant_set(doc, source) {
            rest.need(source.0, &[], true);
            rest.need(set.0, &["name"], false);
        }
    }
    rest.under.objects.push(json!(target.0));
    if placing {
        if is_page(rest, target) {
            rest.only(target.0, &[], false);
        } else {
            rest.need(target.0, &["frame", "kind", "visible", "locked", "layout"], false);
            super::layout::world(rest, target);
            if let Some(parent) = doc.node(target).filter(|n| n.layout.is_some() || n.kind.bounds_follow_children()) {
                rest.need(target.0, &["frame", "layout", "childLayout", "~hierarchy"], true);
                for child in &parent.children {
                    rest.need(child.0, &["frame", "childLayout", "visible"], true);
                    if doc.node(*child).is_some_and(|n| n.kind.text().is_some()) {
                        rest.need(child.0, &["kind"], false);
                    }
                }
            }
            super::layout::settle(rest, target);
        }
    } else {
        rest.need(target.0, &["kind", "frame", "instance", "visible", "locked", "bindings.var_modes"], false);
        super::layout::world(rest, target);
        for layer in doc.subtree(target).into_iter().filter(|layer| *layer != target) {
            rest.need(layer.0, &["kind", "name", "instance", "mirrors"], true);
        }
    }
}

/// Whether the parameters name a variable, collection or mode (by name or in an expression):
/// only then does what a name resolves to matter (`place.rs` referenced).
fn names_resources(value: &Value, variables: &kition_design_doc::Variables) -> bool {
    fn reads_resource(expr: &kition_design_doc::proto::Expr) -> bool {
        use kition_design_doc::proto::Expr;
        match expr {
            Expr::Value(kition_design_doc::VariableValue::Alias(_)) | Expr::Mode { .. } | Expr::InMode { .. } => true,
            Expr::Op { args, .. } => args.iter().any(reads_resource),
            _ => false,
        }
    }
    match value {
        Value::Object(map) => map.iter().any(|(key, value)| match key.as_str() {
            "expression" => {
                value.as_str().is_some_and(|text| kition_design_doc::proto::Expr::parse(text, variables).map_or(true, |expr| reads_resource(&expr)))
            }
            "condition" | "expr" => {
                serde_json::from_value::<kition_design_doc::proto::Expr>(value.clone()).map_or(true, |expr| reads_resource(&expr))
            }
            "collection" | "variable" | "alias" | "mode" => true,
            _ => names_resources(value, variables),
        }),
        Value::Array(items) => items.iter().any(|item| names_resources(item, variables)),
        _ => false,
    }
}

fn proto(rest: &mut Rest<'_>) {
    let doc = rest.doc;
    let command = rest.command;
    let targets = rest.targets();
    // Layers named in the actions (destinations) are only checked to exist.
    let named: Vec<NodeId> = rest.under.objects.iter().filter_map(super::node_id).collect();
    for node in named {
        rest.only(node.0, &[], false);
    }
    if !names_resources(rest.params, &doc.variables) {
        rest.under.covers = None;
    }
    if command.starts_with("proto.flow.") || command == "proto.settings" {
        // Flows and settings live on the current page (`cmd/proto.rs:55-235`); the flow's start
        // frame is only checked.
        if let Some(page) = rest.page() {
            rest.need(page.0, &["kind"], false);
        }
        let start =
            rest.node("start").or_else(|| (command == "proto.flow.add").then(|| rest.design.session().selection().first().copied()).flatten());
        if let Some(start) = start {
            rest.only(start.0, &[], false);
        }
        return;
    }
    // The interaction list is read and replaced (`cmd/proto.rs:20-489`, `proto_actions.rs`).
    for node in targets {
        rest.only(node.0, &["prototype.interactions"], false);
    }
    // A collection named in a set-mode action: its name and modes.
    let collections: Vec<String> =
        rest.needs.values().filter_map(|(id, _, _)| id.as_str()).filter(|id| doc.variables.collection(id).is_some()).map(str::to_owned).collect();
    for collection in collections {
        rest.only(json!(collection), &["name", "modes"], false);
    }
    // A block without a condition gets the default one: the first boolean variable (else the
    // first) equal to its start value in the default modes (`proto_actions.rs:140-156`).
    if command == "proto.conditional.addBlock"
        && !rest.params.get("else").and_then(kition_design_engine::params::bool_of).unwrap_or(false)
        && ["expression", "condition"].iter().all(|key| rest.params.get(*key).is_none_or(Value::is_null))
    {
        let vars = &doc.variables;
        for variable in &vars.variables {
            rest.need(json!(variable.id), &["type"], false);
        }
        let chosen = vars.variables.iter().find(|v| v.kind == kition_design_doc::VariableType::Boolean).or(vars.variables.first());
        let mut pending: Vec<String> = chosen.map(|v| v.id.clone()).into_iter().collect();
        let mut seen = BTreeSet::new();
        while let Some(key) = pending.pop() {
            if !seen.insert(key.clone()) {
                continue;
            }
            let Some(variable) = vars.get(&key) else { continue };
            rest.need(json!(variable.id), &["type", "values"], false);
            if let Some(collection) = vars.collection(&variable.collection) {
                rest.need(json!(collection.id), &["modes"], false);
            }
            for value in variable.values.values() {
                if let kition_design_doc::VariableValue::Alias(alias) = value {
                    pending.push(alias.clone());
                }
            }
        }
    }
    super::resolve::narrow_literal_modes(rest);
}

fn dev(rest: &mut Rest<'_>) {
    let doc = rest.doc;
    let command = rest.command;
    let targets = rest.targets();
    match command {
        // A ready-for-dev layer's change mark cleared and its note set (`cmd/section.rs:84-101`).
        "dev.resolveChange" => {
            reset(rest);
            for node in targets {
                for layer in doc.subtree(node) {
                    rest.whole(layer.0, true);
                }
            }
        }
        // The link lives on the owner (the main, an instance's main, a variant's set); omitted
        // fields keep the owner's link; the description reads the owner's name and definitions
        // (a set: its variants' names) (`dev/code_connect.rs:47-56,527-597`).
        "codeConnect.set" | "codeConnect.remove" => {
            reset(rest);
            let set = command == "codeConnect.set";
            if let Some(&node) = targets.first() {
                let Some(owner) = kition_design_engine::code_connect::owner(doc, node) else { return };
                if node != owner {
                    rest.need(node.0, &["kind"], false);
                }
                rest.need(owner.0, &["dev.code_links"], false);
                if set {
                    describe(rest, owner, node);
                }
            }
        }
        // Components and sets matched by name across the file (the library store's too); each
        // match's link for the language replaced (`dev/code_connect.rs:642-693`).
        "codeConnect.import" => {
            let names: Option<BTreeSet<String>> = rest
                .params
                .get("json")
                .and_then(|json| json.get("links"))
                .and_then(Value::as_array)
                .map(|links| links.iter().filter_map(|link| link.get("component").and_then(Value::as_str)).map(str::to_owned).collect());
            let Some(names) = names else { return };
            reset_layers(rest);
            rest.drop("design");
            for name in &names {
                let field = format!("~codeOwners.{}", super::resolve::name_key(name.trim()));
                rest.root(&[&field]);
            }
            // Owners only: a variant's links live on its set.
            let owners = doc.nodes().filter(|n| {
                matches!(n.kind, NodeKind::ComponentSet { .. })
                    || matches!(n.kind, NodeKind::Component { .. }) && components::variant_set(doc, n.id).is_none()
            });
            for node in owners {
                rest.need(node.id.0, &["name"], false);
                if names.contains(&node.name) {
                    rest.need(node.id.0, &["dev.code_links"], false);
                    describe(rest, node.id, node.id);
                }
            }
        }
        // Annotations by index in the layer's list; a category by id or name in the file's list
        // (or the defaults) (`dev/annotations.rs:246-317`).
        "annotation.add" | "annotation.edit" | "annotation.remove" => {
            if let Some(&node) = targets.first() {
                rest.only(node.0, &["dev.annotations"], false);
            }
            if rest.params.get("category").and_then(Value::as_str).is_some_and(|key| !key.trim().is_empty()) {
                rest.only("design", &["settings.annotation_categories"], false);
            }
        }
        // Every annotation in the removed category becomes uncategorised (`annotations.rs:422-447`).
        "annotation.removeCategory" => {
            let key = rest.params.get("id").and_then(Value::as_str).unwrap_or_default();
            let category = kition_design_engine::annotations::categories(doc).iter().find(|c| c.id == key || c.name == key).map(|c| c.id.clone());
            let Some(category) = category else { return };
            for node in doc.nodes().filter(|n| n.annotations.iter().any(|a| a.category.as_deref() == Some(category.as_str()))) {
                rest.need(node.id.0, &["dev.annotations"], false);
            }
        }
        // Resources by index in the layer's list (the ancestors only refuse a layer inside an
        // instance) (`dev/dev_resources.rs:143-206`).
        "devResource.add" | "devResource.edit" | "devResource.remove" => {
            reset(rest);
            if let Some(&node) = targets.first() {
                rest.only(node.0, &["dev.dev_resources"], false);
                for ancestor in doc.ancestors(node).into_iter().filter(|id| !is_page(rest, *id)).collect::<Vec<_>>() {
                    rest.need(ancestor.0, &["kind"], true);
                }
            }
        }
        // Measurements from `from` (else the selection's first) to `to`, else to its parent
        // (not a page); indexes count in `from`'s list (`dev/dev_measurements.rs:123-186`).
        "devMode.addMeasurement" | "devMode.removeMeasurement" => {
            reset(rest);
            let from = rest.node("from").or_else(|| rest.design.session().selection().first().copied());
            let Some(from) = from else { return };
            rest.only(from.0, &["dev.measurements"], false);
            if command == "devMode.addMeasurement" {
                match rest.node("to") {
                    Some(to) => rest.only(to.0, &[], false),
                    None => {
                        if let Some(parent) = doc.node(from).and_then(|n| n.parent).filter(|p| !is_page(rest, *p)) {
                            rest.need(from.0, &[], true);
                            rest.only(parent.0, &[], false);
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

/// What a code link's description reads of its owner: a component's name and definitions; a
/// set's name and its variants' names (and the named variant's definitions).
fn describe(rest: &mut Rest<'_>, owner: NodeId, node: NodeId) {
    let doc = rest.doc;
    if doc.node(owner).is_some_and(|n| matches!(n.kind, NodeKind::ComponentSet { .. })) {
        rest.need(owner.0, &["name"], false);
        for variant in doc.node(owner).map(|n| n.children.clone()).unwrap_or_default() {
            rest.need(variant.0, &["name"], true);
        }
        if node != owner {
            rest.need(node.0, &["kind"], false);
        }
    } else {
        rest.need(owner.0, &["name", "kind"], false);
    }
}
