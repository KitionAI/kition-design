//! Object parameters follow the engine's resolvers, not the spelling of a schema member.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use kition_kit::KitCommand;

type Reference = (&'static str, &'static [&'static str], bool);
type Types = &'static [(&'static str, &'static [&'static str])];
type Names = &'static [(&'static str, &'static str)];
type Declaration = (&'static [&'static str], Types, Names);

const NODE: &[&str] = &["Node"];
const STYLE: &[&str] = &["Style"];
const VARIABLE: &[&str] = &["Variable"];
const COLLECTION: &[&str] = &["Collection"];
const MODE: &[&str] = &["Mode"];
const LIBRARY: &[&str] = &["Library"];

/// References read beside the selection/target helpers. Library assets resolve in the lent
/// file, so only its local link (and a local variable being aliased) is an object here.
fn additional(id: &str) -> &'static [Reference] {
    match id {
        "node.add"
        | "frame.add"
        | "shape.rectangle"
        | "shape.ellipse"
        | "shape.polygon"
        | "shape.star"
        | "shape.line"
        | "shape.arrow"
        | "text.add"
        | "vector.add"
        | "edit.paste"
        | "edit.pasteDesignLayers"
        | "board.sticky"
        | "board.shape" => &[("parent", NODE, false)],
        "node.reparent" => &[("parent", NODE, false)],
        "image.place" => &[("parent", NODE, false), ("target", NODE, false)],
        "object.moveToPage" | "file.exportFramesPdf" | "paint.documentColors" | "inspect.readyForDev" => &[("page", NODE, false)],
        "fill.pattern" => &[("source", NODE, false)],
        "arrange.reorder" => &[("source", NODE, false), ("target", NODE, false)],
        "guide.add" | "guide.move" | "guide.remove" | "guide.select" | "guide.clear" => &[("frame", NODE, false), ("page", NODE, false)],
        "text.onPath" => &[("text", NODE, false), ("path", NODE, false)],
        "style.add" => &[("from", NODE, false)],
        "style.set" => &[("style", STYLE, true), ("from", NODE, false)],
        "style.apply" | "style.duplicate" | "style.delete" | "select.applyColorStyle" => &[("style", STYLE, true)],
        "style.bind" => &[("style", STYLE, true), ("variable", VARIABLE, true)],
        "style.move" => &[("styles", STYLE, true), ("before", STYLE, true)],
        "edit.duplicate" => &[("style", STYLE, true), ("variable", VARIABLE, true)],
        "variable.set" | "variable.resetOverride" => &[("variable", VARIABLE, true), ("mode", MODE, true)],
        "variable.bind" | "variable.delete" | "variable.update" | "variable.duplicate" | "select.bindColorVariable" => {
            &[("variable", VARIABLE, true)]
        }
        "variable.add" | "variable.renameCollection" | "variable.deleteCollection" => &[("collection", COLLECTION, true)],
        "variable.addMode" => &[("collection", COLLECTION, true), ("from", MODE, true)],
        "variable.setMode" | "variable.renameMode" | "variable.removeMode" | "variable.moveMode" => {
            &[("collection", COLLECTION, true), ("mode", MODE, true)]
        }
        "instance.add" => &[("component", NODE, false), ("parent", NODE, false)],
        "instance.place" | "instance.slotInsert" | "instance.swap" | "instance.swapAt" => &[("component", NODE, false)],
        "component.addProperty" | "component.bindProperty" => &[("layer", NODE, false)],
        "library.swap" => &[("from", LIBRARY, true), ("to", LIBRARY, true)],
        "library.acceptUpdates" => &[("ids", LIBRARY, true)],
        "library.remove" => &[("id", LIBRARY, true)],
        "library.aliasVariable" => &[("library", LIBRARY, true), ("variable", VARIABLE, true), ("mode", MODE, true)],
        "library.place"
        | "library.swapAt"
        | "library.import"
        | "library.applyStyle"
        | "library.importVariable"
        | "library.bindVariable"
        | "library.extendCollection" => &[("library", LIBRARY, true)],
        "proto.interaction.add" => &[("dest", NODE, false)],
        "proto.flow.add" | "proto.flow.rename" | "proto.flow.remove" | "proto.present" | "proto.preview" => &[("start", NODE, false)],
        "comment.add" => &[("page", NODE, false)],
        "comment.zoom" => &[("ids", &["CommentThread"], false)],
        "comment.list" => &[("page", NODE, false), ("frame", NODE, false)],
        "annotation.list" | "devMode.measurements" => &[("page", NODE, false)],
        "devMode.addMeasurement" => &[("from", NODE, false), ("to", NODE, false)],
        "devMode.removeMeasurement" => &[("from", NODE, false)],
        "measure.distances" => &[("to", NODE, false)],
        "library.setVariableMode" => &[("library", LIBRARY, true), ("ids", NODE, false)],
        "page.go" | "page.inspect" => &[("id", NODE, false)],
        "file.render" => &[("target.pageId", NODE, false), ("target.objectId", NODE, false), ("target.frame", NODE, false)],
        _ => &[],
    }
}

/// Immutable slices are interned once per catalogue command; no caller-controlled names enter
/// this cache. Alternate scalar `id` uses the same node resolver as `ids`.
pub(crate) fn declare(command: &mut KitCommand) {
    static KEPT: OnceLock<Mutex<HashMap<String, Declaration>>> = OnceLock::new();
    let mut kept = KEPT.get_or_init(Mutex::default).lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let declaration = kept.entry(command.id.clone()).or_insert_with(|| {
        let mut refs = additional(&command.id).to_vec();
        if matches!(
            command.id.as_str(),
            "node.set"
                | "node.add"
                | "frame.add"
                | "text.add"
                | "vector.add"
                | "shape.rectangle"
                | "shape.ellipse"
                | "shape.polygon"
                | "shape.star"
                | "shape.line"
                | "shape.arrow"
        ) {
            refs.extend([
                ("props.fillStyle", STYLE, true),
                ("props.strokeStyle", STYLE, true),
                ("props.effectStyle", STYLE, true),
                ("props.textStyle", STYLE, true),
                ("props.gridStyle", STYLE, true),
            ]);
        }
        if matches!(command.id.as_str(), "variable.set" | "variable.add") {
            refs.push(("value.alias", VARIABLE, true));
        }
        if let Some(target) = command.targets {
            let types = if command.id.starts_with("comment.") { &["CommentThread"][..] } else { NODE };
            refs.push((target, types, false));
            if target == "ids" && command.schema.as_ref().is_some_and(|schema| schema.get("id").is_some()) {
                refs.push(("id", NODE, false));
            }
        }
        refs.retain(|(path, _, _)| command.schema.as_ref().is_some_and(|schema| schema.get(path.split('.').next().unwrap_or(path)).is_some()));
        refs.sort_by_key(|(path, _, _)| *path);
        refs.dedup_by_key(|(path, _, _)| *path);
        // A list naming objects by id or name declares its items (`ids[]`): the SDK reads the
        // names of a list only at its item position.
        let refs: Vec<Reference> = refs
            .into_iter()
            .map(|(path, types, names)| {
                let listed = names
                    && Some(path) != command.targets
                    && !path.contains('.')
                    && command.schema.as_ref().is_some_and(|schema| schema[path]["type"] == "array" && schema[path].get("items").is_some());
                let path: &'static str = if listed { Box::leak(format!("{path}[]").into_boxed_str()) } else { path };
                (path, types, names)
            })
            .collect();
        let objects = refs.iter().filter(|(path, _, _)| Some(*path) != command.targets).map(|(path, _, _)| *path).collect::<Vec<_>>();
        let types = refs.iter().map(|(path, types, _)| (*path, *types)).collect::<Vec<_>>();
        let names = refs.iter().filter(|(_, _, names)| *names).map(|(path, _, _)| (*path, "name")).collect::<Vec<_>>();
        (Box::leak(objects.into_boxed_slice()), Box::leak(types.into_boxed_slice()), Box::leak(names.into_boxed_slice()))
    });
    (command.objects, command.object_types, command.name_params) = *declaration;
}
