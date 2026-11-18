//! The parameters commands read that their notation does not spell out, added to the schemas the
//! catalogue offers agents. Every command refuses a key its schema does not name (engine rule E7a),
//! so the schema must name every key the command reads: the keys a
//! handler reads beside its listed ones.

use serde_json::{Value, json};

#[derive(Clone, Copy)]
enum Kind {
    Number,
    String,
    Boolean,
    Array,
    Object,
    IndexOrName,
    /// A name, or an object.
    NameOrObject,
}

impl Kind {
    fn node(self) -> Value {
        let description = "read by the command beside its listed parameters";
        let kind = match self {
            Kind::IndexOrName => return json!({ "oneOf": [{ "type": "integer" }, { "type": "string" }], "description": description }),
            Kind::NameOrObject => {
                return json!({ "oneOf": [{ "type": "string" }, { "type": "object", "additionalProperties": true }], "description": description });
            }
            Kind::Number => "number",
            Kind::String => "string",
            Kind::Boolean => "boolean",
            Kind::Array => "array",
            Kind::Object => return json!({ "type": "object", "additionalProperties": true, "description": description }),
        };
        json!({ "type": kind, "description": description })
    }
}

/// Keys a command's handler reads (directly or through a helper it passes its parameters to) that
/// its notation leaves out, found by reading each handler.
const EXTRA: &[(&str, &[(&str, Kind)])] = &[
    ("align.bottom", &[("id", Kind::IndexOrName)]),
    ("align.hcenter", &[("id", Kind::IndexOrName)]),
    ("align.left", &[("id", Kind::IndexOrName)]),
    ("align.right", &[("id", Kind::IndexOrName)]),
    ("align.top", &[("id", Kind::IndexOrName)]),
    ("align.vcenter", &[("id", Kind::IndexOrName)]),
    ("annotation.editCategory", &[("page", Kind::String), ("assets", Kind::Boolean), ("query", Kind::String), ("server", Kind::String)]),
    ("annotation.list", &[("assets", Kind::Boolean), ("query", Kind::String), ("server", Kind::String)]),
    ("annotation.removeCategory", &[("page", Kind::String), ("assets", Kind::Boolean), ("query", Kind::String), ("server", Kind::String)]),
    ("arrange.spacing", &[("id", Kind::IndexOrName)]),
    ("arrange.tidy", &[("id", Kind::IndexOrName)]),
    ("board.connector", &[("ids", Kind::Array), ("id", Kind::IndexOrName)]),
    ("board.label", &[("ids", Kind::Array)]),
    ("board.setStyle", &[("id", Kind::IndexOrName), ("text", Kind::String)]),
    ("board.shape", &[("id", Kind::IndexOrName)]),
    ("board.stamp", &[("id", Kind::IndexOrName)]),
    ("board.sticky", &[("id", Kind::IndexOrName)]),
    ("board.template", &[("id", Kind::IndexOrName), ("text", Kind::String)]),
    ("board.templates", &[("id", Kind::IndexOrName)]),
    ("board.timerState", &[("text", Kind::String)]),
    ("board.voteResults", &[("id", Kind::IndexOrName)]),
    ("command.batch", &[("fuel", Kind::Number), ("plugin", Kind::String), ("x", Kind::Number), ("y", Kind::Number)]),
    ("comment.edit", &[("link", Kind::String), ("index", Kind::Number), ("ids", Kind::Array)]),
    ("comment.link", &[("link", Kind::String)]),
    ("comment.move", &[("link", Kind::String)]),
    ("comment.open", &[("maxLines", Kind::Number), ("orphans", Kind::Boolean), ("minSize", Kind::Number), ("ids", Kind::Array)]),
    ("comment.react", &[("link", Kind::String), ("ids", Kind::Array), ("index", Kind::Number)]),
    ("comment.zoom", &[("maxLines", Kind::Number), ("orphans", Kind::Boolean), ("minSize", Kind::Number), ("id", Kind::IndexOrName)]),
    ("component.bindProperty", &[("ids", Kind::Array), ("id", Kind::IndexOrName)]),
    ("dev.resolveChange", &[("id", Kind::IndexOrName)]),
    ("dev.unfocus", &[("direction", Kind::String), ("ids", Kind::Array), ("id", Kind::IndexOrName)]),
    ("devMode.measurements", &[("ids", Kind::Array)]),
    ("devMode.removeMeasurement", &[("ids", Kind::Array), ("id", Kind::IndexOrName)]),
    ("distribute.horizontal", &[("fromGesture", Kind::Boolean), ("id", Kind::IndexOrName)]),
    ("distribute.vertical", &[("fromGesture", Kind::Boolean), ("id", Kind::IndexOrName)]),
    ("edit.copy", &[("index", Kind::Number), ("id", Kind::IndexOrName)]),
    ("edit.copyProperties", &[("index", Kind::Number), ("ids", Kind::Array)]),
    ("edit.cut", &[("index", Kind::Number), ("id", Kind::IndexOrName)]),
    ("edit.delete", &[("index", Kind::Number), ("id", Kind::IndexOrName)]),
    ("edit.duplicate", &[("index", Kind::Number), ("id", Kind::IndexOrName), ("style", Kind::String), ("variable", Kind::String)]),
    ("edit.findReplace", &[("index", Kind::Number), ("ids", Kind::Array), ("id", Kind::IndexOrName)]),
    ("edit.pasteOver", &[("index", Kind::Number), ("ids", Kind::Array), ("id", Kind::IndexOrName)]),
    ("edit.pasteProperties", &[("index", Kind::Number), ("id", Kind::IndexOrName)]),
    ("edit.pasteReplace", &[("index", Kind::Number), ("id", Kind::IndexOrName)]),
    ("edit.pickColor", &[("index", Kind::Number), ("ids", Kind::Array), ("id", Kind::IndexOrName)]),
    ("edit.setDefaultProperties", &[("index", Kind::Number), ("ids", Kind::Array)]),
    ("effect.add", &[("effect", Kind::Object)]),
    ("effect.copy", &[("ids", Kind::Array)]),
    ("effect.set", &[("effect", Kind::Object)]),
    ("export.add", &[("setting", Kind::Object)]),
    ("frame.swapOrientation", &[("id", Kind::IndexOrName), ("anchor", Kind::String), ("width", Kind::Number), ("height", Kind::Number)]),
    ("guide.clear", &[("id", Kind::IndexOrName)]),
    ("guide.move", &[("id", Kind::IndexOrName)]),
    ("image.presets", &[("id", Kind::IndexOrName)]),
    ("inspect.properties", &[("ids", Kind::Array), ("target", Kind::String)]),
    ("instance.exposed", &[("ids", Kind::Array)]),
    ("instance.place", &[("id", Kind::IndexOrName), ("fuel", Kind::Number), ("plugin", Kind::String)]),
    ("instance.resetSlot", &[("ids", Kind::Array)]),
    ("instance.slotInsert", &[("ids", Kind::Array)]),
    ("instance.swap", &[("id", Kind::IndexOrName)]),
    ("library.acceptUpdates", &[("library", Kind::String), ("collection", Kind::String), ("name", Kind::String)]),
    ("library.publish", &[("include", Kind::Array)]),
    ("library.remove", &[("library", Kind::String)]),
    ("measure.distances", &[("id", Kind::IndexOrName)]),
    ("measure.padding", &[("ids", Kind::Array)]),
    ("node.reparent", &[("id", Kind::IndexOrName)]),
    ("object.mask", &[("id", Kind::IndexOrName)]),
    ("object.opacityKeys", &[("id", Kind::IndexOrName)]),
    ("object.rasterize", &[("id", Kind::IndexOrName)]),
    ("object.releaseMask", &[("id", Kind::IndexOrName)]),
    ("object.resizeToFit", &[("maxLines", Kind::Number), ("orphans", Kind::Boolean), ("minSize", Kind::Number), ("id", Kind::IndexOrName)]),
    ("object.ungroup", &[("id", Kind::IndexOrName)]),
    ("page.move", &[("fuel", Kind::Number), ("plugin", Kind::String), ("x", Kind::Number), ("y", Kind::Number)]),
    ("paint.documentColors", &[("id", Kind::IndexOrName)]),
    ("proto.action.add", &[("action", Kind::Object)]),
    ("proto.action.set", &[("action", Kind::Object), ("path", Kind::Array), ("ids", Kind::Array)]),
    ("proto.closePreview", &[("index", Kind::Number), ("force", Kind::Boolean), ("id", Kind::IndexOrName)]),
    ("proto.conditional.setCondition", &[("text", Kind::String), ("ids", Kind::Array)]),
    ("proto.evaluate", &[("text", Kind::String), ("ids", Kind::Array)]),
    ("proto.flow.rename", &[("id", Kind::IndexOrName)]),
    ("proto.interaction.clear", &[("id", Kind::IndexOrName)]),
    // C8-design-read: the notation's second braces (`interaction`, or the fields it patches) were
    // never in the schema, so every call that set something was refused.
    (
        "proto.interaction.set",
        &[
            ("interaction", Kind::Object),
            ("trigger", Kind::Object),
            ("action", Kind::Object),
            ("actions", Kind::Array),
            ("transition", Kind::Object),
            ("durationMs", Kind::Number),
            ("easing", Kind::NameOrObject),
            ("delayMs", Kind::Number),
        ],
    ),
    ("proto.present", &[("id", Kind::IndexOrName)]),
    ("proto.presentOptions", &[("scale", Kind::String), ("deviceFrame", Kind::Boolean), ("hotspotHints", Kind::Boolean), ("inspect", Kind::Boolean)]),
    ("proto.preview", &[("id", Kind::IndexOrName)]),
    ("select.children", &[("ids", Kind::Array), ("id", Kind::IndexOrName)]),
    ("select.layersAt", &[("id", Kind::IndexOrName), ("ids", Kind::Array), ("item", Kind::String), ("value", Kind::String)]),
    ("select.next", &[("ids", Kind::Array), ("id", Kind::IndexOrName)]),
    ("select.parent", &[("ids", Kind::Array), ("id", Kind::IndexOrName)]),
    ("select.previous", &[("ids", Kind::Array), ("id", Kind::IndexOrName)]),
    ("selection.colors", &[("id", Kind::IndexOrName), ("text", Kind::String)]),
    ("stroke.brush", &[("fromGesture", Kind::Boolean), ("id", Kind::IndexOrName)]),
    ("stroke.copy", &[("ids", Kind::Array)]),
    ("style.move", &[("ids", Kind::Array), ("id", Kind::IndexOrName)]),
    ("style.renameFolder", &[("ids", Kind::Array), ("id", Kind::IndexOrName)]),
    ("text.alignCenter", &[("id", Kind::IndexOrName), ("text", Kind::String)]),
    ("text.alignJustify", &[("id", Kind::IndexOrName), ("text", Kind::String)]),
    ("text.alignLeft", &[("id", Kind::IndexOrName), ("text", Kind::String)]),
    ("text.alignRight", &[("id", Kind::IndexOrName), ("text", Kind::String)]),
    ("text.bold", &[("text", Kind::String)]),
    ("text.caseLower", &[("text", Kind::String)]),
    ("text.caseOriginal", &[("text", Kind::String)]),
    ("text.caseSmallCaps", &[("id", Kind::IndexOrName), ("text", Kind::String)]),
    ("text.caseTitle", &[("text", Kind::String)]),
    ("text.caseUpper", &[("text", Kind::String)]),
    ("text.edit.begin", &[("text", Kind::String), ("index", Kind::Number), ("ids", Kind::Array)]),
    ("text.edit.end", &[("text", Kind::String), ("index", Kind::Number), ("ids", Kind::Array), ("id", Kind::IndexOrName)]),
    ("text.edit.selectLine", &[("text", Kind::String), ("ids", Kind::Array), ("id", Kind::IndexOrName)]),
    ("text.edit.selectWord", &[("text", Kind::String), ("ids", Kind::Array), ("id", Kind::IndexOrName)]),
    ("text.endPreview", &[("text", Kind::String)]),
    ("text.italic", &[("text", Kind::String)]),
    ("text.lineHeightDown", &[("text", Kind::String)]),
    ("text.lineHeightUp", &[("text", Kind::String)]),
    ("text.maxLines", &[("id", Kind::IndexOrName), ("text", Kind::String)]),
    ("text.missingFonts", &[("text", Kind::String)]),
    ("text.onPath", &[("id", Kind::IndexOrName), ("ids", Kind::Array)]),
    ("text.paragraphIndent", &[("id", Kind::IndexOrName), ("text", Kind::String)]),
    ("text.paragraphSpacing", &[("id", Kind::IndexOrName), ("text", Kind::String), ("fromGesture", Kind::Boolean)]),
    ("text.pathOptions", &[("id", Kind::IndexOrName), ("text", Kind::String)]),
    ("text.refit", &[("text", Kind::String), ("id", Kind::IndexOrName)]),
    ("text.sizeDown", &[("text", Kind::String)]),
    ("text.sizeUp", &[("text", Kind::String)]),
    ("text.strikethrough", &[("text", Kind::String)]),
    ("text.trackingDown", &[("fromGesture", Kind::Boolean), ("id", Kind::IndexOrName), ("text", Kind::String)]),
    ("text.trackingUp", &[("fromGesture", Kind::Boolean), ("id", Kind::IndexOrName), ("text", Kind::String)]),
    ("text.underline", &[("text", Kind::String)]),
    ("text.weightDown", &[("text", Kind::String)]),
    ("text.weightUp", &[("text", Kind::String)]),
    ("tool.shape", &[("target", Kind::String), ("id", Kind::IndexOrName), ("text", Kind::String)]),
    ("tool.stamp", &[("target", Kind::String), ("id", Kind::IndexOrName)]),
    ("tool.sticky", &[("target", Kind::String), ("id", Kind::IndexOrName)]),
    ("tool.text", &[("text", Kind::String)]),
    ("transform.move", &[("id", Kind::IndexOrName)]),
    ("transform.resize", &[("id", Kind::IndexOrName)]),
    ("transform.rotationOrigin", &[("fuel", Kind::Number), ("plugin", Kind::String), ("x", Kind::Number), ("y", Kind::Number)]),
    ("transform.setRotationOrigin", &[("fuel", Kind::Number), ("plugin", Kind::String)]),
    ("variable.createCollection", &[("server", Kind::String), ("id", Kind::IndexOrName), ("modes", Kind::Array)]),
    ("variable.delete", &[("ids", Kind::Array), ("id", Kind::IndexOrName)]),
    ("variable.duplicate", &[("id", Kind::IndexOrName)]),
    ("variable.setMode", &[("id", Kind::IndexOrName)]),
    ("variable.update", &[("library", Kind::String)]),
    ("vector.edit.bounds", &[("index", Kind::Number), ("ids", Kind::Array), ("id", Kind::IndexOrName)]),
    ("vector.edit.delete", &[("ids", Kind::Array), ("id", Kind::IndexOrName)]),
    ("vector.edit.position", &[("ids", Kind::Array), ("id", Kind::IndexOrName)]),
    ("vector.edit.resize", &[("index", Kind::Number), ("ids", Kind::Array), ("id", Kind::IndexOrName)]),
    ("vector.edit.selectAll", &[("ids", Kind::Array), ("id", Kind::IndexOrName)]),
    ("vector.edit.strokeCap", &[("ids", Kind::Array), ("id", Kind::IndexOrName)]),
    ("vector.edit.strokeJoin", &[("link", Kind::String), ("code", Kind::String), ("ids", Kind::Array), ("id", Kind::IndexOrName)]),
    ("vector.edit.tool", &[("ids", Kind::Array), ("id", Kind::IndexOrName)]),
    ("vector.edit.transform", &[("index", Kind::Number), ("ids", Kind::Array), ("id", Kind::IndexOrName)]),
    ("vector.join", &[("link", Kind::String), ("code", Kind::String), ("id", Kind::IndexOrName)]),
    ("vector.simplify", &[("id", Kind::IndexOrName)]),
    ("vector.smoothJoin", &[("link", Kind::String), ("code", Kind::String), ("id", Kind::IndexOrName)]),
    ("widget.list", &[("id", Kind::IndexOrName), ("ids", Kind::Array), ("item", Kind::String), ("value", Kind::String)]),
    ("file.render", &[("commands", Kind::Array), ("id", Kind::IndexOrName)]),
];

/// Adds to `schema` (a command's `ParameterSchemaSpec`) the keys `id` reads that its notation
/// leaves out.
pub fn complete(id: &str, schema: &mut Value) {
    let Some(properties) = schema.as_object_mut() else { return };
    if matches!(
        id,
        "branch.open"
            | "branch.review"
            | "branch.archive"
            | "branch.merge"
            | "branch.updateFromMain"
            | "version.restore"
            | "version.rename"
            | "version.delete"
            | "version.preview"
    ) && let Some(record) = properties.get_mut("id").and_then(Value::as_object_mut)
    {
        record.remove("oneOf");
        record.insert("type".into(), json!("string"));
    }
    if (id.starts_with("proto.action.") || id.starts_with("proto.conditional."))
        && let Some(path) = properties.get_mut("path").and_then(Value::as_object_mut)
    {
        path.remove("oneOf");
        path.insert("type".into(), json!("array"));
        path.insert("items".into(), json!({"type":"integer"}));
    }
    for (key, kind) in EXTRA.iter().filter(|(command, _)| *command == id).flat_map(|(_, keys)| keys.iter()) {
        properties.entry(*key).or_insert_with(|| kind.node());
    }
}
