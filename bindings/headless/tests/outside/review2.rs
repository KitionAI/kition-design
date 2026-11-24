//! C8-design review 2: independent writers save between observation and guarded mutation.
use super::*;

fn rewrite(f: &Fixture, edit: impl FnOnce(&mut DesignDocument) -> Value) -> Value {
    let mut writer = DesignDocument::open(FILE, &f.bytes()).unwrap();
    let out = edit(&mut writer);
    std::fs::write(f.workspace.path().join(FILE), writer.seal_and_save(None).unwrap()).unwrap();
    out
}

async fn outcome(f: &Fixture, basis: &Value, command: &str, params: Value, necessary: bool) -> Value {
    let before = f.bytes();
    let answer = f.write(basis, command, params).await;
    assert_eq!(answer["error"]["code"] == "conflict", necessary, "{command}, necessary={necessary}: {answer}");
    if necessary {
        assert_eq!(f.bytes(), before, "refusal must not save");
    } else {
        applied(&answer);
    }
    answer
}

fn library(f: &Fixture, path: &str) -> Value {
    let mut lib = DesignDocument::create(path, 100, 100).unwrap();
    let main = lib.inspect().unwrap()["tree"][0]["id"].clone();
    lib.execute("component.create", json!({"ids":[main]})).unwrap();
    lib.execute("node.create", json!({"type":"rectangle","parent":main,"width":10,"height":10})).unwrap();
    lib.execute("library.publish", json!({})).unwrap();
    let bytes = lib.seal_and_save(None).unwrap();
    std::fs::write(f.workspace.path().join(path), &bytes).unwrap();
    rewrite(f, |d| {
        d.lend_library(path, Some(&bytes)).unwrap();
        let link = d.execute("library.add", json!({"path":path})).unwrap()["id"].clone();
        let local = d.execute("library.import", json!({"library":link,"component":"Frame 1"})).unwrap()["component"].clone();
        let instance = d.execute("instance.create", json!({"component":local,"parent":f.frame})).unwrap()["id"].clone();
        json!({"link":link,"main":local,"instance":instance})
    })
}

fn update_library(f: &Fixture, path: &str) {
    let bytes = std::fs::read(f.workspace.path().join(path)).unwrap();
    let mut lib = DesignDocument::open(path, &bytes).unwrap();
    let main = lib.inspect().unwrap()["tree"][0]["id"].clone();
    lib.execute("node.set", json!({"id":main,"props":{"width":120}})).unwrap();
    lib.execute("library.publish", json!({})).unwrap();
    std::fs::write(f.workspace.path().join(path), lib.seal_and_save(None).unwrap()).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn library_refresh_swap_and_prune_hold_local_inputs() {
    let mut failures = Vec::new();
    for change in ["main", "user", "prune", "swap", "unrelated", "prune-unrelated", "swap-unrelated"] {
        let f = Fixture::new(Guard::Premise);
        let lib = library(&f, "Library.kdesign");
        let other = library(&f, "Other.kdesign");
        update_library(&f, "Library.kdesign");
        let basis = f.basis().await;
        match change {
            "main" => {
                let mut file = kition_design_format::load_with_replica(&f.bytes()).unwrap();
                file.doc.get_mut(kition_design_doc::NodeId(lib["main"].as_u64().unwrap())).unwrap().width += 50.0;
                let bytes = kition_design_format::save_edited(&file).unwrap();
                std::fs::write(f.workspace.path().join(FILE), bytes).unwrap();
            }
            "user" | "swap" => f.colleague(&[("node.set", json!({"id":lib["instance"],"props":{"width":130}}))]),
            "prune" => f.colleague(&[("edit.delete", json!({"ids":[lib["instance"]]}))]),
            _ => f.colleague(&[("node.set", json!({"id":f.b,"props":{"opacity":0.5}}))]),
        }
        let (cmd, params) = match change {
            "swap" | "swap-unrelated" => ("library.swap", json!({"from":lib["link"],"to":other["link"]})),
            "prune" | "prune-unrelated" => ("library.remove", json!({"id":lib["link"]})),
            _ => ("library.acceptUpdates", json!({"ids":[lib["link"]]})),
        };
        let before = f.bytes();
        let necessary = !change.ends_with("unrelated");
        let answer = if necessary { f.write(&basis, cmd, params).await } else { outcome(&f, &basis, cmd, params, false).await };
        if (answer["error"]["code"] == "conflict") != necessary || (!necessary && answer.get("error").is_some()) {
            failures.push(format!("{change}: {answer}"));
        }
        if necessary && answer["error"]["code"] == "conflict" {
            assert_eq!(f.bytes(), before);
        }
        f.shutdown().await;
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn board_file(f: &Fixture) {
    let mut file = kition_design_format::load_with_replica(&f.bytes()).unwrap();
    file.doc.kind = kition_design_doc::FileKind::Board;
    let bytes = kition_design_format::save_edited(&file).unwrap();
    std::fs::write(f.workspace.path().join(FILE), bytes).unwrap();
}

// Item 3: handlers that read beyond the objects they name. For each, a colleague change of what
// the handler reads must refuse; an unrelated colleague change must apply.
#[tokio::test(flavor = "multi_thread")]
async fn audited_families_hold_what_they_read() {
    let mut failures = Vec::new();
    let cases =
        ["data.fill", "variable.import", "dev.resolveChange", "devStatus", "codeConnect.set", "board.tableEdit", "board.vote", "shapeBuilder.merge"];
    for case in cases {
        for necessary in [true, false] {
            let f = Fixture::new(Guard::Premise);
            let d: Vec<u64> = {
                let doc = DesignDocument::open(FILE, &f.bytes()).unwrap();
                doc.session().doc().unwrap().get(kition_design_doc::NodeId(f.destination)).unwrap().children.iter().map(|c| c.0).collect()
            };
            if matches!(case, "board.tableEdit" | "board.vote") {
                board_file(&f);
            }
            let setup = rewrite(&f, |doc| match case {
                "variable.import" => {
                    doc.execute("variable.createCollection", json!({"name":"Brand"})).unwrap();
                    doc.execute("variable.create", json!({"collection":"Brand","name":"Primary","type":"color","value":"#ff0000"})).unwrap();
                    let exported = doc.execute("variable.export", json!({})).unwrap();
                    json!({"export":exported})
                }
                "dev.resolveChange" => {
                    doc.execute("node.set", json!({"id":f.frame,"props":{"devStatus":"readyForDev"}})).unwrap();
                    doc.execute("node.set", json!({"id":f.a,"props":{"opacity":0.3}})).unwrap();
                    json!({})
                }
                "board.tableEdit" => doc.execute("board.table", json!({"cells":[["a","b"],["c","d"]],"x":0,"y":0})).unwrap(),
                "board.vote" => {
                    doc.execute("board.vote", json!({"action":"start"})).unwrap();
                    json!({})
                }
                "shapeBuilder.merge" => doc.execute("shapeBuilder.regions", json!({"ids":[d[0],d[1]]})).unwrap(),
                _ => json!({}),
            });
            let basis = f.basis().await;
            let unrelated = ("node.set", json!({"id":f.destination,"props":{"opacity":0.5}}));
            let (command, params, change): (&str, Value, Vec<(&str, Value)>) = match case {
                "data.fill" => {
                    ("data.fill", json!({"id":f.frame,"rows":[{"A":{"fill":"#00ff00"}}]}), vec![("object.rename", json!({"ids":[f.a],"name":"Z"}))])
                }
                "variable.import" => (
                    "variable.import",
                    json!({"variables":setup["export"]["text"].as_str().map(|t| json!(t)).unwrap_or(setup["export"].clone())}),
                    vec![("variable.renameCollection", json!({"collection":"Brand","name":"Old"}))],
                ),
                "dev.resolveChange" => ("dev.resolveChange", json!({"ids":[f.frame]}), vec![("node.set", json!({"id":f.b,"props":{"opacity":0.4}}))]),
                "devStatus" => (
                    "node.set",
                    json!({"id":f.frame,"props":{"devStatus":"readyForDev"}}),
                    vec![("node.set", json!({"id":f.b,"props":{"opacity":0.4}}))],
                ),
                "codeConnect.set" => (
                    "codeConnect.set",
                    json!({"id":f.instance,"template":"<Source />"}),
                    vec![("codeConnect.set", json!({"id":f.component,"name":"Colleague","importPath":"./colleague"}))],
                ),
                "board.tableEdit" => (
                    "board.tableEdit",
                    json!({"id":setup["id"],"action":"removeColumn","index":1}),
                    // The indexed column changes when the preceding one is removed.
                    vec![("board.tableEdit", json!({"id":setup["id"],"action":"removeColumn","index":0}))],
                ),
                "board.vote" => (
                    "board.vote",
                    json!({"action":"end"}),
                    vec![("board.vote", json!({"action":"end"})), ("board.vote", json!({"action":"start","votes":5}))],
                ),
                _ => (
                    "shapeBuilder.merge",
                    json!({"ids":[d[0],d[1]],"points":[setup["regions"][0]["point"].clone()]}),
                    // D1 now lies where D0 was observed: the same page point picks another shape.
                    vec![("transform.move", json!({"ids":[f.destination],"dx":-20,"dy":0}))],
                ),
            };
            if necessary {
                f.colleague(&change);
            } else {
                f.colleague(&[unrelated]);
            }
            let before = f.bytes();
            let answer = f.write(&basis, command, params).await;
            let refused = answer["error"]["code"] == "conflict";
            if refused != necessary || (!necessary && answer.get("error").is_some()) {
                failures.push(format!("{case} necessary={necessary}: {answer}"));
            } else if refused && f.bytes() != before {
                failures.push(format!("{case}: a refusal saved"));
            }
            f.shutdown().await;
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn changed(answer: &Value) -> Vec<Value> {
    answer["changes"]["changed"].as_array().into_iter().flatten().map(|c| c["id"].clone()).collect()
}

// Item 4: a write the engine propagates lands on what the agent observed. Propagation sources
// it reads are held; the propagated targets are consequences named in the receipt.
#[tokio::test(flavor = "multi_thread")]
async fn propagated_writes_hold_sources_and_report_targets() {
    let mut failures = Vec::new();
    for family in ["style", "variable", "component", "autoLayout", "constraints"] {
        for colleague in ["source", "consequence", "unrelated"] {
            let f = Fixture::new(Guard::Premise);
            let setup = rewrite(&f, |d| match family {
                "style" => {
                    d.execute("style.apply", json!({"ids":[f.a],"style":f.style})).unwrap();
                    json!({})
                }
                "variable" => {
                    d.execute("variable.createCollection", json!({"name":"Tokens"})).unwrap();
                    let v =
                        d.execute("variable.create", json!({"collection":"Tokens","name":"Brand","type":"color","value":"#ff0000"})).unwrap()["id"]
                            .clone();
                    d.execute("variable.bind", json!({"ids":[f.a],"field":"fill","variable":v})).unwrap();
                    json!({"variable":v})
                }
                "component" => {
                    let child = d
                        .execute("node.create", json!({"type":"rectangle","parent":f.component,"name":"Inner","x":5,"y":5,"width":10,"height":10}))
                        .unwrap()["id"]
                        .clone();
                    json!({"child":child})
                }
                "autoLayout" => {
                    d.execute("layout.add", json!({"ids":[f.destination],"direction":"horizontal"})).unwrap();
                    json!({})
                }
                _ => {
                    d.execute("node.set", json!({"id":f.a,"props":{"constraints":{"horizontal":"scale"}}})).unwrap();
                    json!({})
                }
            });
            let doc = DesignDocument::open(FILE, &f.bytes()).unwrap();
            let kids =
                |id: u64| doc.session().doc().unwrap().get(kition_design_doc::NodeId(id)).unwrap().children.iter().map(|c| c.0).collect::<Vec<_>>();
            let copy = if family == "component" { kids(f.instance).first().copied() } else { None };
            let d = kids(f.destination);
            let basis = f.basis().await;
            // (agent write, the source change that must refuse, the consequence change that must not, ids the receipt must name)
            let (command, params, source, consequence, expect) = match family {
                "style" => (
                    "style.update",
                    json!({"style":f.style,"value":["#00f"]}),
                    ("style.update", json!({"style":f.style,"value":["#0f0"]})),
                    ("node.set", json!({"id":f.a,"props":{"name":"Renamed"}})),
                    vec![json!(f.a)],
                ),
                "variable" => (
                    "variable.set",
                    json!({"variable":setup["variable"],"value":"#0000ff"}),
                    ("variable.set", json!({"variable":setup["variable"],"value":"#00ff00"})),
                    ("node.set", json!({"id":f.a,"props":{"name":"Renamed"}})),
                    vec![json!(f.a)],
                ),
                "component" => (
                    "node.set",
                    json!({"id":setup["child"],"props":{"opacity":0.5}}),
                    ("node.set", json!({"id":setup["child"],"props":{"opacity":0.25}})),
                    ("node.set", json!({"id":f.instance,"props":{"name":"Renamed"}})),
                    vec![json!(copy.unwrap())],
                ),
                "autoLayout" => (
                    "node.set",
                    json!({"id":d[0],"props":{"width":30}}),
                    ("node.set", json!({"id":f.destination,"props":{"itemSpacing":7}})),
                    ("node.set", json!({"id":d[2],"props":{"name":"Renamed"}})),
                    vec![json!(d[1]), json!(d[2])],
                ),
                _ => (
                    "node.set",
                    json!({"id":f.frame,"props":{"width":1600}}),
                    ("node.set", json!({"id":f.a,"props":{"constraints":{"horizontal":"left"}}})),
                    ("node.set", json!({"id":f.a,"props":{"name":"Renamed"}})),
                    vec![json!(f.a)],
                ),
            };
            let necessary = colleague == "source";
            let change = match colleague {
                "source" => source,
                "consequence" => consequence,
                _ => ("node.set", json!({"id":f.b,"props":{"opacity":0.5}})),
            };
            f.colleague(&[(change.0, change.1)]);
            let before = f.bytes();
            let answer = f.write(&basis, command, params).await;
            let refused = answer["error"]["code"] == "conflict";
            if refused != necessary {
                failures.push(format!("{family}/{colleague}: refused={refused}: {answer}"));
            } else if refused {
                if f.bytes() != before {
                    failures.push(format!("{family}/{colleague}: a refusal saved"));
                }
            } else if answer.get("error").is_some() {
                failures.push(format!("{family}/{colleague}: {answer}"));
            } else {
                let named = changed(&answer);
                let missing: Vec<_> = expect.iter().filter(|id| !named.contains(id)).collect();
                if !missing.is_empty() {
                    failures.push(format!("{family}/{colleague}: receipt misses propagated {missing:?}: {}", answer["changes"]));
                }
            }
            f.shutdown().await;
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// Item 5, pinned at SDK 1935d52: a linked library file is outside the document's revision. When
// only the library changed between observation and write there is no change set since `basis`,
// so the guard has nothing to answer and a write consuming the library applies with the bytes
// as they are now. The receipt names everything it changed. Answering `unknown` here needs an
// SDK hook for external inputs (a kit-reported stamp per observed revision); when the SDK gains
// one this test must change.
#[tokio::test(flavor = "multi_thread")]
async fn pinned_sdk_library_only_change_is_invisible_to_the_guard() {
    let f = Fixture::new(Guard::Premise);
    let lib = library(&f, "Library.kdesign");
    let basis = f.basis().await;
    let observed = f.call("run", json!({"steps":[{"command":"library.updates","params":{}}]})).await;
    update_library(&f, "Library.kdesign");
    let before = f.bytes();
    let answer = f.write(&basis, "library.acceptUpdates", json!({"ids":[lib["link"]]})).await;
    assert!(answer.get("error").is_none(), "SDK external-input limitation changed: {answer}");
    assert_ne!(f.bytes(), before);
    assert!(changed(&answer).contains(&lib["main"]), "the receipt names the refreshed copy: {}", answer["changes"]);
    println!("SDK EXTERNAL LIBRARY REPRO observed {} then applied {}", observed["completed"], answer["changes"]);
    f.shutdown().await;
}
