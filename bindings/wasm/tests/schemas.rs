//! A command's schema offers every parameter its engine reads (KHP A5, A36): a key that drops out
//! of the schema is one an agent never learns the command takes.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;
use serde_json::json;

#[test]
fn layout_add_offers_its_direction() {
    let doc = DesignDocument::create("Design.kdesign", 400, 300).unwrap();
    let add = doc.commands().into_iter().find(|command| command.id == "layout.add").unwrap();
    let schema = add.schema.unwrap();
    assert_eq!(schema["direction"]["enum"], json!(["horizontal", "vertical", "grid"]), "{schema}");
}
