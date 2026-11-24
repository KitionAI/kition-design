//! C8-design-3: expectations are declared before exercising independent file rewrites.
use super::*;

#[path = "addresses.rs"]
mod addresses;
#[path = "normalisation.rs"]
mod normalisation;

fn rewrite(f: &Fixture, edit: impl FnOnce(&mut DesignDocument) -> Value) -> Value {
    let mut writer = DesignDocument::open(FILE, &f.bytes()).unwrap();
    let out = edit(&mut writer);
    std::fs::write(f.workspace.path().join(FILE), writer.seal_and_save(None).unwrap()).unwrap();
    out
}

async fn outcome(f: &Fixture, basis: &Value, command: &str, params: Value, necessary: bool) {
    let before = f.bytes();
    let answer = f.write(basis, command, params).await;
    assert_eq!(answer["error"]["code"] == "conflict", necessary, "{command}, necessary={necessary}: {answer}");
    if necessary {
        assert_eq!(f.bytes(), before, "refusal must not save");
    } else {
        applied(&answer);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn tool_place_holds_the_actual_pick() {
    // Each row pairs an interfering edit with a spatially unrelated edit. The place is the
    // pointer-down corner (500, 400), not the supplied centre (550, 450).
    for change in ["move", "delete", "cover", "layout", "unrelated-move", "unrelated-field"] {
        let f = Fixture::new(Guard::Premise);
        let basis = f.basis().await;
        match change {
            "move" => f.colleague(&[("transform.move", json!({"ids":[f.destination],"dx":300}))]),
            "delete" => f.colleague(&[("edit.delete", json!({"ids":[f.destination]}))]),
            "cover" => {
                rewrite(&f, |d| {
                    let page = d.inspect().unwrap()["page"].clone();
                    d.execute("frame.create", json!({"parent":page,"x":490,"y":390,"width":120,"height":120})).unwrap()
                });
            }
            "layout" => f.colleague(&[("layout.add", json!({"ids":[f.destination]}))]),
            "unrelated-move" => f.colleague(&[("transform.move", json!({"ids":[f.b],"dx":10}))]),
            _ => f.colleague(&[("node.set", json!({"id":f.b,"props":{"opacity":0.5}}))]),
        }
        outcome(&f, &basis, "tool.place", json!({"tool":"rectangle","x":550,"y":450}), !change.starts_with("unrelated")).await;
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn tool_place_holds_enclosed_geometry_but_not_unrelated_fields() {
    for necessary in [true, false] {
        let f = Fixture::new(Guard::Premise);
        let child = {
            let doc = DesignDocument::open(FILE, &f.bytes()).unwrap();
            doc.session().doc().unwrap().get(kition_design_doc::NodeId(f.destination)).unwrap().children[0].0
        };
        let basis = f.basis().await;
        let props = if necessary { json!({"x":200}) } else { json!({"opacity":0.5}) };
        f.colleague(&[("node.set", json!({"id":child,"props":props}))]);
        outcome(&f, &basis, "tool.place", json!({"tool":"frame","x":550,"y":450}), necessary).await;
        if !necessary {
            assert_eq!(f.node(child)["opacity"], 0.5);
        }
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn vector_session_does_not_delete_a_colleagues_isolated_point() {
    for necessary in [true, false] {
        let f = Fixture::new(Guard::Premise);
        let vector = rewrite(&f, |d| d.execute("node.create", json!({"type":"vector","parent":f.destination})).unwrap())["id"].clone();
        let basis = f.basis().await;
        if necessary {
            f.colleague(&[("vector.edit.begin", json!({"ids":[vector]})), ("vector.edit.pen", json!({"x":510,"y":410}))]);
        } else {
            f.colleague(&[("node.set", json!({"id":f.b,"props":{"opacity":0.5}}))]);
        }
        let bytes = f.bytes();
        let answer = f
            .call(
                "run",
                json!({"basis":basis,"steps":[
                    {"command":"vector.edit.begin","params":{"ids":[vector]}},
                    {"command":"vector.edit.end","params":{}}
                ]}),
            )
            .await;
        assert_eq!(answer["error"]["code"] == "conflict", necessary, "necessary={necessary}: {answer}");
        if necessary {
            assert_eq!(f.bytes(), bytes, "refused session must not delete the colleague's point");
        } else {
            applied(&answer);
            let doc = DesignDocument::open(FILE, &f.bytes()).unwrap();
            assert!(!doc.session().doc().unwrap().contains(kition_design_doc::NodeId(vector.as_u64().unwrap())));
        }
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn vector_session_holds_boolean_conversion_operands() {
    for necessary in [true, false] {
        let f = Fixture::new(Guard::Premise);
        let boolean = rewrite(&f, |d| d.execute("bool.union", json!({"ids":[f.a,f.b]})).unwrap())["id"].clone();
        let basis = f.basis().await;
        if necessary {
            f.colleague(&[("node.set", json!({"id":f.a,"props":{"cornerRadius":8}}))]);
        } else {
            f.colleague(&[("node.set", json!({"id":f.destination,"props":{"opacity":0.5}}))]);
        }
        outcome(&f, &basis, "vector.edit.begin", json!({"ids":[boolean]}), necessary).await;
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn tool_switch_cleanup_pairs_interfering_and_unrelated_writers() {
    for command in ["tool.rectangle", "tool.set", "edit.pickColor"] {
        for necessary in [true, false] {
            let f = Fixture::new(Guard::Premise);
            let vector = rewrite(&f, |d| d.execute("node.create", json!({"type":"vector","parent":f.destination})).unwrap())["id"].clone();
            let basis = f.basis().await;
            if necessary {
                f.colleague(&[("vector.edit.begin", json!({"ids":[vector]})), ("vector.edit.pen", json!({"x":510,"y":410}))]);
            } else {
                f.colleague(&[("node.set", json!({"id":f.b,"props":{"opacity":0.5}}))]);
            }
            let bytes = f.bytes();
            let params = if command == "tool.set" { json!({"tool":"ellipse"}) } else { json!({}) };
            let answer = f
                .call(
                    "run",
                    json!({"basis":basis,"steps":[
                        {"command":"vector.edit.begin","params":{"ids":[vector]}},
                        {"command":command,"params":params}
                    ]}),
                )
                .await;
            assert_eq!(answer["error"]["code"] == "conflict", necessary, "{command}, necessary={necessary}: {answer}");
            if necessary {
                assert_eq!(f.bytes(), bytes);
            } else {
                applied(&answer);
                let doc = DesignDocument::open(FILE, &f.bytes()).unwrap();
                assert!(!doc.session().doc().unwrap().contains(kition_design_doc::NodeId(vector.as_u64().unwrap())));
            }
            f.shutdown().await;
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn waiting_for_sdk_tool_place_without_place_parameters() {
    let f = Fixture::new(Guard::Premise);
    let frame = rewrite(&f, |d| {
        let view = d.session().ds().unwrap().view.clone();
        let page = d.inspect().unwrap()["page"].clone();
        d.execute("frame.create", json!({"parent":page,"x":view.cx-60.0,"y":view.cy-60.0,"width":120,"height":120})).unwrap()
    })["id"]
        .clone();
    let basis = f.basis().await;
    f.colleague(&[("transform.move", json!({"ids":[frame],"dx":1000}))]);
    let before = f.bytes();
    let answer = f
        .call(
            "run",
            json!({"basis":basis,"steps":[
                {"command":"tool.rectangle","params":{}},
                {"command":"tool.place","params":{}}
            ]}),
        )
        .await;
    // The tool and default point of this step come from session state established by its
    // prefix. Since SDK 2ce294a1 the guard plans that prefix on an isolated copy, so the
    // implicit place is replayed and the frame others moved away from it is `relocated`
    // (at 1935d52 replay ran only for supplied place keys and the write went into the
    // newly picked parent).
    assert_eq!(answer["error"]["code"], "conflict", "{answer}");
    assert!(
        answer["conflicts"].as_array().unwrap().iter().any(|c| c["id"] == frame && c["reason"] == "relocated"),
        "the moved frame is relocated: {answer}"
    );
    assert_eq!(f.bytes(), before, "refusal must not save");
    f.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn audited_dev_resource_fields_and_document_wide_font_refit() {
    let mut failures = Vec::new();
    for family in ["dev-resource", "font-refit"] {
        for necessary in [true, false] {
            let f = Fixture::new(Guard::Premise);
            let text = rewrite(&f, |d| {
                d.execute("devResource.add", json!({"id":f.a,"url":"https://example.test/spec","name":"Before"})).unwrap();
                d.execute("text.create", json!({"parent":f.destination,"text":"Before"})).unwrap()["id"].clone()
            });
            let basis = f.basis().await;
            match (family, necessary) {
                ("dev-resource", true) => f.colleague(&[("devResource.edit", json!({"id":f.a,"index":0,"name":"Colleague"}))]),
                ("font-refit", true) => f.colleague(&[("text.set", json!({"id":text,"characters":"A much longer colleague string"}))]),
                _ => f.colleague(&[("node.set", json!({"id":f.a,"props":{"opacity":0.5}}))]),
            }
            let bytes = f.bytes();
            let answer = if family == "dev-resource" {
                f.write(&basis, "devResource.edit", json!({"id":f.a,"index":0,"name":"Agent"})).await
            } else {
                f.write(&basis, "text.refit", json!({})).await
            };
            let refused = answer["error"]["code"] == "conflict";
            if refused {
                assert_eq!(f.bytes(), bytes);
            }
            if refused != necessary || (!refused && answer.get("error").is_some()) {
                failures.push(format!("{family}/necessary={necessary}: {answer}"));
            }
            f.shutdown().await;
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[tokio::test(flavor = "multi_thread")]
async fn board_reattach_by_label_holds_the_connector_and_fixed_end() {
    for change in ["fixed-end", "routing", "unrelated"] {
        let f = Fixture::new(Guard::Premise);
        let mut file = kition_design_format::load_bytes_with_branches(&f.bytes()).unwrap();
        file.doc.kind = kition_design_doc::FileKind::Board;
        std::fs::write(f.workspace.path().join(FILE), kition_design_format::save_edited(&file, None).unwrap()).unwrap();
        let connector = rewrite(&f, |d| {
            let ends = if change == "routing" {
                json!({"from":{"x":0,"y":0},"to":{"x":200,"y":0},"label":"Route"})
            } else {
                json!({"from":f.a,"to":f.destination,"label":"Route"})
            };
            d.execute("board.connector", ends).unwrap()
        });
        let basis = f.basis().await;
        match change {
            "fixed-end" => f.colleague(&[("transform.move", json!({"ids":[f.destination],"dx":100}))]),
            "routing" => f.colleague(&[("board.setStyle", json!({"ids":[connector["id"]],"style":"straight"}))]),
            _ => f.colleague(&[("node.set", json!({"id":f.b,"props":{"opacity":0.5}}))]),
        }
        outcome(&f, &basis, "board.reattach", json!({"id":connector["label"],"end":"from","target":f.b}), change != "unrelated").await;
        f.shutdown().await;
    }
}
