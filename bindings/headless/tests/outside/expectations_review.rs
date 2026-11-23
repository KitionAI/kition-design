//! C8-expectations review design: independent file rewrites of omitted handler inputs.
use super::*;

fn prepare(f: &Fixture, edit: impl FnOnce(&mut DesignDocument) -> Value) -> Value {
    let mut writer = DesignDocument::open(FILE, &f.bytes()).unwrap();
    let value = edit(&mut writer);
    std::fs::write(f.workspace.path().join(FILE), writer.save(None).unwrap()).unwrap();
    value
}

#[tokio::test(flavor = "multi_thread")]
async fn review_grouping_holds_the_kind_used_to_reject_sections() {
    let f = Fixture::new(Guard::Premise);
    let other = prepare(&f, |d| {
        let page = d.inspect().unwrap()["page"].clone();
        d.execute("shape.rectangle", json!({"parent":page,"x":900,"y":600,"width":20,"height":20})).unwrap()["id"].clone()
    });
    let basis = f.basis().await;
    f.colleague(&[("object.convertToSection", json!({"ids":[f.destination]}))]);
    let before = f.bytes();
    let answer = f.write(&basis, "object.group", json!({"ids":[f.destination,other]})).await;
    assert_eq!(answer["error"]["code"], "conflict", "grouping tests whether a target is a section: {}", answer["error"]);
    assert_eq!(f.bytes(), before);
    f.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn review_board_commands_hold_the_file_kind_they_require() {
    let mut failures = Vec::new();
    for command in ["board.label", "tool.stamp"] {
        let f = Fixture::new(Guard::Premise);
        let mut file = kition_design_format::load_bytes_with_branches(&f.bytes()).unwrap();
        file.doc.kind = kition_design_doc::FileKind::Board;
        std::fs::write(f.workspace.path().join(FILE), kition_design_format::save_edited(&file, None).unwrap()).unwrap();
        let made = prepare(&f, |d| d.execute("board.connector", json!({"from":{"x":0,"y":0},"to":{"x":100,"y":0},"label":"Before"})).unwrap());
        let basis = f.basis().await;
        // A separate native-file writer changes this persisted precondition.
        let mut file = kition_design_format::load_bytes_with_branches(&f.bytes()).unwrap();
        file.doc.kind = kition_design_doc::FileKind::Design;
        std::fs::write(f.workspace.path().join(FILE), kition_design_format::save_edited(&file, None).unwrap()).unwrap();
        let before = f.bytes();
        let steps = if command == "board.label" {
            json!([{"command":command,"params":{"id":made["id"],"text":"Agent"}}])
        } else {
            json!([{"command":"vector.edit.begin","params":{"id":f.a}}, {"command":command,"params":{}}])
        };
        let answer = f.call("run", json!({"basis":basis,"steps":steps})).await;
        if answer["error"]["code"] != "conflict" {
            failures.push(format!("{command}: {}", answer["error"]));
        }
        assert_eq!(f.bytes(), before);
        f.shutdown().await;
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[tokio::test(flavor = "multi_thread")]
async fn review_publication_holds_the_document_name_its_snapshot_uses() {
    for explicit in [true, false] {
        let f = Fixture::new(Guard::Premise);
        let basis = f.basis().await;
        f.colleague(&[("file.rename", json!({"name":"Colleague library"}))]);
        let before = f.bytes();
        let params = if explicit { json!({"include":[f.component]}) } else { json!({}) };
        let answer = f.write(&basis, "library.publish", params).await;
        assert_eq!(answer["error"]["code"], "conflict", "publication copies the file name: {}", answer["error"]);
        assert_eq!(f.bytes(), before);
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn review_a_new_image_preset_holds_the_capacity_it_checks() {
    for adding in [true, false] {
        let f = Fixture::new(Guard::Premise);
        // The reader can append Mine. A second writer either fills the final slot or
        // changes an independent existing record without changing capacity.
        let mut file = kition_design_format::load_bytes_with_branches(&f.bytes()).unwrap();
        file.doc.image_presets =
            (0..499).map(|i| kition_design_doc::ImagePreset { name: format!("Other {i}"), filters: Default::default() }).collect();
        std::fs::write(f.workspace.path().join(FILE), kition_design_format::save_edited(&file, None).unwrap()).unwrap();
        let basis = f.basis().await;
        let name = if adding { "Final slot" } else { "Other 0" };
        f.colleague(&[("image.savePreset", json!({"name":name,"filters":{"exposure":0.5}}))]);
        let before = f.bytes();
        let answer = f.write(&basis, "image.savePreset", json!({"name":"Mine","filters":{"exposure":0.2}})).await;
        if adding {
            assert_eq!(answer["error"]["code"], "conflict", "capacity changed: {answer}");
            assert_eq!(f.bytes(), before, "refusal must leave the independent writer's file intact");
        } else {
            applied(&answer);
            let file = kition_design_format::load_bytes_with_branches(&f.bytes()).unwrap();
            assert_eq!(file.doc.image_presets.len(), 500);
            assert_eq!(file.doc.image_presets.iter().find(|p| p.name == "Other 0").unwrap().filters.exposure, 0.5);
            assert_eq!(file.doc.image_presets.iter().find(|p| p.name == "Mine").unwrap().filters.exposure, 0.2);
        }
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn review_an_id_addressed_swap_reads_the_instances_variable_mode() {
    let f = Fixture::new(Guard::Premise);
    let made = prepare(&f, |d| {
        let collection = d.execute("variable.createCollection", json!({"name":"Opacity modes","modes":["Light","Dark"]})).unwrap()["id"].clone();
        let variable = d
            .execute("variable.create", json!({"collection":collection,"name":"opacity","type":"number","values":{"Light":0.2,"Dark":0.8}}))
            .unwrap()["id"]
            .clone();
        let main = d.execute("frame.create", json!({"name":"Bound main","x":900,"y":100})).unwrap()["id"].clone();
        d.execute("component.create", json!({"ids":[main]})).unwrap();
        d.execute("variable.bind", json!({"ids":[main],"variable":variable,"field":"opacity"})).unwrap();
        json!({"collection":collection,"main":main})
    });
    let basis = f.basis().await;
    f.colleague(&[("variable.setMode", json!({"ids":[f.instance],"collection":made["collection"],"mode":"Dark"}))]);
    let before = f.bytes();
    let answer = f.write(&basis, "instance.swap", json!({"ids":[f.instance],"component":made["main"]})).await;
    assert_eq!(answer["error"]["code"], "conflict", "the new main resolves its opacity in the preserved instance mode: {answer}");
    assert_eq!(f.bytes(), before);
    f.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn review_column_removal_keeps_the_original_text_edit_control() {
    let f = Fixture::new(Guard::Premise);
    let mut file = kition_design_format::load_bytes_with_branches(&f.bytes()).unwrap();
    file.doc.kind = kition_design_doc::FileKind::Board;
    std::fs::write(f.workspace.path().join(FILE), kition_design_format::save_edited(&file, None).unwrap()).unwrap();
    let table = prepare(&f, |board| board.execute("board.table", json!({"cells":[["a","b"],["c","d"]],"x":0,"y":0})).unwrap());
    let text = table["cells"][0][1].clone();
    let basis = f.basis().await;
    // The previous executor replaced this exact kind of interference with removal of
    // column zero. Retain it as a negative control: deletion never consumes its text.
    f.colleague(&[("node.set", json!({"id":text,"props":{"characters":"typed"}}))]);
    let answer = f.write(&basis, "board.tableEdit", json!({"id":table["id"],"action":"removeColumn","index":1})).await;
    applied(&answer);
    let file = kition_design_format::load_bytes_with_branches(&f.bytes()).unwrap();
    assert!(!file.doc.contains(kition_design_doc::NodeId(text.as_u64().unwrap())));
    f.shutdown().await;
}
