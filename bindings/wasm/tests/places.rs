//! C8-land-11-design: per-call premises follow the engine's stored registers.
#![allow(clippy::unwrap_used, clippy::panic)]

use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;
use serde_json::json;

#[test]
fn a_property_write_holds_only_its_property_and_a_move_holds_its_parent() {
    let mut doc = DesignDocument::create("Premises.kdesign", 320, 240).unwrap();
    let frame = doc.inspect().unwrap()["tree"][0]["id"].clone();
    let a = doc.execute("node.create", json!({"type":"rectangle","parent":frame,"width":20,"height":20})).unwrap()["id"].clone();
    let under = doc.under("node.set", &json!({"id":a,"props":{"name":"New"}})).unwrap().unwrap();
    let need = under.premises.iter().find(|p| p.id == a).unwrap();
    assert_eq!(need.fields.as_ref().unwrap(), &["name"]);
    assert!(need.positions.as_ref().unwrap().is_empty());
    let under = doc.under("transform.move", &json!({"ids":[a],"dx":5})).unwrap().unwrap();
    let parent = under.premises.iter().find(|p| p.id == frame).unwrap();
    assert!(parent.positions.as_ref().unwrap().contains(&"frame".into()));
    // A local translation reads the transform, independently of the layer's dimensions.
    let moved = under.premises.iter().find(|p| p.id == a).unwrap();
    assert_eq!(moved.fields.as_ref().unwrap(), &["~transform"]);
    assert!(!moved.positions.as_ref().unwrap().contains(&"frame".into()));
}

#[test]
fn indexed_paint_writes_hold_the_item_and_bindings() {
    let mut doc = DesignDocument::create("Premises.kdesign", 320, 240).unwrap();
    let id = doc.inspect().unwrap()["tree"][0]["id"].clone();
    doc.execute("fill.add", json!({"ids":[id],"paint":"#f00"})).unwrap();
    let under = doc.under("fill.set", &json!({"ids":[id],"index":0,"color":"#0f0"})).unwrap().unwrap();
    let need = under.premises.iter().find(|p| p.id == id).unwrap();
    // List-style synchronization reads the list; mode bindings are independent of this edit.
    assert_eq!(need.fields.as_ref().unwrap(), &["bindings.bound", "bindings.styles", "fills"]);
    assert!(need.positions.as_ref().unwrap().is_empty());
}

#[test]
fn point_or_id_connector_endpoints_hold_only_object_alternatives() {
    let doc = DesignDocument::create("Premises.kdesign", 320, 240).unwrap();
    let id = doc.inspect().unwrap()["tree"][0]["id"].clone();
    let under = doc.under("board.connector", &json!({"from":id,"to":{"x":10,"y":20}})).unwrap().unwrap();
    assert!(under.objects.contains(&id));
    assert!(!under.objects.contains(&json!(10)) && !under.objects.contains(&json!(20)));
}

#[test]
fn a_point_swap_names_the_same_instance_as_the_engine() {
    let mut doc = DesignDocument::create("Point.kdesign", 320, 240).unwrap();
    let frame = doc.inspect().unwrap()["tree"][0]["id"].clone();
    doc.execute("component.create", json!({"ids":[frame]})).unwrap();
    let page = doc.inspect().unwrap()["page"].clone();
    let made = doc.execute("instance.create", json!({"component":frame,"parent":page,"x":400,"y":0})).unwrap()["id"].clone();
    let params = json!({"component":frame,"x":410,"y":10});
    let under = doc.under("instance.swapAt", &params).unwrap().unwrap();
    assert!(under.objects.contains(&made), "{under:?}");
    assert!(under.covers.is_some());
    assert_eq!(doc.execute("instance.swapAt", params).unwrap()["id"], made);
}

#[test]
fn carries_distinguish_a_transform_from_a_rename_or_sibling_reorder() {
    let mut doc = DesignDocument::create("Carries.kdesign", 320, 240).unwrap();
    let frame = doc.inspect().unwrap()["tree"][0]["id"].clone();
    doc.execute("node.create", json!({"type":"rectangle","parent":frame,"width":10,"height":10})).unwrap();
    for change in [json!({"id":frame,"fields":["name"]}), json!({"id":frame,"from":{"parent":"page","index":0},"to":{"parent":"page","index":1}})] {
        assert!(doc.carried(std::slice::from_ref(&frame), &[change]).unwrap().is_empty());
    }
    let carries = doc.carried(std::slice::from_ref(&frame), &[json!({"id":frame,"fields":["frame.transform.4"]})]).unwrap();
    assert_eq!(carries[0].carries.count, 1);
}

#[test]
fn local_derived_bounds_do_not_change_when_an_ancestor_moves() {
    let mut doc = DesignDocument::create("Bounds.kdesign", 320, 240).unwrap();
    let frame = doc.inspect().unwrap()["tree"][0]["id"].clone();
    let child = doc.execute("node.create", json!({"type":"rectangle","parent":frame,"width":10,"height":10})).unwrap()["id"].clone();
    let before = doc.derived(std::slice::from_ref(&child), &["bounds".into()]).unwrap();
    doc.execute("transform.move", json!({"ids":[frame],"dx":20,"dy":30})).unwrap();
    assert_eq!(doc.derived(&[child], &["bounds".into()]).unwrap(), before);
}

#[test]
fn prototype_expression_values_hold_the_variables_the_parser_resolves() {
    let mut doc = DesignDocument::create("Expressions.kdesign", 320, 240).unwrap();
    let collection = doc.execute("variable.createCollection", json!({"name":"Counts"})).unwrap()["id"].clone();
    let variable = doc.execute("variable.create", json!({"collection":collection,"name":"count","type":"number","value":1})).unwrap()["id"].clone();
    let frame = doc.inspect().unwrap()["tree"][0]["id"].clone();
    let under = doc
        .under("proto.interaction.add", &json!({"id":frame,"action":{"type":"setVariable","variable":"count","expression":"count + 1"}}))
        .unwrap()
        .unwrap();
    assert!(under.premises.iter().any(|p| p.id == variable), "{under:?}");
    let under = doc.under("proto.conditional.setCondition", &json!({"id":frame,"expression":"count > 0"})).unwrap().unwrap();
    assert!(under.premises.iter().any(|p| p.id == variable), "{under:?}");
}
