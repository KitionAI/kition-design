//! The environment sees human labels and avoids identity/type bookkeeping.
#![allow(clippy::unwrap_used)]

use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;
use kition_kit::session::model::Exchange;
use serde_json::json;

#[test]
fn environment_description_names_objects_and_bounds_sampling() {
    let mut doc = DesignDocument::create("Words.kdesign", 320, 240).unwrap();
    let described = Exchange::new().describe(&mut doc).unwrap();
    let types = &described["types"];
    for ty in ["Node", "Style", "Collection", "Mode", "Variable", "Library"] {
        assert_eq!(types[ty]["label"], "name", "{ty}");
    }
    assert_eq!(types["Comment"]["label"], "body");
    let fields = &types["Node"]["fields"];
    assert_eq!(fields["visible"]["label"], "visible");
    assert_eq!(fields["locked"]["label"], "locked");
    assert_eq!(fields["opacity"]["minimum"].as_f64(), Some(0.0));
    assert_eq!(fields["opacity"]["maximum"].as_f64(), Some(1.0));
    for field in ["kind", "mirrors"] {
        assert_eq!(fields[field]["sample"], false, "{field}");
    }
    assert_eq!(types["Variable"]["fields"]["collection"]["sample"], false);
    assert_eq!(types["Comment"]["fields"]["key"]["readOnly"], true);
    assert_eq!(types["Comment"]["fields"]["created"]["sample"], false);
    // Unix timestamps and pixels are not timecodes; they need no display conversion.
    assert!(types["Comment"]["fields"]["edited"].get("display").is_none());
}

#[test]
fn the_pinned_environment_create_sampler_needs_the_required_json_kind() {
    let mut doc = DesignDocument::create("Create.kdesign", 320, 240).unwrap();
    let mut exchange = Exchange::new();
    // SDK generate::build(Create) copies only scalar/text fields, omitting required JSON.
    let mut create = json!({"ops":[{"op":"create","type":"Node","in":"root.nodes","fields":{"name":"Sampled"}}]});
    let error = exchange.batch(&mut doc, &create, false).unwrap_err().to_string();
    assert!(error.contains("kind"), "{error}");
    println!("SDK CREATE REPRO {error}");
    create["ops"][0]["fields"]["kind"] = json!({"type":"frame"});
    assert!(exchange.batch(&mut doc, &create, false).is_ok());
}

/// SDK 0000380 refused numeric bounds on a JSON field; since 2ce294a1 a schema bounds a JSON
/// field's members, and a value outside them is refused.
#[test]
fn the_schema_bounds_members_of_json_fields() {
    let mut schema: serde_json::Value = serde_json::from_str(kition_design_kdm::SCHEMA_JSON).unwrap();
    schema["types"]["Node"]["fields"]["frame"]["members"] = json!({"width": {"minimum": 0}});
    let parsed = kition_kit::kdm::Schema::parse(&schema.to_string()).unwrap();
    assert!(parsed.check_value("Node", "frame", &json!({"width": 40})).is_ok());
    let error = parsed.check_value("Node", "frame", &json!({"width": -1})).unwrap_err().to_string();
    assert!(error.contains("minimum"), "{error}");
}
