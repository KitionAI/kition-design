//! A command refuses a parameter its schema does not name, saying which, instead of running
//! without it (engine rule E7a, kit requirement AX15).
#![allow(clippy::unwrap_used, clippy::panic)]

use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;
use serde_json::json;

fn design() -> DesignDocument {
    DesignDocument::create("Design.kdesign", 64, 36).unwrap()
}

#[test]
fn every_command_refuses_a_parameter_it_does_not_take() {
    if let Err(problems) = kition_kit::testing::check_unknown_params(design, &[]) {
        panic!("{} commands:\n{}", problems.len(), problems.join("\n"));
    }
}

#[test]
fn a_misspelled_parameter_names_the_one_it_resembles_and_changes_nothing() {
    let mut doc = design();
    let before = doc.seal_and_save(None).unwrap();
    let error = doc.execute("frame.create", json!({ "x": 0, "y": 0, "widht": 10, "height": 10 })).unwrap_err();
    assert!(error.contains("`widht`") && error.contains("did you mean `width`"), "{error}");
    assert_eq!(doc.seal_and_save(None).unwrap(), before);
}

/// The editor's own calls name only parameters the commands take: it passes the selection as
/// `ids` to commands that read `id` or `ids`, and the export options as their own keys. Before
/// their schemas named them, every one of these was refused (Copy fill, an OpenType feature,
/// Include id attribute and Add interaction did nothing in the editor).
#[test]
fn the_editors_parameters_are_named_in_the_schemas() {
    let calls = [
        ("fill.copy", json!({ "ids": [1], "index": 0 })),
        ("stroke.copy", json!({ "ids": [1], "index": 0 })),
        ("effect.copy", json!({ "ids": [1], "index": 0 })),
        ("text.setFeatures", json!({ "ids": [1], "features": { "tnum": true } })),
        ("text.setVariations", json!({ "ids": [1], "variations": { "wght": 500 } })),
        ("text.onPath", json!({ "ids": [1, 2] })),
        (
            "export.update",
            json!({ "ids": [1], "index": 0, "contentsOnly": true, "includeBoundingBox": true, "svgOutlineText": false, "svgIdAttribute": true, "svgSimplifyStroke": false }),
        ),
        ("proto.interaction.add", json!({ "ids": [1], "trigger": { "type": "onClick" }, "action": { "type": "back" } })),
        ("proto.interaction.set", json!({ "ids": [1], "index": 0, "durationMs": 300 })),
        ("proto.interaction.remove", json!({ "ids": [1], "index": 0 })),
        ("proto.action.add", json!({ "ids": [1], "index": 0, "path": [1], "action": { "type": "back" } })),
        ("proto.action.set", json!({ "ids": [1], "index": 0, "path": [0], "action": { "type": "back" } })),
        ("proto.action.remove", json!({ "ids": [1], "index": 0, "path": [0] })),
        ("proto.action.move", json!({ "ids": [1], "index": 0, "path": [0], "to": 1 })),
        ("proto.conditional.addBlock", json!({ "ids": [1], "index": 0, "path": [0] })),
        ("proto.conditional.removeBlock", json!({ "ids": [1], "index": 0, "path": [0], "block": 0 })),
        ("proto.conditional.setCondition", json!({ "ids": [1], "index": 0, "path": [0], "block": 0, "expression": "true" })),
    ];
    let mut refused = Vec::new();
    for (command, params) in calls {
        if let Err(error) = design().execute(command, params)
            && error.contains("has no parameter")
        {
            refused.push(format!("{command}: {error}"));
        }
    }
    assert!(refused.is_empty(), "{}", refused.join("\n"));
}
