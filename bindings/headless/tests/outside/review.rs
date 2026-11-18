//! C8-design review: each colleague opens and rewrites the actual file independently.
use super::*;

fn rewrite(f: &Fixture, edit: impl FnOnce(&mut DesignDocument) -> Value) -> Value {
    let mut writer = DesignDocument::open(FILE, &f.bytes()).unwrap();
    let out = edit(&mut writer);
    std::fs::write(f.workspace.path().join(FILE), writer.save(None).unwrap()).unwrap();
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
async fn instance_place_holds_the_drop_destination_and_source_descendants() {
    for change in ["destination", "source", "unrelated"] {
        let f = Fixture::new(Guard::Premise);
        let child =
            rewrite(&f, |d| d.execute("node.create", json!({"parent":f.component,"type":"rectangle","width":10,"height":10})).unwrap())["id"].clone();
        let basis = f.basis().await;
        match change {
            "destination" => f.colleague(&[("transform.move", json!({"ids":[f.destination],"dx":300,"dy":0}))]),
            "source" => f.colleague(&[("node.set", json!({"id":child,"props":{"fill":"#0f0"}}))]),
            _ => f.colleague(&[("node.set", json!({"id":f.b,"props":{"opacity":0.5}}))]),
        }
        outcome(&f, &basis, "instance.place", json!({"component":f.component,"x":520,"y":420}), change != "unrelated").await;
        f.shutdown().await;
    }
}

fn library(f: &Fixture) -> Value {
    let mut lib = DesignDocument::create("Library.kdesign", 100, 100).unwrap();
    let component = lib.inspect().unwrap()["tree"][0]["id"].clone();
    lib.execute("component.create", json!({"ids":[component]})).unwrap();
    lib.execute("library.publish", json!({})).unwrap();
    let bytes = lib.save(None).unwrap();
    std::fs::write(f.workspace.path().join("Library.kdesign"), &bytes).unwrap();
    rewrite(f, |d| {
        d.lend_library("Library.kdesign", Some(&bytes)).unwrap();
        d.execute("library.add", json!({"path":"Library.kdesign"})).unwrap()
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn accepting_updates_holds_the_named_library_links() {
    for necessary in [true, false] {
        let f = Fixture::new(Guard::Premise);
        let link = library(&f);
        let basis = f.basis().await;
        if necessary {
            f.colleague(&[("library.remove", json!({"id":link["id"]}))]);
        } else {
            f.colleague(&[("node.set", json!({"id":f.b,"props":{"opacity":0.5}}))]);
        }
        outcome(&f, &basis, "library.acceptUpdates", json!({"ids":[link["id"]]}), necessary).await;
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn library_swap_holds_the_instance_at_the_point() {
    for command in ["library.swapAt", "library.place"] {
        for necessary in [true, false] {
            let f = Fixture::new(Guard::Premise);
            let link = library(&f);
            let basis = f.basis().await;
            let target = if command == "library.swapAt" { f.instance } else { f.destination };
            f.colleague(&[("transform.move", json!({"ids":[if necessary {target} else {f.b}],"dx":100,"dy":0}))]);
            let (x, y) = if command == "library.swapAt" { (210, 30) } else { (520, 420) };
            outcome(&f, &basis, command, json!({"library":link["id"],"component":"Frame 1","x":x,"y":y}), necessary).await;
            f.shutdown().await;
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn library_place_uses_the_view_point_when_coordinates_are_omitted() {
    for necessary in [true, false] {
        let f = Fixture::new(Guard::Premise);
        let link = library(&f);
        let destination = rewrite(&f, |d| {
            let view = d.session().ds().unwrap().view.clone();
            let page = d.inspect().unwrap()["page"].clone();
            d.execute("frame.create", json!({"parent":page,"x":view.cx-50.0,"y":view.cy-50.0,"width":100,"height":100})).unwrap()["id"].clone()
        });
        let basis = f.basis().await;
        f.colleague(&[("transform.move", json!({"ids":[if necessary {destination} else {json!(f.b)}],"dx":300,"dy":0}))]);
        outcome(&f, &basis, "library.place", json!({"library":link["id"],"component":"Frame 1"}), necessary).await;
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn library_point_writes_hold_an_existing_local_main() {
    for command in ["library.place", "library.swapAt"] {
        for necessary in [true, false] {
            let f = Fixture::new(Guard::Premise);
            let link = library(&f);
            let local = rewrite(&f, |d| {
                let bytes = std::fs::read(f.workspace.path().join("Library.kdesign")).unwrap();
                d.lend_library("Library.kdesign", Some(&bytes)).unwrap();
                d.execute("library.import", json!({"library":link["id"],"component":"Frame 1"})).unwrap()["component"].clone()
            })
            .as_u64()
            .unwrap();
            let basis = f.basis().await;
            if necessary {
                let mut file = kition_design_format::load_bytes_with_versions(&f.bytes()).unwrap();
                file.doc.get_mut(kition_design_doc::NodeId(local)).unwrap().width += 50.0;
                let bytes = kition_design_format::save_bytes_with_history(&file.doc, None, &file.versions, &file.branches).unwrap();
                std::fs::write(f.workspace.path().join(FILE), bytes).unwrap();
            } else {
                f.colleague(&[("node.set", json!({"id":f.b,"props":{"opacity":0.5}}))]);
            }
            let (x, y) = if command == "library.place" { (520, 420) } else { (210, 30) };
            outcome(&f, &basis, command, json!({"library":link["id"],"component":"Frame 1","x":x,"y":y}), necessary).await;
            f.shutdown().await;
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn expression_names_cannot_silently_rebind() {
    for necessary in [true, false] {
        let f = Fixture::new(Guard::Premise);
        let ids = rewrite(&f, |d| {
            let c = d.execute("variable.createCollection", json!({"name":"Counts"})).unwrap()["id"].clone();
            let v = d.execute("variable.create", json!({"collection":c,"name":"count","type":"number","value":1})).unwrap()["id"].clone();
            let dest = d.execute("variable.create", json!({"collection":c,"name":"result","type":"number","value":0})).unwrap()["id"].clone();
            json!({"collection":c,"variable":v,"dest":dest})
        });
        let basis = f.basis().await;
        if necessary {
            f.colleague(&[
                ("variable.update", json!({"variable":ids["variable"],"name":"away"})),
                ("variable.create", json!({"collection":ids["collection"],"name":"count","type":"number","value":99})),
            ]);
        } else {
            f.colleague(&[("node.set", json!({"id":f.b,"props":{"opacity":0.5}}))]);
        }
        outcome(
            &f,
            &basis,
            "proto.interaction.add",
            json!({"id":f.a,"action":{"type":"setVariable","variable":ids["dest"],"expression":"count + 1"}}),
            necessary,
        )
        .await;
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn implicit_prototype_conditions_hold_the_default_variable() {
    for necessary in [true, false] {
        let f = Fixture::new(Guard::Premise);
        let variable = rewrite(&f, |d| {
            let c = d.execute("variable.createCollection", json!({"name":"Counts"})).unwrap()["id"].clone();
            let v = d.execute("variable.create", json!({"collection":c,"name":"count","type":"number","value":1})).unwrap()["id"].clone();
            d.execute("proto.interaction.add", json!({"id":f.a,"action":{"type":"conditional","blocks":[{"expression":"count == 1","actions":[]}]}}))
                .unwrap();
            v
        });
        let basis = f.basis().await;
        if necessary {
            f.colleague(&[("variable.set", json!({"variable":variable,"value":2}))]);
        } else {
            f.colleague(&[("node.set", json!({"id":f.b,"props":{"opacity":0.5}}))]);
        }
        outcome(&f, &basis, "proto.conditional.addBlock", json!({"id":f.a,"index":0,"path":[0]}), necessary).await;
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn variable_bind_holds_the_alias_chain_it_evaluates() {
    for necessary in [true, false] {
        let f = Fixture::new(Guard::Premise);
        let variables = rewrite(&f, |d| {
            let c = d.execute("variable.createCollection", json!({"name":"Numbers"})).unwrap()["id"].clone();
            let base = d.execute("variable.create", json!({"collection":c,"name":"Base","type":"number","value":0.5})).unwrap()["id"].clone();
            let alias =
                d.execute("variable.create", json!({"collection":c,"name":"Alias","type":"number","value":{"alias":base}})).unwrap()["id"].clone();
            json!({"base":base,"alias":alias})
        });
        let basis = f.basis().await;
        if necessary {
            f.colleague(&[("variable.set", json!({"variable":variables["base"],"value":0.8}))]);
        } else {
            f.colleague(&[("node.set", json!({"id":f.b,"props":{"opacity":0.5}}))]);
        }
        outcome(&f, &basis, "variable.bind", json!({"ids":[f.a],"variable":variables["alias"],"field":"opacity"}), necessary).await;
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn component_property_references_and_definitions_are_premises() {
    for form in ["default", "preferred", "definition", "property", "stored-default"] {
        for necessary in [true, false] {
            let f = Fixture::new(Guard::Premise);
            let other = rewrite(&f, |d| {
                let c = d.execute("frame.create", json!({"x":900,"width":30,"height":30})).unwrap()["id"].clone();
                d.execute("component.create", json!({"ids":[c]})).unwrap();
                d.execute("component.addProperty", json!({"id":f.component,"name":"Swap","type":"instanceSwap","default":c})).unwrap();
                c
            });
            let basis = f.basis().await;
            if necessary {
                match form {
                    "definition" => f.colleague(&[("component.editProperty", json!({"id":f.component,"name":"Swap","default":f.component}))]),
                    _ => f.colleague(&[("edit.delete", json!({"ids":[other]}))]),
                }
            } else {
                f.colleague(&[("node.set", json!({"id":f.b,"props":{"opacity":0.5}}))]);
            }
            let (command, params) = match form {
                "default" => ("component.addProperty", json!({"id":f.component,"name":"New","type":"instanceSwap","default":other})),
                "preferred" => ("component.editProperty", json!({"id":f.component,"name":"Swap","preferred":[other.to_string()]})),
                "definition" => ("instance.setProperties", json!({"ids":[f.instance],"properties":{"Swap":f.component}})),
                "stored-default" => ("instance.create", json!({"component":f.component,"parent":f.frame,"x":300,"y":20})),
                _ => ("instance.setProperties", json!({"ids":[f.instance],"properties":{"Swap":other}})),
            };
            outcome(&f, &basis, command, params, necessary).await;
            f.shutdown().await;
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn traversing_writes_hold_the_descendants_they_consume() {
    let mut failures = Vec::new();
    for family in ["text", "layout", "flatten", "page", "publish"] {
        for necessary in [true, false] {
            let f = Fixture::new(Guard::Premise);
            let text = rewrite(&f, |d| d.execute("text.create", json!({"parent":f.frame,"text":"Original","x":0,"y":100})).unwrap())["id"].clone();
            let page = DesignDocument::open(FILE, &f.bytes()).unwrap().inspect().unwrap()["page"].clone();
            let basis = f.basis().await;
            if necessary {
                if family == "publish" {
                    f.colleague(&[("object.rename", json!({"ids":[f.component],"name":"Colleague"}))]);
                } else {
                    f.colleague(&[("text.set", json!({"id":text,"characters":"Colleague"}))]);
                }
            } else {
                if family == "publish" {
                    f.colleague(&[("node.set", json!({"id":f.destination,"props":{"opacity":0.5}}))]);
                } else {
                    f.colleague(&[("style.update", json!({"style":f.style,"description":"Colleague"}))]);
                }
            }
            let (command, params) = match family {
                "text" => ("text.bold", json!({"ids":[f.frame]})),
                "layout" => ("layout.add", json!({"ids":[f.frame],"direction":"horizontal"})),
                "flatten" => ("vector.flatten", json!({"ids":[f.frame]})),
                "page" => ("page.duplicate", json!({"id":page})),
                _ => ("library.publish", json!({})),
            };
            let answer = f.write(&basis, command, params).await;
            if (answer["error"]["code"] == "conflict") != necessary || (!necessary && answer.get("error").is_some()) {
                failures.push(format!("{family}, necessary={necessary}: {answer}"));
            }
            f.shutdown().await;
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[tokio::test(flavor = "multi_thread")]
async fn resizing_a_plain_frame_holds_its_constrained_children() {
    for necessary in [true, false] {
        let f = Fixture::new(Guard::Premise);
        f.colleague(&[("node.set", json!({"id":f.a,"props":{"constraints":{"horizontal":"right"}}}))]);
        let basis = f.basis().await;
        f.colleague(&[("node.set", json!({"id":if necessary {f.a} else {f.destination},"props":{"x":35}}))]);
        outcome(&f, &basis, "node.set", json!({"id":f.frame,"props":{"width":900}}), necessary).await;
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn comment_reply_indexes_hold_the_message_and_order() {
    for necessary in [true, false] {
        let f = Fixture::new(Guard::Premise);
        let thread = rewrite(&f, |d| {
            let id = d.execute("comment.add", json!({"x":10,"y":10,"text":"Thread"})).unwrap()["id"].clone();
            for text in ["First", "Second"] {
                d.execute("comment.reply", json!({"id":id,"text":text})).unwrap();
            }
            id
        });
        let basis = f.basis().await;
        if necessary {
            f.colleague(&[("comment.delete", json!({"id":thread,"reply":0}))]);
        } else {
            f.colleague(&[("node.set", json!({"id":f.b,"props":{"opacity":0.5}}))]);
        }
        outcome(&f, &basis, "comment.edit", json!({"id":thread,"reply":0,"text":"Agent"}), necessary).await;
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn saved_category_ids_hold_the_record_they_resolve() {
    for necessary in [true, false] {
        let f = Fixture::new(Guard::Premise);
        let category = rewrite(&f, |d| d.execute("annotation.addCategory", json!({"name":"Review","color":"#f00"})).unwrap())["id"].clone();
        let other = rewrite(&f, |d| d.execute("annotation.addCategory", json!({"name":"Other","color":"#0f0"})).unwrap())["id"].clone();
        let basis = f.basis().await;
        if necessary {
            f.colleague(&[("annotation.editCategory", json!({"id":category,"name":"Colleague"}))]);
        } else {
            f.colleague(&[("annotation.editCategory", json!({"id":other,"name":"Unrelated"}))]);
        }
        outcome(&f, &basis, "annotation.editCategory", json!({"id":category,"name":"Agent"}), necessary).await;
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn normalization_search_and_layout_dependencies_are_held() {
    let mut failures = Vec::new();
    for family in ["component-users", "layout-props", "search", "text-membership"] {
        for necessary in [true, false] {
            let f = Fixture::new(Guard::Premise);
            let text = rewrite(&f, |d| {
                d.execute("component.addProperty", json!({"id":f.component,"name":"Label","type":"text","default":"Original"})).unwrap();
                d.execute("node.set", json!({"id":f.frame,"props":{"layout":"horizontal"}})).unwrap();
                if family == "layout-props" {
                    d.execute("node.set", json!({"id":f.frame,"props":{"sizingH":"fixed","sizingV":"fixed","width":800,"height":600}})).unwrap();
                }
                d.execute("text.create", json!({"parent":f.frame,"text":"Original"})).unwrap()["id"].clone()
            });
            let basis = f.basis().await;
            if necessary {
                match family {
                    "component-users" => f.colleague(&[("instance.setProperties", json!({"ids":[f.instance],"properties":{"Label":"Colleague"}}))]),
                    "layout-props" => f.colleague(&[("node.set", json!({"id":f.a,"props":{"width":100}}))]),
                    "text-membership" => f.colleague(&[("text.create", json!({"parent":f.frame,"text":"New"}))]),
                    _ => f.colleague(&[("text.set", json!({"id":text,"characters":"Colleague Original"}))]),
                }
            } else {
                f.colleague(&[("style.update", json!({"style":f.style,"description":"Unrelated"}))]);
            }
            let (command, params) = match family {
                "component-users" => ("component.editProperty", json!({"id":f.component,"name":"Label","newName":"Title"})),
                "layout-props" => ("node.set", json!({"id":f.frame,"props":{"gap":30}})),
                "text-membership" => ("text.bold", json!({"ids":[f.frame]})),
                _ => ("edit.findReplace", json!({"find":"Original","replace":"Agent"})),
            };
            let answer = f.write(&basis, command, params).await;
            if (answer["error"]["code"] == "conflict") != necessary || (!necessary && answer.get("error").is_some()) {
                failures.push(format!("{family}, necessary={necessary}: {answer}"));
            }
            f.shutdown().await;
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[tokio::test(flavor = "multi_thread")]
async fn stored_resources_and_nested_setters_hold_their_inputs() {
    let mut failures = Vec::new();
    for family in ["mode-copy", "default-mode", "style-name", "folder", "nested-action", "history", "new-child"] {
        for necessary in [true, false] {
            let f = Fixture::new(Guard::Premise);
            let setup = rewrite(&f, |d| {
                let c = d.execute("variable.createCollection", json!({"name":"Counts","modes":["Day","Night"]})).unwrap()["id"].clone();
                let v = d.execute("variable.create", json!({"collection":c,"name":"count","type":"number","value":1})).unwrap()["id"].clone();
                let dest = d.execute("variable.create", json!({"collection":c,"name":"result","type":"number","value":0})).unwrap()["id"].clone();
                let version = d.execute("file.saveVersion", json!({"name":"Original"})).unwrap()["id"].clone();
                d.execute("style.update", json!({"style":f.style,"name":"Folder/Accent"})).unwrap();
                json!({"collection":c,"variable":v,"dest":dest,"version":version})
            });
            let basis = f.basis().await;
            if necessary {
                match family {
                    "mode-copy" => f.colleague(&[("variable.set", json!({"variable":setup["variable"],"value":9}))]),
                    "default-mode" => f.colleague(&[("variable.moveMode", json!({"collection":setup["collection"],"mode":"Night","to":0}))]),
                    "style-name" | "folder" => f.colleague(&[
                        ("style.update", json!({"style":f.style,"name":"Away"})),
                        ("style.create", json!({"type":"paint","name":"Folder/Accent","value":["#0f0"]})),
                    ]),
                    "nested-action" => f.colleague(&[
                        ("variable.update", json!({"variable":setup["variable"],"name":"away"})),
                        ("variable.create", json!({"collection":setup["collection"],"name":"count","type":"number","value":9})),
                    ]),
                    "history" => f.colleague(&[("version.rename", json!({"id":setup["version"],"name":"Colleague"}))]),
                    _ => f.colleague(&[("node.create", json!({"parent":f.frame,"type":"rectangle","width":10,"height":10}))]),
                }
            } else {
                f.colleague(&[("node.set", json!({"id":f.destination,"props":{"opacity":0.5}}))]);
            }
            let (command, params) = match family {
                "mode-copy" => ("variable.addMode", json!({"collection":setup["collection"],"name":"Copy"})),
                "default-mode" => ("variable.set", json!({"variable":setup["variable"],"value":3})),
                "style-name" => ("style.move", json!({"styles":["Folder/Accent"],"folder":"Moved"})),
                "folder" => ("style.renameFolder", json!({"folder":"Folder","name":"Moved"})),
                "nested-action" => (
                    "node.set",
                    json!({"id":f.a,"props":{"interactions":[{"trigger":{"type":"onClick"},"actions":[{"type":"setVariable","variable":setup["dest"],"value":0,"expr":{"value":{"type":"alias","value":"count"}}}]}]}}),
                ),
                "history" => ("version.rename", json!({"id":setup["version"],"name":"Agent"})),
                _ => ("node.set", json!({"id":f.frame,"props":{"width":900}})),
            };
            let answer = f.write(&basis, command, params).await;
            if (answer["error"]["code"] == "conflict") != necessary || (!necessary && answer.get("error").is_some()) {
                failures.push(format!("{family}, necessary={necessary}: {answer}"));
            }
            f.shutdown().await;
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[tokio::test(flavor = "multi_thread")]
async fn legacy_numeric_thread_and_node_ids_do_not_share_a_guard_identity() {
    for necessary in [true, false] {
        let f = Fixture::new(Guard::Premise);
        rewrite(&f, |d| d.execute("comment.add", json!({"text":"Original","x":10,"y":10})).unwrap());
        let opened = DesignDocument::open(FILE, &f.bytes()).unwrap();
        let mut legacy = opened.session().doc().unwrap().clone();
        legacy.comments[0].id = f.a;
        let bytes = kition_design_format::save_bytes(&legacy, None).unwrap();
        std::fs::write(f.workspace.path().join(FILE), bytes).unwrap();
        let thread = DesignDocument::open(FILE, &f.bytes()).unwrap().session().doc().unwrap().comments[0].id;
        let basis = f.basis().await;
        if necessary {
            f.colleague(&[("comment.edit", json!({"id":thread,"text":"Colleague"}))]);
        } else {
            f.colleague(&[("node.set", json!({"id":f.a,"props":{"opacity":0.5}}))]);
        }
        outcome(&f, &basis, "comment.edit", json!({"id":thread,"text":"Agent"}), necessary).await;
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn saved_presets_and_category_names_are_inputs_not_literals() {
    let mut failures = Vec::new();
    for family in ["preset", "category"] {
        for necessary in [true, false] {
            let f = Fixture::new(Guard::Premise);
            rewrite(&f, |d| {
                d.execute("fill.add", json!({"ids":[f.a],"paint":{"type":"image","image":"synthetic"}})).unwrap();
                d.execute("image.savePreset", json!({"name":"Review","filters":{"contrast":0.2}})).unwrap();
                d.execute("annotation.addCategory", json!({"name":"Review","color":"#f00"})).unwrap()
            });
            let basis = f.basis().await;
            if necessary {
                if family == "preset" {
                    f.colleague(&[("image.savePreset", json!({"name":"Review","filters":{"contrast":0.8}}))]);
                } else {
                    f.colleague(&[
                        ("annotation.editCategory", json!({"id":"Review","name":"Away"})),
                        ("annotation.addCategory", json!({"name":"Review"})),
                    ]);
                }
            } else {
                f.colleague(&[("node.set", json!({"id":f.b,"props":{"opacity":0.5}}))]);
            }
            let (command, params) = if family == "preset" {
                ("image.adjust", json!({"ids":[f.a],"preset":"Review"}))
            } else {
                ("annotation.add", json!({"id":f.a,"label":"Agent","category":"Review"}))
            };
            let answer = f.write(&basis, command, params).await;
            if (answer["error"]["code"] == "conflict") != necessary || (!necessary && answer.get("error").is_some()) {
                failures.push(format!("{family}, necessary={necessary}: {answer}"));
            }
            f.shutdown().await;
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[tokio::test(flavor = "multi_thread")]
async fn page_move_holds_the_destination_window() {
    for necessary in [true, false] {
        let f = Fixture::new(Guard::Premise);
        let pages = rewrite(&f, |d| {
            let first = d.inspect().unwrap()["page"].clone();
            d.execute("page.add", json!({"name":"Second"})).unwrap();
            let third = d.execute("page.add", json!({"name":"Third"})).unwrap()["id"].clone();
            json!({"first":first,"third":third})
        });
        let basis = f.basis().await;
        if necessary {
            f.colleague(&[("page.move", json!({"id":pages["third"],"index":1}))]);
        } else {
            f.colleague(&[("style.update", json!({"style":f.style,"description":"Unrelated"}))]);
        }
        outcome(&f, &basis, "page.move", json!({"id":pages["first"],"index":1}), necessary).await;
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn pinned_sdk_root_position_hold_also_conflicts_with_parent_size() {
    let f = Fixture::new(Guard::Premise);
    let basis = f.basis().await;
    f.colleague(&[("node.set", json!({"id":f.frame,"props":{"width":900}}))]);
    // SDK 1935d52 Need::position matches exact paths or their root only. Keeping the
    // required parent transform held means using `frame`, which also holds its width.
    let answer = f.write(&basis, "transform.move", json!({"ids":[f.a],"dx":5,"dy":0})).await;
    assert_eq!(answer["error"]["code"], "conflict", "SDK position limitation changed: {answer}");
    println!("SDK POSITION REPRO parent width alone refuses child move: {}", answer["conflicts"]);
    f.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn restoring_a_version_holds_its_snapshot_not_just_its_label() {
    for necessary in [true, false] {
        let f = Fixture::new(Guard::Premise);
        let version = rewrite(&f, |d| d.execute("file.saveVersion", json!({"name":"Original"})).unwrap())["id"].clone();
        let basis = f.basis().await;
        if necessary {
            let mut file = kition_design_format::load_bytes_with_versions(&f.bytes()).unwrap();
            let old = &file.versions[0];
            let mut snapshot = (*old.document().unwrap()).clone();
            snapshot.get_mut(kition_design_doc::NodeId(f.a)).unwrap().name = "Replaced snapshot".into();
            file.versions[0] = kition_design_format::Version::new(&old.id, &old.name, &old.description, old.time, std::sync::Arc::new(snapshot));
            let bytes = kition_design_format::save_bytes_with_replica(&file.doc, None, &file.versions, &file.branches, &file.replica).unwrap();
            std::fs::write(f.workspace.path().join(FILE), bytes).unwrap();
        } else {
            f.colleague(&[("comment.add", json!({"text":"Unrelated conversation","x":1,"y":1}))]);
        }
        outcome(&f, &basis, "version.restore", json!({"id":version}), necessary).await;
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn restoring_a_version_holds_the_styles_it_replaces() {
    for necessary in [true, false] {
        let f = Fixture::new(Guard::Premise);
        let version = rewrite(&f, |d| d.execute("file.saveVersion", json!({"name":"Original"})).unwrap())["id"].clone();
        let basis = f.basis().await;
        if necessary {
            f.colleague(&[("style.update", json!({"style":f.style,"description":"Colleague"}))]);
        } else {
            f.colleague(&[("comment.add", json!({"text":"Unrelated conversation","x":1,"y":1}))]);
        }
        outcome(&f, &basis, "version.restore", json!({"id":version}), necessary).await;
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn publishing_holds_the_previous_asset_snapshot_not_only_its_manifest() {
    for necessary in [true, false] {
        let f = Fixture::new(Guard::Premise);
        rewrite(&f, |d| {
            d.execute("library.publish", json!({})).unwrap();
            d.execute("object.rename", json!({"ids":[f.component],"name":"Pending publication"})).unwrap()
        });
        let basis = f.basis().await;
        if necessary {
            let mut file = kition_design_format::load_bytes_with_versions(&f.bytes()).unwrap();
            let publication = std::sync::Arc::make_mut(file.doc.published.as_mut().unwrap());
            let snapshot = std::sync::Arc::make_mut(&mut publication.doc);
            snapshot.styles.iter_mut().find(|s| json!(s.id) == f.style).unwrap().name = "Replaced snapshot".into();
            let bytes = kition_design_format::save_bytes_with_replica(&file.doc, None, &file.versions, &file.branches, &file.replica).unwrap();
            std::fs::write(f.workspace.path().join(FILE), bytes).unwrap();
        } else {
            f.colleague(&[("node.set", json!({"id":f.b,"props":{"opacity":0.5}}))]);
        }
        outcome(&f, &basis, "library.publish", json!({}), necessary).await;
        f.shutdown().await;
    }
}
