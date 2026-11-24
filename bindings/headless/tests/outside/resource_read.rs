//! C8-design-read: a resource dependency holds the references it uses, with outside-writer controls.
use super::*;

fn prepare(f: &Fixture, edit: impl FnOnce(&mut DesignDocument) -> Value) -> Value {
    let mut writer = DesignDocument::open(FILE, &f.bytes()).unwrap();
    let out = edit(&mut writer);
    std::fs::write(f.workspace.path().join(FILE), writer.seal_and_save(None).unwrap()).unwrap();
    out
}

async fn check(f: &Fixture, basis: &Value, command: &str, params: Value, necessary: bool) -> Option<String> {
    let before = f.bytes();
    let answer = f.write(basis, command, params).await;
    if answer["error"]["code"] == "conflict" {
        assert_eq!(f.bytes(), before, "a refusal must not save");
        (!necessary).then(|| format!("{command}: false refusal: {answer}"))
    } else if necessary || answer.get("error").is_some() {
        Some(format!("{command}: necessary={necessary}: {answer}"))
    } else {
        None
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn pruning_reads_alias_edges_and_not_unrelated_literal_values() {
    let mut failures = Vec::new();
    for necessary in [true, false] {
        let f = Fixture::new(Guard::Premise);
        let path = "Resource Library.kdesign";
        let mut library = DesignDocument::create(path, 100, 100).unwrap();
        library.execute("variable.createCollection", json!({"name":"Library Tokens"})).unwrap();
        library.execute("variable.create", json!({"collection":"Library Tokens","name":"size","type":"number","value":4})).unwrap();
        library.execute("library.publish", json!({})).unwrap();
        let bytes = library.seal_and_save(None).unwrap();
        std::fs::write(f.workspace.path().join(path), &bytes).unwrap();
        let made = prepare(&f, |d| {
            d.lend_library(path, Some(&bytes)).unwrap();
            let link = d.execute("library.add", json!({"path":path})).unwrap()["id"].clone();
            let copied = d.execute("library.importVariable", json!({"library":link,"variable":"size"})).unwrap()["variable"].clone();
            let collection = d.execute("variable.createCollection", json!({"name":"Local"})).unwrap()["id"].clone();
            let plain =
                d.execute("variable.create", json!({"collection":collection,"name":"plain","type":"number","value":1})).unwrap()["id"].clone();
            json!({"link":link,"copied":copied,"plain":plain})
        });
        let basis = f.basis().await;
        let value = if necessary { json!({"alias":made["copied"]}) } else { json!(2) };
        f.colleague(&[("variable.set", json!({"variable":made["plain"],"value":value}))]);
        if let Some(failure) = check(&f, &basis, "library.acceptUpdates", json!({"ids":[made["link"]]}), necessary).await {
            failures.push(failure);
        }
        f.shutdown().await;
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_literal_mode_action_holds_the_named_mode() {
    let mut failures = Vec::new();
    for necessary in [true, false] {
        let f = Fixture::new(Guard::Premise);
        prepare(&f, |d| d.execute("variable.createCollection", json!({"name":"Colors","modes":["Light","Dark"]})).unwrap());
        let basis = f.basis().await;
        let mode = if necessary { "Dark" } else { "Light" };
        f.colleague(&[("variable.renameMode", json!({"collection":"Colors","mode":mode,"name":"Dawn"}))]);
        let params = json!({"id":f.a,"action":{"type":"setVariableMode","collection":"Colors","mode":"Dark"}});
        if let Some(failure) = check(&f, &basis, "proto.interaction.add", params, necessary).await {
            failures.push(failure);
        }
        f.shutdown().await;
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[tokio::test(flavor = "multi_thread")]
async fn property_setters_do_not_hold_unrelated_variable_modes() {
    let mut failures = Vec::new();
    for property in ["opacity", "fill"] {
        for necessary in [true, false] {
            let f = Fixture::new(Guard::Premise);
            prepare(&f, |d| d.execute("variable.createCollection", json!({"name":"Colors","modes":["Light","Dark"]})).unwrap());
            let basis = f.basis().await;
            if necessary {
                let value = if property == "opacity" { json!(0.5) } else { json!("#ff0000") };
                f.colleague(&[("node.set", json!({"id":f.a,"props":{property:value}}))]);
            } else {
                f.colleague(&[("variable.setMode", json!({"ids":[f.a],"collection":"Colors","mode":"Dark"}))]);
            }
            let value = if property == "opacity" { json!(0.8) } else { json!("#00ff00") };
            if let Some(failure) = check(&f, &basis, "node.set", json!({"id":f.a,"props":{property:value}}), necessary).await {
                failures.push(format!("{property}: {failure}"));
            }
            f.shutdown().await;
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[tokio::test(flavor = "multi_thread")]
async fn importing_a_new_mode_holds_unimported_default_values() {
    let mut failures = Vec::new();
    for change in ["default value", "default mode", "new local variable", "nondefault value", "other collection"] {
        let f = Fixture::new(Guard::Premise);
        let made = prepare(&f, |d| {
            d.execute("variable.createCollection", json!({"name":"Brand","modes":["Light","Dark"]})).unwrap();
            d.execute("variable.create", json!({"collection":"Brand","name":"Primary","type":"number","value":8})).unwrap();
            let imported = d.execute("variable.export", json!({"collections":["Brand"]})).unwrap()["text"].clone();
            d.execute("variable.removeMode", json!({"collection":"Brand","mode":"Dark"})).unwrap();
            d.execute("variable.addMode", json!({"collection":"Brand","name":"Extra"})).unwrap();
            let local = d
                .execute("variable.create", json!({"collection":"Brand","name":"Local Only","type":"number","values":{"Light":1,"Extra":2}}))
                .unwrap()["id"]
                .clone();
            d.execute("variable.createCollection", json!({"name":"Other"})).unwrap();
            let other =
                d.execute("variable.create", json!({"collection":"Other","name":"Unrelated","type":"number","value":1})).unwrap()["id"].clone();
            json!({"imported":imported,"local":local,"other":other})
        });
        let basis = f.basis().await;
        let edit = match change {
            "default value" => ("variable.set", json!({"variable":made["local"],"mode":"Light","value":9})),
            "default mode" => ("variable.moveMode", json!({"collection":"Brand","mode":"Extra","to":0})),
            "new local variable" => ("variable.create", json!({"collection":"Brand","name":"New Local","type":"number","value":9})),
            "nondefault value" => ("variable.set", json!({"variable":made["local"],"mode":"Extra","value":9})),
            _ => ("variable.set", json!({"variable":made["other"],"value":9})),
        };
        f.colleague(&[edit]);
        let necessary = !matches!(change, "nondefault value" | "other collection");
        if let Some(failure) = check(&f, &basis, "variable.import", json!({"variables":made["imported"]}), necessary).await {
            failures.push(format!("{change}: {failure}"));
        }
        f.shutdown().await;
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
