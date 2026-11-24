//! What each command acts on and the editor state it reads besides its parameters
//! (`KitCommand::targets`, `KitCommand::reads`), for kition-sdk's explicit call context: a host
//! sets the current page and the selection a command reads from a run step's `at.part` and `on`,
//! and puts them back after the call (task card C4/C5/C6-design).
//!
//! Both are what the engine's code does, not what descriptions say: `tests/acts.rs` follows each
//! command's `run` code through the engine to the accessors of the selection (and
//! `params::targets`, which falls back to it when a command is given no ids), of the current page
//! and of the edit modes and the tool, and to the helpers that read a parameter as a layer id, and
//! fails when the scan tables say otherwise. The independent handler reading in
//! `tests/declarations_independent.rs` gates the final declarations, including corrections to
//! that name-based scan. The engine has no enabled predicate, so `run` is all there is to follow.
//! No call context sets `Mode`: a run sets it with the kit's own command
//! (`text.edit.begin`, `vector.edit.begin`, `tool.*`) in an earlier step.
use kition_kit::EditorState::{self, Mode, Part, Selection};

const ALL: &[EditorState] = &[Mode, Part, Selection];
const MODE: &[EditorState] = &[Mode];
const MODE_AND_SELECTION: &[EditorState] = &[Mode, Selection];
const PART: &[EditorState] = &[Part];
const SELECTION: &[EditorState] = &[Selection];
const SELECTION_AND_PART: &[EditorState] = &[Part, Selection];

/// The editor state engine command `id` reads besides its parameters: the selection (what a
/// command given no `ids` acts on), the current page (where a command given no parent or page
/// acts) and the modes (text and vector editing, the tool). The independent handler reading
/// corrects the source scan where its call graph follows unrelated helpers or restoration.
pub fn reads(id: &str) -> &'static [EditorState] {
    if let Some((_, states)) = REVIEWED.iter().find(|(command, _)| *command == id) {
        return states;
    }
    let scanned = scanned_reads(id);
    let Some((_, more)) = READING.iter().find(|(command, _)| *command == id) else { return scanned };
    let contains = |state| scanned.contains(&state) || more.contains(&state);
    match (contains(Mode), contains(Part), contains(Selection)) {
        (true, true, true) => ALL,
        (true, true, false) => &[Mode, Part],
        (true, false, true) => MODE_AND_SELECTION,
        (true, false, false) => MODE,
        (false, true, true) => SELECTION_AND_PART,
        (false, true, false) => PART,
        (false, false, true) => SELECTION,
        (false, false, false) => &[],
    }
}

/// Full editor-state declarations reviewed against the handlers (C8-design-read). The source
/// scan resolves an unqualified helper name to every definition when none is in the caller's
/// file, follows branches without their parameter predicates, and includes state restored after
/// an edit. These are useful scan candidates, but are not all inputs to the command's result.
/// At SDK 1935d52 extra mode/part metadata does not itself create a premise conflict; it still
/// misdescribes what a caller must establish. The independent rows gate these published reads.
pub const REVIEWED: &[(&str, &[EditorState])] = &[
    // These handlers take explicit objects, falling back only to the selection. Their helpers
    // do not use the current page or an edit mode to compute the result.
    ("arrange.reorder", SELECTION),
    ("arrange.spacing", SELECTION),
    ("board.label", SELECTION),
    ("board.reattach", SELECTION),
    ("bool.exclude", SELECTION),
    ("bool.intersect", SELECTION),
    ("bool.subtract", SELECTION),
    ("bool.union", SELECTION),
    ("codeConnect.remove", SELECTION),
    ("codeConnect.set", SELECTION),
    ("component.combineVariants", SELECTION),
    ("component.create", SELECTION),
    ("component.createMultiple", SELECTION),
    ("component.detach", SELECTION),
    ("component.editProperty", SELECTION),
    ("edit.cut", SELECTION),
    ("library.applyStyle", SELECTION),
    ("node.set", SELECTION),
    ("object.batchRename", SELECTION),
    ("object.toggleLock", SELECTION),
    ("object.toggleVisible", SELECTION),
    ("shapeBuilder.merge", SELECTION),
    ("shapeBuilder.remove", SELECTION),
    ("stroke.widthProfile", SELECTION),
    ("style.apply", SELECTION),
    ("vector.edit.begin", SELECTION),
    // Creation uses the current page and, for a missing position/style, the view or tool.
    // Selection changes after creation are outputs, not inputs.
    ("board.connector", &[Mode, Part]),
    ("board.ink", &[Mode, Part]),
    ("file.import", &[Mode, Part]),
    ("frame.create", &[Mode, Part]),
    ("library.place", &[Mode, Part]),
    ("tool.place", &[Mode, Part]),
    // Named document records and whole-document operations do not read editor navigation.
    ("codeConnect.import", &[]),
    ("comment.delete", &[]),
    ("comment.edit", &[]),
    ("library.add", &[]),
    ("library.import", &[]),
    ("library.publish", &[]),
    ("library.swap", &[]),
    ("style.bind", &[]),
    ("text.refit", &[]),
    ("version.restore", &[]),
    // Formatting uses the text-edit range or the selection; rotation uses the saved origin.
    // The objects themselves identify their pages.
    ("edit.pasteProperties", MODE_AND_SELECTION),
    ("edit.repeat", MODE_AND_SELECTION),
    ("object.flipH", MODE_AND_SELECTION),
    ("object.flipV", MODE_AND_SELECTION),
    ("object.rotate180", MODE_AND_SELECTION),
    ("object.rotateLeft", MODE_AND_SELECTION),
    ("object.rotateRight", MODE_AND_SELECTION),
    ("text.alignCenter", MODE_AND_SELECTION),
    ("text.alignJustify", MODE_AND_SELECTION),
    ("text.alignLeft", MODE_AND_SELECTION),
    ("text.alignRight", MODE_AND_SELECTION),
    ("text.bold", MODE_AND_SELECTION),
    ("text.bulletList", MODE_AND_SELECTION),
    ("text.case", MODE_AND_SELECTION),
    ("text.caseLower", MODE_AND_SELECTION),
    ("text.caseOriginal", MODE_AND_SELECTION),
    ("text.caseSmallCaps", MODE_AND_SELECTION),
    ("text.caseTitle", MODE_AND_SELECTION),
    ("text.caseUpper", MODE_AND_SELECTION),
    ("text.decoration", MODE_AND_SELECTION),
    ("text.italic", MODE_AND_SELECTION),
    ("text.lineHeightDown", MODE_AND_SELECTION),
    ("text.lineHeightUp", MODE_AND_SELECTION),
    ("text.link", MODE_AND_SELECTION),
    ("text.listStyle", MODE_AND_SELECTION),
    ("text.maxLines", MODE_AND_SELECTION),
    ("text.numberList", MODE_AND_SELECTION),
    ("text.paragraphIndent", MODE_AND_SELECTION),
    ("text.paragraphSpacing", MODE_AND_SELECTION),
    ("text.pathOptions", MODE_AND_SELECTION),
    ("text.removeFromPath", MODE_AND_SELECTION),
    ("text.set", MODE_AND_SELECTION),
    ("text.setFeatures", MODE_AND_SELECTION),
    ("text.setVariations", MODE_AND_SELECTION),
    ("text.sizeDown", MODE_AND_SELECTION),
    ("text.sizeUp", MODE_AND_SELECTION),
    ("text.strikethrough", MODE_AND_SELECTION),
    ("text.trackingDown", MODE_AND_SELECTION),
    ("text.trackingUp", MODE_AND_SELECTION),
    ("text.underline", MODE_AND_SELECTION),
    ("text.weightDown", MODE_AND_SELECTION),
    ("text.weightUp", MODE_AND_SELECTION),
    ("transform.nudge", MODE_AND_SELECTION),
    // These placement paths use a named parent or the current page, never the selection.
    ("file.placeImage", PART),
    ("image.place", PART),
    ("instance.swapAt", PART),
    ("library.swapAt", PART),
    ("shape.arrow", PART),
    ("shape.ellipse", PART),
    ("shape.line", PART),
    ("shape.polygon", PART),
    ("shape.rectangle", PART),
    ("shape.star", PART),
    ("vector.create", PART),
    // Text creation also reads ui_dark to select a readable default fill in the editor.
    ("node.create", &[Mode, Part]),
    ("text.create", &[Mode, Part]),
    // These act on the layer/range already held by the edit session, not the layer selection.
    ("text.edit.insert", MODE),
    ("text.edit.key", MODE),
    ("text.listIndent", MODE),
    ("text.listOutdent", MODE),
    ("vector.edit.resize", MODE),
    ("vector.edit.rotate", MODE),
    ("vector.edit.transform", MODE),
    ("vector.edit.widthPoint", MODE),
];

/// Editor state the independent reading of the handlers found (task card C8-design-read,
/// `tests/declarations_independent.rs`) that the code scan of `tests/acts.rs` does not reach:
/// the tool switches end an active vector edit (`Session::set_tool`, `canvas/tools/pointer.rs`),
/// and with no colour a board shape takes the shape tool's; a batch's steps read the selection
/// when they name nothing; `guide.remove` without an index removes the selected guide
/// (`cmd/page.rs`); `text.style` styles the edited characters while text is edited;
/// `library.setVariableMode` without ids sets the mode on the selection, else the current page
/// (`cmd/styles.rs`); `tool.place` draws on the current page.
pub const READING: &[(&str, &[EditorState])] = &[
    ("board.shape", MODE),
    ("command.batch", SELECTION),
    ("edit.pickColor", MODE),
    ("guide.remove", MODE),
    ("library.setVariableMode", SELECTION_AND_PART),
    ("text.style", MODE),
    ("tool.arrow", MODE),
    ("tool.comment", MODE),
    ("tool.connector", MODE),
    ("tool.ellipse", MODE),
    ("tool.eraser", MODE),
    ("tool.frame", MODE),
    ("tool.hand", MODE),
    ("tool.highlighter", MODE),
    ("tool.line", MODE),
    ("tool.marker", MODE),
    ("tool.move", MODE),
    ("tool.pen", MODE),
    ("tool.pencil", MODE),
    ("tool.place", &[Mode, Part]),
    ("tool.polygon", MODE),
    ("tool.rectangle", MODE),
    ("tool.scale", MODE),
    ("tool.section", MODE),
    ("tool.set", MODE),
    ("tool.shape", MODE),
    ("tool.shapeBuilder", MODE),
    ("tool.slice", MODE),
    ("tool.stamp", MODE),
    ("tool.star", MODE),
    ("tool.sticky", MODE),
    ("tool.text", MODE),
    ("tool.textOnPath", MODE),
];

/// What the code scan (`tests/acts.rs`) finds each command reads.
pub fn scanned_reads(id: &str) -> &'static [EditorState] {
    let base = base_reads(id);
    if matches!(
        id,
        "file.placeImage"
            | "edit.cut"
            | "edit.copy"
            | "edit.paste"
            | "edit.pasteOver"
            | "edit.pasteReplace"
            | "edit.duplicate"
            | "edit.copyProperties"
            | "edit.setDefaultProperties"
            | "view.pixelGrid"
            | "view.layoutGrids"
            | "view.rulers"
            | "view.slices"
            | "view.comments"
            | "view.outlines"
            | "view.outlinesSelection"
            | "view.outlinesHiddenLayers"
            | "view.outlinesObjectBounds"
            | "view.pixelPreview"
            | "view.maskOutlines"
            | "view.frameOutlines"
            | "view.proofSrgb"
            | "view.prevFrame"
            | "view.nextFrame"
            | "component.create"
            | "component.createMultiple"
            | "component.goToMain"
            | "object.toggleVisible"
            | "object.toggleLock"
            | "view.snapGeometry"
            | "view.snapObjects"
            | "view.snapPixel"
            | "pref.keepTool"
            | "pref.highlightHover"
            | "pref.renameDuplicates"
            | "pref.showDimensions"
            | "pref.numberKeysOpacity"
            | "pref.nudge"
            | "image.place"
            | "select.layersAt"
            | "transform.nudgeSize"
            | "fill.copy"
            | "fill.paste"
            | "stroke.copy"
            | "stroke.paste"
            | "effect.copy"
            | "effect.paste"
            | "arrange.spacing"
            | "transform.rotate"
            | "transform.rotationOrigin"
            | "transform.setRotationOrigin"
            | "dev.focus"
            | "library.list"
            | "library.add"
            | "proto.linkNotice"
            | "comment.add"
            | "comment.reply"
            | "comment.edit"
            | "comment.react"
            | "comment.delete"
            | "comment.markRead"
            | "comment.open"
            | "comment.next"
            | "comment.zoom"
            | "comment.people"
            | "comment.list"
            | "annotation.list"
            | "board.sticky"
            | "board.stamp"
            | "board.vote"
            | "board.voteResults"
            | "board.table"
            | "board.template"
    ) {
        return match (base.contains(&Part), base.contains(&Selection)) {
            (true, true) => ALL,
            (false, true) => MODE_AND_SELECTION,
            (true, false) => &[Mode, Part],
            (false, false) => MODE,
        };
    }
    base
}

fn base_reads(id: &str) -> &'static [EditorState] {
    match id {
        "board.connector"
        | "board.ink"
        | "board.label"
        | "bool.exclude"
        | "bool.intersect"
        | "bool.subtract"
        | "bool.union"
        | "codeConnect.import"
        | "codeConnect.remove"
        | "codeConnect.set"
        | "component.combineVariants"
        | "component.editProperty"
        | "component.playground"
        | "edit.pasteDesignLayers"
        | "edit.pasteProperties"
        | "edit.repeat"
        | "file.export"
        | "file.exportSvg"
        | "file.import"
        | "frame.create"
        | "inspect.code"
        | "inspect.componentCode"
        | "library.applyStyle"
        | "library.import"
        | "library.place"
        | "library.publish"
        | "library.swap"
        | "library.swapAt"
        | "node.create"
        | "node.set"
        | "object.flipH"
        | "object.flipV"
        | "object.rotate180"
        | "object.rotateLeft"
        | "object.rotateRight"
        | "selection.colors"
        | "shape.arrow"
        | "shape.ellipse"
        | "shape.line"
        | "shape.polygon"
        | "shape.rectangle"
        | "shape.star"
        | "shapeBuilder.merge"
        | "shapeBuilder.remove"
        | "stroke.widthProfile"
        | "text.alignCenter"
        | "text.alignJustify"
        | "text.alignLeft"
        | "text.alignRight"
        | "text.bold"
        | "text.bulletList"
        | "text.case"
        | "text.caseLower"
        | "text.caseOriginal"
        | "text.caseSmallCaps"
        | "text.caseTitle"
        | "text.caseUpper"
        | "text.create"
        | "text.decoration"
        | "text.edit.begin"
        | "text.edit.insert"
        | "text.edit.key"
        | "text.edit.preedit"
        | "text.italic"
        | "text.lineHeightDown"
        | "text.lineHeightUp"
        | "text.link"
        | "text.listIndent"
        | "text.listOutdent"
        | "text.listStyle"
        | "text.maxLines"
        | "text.missingFonts"
        | "text.numberList"
        | "text.paragraphIndent"
        | "text.paragraphSpacing"
        | "text.pathOptions"
        | "text.preview"
        | "text.refit"
        | "text.removeFromPath"
        | "text.set"
        | "text.setFeatures"
        | "text.setVariations"
        | "text.sizeDown"
        | "text.sizeUp"
        | "text.strikethrough"
        | "text.trackingDown"
        | "text.trackingUp"
        | "text.underline"
        | "text.weightDown"
        | "text.weightUp"
        | "transform.nudge"
        | "vector.create"
        | "vector.edit.begin"
        | "vector.edit.resize"
        | "vector.edit.rotate"
        | "vector.edit.transform"
        | "vector.edit.widthPoint"
        | "widget.click"
        | "widget.insert"
        | "widget.menu"
        | "widget.setText" => ALL,
        "text.edit.caret"
        | "text.edit.end"
        | "text.edit.selectLine"
        | "text.edit.selectWord"
        | "text.edit.selection"
        | "text.endPreview"
        | "vector.deleteHeal"
        | "vector.edit.addVertex"
        | "vector.edit.bend"
        | "vector.edit.bounds"
        | "vector.edit.connect"
        | "vector.edit.cornerRadius"
        | "vector.edit.cut"
        | "vector.edit.delete"
        | "vector.edit.end"
        | "vector.edit.erase"
        | "vector.edit.lasso"
        | "vector.edit.mirroring"
        | "vector.edit.moveHandles"
        | "vector.edit.moveVertices"
        | "vector.edit.nudge"
        | "vector.edit.paint"
        | "vector.edit.pen"
        | "vector.edit.penEnd"
        | "vector.edit.position"
        | "vector.edit.select"
        | "vector.edit.selectAll"
        | "vector.edit.setHandle"
        | "vector.edit.smooth"
        | "vector.edit.state"
        | "vector.edit.strokeCap"
        | "vector.edit.strokeJoin"
        | "vector.edit.tool" => MODE,
        "arrange.reorder" | "board.nextSticky" | "component.detach" | "object.batchRename" | "style.apply" | "style.create" | "style.detach"
        | "variable.export" | "vector.join" | "vector.smoothJoin" => MODE_AND_SELECTION,
        "board.shape"
        | "board.stamp"
        | "board.sticky"
        | "board.table"
        | "board.template"
        | "comment.add"
        | "comment.list"
        | "comment.next"
        | "comment.open"
        | "comment.zoom"
        | "devMode.measurements"
        | "edit.findReplace"
        | "file.exportFramesPdf"
        | "file.placeImage"
        | "file.saveVersion"
        | "guide.add"
        | "guide.clear"
        | "guide.move"
        | "guide.remove"
        | "guide.select"
        | "image.place"
        | "instance.create"
        | "instance.place"
        | "page.add"
        | "page.delete"
        | "page.duplicate"
        | "page.list"
        | "page.move"
        | "page.rename"
        | "page.setBackground"
        | "paint.documentColors"
        | "proto.flow.remove"
        | "proto.flow.rename"
        | "proto.present"
        | "proto.preview"
        | "proto.settings"
        | "select.byName"
        | "selection.selectColor"
        | "style.bind"
        | "version.restore"
        | "view.nextPage"
        | "view.prevPage" => PART,
        "align.bottom"
        | "align.hcenter"
        | "align.left"
        | "align.right"
        | "align.top"
        | "align.vcenter"
        | "annotation.add"
        | "annotation.edit"
        | "annotation.remove"
        | "arrange.back"
        | "arrange.backward"
        | "arrange.forward"
        | "arrange.front"
        | "arrange.packH"
        | "arrange.packV"
        | "arrange.roundToPixels"
        | "arrange.spacing"
        | "arrange.tidy"
        | "board.setColor"
        | "board.setStyle"
        | "board.tableEdit"
        | "board.vote"
        | "bool.flatten"
        | "component.addProperty"
        | "component.addVariant"
        | "component.bindProperty"
        | "component.deleteProperty"
        | "component.exposeInstance"
        | "component.hideWhenPublishing"
        | "component.makeSlot"
        | "component.properties"
        | "component.pushChanges"
        | "component.removeSlot"
        | "component.resetInstance"
        | "component.resetOverride"
        | "component.restoreMain"
        | "dev.compare"
        | "dev.resolveChange"
        | "devMode.addMeasurement"
        | "devMode.removeMeasurement"
        | "devResource.add"
        | "devResource.edit"
        | "devResource.list"
        | "devResource.remove"
        | "distribute.horizontal"
        | "distribute.vertical"
        | "edit.copy"
        | "edit.copyProperties"
        | "edit.cut"
        | "edit.delete"
        | "edit.duplicate"
        | "edit.pasteReplace"
        | "edit.setDefaultProperties"
        | "effect.add"
        | "effect.copy"
        | "effect.move"
        | "effect.paste"
        | "effect.remove"
        | "effect.set"
        | "export.add"
        | "export.remove"
        | "export.update"
        | "fill.add"
        | "fill.copy"
        | "fill.move"
        | "fill.paste"
        | "fill.pattern"
        | "fill.remove"
        | "fill.set"
        | "frame.swapOrientation"
        | "grid.add"
        | "grid.move"
        | "grid.remove"
        | "image.adjust"
        | "image.savePreset"
        | "inspect.assets"
        | "inspect.properties"
        | "instance.exposed"
        | "instance.resetSlot"
        | "instance.setProperties"
        | "instance.slotInsert"
        | "instance.swap"
        | "layout.add"
        | "layout.remove"
        | "layout.setTrack"
        | "layout.suggest"
        | "library.bindVariable"
        | "measure.padding"
        | "node.reparent"
        | "object.convertToFrame"
        | "object.convertToGroup"
        | "object.convertToSection"
        | "object.frameSelection"
        | "object.group"
        | "object.mask"
        | "object.moveToPage"
        | "object.opacityKeys"
        | "object.rasterize"
        | "object.releaseMask"
        | "object.removeFill"
        | "object.removeFrame"
        | "object.removeStroke"
        | "object.rename"
        | "object.resizeToFit"
        | "object.setThumbnail"
        | "object.swapFillStroke"
        | "object.toggleLock"
        | "object.toggleVisible"
        | "object.ungroup"
        | "object.wrapInSection"
        | "proto.action.add"
        | "proto.action.move"
        | "proto.action.remove"
        | "proto.action.set"
        | "proto.conditional.addBlock"
        | "proto.conditional.removeBlock"
        | "proto.conditional.setCondition"
        | "proto.interaction.add"
        | "proto.interaction.clear"
        | "proto.interaction.remove"
        | "proto.interaction.set"
        | "section.toggleContents"
        | "select.add"
        | "select.area"
        | "select.children"
        | "select.parent"
        | "select.remove"
        | "shapeBuilder.regions"
        | "stroke.add"
        | "stroke.brush"
        | "stroke.copy"
        | "stroke.move"
        | "stroke.paste"
        | "stroke.remove"
        | "stroke.set"
        | "text.fit"
        | "text.onPath"
        | "text.style"
        | "tool.place"
        | "transform.move"
        | "transform.moveTo"
        | "transform.nudgeSize"
        | "transform.resize"
        | "transform.rotate"
        | "transform.scale"
        | "variable.bind"
        | "vector.fillRule"
        | "vector.flatten"
        | "vector.offset"
        | "vector.outlineStroke"
        | "vector.reverse"
        | "vector.reverseLoop"
        | "vector.simplify" => SELECTION,
        "annotation.list"
        | "board.reattach"
        | "component.create"
        | "component.createMultiple"
        | "component.goToMain"
        | "dev.focus"
        | "edit.paste"
        | "edit.pasteOver"
        | "file.exportLayers"
        | "instance.swapAt"
        | "measure.distances"
        | "object.hideOthers"
        | "proto.flow.add"
        | "select.all"
        | "select.inverse"
        | "select.layersAt"
        | "select.matching"
        | "select.next"
        | "select.previous"
        | "select.sameEffect"
        | "select.sameFill"
        | "select.sameFont"
        | "select.sameInstance"
        | "select.sameProperties"
        | "select.sameStroke"
        | "select.sameText"
        | "select.set"
        | "variable.setMode"
        | "view.nextFrame"
        | "view.prevFrame" => SELECTION_AND_PART,
        _ => &[],
    }
}

/// The parameter an agent's `on` fills for engine command `id`: `ids` where the command reads it
/// as layer ids, else `id`; `None` where it takes neither as a layer id, or kition-sdk's
/// `schema::check_targets` does not take its schema for one (an untyped list): `on` then reaches
/// the command through the selection it reads. Derived from the code (`tests/acts.rs`).
pub fn targets(id: &str) -> Option<&'static str> {
    match id {
        "align.bottom"
        | "align.hcenter"
        | "align.left"
        | "align.right"
        | "align.top"
        | "align.vcenter"
        | "annotation.list"
        | "arrange.back"
        | "arrange.backward"
        | "arrange.forward"
        | "arrange.front"
        | "arrange.packH"
        | "arrange.packV"
        | "arrange.reorder"
        | "arrange.roundToPixels"
        | "arrange.spacing"
        | "arrange.tidy"
        | "board.setColor"
        | "board.setStyle"
        | "board.vote"
        | "bool.exclude"
        | "bool.flatten"
        | "bool.intersect"
        | "bool.subtract"
        | "bool.union"
        | "component.combineVariants"
        | "component.create"
        | "component.createMultiple"
        | "component.detach"
        | "component.hideWhenPublishing"
        | "component.resetInstance"
        | "dev.resolveChange"
        | "distribute.horizontal"
        | "distribute.vertical"
        | "edit.copy"
        | "edit.cut"
        | "edit.delete"
        | "edit.duplicate"
        | "edit.pasteProperties"
        | "edit.pasteReplace"
        | "effect.add"
        | "effect.copy"
        | "effect.move"
        | "effect.paste"
        | "effect.remove"
        | "effect.set"
        | "export.add"
        | "export.remove"
        | "export.update"
        | "file.export"
        | "file.exportLayers"
        | "file.exportSvg"
        | "fill.add"
        | "fill.copy"
        | "fill.move"
        | "fill.paste"
        | "fill.pattern"
        | "fill.remove"
        | "fill.set"
        | "frame.swapOrientation"
        | "grid.add"
        | "grid.move"
        | "grid.remove"
        | "image.adjust"
        | "image.savePreset"
        | "inspect.assets"
        | "instance.setProperties"
        | "instance.swap"
        | "layout.add"
        | "layout.remove"
        | "layout.setTrack"
        | "layout.suggest"
        | "library.applyStyle"
        | "library.bindVariable"
        | "measure.distances"
        | "node.reparent"
        | "node.set"
        | "object.batchRename"
        | "object.convertToFrame"
        | "object.convertToGroup"
        | "object.convertToSection"
        | "object.flipH"
        | "object.flipV"
        | "object.frameSelection"
        | "object.group"
        | "object.mask"
        | "object.moveToPage"
        | "object.opacityKeys"
        | "object.rasterize"
        | "object.releaseMask"
        | "object.removeFill"
        | "object.removeFrame"
        | "object.removeStroke"
        | "object.rename"
        | "object.resizeToFit"
        | "object.rotate180"
        | "object.rotateLeft"
        | "object.rotateRight"
        | "object.swapFillStroke"
        | "object.toggleLock"
        | "object.toggleVisible"
        | "object.ungroup"
        | "object.wrapInSection"
        | "proto.action.add"
        | "proto.action.move"
        | "proto.action.remove"
        | "proto.action.set"
        | "proto.conditional.addBlock"
        | "proto.conditional.removeBlock"
        | "proto.conditional.setCondition"
        | "proto.interaction.add"
        | "proto.interaction.clear"
        | "proto.interaction.remove"
        | "proto.interaction.set"
        | "section.toggleContents"
        | "select.add"
        | "select.remove"
        | "selection.colors"
        | "shapeBuilder.merge"
        | "shapeBuilder.regions"
        | "shapeBuilder.remove"
        | "stroke.add"
        | "stroke.brush"
        | "stroke.copy"
        | "stroke.move"
        | "stroke.paste"
        | "stroke.remove"
        | "stroke.set"
        | "stroke.widthProfile"
        | "style.apply"
        | "style.detach"
        | "text.alignCenter"
        | "text.alignJustify"
        | "text.alignLeft"
        | "text.alignRight"
        | "text.bold"
        | "text.bulletList"
        | "text.case"
        | "text.caseLower"
        | "text.caseOriginal"
        | "text.caseSmallCaps"
        | "text.caseTitle"
        | "text.caseUpper"
        | "text.decoration"
        | "text.fit"
        | "text.italic"
        | "text.lineHeightDown"
        | "text.lineHeightUp"
        | "text.link"
        | "text.listStyle"
        | "text.maxLines"
        | "text.numberList"
        | "text.onPath"
        | "text.paragraphIndent"
        | "text.paragraphSpacing"
        | "text.pathOptions"
        | "text.refit"
        | "text.removeFromPath"
        | "text.setFeatures"
        | "text.setVariations"
        | "text.sizeDown"
        | "text.sizeUp"
        | "text.strikethrough"
        | "text.trackingDown"
        | "text.trackingUp"
        | "text.underline"
        | "text.weightDown"
        | "text.weightUp"
        | "transform.move"
        | "transform.moveTo"
        | "transform.resize"
        | "transform.rotate"
        | "transform.scale"
        | "variable.bind"
        | "variable.setMode"
        | "vector.edit.begin"
        | "vector.fillRule"
        | "vector.flatten"
        | "vector.join"
        | "vector.offset"
        | "vector.outlineStroke"
        | "vector.reverse"
        | "vector.reverseLoop"
        | "vector.simplify"
        | "vector.smoothJoin" => Some("ids"),
        "annotation.add"
        | "annotation.edit"
        | "annotation.remove"
        | "board.connector"
        | "board.label"
        | "board.nextSticky"
        | "board.reattach"
        | "board.tableEdit"
        | "codeConnect.remove"
        | "codeConnect.set"
        | "comment.delete"
        | "comment.edit"
        | "comment.link"
        | "comment.markRead"
        | "comment.move"
        | "comment.next"
        | "comment.open"
        | "comment.react"
        | "comment.reply"
        | "comment.resolve"
        | "component.addProperty"
        | "component.addVariant"
        | "component.bindProperty"
        | "component.deleteProperty"
        | "component.editProperty"
        | "component.exposeInstance"
        | "component.goToMain"
        | "component.makeSlot"
        | "component.playground"
        | "component.properties"
        | "component.pushChanges"
        | "component.removeSlot"
        | "component.resetOverride"
        | "component.restoreMain"
        | "data.fill"
        | "dev.compare"
        | "dev.focus"
        | "devMode.measurements"
        | "devResource.add"
        | "devResource.edit"
        | "devResource.list"
        | "devResource.remove"
        | "edit.copyProperties"
        | "edit.setDefaultProperties"
        | "inspect.code"
        | "inspect.componentCode"
        | "inspect.properties"
        | "instance.exposed"
        | "instance.resetSlot"
        | "instance.slotInsert"
        | "measure.padding"
        | "node.get"
        | "node.reorder"
        | "object.setThumbnail"
        | "page.delete"
        | "page.duplicate"
        | "page.move"
        | "page.rename"
        | "page.select"
        | "page.setBackground"
        | "proto.evaluate"
        | "text.edit.begin"
        | "text.set"
        | "text.style"
        | "vector.edit.resize"
        | "vector.edit.select"
        | "vector.edit.transform"
        | "widget.click"
        | "widget.menu"
        | "widget.setText" => Some("id"),
        _ => None,
    }
}

/// The commands the binding adds: what they read, the parameter `on` fills, and why (by hand: the
/// scan reads the engine's code).
pub const BINDING_COMMANDS: &[(&str, &[EditorState], Option<&str>, &str)] = &[
    (
        "file.render",
        &[],
        None,
        "draws the page, layer, frame or area its `target` names, the first page without one: never the current page or the selection",
    ),
    ("page.go", &[], None, "goes to the page its `id` or `index` names: navigation"),
    ("page.inspect", &[], None, "lists the page or layer its `id` or `index` names, wherever the editor is"),
];

/// [`reads`] for any command the kit lists: the engine's, or the binding's own.
pub fn reads_any(id: &str) -> &'static [EditorState] {
    BINDING_COMMANDS.iter().find(|(command, ..)| *command == id).map_or_else(|| reads(id), |(_, state, ..)| *state)
}

/// [`targets`] for any command the kit lists.
pub fn targets_any(id: &str) -> Option<&'static str> {
    BINDING_COMMANDS.iter().find(|(command, ..)| *command == id).map_or_else(|| targets(id), |(_, _, target, _)| *target)
}
