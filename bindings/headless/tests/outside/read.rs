//! C8-design-read: one behaviour per class of what the declarations held less than the handlers
//! read (`bindings/wasm/tests/declarations_independent.rs`). An independent writer rewrites the
//! file between the agent's observation and its write: a change that alters what the write rests
//! on must refuse it, an unrelated one must not.
use super::*;

/// The layers a case adds to the fixture's design before the agent observes it.
pub(super) fn prepare(f: &Fixture, edit: impl FnOnce(&mut DesignDocument) -> Value) -> Value {
    let mut writer = DesignDocument::open(FILE, &f.bytes()).unwrap();
    let made = edit(&mut writer);
    std::fs::write(f.workspace.path().join(FILE), writer.seal_and_save(None).unwrap()).unwrap();
    made
}

fn id(answer: &Value) -> Value {
    answer.get("id").or_else(|| answer.get("ids").and_then(|ids| ids.get(0))).cloned().unwrap_or_else(|| panic!("no id in {answer}"))
}

/// An auto layout frame `Stack` holding `First` and `Second` (bottom first).
fn stack(f: &Fixture) -> (Value, Value, Value) {
    let made = prepare(f, |d| {
        let page = d.inspect().unwrap()["page"].clone();
        let stack = id(&d.execute("frame.create", json!({"parent":page,"name":"Stack","x":0,"y":700,"width":200,"height":200})).unwrap());
        d.execute("layout.add", json!({"ids":[stack],"direction":"vertical"})).unwrap();
        let first = id(&d.execute("node.create", json!({"type":"rectangle","parent":stack,"name":"First","width":50,"height":20})).unwrap());
        let second = id(&d.execute("node.create", json!({"type":"rectangle","parent":stack,"name":"Second","width":50,"height":30})).unwrap());
        json!([stack, first, second])
    });
    (made[0].clone(), made[1].clone(), made[2].clone())
}

/// What the agent's write after the colleague's change did: refused (`true`) or applied.
async fn refused(f: &Fixture, basis: &Value, command: &str, params: Value) -> bool {
    let answer = f.write(basis, command, params).await;
    let conflict = answer["error"]["code"] == "conflict";
    assert!(conflict || answer.get("error").is_none(), "{command}: {answer}");
    conflict
}

/// Each case: a write, the change that must refuse it, the unrelated one that must not.
pub(super) async fn case(
    name: &str,
    setup: impl Fn(&Fixture) -> Value,
    write: impl Fn(&Fixture, &Value) -> (&'static str, Value),
    related: impl Fn(&Fixture, &Value) -> Vec<(&'static str, Value)>,
    unrelated: impl Fn(&Fixture, &Value) -> Vec<(&'static str, Value)>,
) -> Vec<String> {
    let mut failures = Vec::new();
    for necessary in [true, false] {
        let f = Fixture::new(Guard::Premise);
        let made = setup(&f);
        let basis = f.basis().await;
        let change = if necessary { related(&f, &made) } else { unrelated(&f, &made) };
        f.colleague(&change);
        let (command, params) = write(&f, &made);
        if refused(&f, &basis, command, params).await != necessary {
            failures.push(format!("{name}: necessary={necessary} after {change:?}"));
        }
        f.shutdown().await;
    }
    failures
}

/// A read that is not held, another object: deleting a layer from an auto layout frame re-flows
/// its siblings from their sizes.
#[tokio::test(flavor = "multi_thread")]
async fn a_delete_in_auto_layout_rests_on_the_siblings_it_reflows() {
    let failures = case(
        "edit.delete in auto layout",
        |f| json!(stack(f)),
        |_, made| ("edit.delete", json!({"ids":[made[1]]})),
        |_, made| vec![("node.set", json!({"id":made[2],"props":{"height":80}}))],
        |_, made| vec![("object.rename", json!({"ids":[made[2]],"name":"Colleague"}))],
    )
    .await;
    assert!(failures.is_empty(), "{failures:?}");
}

/// A read that is not held, a field of a named object: a duplicate in an auto layout frame lands
/// where its layout puts it.
#[tokio::test(flavor = "multi_thread")]
async fn a_duplicate_in_auto_layout_rests_on_the_frames_layout() {
    let failures = case(
        "edit.duplicate in auto layout",
        |f| json!(stack(f)),
        |_, made| ("edit.duplicate", json!({"ids":[made[1]]})),
        |_, made| vec![("node.set", json!({"id":made[0],"props":{"gap":40}}))],
        |_, made| vec![("object.rename", json!({"ids":[made[0]],"name":"Colleague"}))],
    )
    .await;
    assert!(failures.is_empty(), "{failures:?}");
}

/// A default taken from elsewhere: a preset frame given no place lands right of the page's
/// top-level layers.
#[tokio::test(flavor = "multi_thread")]
async fn a_preset_frame_rests_on_where_the_pages_layers_are() {
    let failures = case(
        "frame.create preset",
        |_| Value::Null,
        |_, _| ("frame.create", json!({"preset":"iPhone 16"})),
        |f, _| vec![("transform.move", json!({"ids":[f.destination],"dx":2000,"dy":0}))],
        |f, _| vec![("object.rename", json!({"ids":[f.destination],"name":"Colleague"}))],
    )
    .await;
    assert!(failures.is_empty(), "{failures:?}");
}

/// A place not held: a new layer at an index of a parent's children.
#[tokio::test(flavor = "multi_thread")]
async fn a_new_layer_at_an_index_rests_on_what_is_there() {
    let failures = case(
        "node.create at an index",
        |_| Value::Null,
        |f, _| ("node.create", json!({"type":"ellipse","parent":f.destination,"index":0,"width":5,"height":5})),
        |f, _| vec![("node.create", json!({"type":"rectangle","parent":f.destination,"index":0,"width":5,"height":5}))],
        |f, _| vec![("node.create", json!({"type":"rectangle","parent":f.frame,"width":5,"height":5}))],
    )
    .await;
    assert!(failures.is_empty(), "{failures:?}");
}

/// A value naming another object: a batch's steps name the layers they act on.
#[tokio::test(flavor = "multi_thread")]
async fn a_batch_rests_on_the_layers_its_steps_name() {
    let failures = case(
        "command.batch",
        |_| Value::Null,
        |f, _| ("command.batch", json!({"commands":[{"command":"object.rename","params":{"ids":[f.a],"name":"Agent"}}]})),
        |f, _| vec![("object.rename", json!({"ids":[f.a],"name":"Colleague"}))],
        |f, _| vec![("object.rename", json!({"ids":[f.b],"name":"Colleague"}))],
    )
    .await;
    assert!(failures.is_empty(), "{failures:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_nested_batch_rests_on_the_layers_its_steps_name() {
    let failures = case(
        "nested command.batch",
        |_| Value::Null,
        |f, _| {
            (
                "command.batch",
                json!({"commands":[{"command":"command.batch","params":{"commands":[
                    {"command":"object.rename","params":{"ids":[f.a],"name":"Agent"}}
                ]}}]}),
            )
        },
        |f, _| vec![("object.rename", json!({"ids":[f.a],"name":"Colleague"}))],
        |f, _| vec![("object.rename", json!({"ids":[f.b],"name":"Colleague"}))],
    )
    .await;
    assert!(failures.is_empty(), "{failures:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_batch_selection_prefix_names_the_later_writes_target() {
    let failures = case(
        "command.batch selection prefix",
        |_| Value::Null,
        |f, _| {
            (
                "command.batch",
                json!({"commands":[
                    {"command":"select.set","params":{"ids":[f.a]}},
                    {"command":"object.rename","params":{"name":"Agent"}}
                ]}),
            )
        },
        |f, _| vec![("object.rename", json!({"ids":[f.a],"name":"Colleague"}))],
        |f, _| vec![("object.rename", json!({"ids":[f.b],"name":"Colleague"}))],
    )
    .await;
    assert!(failures.is_empty(), "{failures:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_batch_holds_the_read_result_a_later_write_uses() {
    let failures = case(
        "command.batch read result",
        |_| Value::Null,
        |f, _| {
            (
                "command.batch",
                json!({"commands":[
                    {"command":"node.get","params":{"id":f.a}},
                    {"command":"node.set","params":{"id":f.b,"props":{"width":{"$step":0,"field":"width"}}}}
                ]}),
            )
        },
        |f, _| vec![("node.set", json!({"id":f.a,"props":{"width":80}}))],
        |f, _| vec![("node.set", json!({"id":f.a,"props":{"opacity":0.5}}))],
    )
    .await;
    assert!(failures.is_empty(), "{failures:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn empty_vector_cleanup_reads_the_network_and_not_appearance() {
    let mut failures = Vec::new();
    for command in ["vector.edit.end", "tool.rectangle", "tool.set", "edit.pickColor"] {
        for necessary in [true, false] {
            let f = Fixture::new(Guard::Premise);
            let vector = prepare(&f, |d| d.execute("node.create", json!({"type":"vector","parent":f.destination})).unwrap())["id"].clone();
            let basis = f.basis().await;
            if necessary {
                f.colleague(&[("vector.edit.begin", json!({"ids":[vector]})), ("vector.edit.pen", json!({"x":510,"y":410}))]);
            } else {
                f.colleague(&[("node.set", json!({"id":vector,"props":{"opacity":0.5}}))]);
            }
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
            let conflict = answer["error"]["code"] == "conflict";
            assert!(conflict || answer.get("error").is_none(), "{answer}");
            if conflict != necessary {
                failures.push(format!("{command}: necessary={necessary}: {answer}"));
            }
            f.shutdown().await;
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn dev_resources_read_the_list_and_not_sibling_order() {
    let failures = case(
        "devResource.edit",
        |f| prepare(f, |d| d.execute("devResource.add", json!({"id":f.a,"url":"https://example.com/a","name":"Initial"})).unwrap()),
        |f, _| ("devResource.edit", json!({"id":f.a,"index":0,"name":"Agent"})),
        |f, _| vec![("devResource.edit", json!({"id":f.a,"index":0,"name":"Colleague"}))],
        |f, _| vec![("node.reorder", json!({"id":f.a,"index":1}))],
    )
    .await;
    assert!(failures.is_empty(), "{failures:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn global_text_refit_holds_the_text_membership() {
    let failures = case(
        "text.refit membership",
        |f| prepare(f, |d| d.execute("text.create", json!({"parent":f.destination,"text":"Body"})).unwrap()["id"].clone()),
        |_, _| ("text.refit", json!({})),
        |_, text| vec![("edit.delete", json!({"ids":[text]}))],
        |f, _| vec![("object.rename", json!({"ids":[f.b],"name":"Unrelated"}))],
    )
    .await;
    assert!(failures.is_empty(), "{failures:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn binding_instance_opacity_holds_the_source_value() {
    let failures = case(
        "variable.bind inherited opacity",
        |f| {
            prepare(f, |d| {
                d.execute("node.set", json!({"id":f.instance,"props":{"opacity":0.3}})).unwrap();
                let collection = d.execute("variable.createCollection", json!({"name":"Tokens"})).unwrap()["id"].clone();
                d.execute("variable.create", json!({"collection":collection,"name":"Opacity","type":"number","value":0.5})).unwrap()["id"].clone()
            })
        },
        |f, variable| ("variable.bind", json!({"ids":[f.instance],"variable":variable,"field":"opacity"})),
        |f, _| vec![("node.set", json!({"id":f.component,"props":{"opacity":0.5}}))],
        |f, _| vec![("object.rename", json!({"ids":[f.component],"name":"Unrelated"}))],
    )
    .await;
    assert!(failures.is_empty(), "{failures:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn moving_a_layer_reads_its_transform_and_not_its_size() {
    let failures = case(
        "transform.move size precision",
        |_| Value::Null,
        |f, _| ("transform.move", json!({"ids":[f.a],"dx":10})),
        |f, _| vec![("transform.move", json!({"ids":[f.a],"dx":20}))],
        |f, _| vec![("node.set", json!({"id":f.a,"props":{"width":80}}))],
    )
    .await;
    assert!(failures.is_empty(), "{failures:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn reparenting_reads_transforms_and_not_unused_sizes() {
    let failures = case(
        "reparent transform",
        |_| Value::Null,
        |f, _| ("node.reparent", json!({"ids":[f.a],"parent":f.destination})),
        |f, _| vec![("transform.move", json!({"ids":[f.a],"dx":20,"dy":0}))],
        |f, _| vec![("node.set", json!({"id":f.a,"props":{"width":65}}))],
    )
    .await;
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[tokio::test(flavor = "multi_thread")]
async fn releasing_a_translucent_frame_reads_child_opacity() {
    let failures = case(
        "removeFrame inherited opacity",
        |f| prepare(f, |d| d.execute("node.set", json!({"id":f.frame,"props":{"opacity":0.5}})).unwrap()),
        |f, _| ("object.removeFrame", json!({"ids":[f.frame]})),
        |f, _| vec![("node.set", json!({"id":f.a,"props":{"opacity":0.7}}))],
        |f, _| vec![("node.set", json!({"id":f.a,"props":{"fill":"#123456"}}))],
    )
    .await;
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[tokio::test(flavor = "multi_thread")]
async fn resizing_a_frame_reads_which_children_it_constrains() {
    let failures = case(
        "resize child membership",
        |_| Value::Null,
        |f, _| ("node.set", json!({"id":f.frame,"props":{"width":1000}})),
        |f, _| {
            vec![(
                "node.create",
                json!({"type":"rectangle","parent":f.frame,"width":20,"height":20,"props":{"constraints":{"horizontal":"right","vertical":"top"}}}),
            )]
        },
        |f, _| vec![("node.create", json!({"type":"rectangle","parent":f.destination,"width":20,"height":20}))],
    )
    .await;
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[tokio::test(flavor = "multi_thread")]
async fn placing_an_instance_reads_the_parent_geometry_and_not_its_paint() {
    let failures = case(
        "instance drop parent",
        |_| Value::Null,
        |f, _| ("instance.place", json!({"component":f.component,"x":550,"y":450})),
        |f, _| vec![("transform.move", json!({"ids":[f.destination],"dx":100,"dy":0}))],
        |f, _| vec![("node.set", json!({"id":f.destination,"props":{"fill":"#123456","clip":true}}))],
    )
    .await;
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[tokio::test(flavor = "multi_thread")]
async fn removing_a_table_row_reads_its_place_and_not_its_text() {
    let failures = case(
        "table row removal",
        |f| {
            let mut document = kition_design_doc::Document::new("Synthetic board");
            document.kind = kition_design_doc::FileKind::Board;
            let bytes = kition_design_format::save_bytes(&document).unwrap();
            let mut board = DesignDocument::open(FILE, &bytes).unwrap();
            let table = board.execute("board.table", json!({"rows":2,"columns":2,"x":0,"y":0})).unwrap();
            let doc = board.session().doc().unwrap();
            let cell = kition_design_doc::NodeId(table["cells"][1][0].as_u64().unwrap());
            let row = doc.node(cell).unwrap().parent.and_then(|id| doc.node(id)).unwrap().parent.unwrap();
            let made = json!({"table":table["id"],"text":cell.0,"row":row.0});
            std::fs::write(f.workspace.path().join(FILE), board.seal_and_save(None).unwrap()).unwrap();
            made
        },
        |_, made| ("board.tableEdit", json!({"id":made["table"],"action":"removeRow","index":1})),
        |_, made| vec![("node.reorder", json!({"id":made["row"],"index":0}))],
        |_, made| vec![("text.set", json!({"id":made["text"],"characters":"Colleague text"}))],
    )
    .await;
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[tokio::test(flavor = "multi_thread")]
async fn styling_text_reads_its_size_and_not_its_transform() {
    let failures = case(
        "text.bold size precision",
        |f| prepare(f, |d| d.execute("text.create", json!({"parent":f.destination,"text":"Body"})).unwrap()["id"].clone()),
        |_, text| ("text.bold", json!({"ids":[text]})),
        |_, text| vec![("text.set", json!({"id":text,"characters":"Colleague"}))],
        |_, text| vec![("transform.move", json!({"ids":[text],"dx":20}))],
    )
    .await;
    assert!(failures.is_empty(), "{failures:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn text_size_changes_read_the_hug_parents_limits() {
    let failures = case(
        "text.sizeUp parent limits",
        |f| {
            prepare(f, |d| {
                let parent = d.execute("frame.create", json!({"parent":f.destination,"width":100,"height":100})).unwrap()["id"].clone();
                d.execute("layout.add", json!({"ids":[parent],"direction":"horizontal"})).unwrap();
                d.execute("node.set", json!({"id":parent,"props":{"sizingH":"hug","sizingV":"hug"}})).unwrap();
                let text = d.execute("text.create", json!({"parent":parent,"text":"MMMMMMMMMMMM"})).unwrap()["id"].clone();
                let width = |d: &DesignDocument| d.session().doc().unwrap().node(kition_design_doc::NodeId(parent.as_u64().unwrap())).unwrap().width;
                let before = width(d);
                d.execute("text.sizeUp", json!({"ids":[text]})).unwrap();
                let after = width(d);
                d.execute("edit.undo", json!({})).unwrap();
                assert!(after > before, "{before} -> {after}");
                json!({"parent":parent,"text":text,"limit":(before+after)/2.0})
            })
        },
        |_, made| ("text.sizeUp", json!({"ids":[made["text"]]})),
        |_, made| vec![("node.set", json!({"id":made["parent"],"props":{"maxWidth":made["limit"]}}))],
        |_, made| vec![("node.set", json!({"id":made["parent"],"props":{"opacity":0.5}}))],
    )
    .await;
    assert!(failures.is_empty(), "{failures:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn inserted_vectors_and_images_read_the_parents_limits() {
    let mut failures = Vec::new();
    for command in ["vector.create", "image.place"] {
        failures.extend(case(
            command,
            |f| prepare(f, |d| {
                let parent = d.execute("frame.create", json!({"parent":f.destination,"width":100,"height":100})).unwrap()["id"].clone();
                d.execute("layout.add", json!({"ids":[parent],"direction":"horizontal"})).unwrap();
                d.execute("node.set", json!({"id":parent,"props":{"sizingH":"hug","sizingV":"hug"}})).unwrap();
                d.execute("shape.rectangle", json!({"parent":parent,"width":50,"height":50})).unwrap();
                let width = d.session().doc().unwrap().node(kition_design_doc::NodeId(parent.as_u64().unwrap())).unwrap().width;
                json!({"parent":parent,"limit":width+if command == "vector.create" {50.0} else {1.0}})
            }),
            |_, made| (command, if command == "vector.create" { json!({"parent":made["parent"],"path":"M0 0 L100 100"}) } else {
                json!({"parent":made["parent"],"data":"iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAFElEQVR4nGP8z8DwnwEJMDGgAQBVBgICyX5rtAAAAABJRU5ErkJggg=="})
            }),
            |_, made| vec![("node.set", json!({"id":made["parent"],"props":{"maxWidth":made["limit"]}}))],
            |_, made| vec![("node.set", json!({"id":made["parent"],"props":{"opacity":0.5}}))],
        ).await);
    }
    assert!(failures.is_empty(), "{failures:?}");
}

/// Held-more fields: a rename reads neither appearance nor the layer's tree position.
#[tokio::test(flavor = "multi_thread")]
async fn a_name_write_does_not_hold_appearance_or_tree_position() {
    for change in ["opacity", "move", "reorder"] {
        let f = Fixture::new(Guard::Premise);
        let basis = f.basis().await;
        match change {
            "opacity" => f.colleague(&[("node.set", json!({"id":f.a,"props":{"opacity":0.4}}))]),
            "move" => f.colleague(&[("transform.move", json!({"ids":[f.a],"dx":70}))]),
            _ => f.colleague(&[("node.reorder", json!({"id":f.a,"index":1}))]),
        }
        applied(&f.write(&basis, "object.rename", json!({"ids":[f.a],"name":"Agent"})).await);
        assert_eq!(f.node(f.a)["name"], "Agent");
        f.shutdown().await;
    }
}

/// Held-more objects: an explicit paste offset reads the copied layers, not another layer
/// selected in the editor after the copy.
#[tokio::test(flavor = "multi_thread")]
async fn copying_a_layer_does_not_hold_an_unrelated_layer() {
    let f = Fixture::new(Guard::Premise);
    let basis = f.basis().await;
    f.colleague(&[("node.set", json!({"id":f.b,"props":{"opacity":0.4}}))]);
    let answer = f
        .call(
            "run",
            json!({"basis":basis,"steps":[
                {"command":"edit.copy","params":{"ids":[f.a]}},
                {"command":"edit.paste","params":{"parent":f.destination,"x":500,"y":500}}
            ]}),
        )
        .await;
    applied(&answer);
    f.shutdown().await;
}

/// A saved preset is a named record. Replacing another record does not change the result.
#[tokio::test(flavor = "multi_thread")]
async fn a_saved_preset_holds_its_record_and_not_other_presets() {
    let failures = case(
        "image.savePreset",
        |f| {
            prepare(f, |d| {
                d.execute("image.savePreset", json!({"name":"Mine","filters":{"exposure":0.1}})).unwrap();
                d.execute("image.savePreset", json!({"name":"Other","filters":{"exposure":0.1}})).unwrap();
                Value::Null
            })
        },
        |_, _| ("image.savePreset", json!({"name":"Mine","filters":{"exposure":0.3}})),
        |_, _| vec![("image.savePreset", json!({"name":"Mine","filters":{"exposure":0.8}}))],
        |_, _| vec![("image.savePreset", json!({"name":"Other","filters":{"exposure":0.8}}))],
    )
    .await;
    assert!(failures.is_empty(), "{failures:?}");
}

/// Vertex edits address the network, so changing that network interferes while appearance
/// changes on the same vector survive.
#[tokio::test(flavor = "multi_thread")]
async fn a_vector_network_edit_does_not_hold_unread_appearance() {
    for necessary in [true, false] {
        let f = Fixture::new(Guard::Premise);
        let vector = prepare(&f, |d| id(&d.execute("vector.create", json!({"path":"M0 0 L40 0 L40 40","parent":f.destination})).unwrap()));
        let basis = f.basis().await;
        if necessary {
            f.colleague(&[("vector.reverse", json!({"ids":[vector]}))]);
        } else {
            f.colleague(&[("node.set", json!({"id":vector,"props":{"opacity":0.4}}))]);
        }
        let answer = f
            .call(
                "run",
                json!({"basis":basis,"steps":[
                    {"command":"vector.edit.begin","params":{"ids":[vector]}},
                    {"command":"vector.edit.moveVertices","params":{"vertices":[0],"dx":1,"dy":0}}
                ]}),
            )
            .await;
        assert_eq!(answer["error"]["code"] == "conflict", necessary, "{answer}");
        if !necessary {
            applied(&answer);
        }
        f.shutdown().await;
    }
}
