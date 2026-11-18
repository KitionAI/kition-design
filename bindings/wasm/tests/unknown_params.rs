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
    let before = doc.save(None).unwrap();
    let error = doc.execute("frame.create", json!({ "x": 0, "y": 0, "widht": 10, "height": 10 })).unwrap_err();
    assert!(error.contains("`widht`") && error.contains("did you mean `width`"), "{error}");
    assert_eq!(doc.save(None).unwrap(), before);
}
