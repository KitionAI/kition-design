//! Compact change sets for properties, offsets and parents carrying many children.
#![allow(clippy::unwrap_used, clippy::panic)]

use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;
use kition_kits::{KitDefinition, KitManager, tools};
use serde_json::json;
use std::path::PathBuf;

async fn measure(children: usize, form: &str) {
    let workspace = tempfile::tempdir().unwrap();
    let mut doc = DesignDocument::create("Size.kdesign", 800, 600).unwrap();
    let frame = doc.inspect().unwrap()["tree"][0]["id"].clone();
    let mut ids = Vec::new();
    for i in 0..children {
        ids.push(
            doc.execute("node.create", json!({"type":"rectangle","parent":frame,"x":(i%20)*20,"y":(i/20)*20,"width":10,"height":10})).unwrap()["id"]
                .clone(),
        );
    }
    std::fs::write(workspace.path().join("Size.kdesign"), doc.save(None).unwrap()).unwrap();
    let kit = KitDefinition::from_manifest_text(include_str!("../../../kit.json")).unwrap();
    let kits = KitManager::new(Some(workspace.path().to_path_buf()), vec![(kit, Some(PathBuf::from(env!("CARGO_BIN_EXE_kition-design"))))]);
    let (command, params) = match form {
        "property" => ("node.set", json!({"id":ids[0],"props":{"name":"Changed"}})),
        "one" => ("transform.move", json!({"ids":[ids[0]],"dx":5,"dy":3})),
        "parent" => ("transform.move", json!({"ids":[frame],"dx":5,"dy":3})),
        _ => ("transform.move", json!({"ids":ids,"dx":5,"dy":3})),
    };
    let answer = tools::call(&kits, "design_run", &json!({"path":"Size.kdesign","steps":[{"command":command,"params":params}]}))
        .await
        .unwrap()
        .unwrap()
        .output;
    assert!(answer.get("error").is_none(), "{answer}");
    if form == "parent" {
        let carried = answer["changes"]["changed"].as_array().unwrap().iter().find(|c| c["id"] == frame).unwrap();
        assert_eq!(carried["carries"]["count"], children, "{answer}");
    }
    println!("RECEIPT {form} children={children} changes={} response={}", answer["changes"].to_string().len(), answer.to_string().len());
    if form == "all" {
        println!("RECEIPT-ALL {}", answer["changes"]);
    }
    kits.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn sizes() {
    for (children, form) in [(1, "property"), (1, "one"), (5, "parent"), (200, "parent"), (200, "all")] {
        measure(children, form).await;
    }
}
