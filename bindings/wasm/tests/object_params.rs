//! C8-land-11-design: declarations are checked independently of the binding's metadata builder.
#![allow(clippy::unwrap_used, clippy::panic)]

use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;

// Each exception is a non-object parameter or a schema member the handler never reads.
// Embedded history/category records and assets in a lent library still need separate guards.
const ALLOW: &[(&str, &str, &str)] = &[
    ("edit.pasteOver", "id", "paste_over reads the selection and clipboard, not this extra schema member"),
    ("edit.findReplace", "id", "find_replace searches the page, not this extra schema member"),
    ("edit.pickColor", "id", "pickColor changes the tool; this extra schema member is unused"),
    ("select.children", "id", "select_children reads the selection, not this extra schema member"),
    ("select.parent", "id", "select_parent reads the selection, not this extra schema member"),
    ("select.next", "id", "select_sibling reads the selection, not this extra schema member"),
    ("select.previous", "id", "select_sibling reads the selection, not this extra schema member"),
    ("select.layersAt", "id", "layersAt picks at a point; this extra schema member is unused"),
    ("dev.unfocus", "id", "unfocus clears the editor focus; this extra schema member is unused"),
    ("version.restore", "id", "a saved-history record outside the KDM replica; held through the root's derived history"),
    ("version.rename", "id", "a saved-history record outside the KDM replica; held through the root's derived history"),
    ("version.delete", "id", "a saved-history record outside the KDM replica; held through the root's derived history"),
    ("version.preview", "id", "a saved-history record outside the KDM replica; held through the root's derived history"),
    ("guide.move", "id", "guides are addressed by index and owner; this extra schema member is unused"),
    ("guide.clear", "id", "guides are addressed by owner; this extra schema member is unused"),
    ("text.edit.selectWord", "id", "uses the active text editor; this extra schema member is unused"),
    ("text.edit.selectLine", "id", "uses the active text editor; this extra schema member is unused"),
    ("text.edit.end", "id", "ends the active text editor; this extra schema member is unused"),
    ("paint.documentColors", "id", "reads page; this extra schema member is unused"),
    ("style.move", "id", "reads styles and before; this extra schema member is unused"),
    ("style.renameFolder", "id", "reads folder and name; this extra schema member is unused"),
    ("variable.createCollection", "id", "allocates its own collection id; this extra schema member is unused"),
    ("variable.delete", "id", "reads variable; this extra schema member is unused"),
    ("image.presets", "id", "lists presets; this extra schema member is unused"),
    ("variable.duplicate", "id", "reads variable; this extra schema member is unused"),
    ("instance.place", "id", "reads component; this extra schema member is unused"),
    ("library.place", "component", "resolves in the lent library file, not this document's node namespace"),
    ("library.swapAt", "component", "resolves in the lent library file, not this document's node namespace"),
    ("library.import", "component", "resolves in the lent library file, not this document's node namespace"),
    ("library.importVariable", "variable", "resolves in the lent library file, not this document's variable namespace"),
    ("library.bindVariable", "variable", "resolves in the lent library file, not this document's variable namespace"),
    ("library.aliasVariable", "alias", "resolves in the lent library file, not this document's variable namespace"),
    ("library.setVariableMode", "collection", "resolves in the lent library file, not this document's collection namespace"),
    ("library.setVariableMode", "mode", "resolves in the lent library file, not this document's mode namespace"),
    ("library.extendCollection", "collection", "resolves in the lent library file, not this document's collection namespace"),
    ("proto.flow.rename", "id", "reads start; this extra schema member is unused"),
    ("proto.present", "id", "reads start; this extra schema member is unused"),
    ("proto.preview", "id", "reads start; this extra schema member is unused"),
    ("proto.closePreview", "id", "closes the preview; this extra schema member is unused"),
    ("comment.zoom", "id", "reads ids of threads; this extra schema member is unused"),
    ("vector.edit.tool", "id", "uses the active vector editor; this extra schema member is unused"),
    ("vector.edit.selectAll", "id", "uses the active vector editor; this extra schema member is unused"),
    ("vector.edit.position", "id", "uses the active vector editor; this extra schema member is unused"),
    ("vector.edit.delete", "id", "uses the active vector editor; this extra schema member is unused"),
    ("vector.edit.strokeCap", "id", "uses the active vector editor; this extra schema member is unused"),
    ("vector.edit.strokeJoin", "id", "uses the active vector editor; this extra schema member is unused"),
    ("vector.edit.bounds", "id", "uses the active vector editor; this extra schema member is unused"),
    ("annotation.editCategory", "id", "a category token in settings, not a KDM object id"),
    ("annotation.removeCategory", "id", "a category token in settings, not a KDM object id"),
    ("devMode.removeMeasurement", "id", "reads from; this extra schema member is unused"),
    ("branch.archive", "id", "a saved branch record outside the KDM replica; held through the root's derived branches"),
    ("widget.click", "target", "a callback target string inside the widget, not a node id"),
    ("widget.list", "id", "lists widgets; this extra schema member is unused"),
    ("board.sticky", "id", "creates a sticky; this extra schema member is unused"),
    ("board.shape", "id", "creates a shape; this extra schema member is unused"),
    ("board.stamp", "id", "creates a stamp; this extra schema member is unused"),
    ("tool.sticky", "id", "sets board tool options; this extra schema member is unused"),
    ("tool.shape", "id", "sets board tool options; this extra schema member is unused"),
    ("tool.stamp", "id", "sets board tool options; this extra schema member is unused"),
    ("board.voteResults", "id", "reads voter; this extra schema member is unused"),
    ("board.template", "id", "reads template name; this extra schema member is unused"),
    ("board.templates", "id", "lists templates; this extra schema member is unused"),
    ("file.render", "id", "reads target; this extra schema member is unused"),
];

#[test]
fn every_integer_id_parameter_is_marked() {
    let doc = DesignDocument::create("Ids.kdesign", 320, 240).unwrap();
    let commands = doc.commands();
    for (command, path, reason) in ALLOW {
        assert!(!reason.is_empty());
        let command = commands.iter().find(|c| c.id == *command).unwrap();
        assert!(command.schema.as_ref().and_then(|s| s.get(path)).is_some());
        assert!(!command.objects.contains(path), "stale exception: {} {path}", command.id);
    }
    let allow: Vec<_> = ALLOW.iter().map(|(c, p, _)| (*c, *p)).collect();
    kition_kit::testing::check_id_params(&commands, &allow).unwrap_or_else(|gaps| panic!("{}", gaps.join("\n")));
    let marked: usize = commands.iter().map(|c| c.objects.len() + usize::from(c.targets.is_some())).sum();
    println!("id scanner: {} commands, {marked} marked, {} allowed with reasons", commands.len(), allow.len());
}

#[test]
fn saved_branch_and_version_ids_are_strings_not_node_targets() {
    let doc = DesignDocument::create("Records.kdesign", 320, 240).unwrap();
    for command in doc.commands().into_iter().filter(|c| {
        matches!(
            c.id.as_str(),
            "branch.open"
                | "branch.review"
                | "branch.archive"
                | "branch.merge"
                | "branch.updateFromMain"
                | "version.restore"
                | "version.rename"
                | "version.delete"
                | "version.preview"
        )
    }) {
        assert!(command.targets.is_none(), "{}: saved record, not a layer: {:?}", command.id, command.targets);
        assert_eq!(command.schema.as_ref().unwrap()["id"]["type"], "string", "{}", command.id);
    }
}

#[test]
fn update_library_ids_name_library_links() {
    let doc = DesignDocument::create("Libraries.kdesign", 320, 240).unwrap();
    let command = doc.commands().into_iter().find(|c| c.id == "library.acceptUpdates").unwrap();
    // A list naming objects declares its items: the SDK reads the names of a list only there.
    assert!(command.objects.contains(&"ids[]"));
    assert!(command.object_types.contains(&("ids[]", &["Library"][..])));
    assert!(command.name_params.contains(&("ids[]", "name")));
}

// Audited resolvers: Document::style, Variables::get/collection/find_mode, and library links.
// Names used to create/rename an object and local property/annotation names are not references.
fn name_parameters(command: &str) -> Vec<(&'static str, &'static str)> {
    let mut out = Vec::new();
    if matches!(command, "style.apply" | "style.update" | "style.bind" | "style.duplicate" | "style.delete") {
        out.push(("style", "Style"));
    }
    if matches!(
        command,
        "variable.set" | "variable.bind" | "variable.delete" | "variable.update" | "variable.duplicate" | "variable.resetOverride" | "style.bind"
    ) {
        out.push(("variable", "Variable"));
    }
    if matches!(
        command,
        "variable.create"
            | "variable.addMode"
            | "variable.setMode"
            | "variable.renameMode"
            | "variable.removeMode"
            | "variable.moveMode"
            | "variable.renameCollection"
            | "variable.deleteCollection"
    ) {
        out.push(("collection", "Collection"));
    }
    if matches!(
        command,
        "variable.set" | "variable.setMode" | "variable.renameMode" | "variable.removeMode" | "variable.moveMode" | "variable.resetOverride"
    ) {
        out.push(("mode", "Mode"));
    }
    if command == "variable.addMode" {
        out.push(("from", "Mode"));
    }
    out
}

#[test]
fn resources_keep_their_command_ids_in_the_model_after_reopen() {
    use kition_kit::session::model::Exchange;

    let mut doc = DesignDocument::create("Ids.kdesign", 320, 240).unwrap();
    let style = doc.execute("style.create", serde_json::json!({"type":"paint", "name":"Accent", "value":[]})).unwrap()["id"].clone();
    let collection = doc.execute("variable.createCollection", serde_json::json!({"name":"Theme", "modes":["Day"]})).unwrap()["id"].clone();
    let variable =
        doc.execute("variable.create", serde_json::json!({"collection":collection, "name":"Spacing", "type":"number", "value":8})).unwrap()["id"]
            .clone();
    let thread = doc.execute("comment.add", serde_json::json!({"text":"Review", "x":5, "y":5})).unwrap()["id"].clone();
    let bytes = doc.save(None).unwrap();
    for mut doc in [doc, DesignDocument::open("Ids.kdesign", &bytes).unwrap()] {
        let mut exchange = Exchange::new();
        let loaded = exchange
            .batch(
                &mut doc,
                &serde_json::json!({"ops":[], "load":[{"target":"root", "fields":["styles","collections","variables","comments"]}]}),
                false,
            )
            .unwrap();
        for (field, id) in [("styles", &style), ("collections", &collection), ("variables", &variable), ("comments", &thread)] {
            // The raw KDM exchange encodes references as strings; MCP normalizes integer ids.
            let canonical = id.as_str().map(str::to_owned).unwrap_or_else(|| id.to_string());
            assert_eq!(loaded["loaded"][0]["fields"][field][0], canonical, "{field}: {loaded}");
        }
    }
}

#[test]
fn references_are_typed_and_nested_style_members_are_marked() {
    let doc = DesignDocument::create("Ids.kdesign", 320, 240).unwrap();
    for c in doc.commands() {
        for path in c.targets.into_iter().chain(c.objects.iter().copied()) {
            assert!(c.object_types.iter().any(|(p, ts)| *p == path && !ts.is_empty()), "{} {path}", c.id);
        }
        if matches!(c.id.as_str(), "node.set" | "node.create" | "text.create") {
            for path in ["props.fillStyle", "props.strokeStyle", "props.effectStyle", "props.textStyle", "props.gridStyle"] {
                assert!(c.objects.contains(&path), "{} {path}", c.id);
                assert!(c.object_types.contains(&(path, &["Style"][..])), "{} {path}", c.id);
            }
        }
        if matches!(c.id.as_str(), "board.connector" | "board.reattach") {
            for path in ["from", "to", "target"] {
                assert!(!c.objects.contains(&path), "{} coordinates are not ids", c.id);
            }
        }
    }
}

fn name_gaps(commands: &[kition_kit::KitCommand]) -> Vec<String> {
    let mut gaps = Vec::new();
    for c in commands {
        for (path, ty) in name_parameters(&c.id) {
            if c.schema.as_ref().and_then(|s| s.get(path)).is_none() {
                continue;
            }
            let names = if c.targets == Some(path) { c.name_field == Some("name") } else { c.name_params.contains(&(path, "name")) };
            if !names || !c.object_types.iter().any(|(p, ts)| *p == path && ts.contains(&ty)) {
                gaps.push(format!("{} {path}: {ty}", c.id));
            }
        }
        if c.id == "style.move" {
            // `styles` is a list: its items are declared (`styles[]`).
            for path in ["styles[]", "before"] {
                if !c.name_params.contains(&(path, "name")) {
                    gaps.push(format!("{} {path}: Style", c.id));
                }
            }
        }
    }
    gaps
}

#[test]
fn resource_name_parameters_are_typed_and_declared() {
    let doc = DesignDocument::create("Ids.kdesign", 320, 240).unwrap();
    let mut commands = doc.commands();
    let gaps = name_gaps(&commands);
    assert!(gaps.is_empty(), "{}", gaps.join("\n"));
    let marked: usize = commands.iter().map(|c| c.name_params.len() + usize::from(c.name_field.is_some())).sum();
    println!("name scanner: {marked} marked");
    commands.iter_mut().find(|c| c.id == "style.apply").unwrap().name_params = &[];
    assert!(name_gaps(&commands).iter().any(|gap| gap == "style.apply style: Style"));
}
