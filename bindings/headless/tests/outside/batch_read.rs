//! C8-design-read: batch results carry dependencies from their source queries into later writes.
use super::*;

fn prepare(f: &Fixture, edit: impl FnOnce(&mut DesignDocument) -> Value) -> Value {
    let mut writer = DesignDocument::open(FILE, &f.bytes()).unwrap();
    let made = edit(&mut writer);
    std::fs::write(f.workspace.path().join(FILE), writer.save_file(None).unwrap()).unwrap();
    made
}

async fn case(
    setup: impl Fn(&Fixture) -> Value,
    batch: impl Fn(&Fixture, &Value) -> Value,
    related: impl Fn(&Fixture, &Value) -> Vec<(&'static str, Value)>,
    unrelated: impl Fn(&Fixture, &Value) -> Vec<(&'static str, Value)>,
) {
    let mut failures = Vec::new();
    for necessary in [true, false] {
        let f = Fixture::new(Guard::Premise);
        let made = setup(&f);
        let basis = f.basis().await;
        let change = if necessary { related(&f, &made) } else { unrelated(&f, &made) };
        f.colleague(&change);
        let answer = f.write(&basis, "command.batch", batch(&f, &made)).await;
        let conflict = answer["error"]["code"] == "conflict";
        assert!(conflict || answer.get("error").is_none(), "{answer}");
        if conflict != necessary {
            failures.push(format!("necessary={necessary}, change={change:?}, answer={answer}"));
        }
        f.shutdown().await;
    }
    assert!(failures.is_empty(), "{failures:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn node_get_grid_cell_reads_parent_grid_without_holding_parent_appearance() {
    case(
        |f| {
            prepare(f, |d| {
                let page = d.inspect().unwrap()["page"].clone();
                let grid = d.execute("frame.add", json!({"parent":page,"width":0,"height":0,"x":900,"y":900})).unwrap()["id"].clone();
                d.execute("node.set", json!({"id":grid,"props":{"layout":"grid","gridColumns":2,"gridRows":2}})).unwrap();
                let mut child = Value::Null;
                for _ in 0..3 {
                    child = d.execute("node.add", json!({"type":"rectangle","parent":grid,"width":0,"height":0})).unwrap()["id"].clone();
                }
                json!({"grid":grid,"child":child})
            })
        },
        |f, made| {
            json!({"commands":[
                {"command":"node.get","params":{"id":made["child"]}},
                {"command":"node.set","params":{"id":f.b,"props":{"width":{"$step":0,"field":"gridCell/column"}}}}
            ]})
        },
        |_, made| vec![("node.set", json!({"id":made["grid"],"props":{"gridColumns":3}}))],
        |_, made| vec![("node.set", json!({"id":made["grid"],"props":{"opacity":0.5}}))],
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn node_get_variable_modes_read_mode_names_without_holding_appearance() {
    case(
        |f| {
            prepare(f, |d| {
                d.execute("variable.addCollection", json!({"name":"Theme","modes":["Light","Dark"]})).unwrap();
                d.execute("variable.setMode", json!({"ids":[f.frame],"collection":"Theme","mode":"Dark"})).unwrap();
                Value::Null
            })
        },
        |f, _| {
            json!({"commands":[
                {"command":"node.get","params":{"id":f.frame}},
                {"command":"object.rename","params":{"ids":[f.b],"name":{"$step":0,"field":"variableModes/Theme"}}}
            ]})
        },
        |_, _| vec![("variable.renameMode", json!({"collection":"Theme","mode":"Dark","name":"Night"}))],
        |f, _| vec![("node.set", json!({"id":f.frame,"props":{"opacity":0.5}}))],
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn node_get_dev_resources_read_the_main_without_holding_appearance() {
    case(
        |f| {
            prepare(f, |d| {
                d.execute("devResource.add", json!({"id":f.component,"url":"https://example.com/main","name":"Initial"})).unwrap();
                Value::Null
            })
        },
        |f, _| {
            json!({"commands":[
                {"command":"node.get","params":{"id":f.instance}},
                {"command":"object.rename","params":{"ids":[f.b],"name":{"$step":0,"field":"devResources/0/name"}}}
            ]})
        },
        |f, _| vec![("devResource.edit", json!({"id":f.component,"index":0,"name":"Colleague"}))],
        |f, _| vec![("node.set", json!({"id":f.component,"props":{"opacity":0.5}}))],
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn style_list_result_reads_style_name_without_holding_layers() {
    case(
        |_| Value::Null,
        |f, _| {
            json!({"commands":[
                {"command":"style.list","params":{"type":"paint"}},
                {"command":"object.rename","params":{"ids":[f.b],"name":{"$step":0,"field":"0/name"}}}
            ]})
        },
        |f, _| vec![("style.set", json!({"style":f.style,"name":"Colleague"}))],
        |f, _| vec![("node.set", json!({"id":f.a,"props":{"opacity":0.5}}))],
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn nested_batch_results_keep_inner_reference_scopes_and_provenance() {
    case(
        |_| Value::Null,
        |f, _| {
            json!({"commands":[
                {"command":"node.get","params":{"id":f.b},"as":"source"},
                {"command":"command.batch","as":"inner","params":{"commands":[
                    {"command":"node.get","params":{"id":f.a},"as":"source"},
                    {"command":"node.set","params":{"id":f.b,"props":{"height":{"$ref":"source","field":"height"}}}}
                ]}},
                {"command":"node.set","params":{"id":f.b,"props":{"width":{"$ref":"inner","field":"0/width"}}}}
            ]})
        },
        |f, _| vec![("node.set", json!({"id":f.a,"props":{"width":80}}))],
        |f, _| vec![("node.set", json!({"id":f.a,"props":{"opacity":0.5}}))],
    )
    .await;
}
