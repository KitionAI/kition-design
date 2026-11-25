//! C8-land-11-design: another program rewrites the file between observation and writing.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;
use kition_kits::tools::{self, CallOptions, Guard};
use kition_kits::{KitDefinition, KitManager};
use serde_json::{Value, json};

const FILE: &str = "Outside.kdesign";

#[path = "outside/batch_read.rs"]
mod batch_read;
#[path = "outside/c8_3.rs"]
mod c8_3;
#[path = "outside/expectations_review.rs"]
mod expectations_review;
#[path = "outside/precision_read.rs"]
mod precision_read;
#[path = "outside/read.rs"]
mod read;
#[path = "outside/resource_read.rs"]
mod resource_read;
#[path = "outside/review.rs"]
mod review;
#[path = "outside/review2.rs"]
mod review2;
#[path = "outside/sdk_waiting.rs"]
mod sdk_waiting;

struct Fixture {
    kits: KitManager,
    workspace: tempfile::TempDir,
    options: CallOptions,
    frame: u64,
    a: u64,
    b: u64,
    destination: u64,
    component: u64,
    instance: u64,
    style: Value,
}

impl Fixture {
    fn new(guard: Guard) -> Self {
        let workspace = tempfile::tempdir().unwrap();
        let mut doc = DesignDocument::create(FILE, 800, 600).unwrap();
        let frame = doc.inspect().unwrap()["tree"][0]["id"].as_u64().unwrap();
        let mut rect = |name, x| {
            doc.execute("node.add", json!({"type":"rectangle", "parent":frame, "name":name, "x":x,"y":20,"width":40,"height":40})).unwrap()["id"]
                .as_u64()
                .unwrap()
        };
        let a = rect("A", 20);
        let b = rect("B", 100);
        let page = doc.inspect().unwrap()["page"].clone();
        let destination =
            doc.execute("frame.add", json!({"parent":page,"name":"Destination","x":500,"y":400,"width":100,"height":100})).unwrap()["id"]
                .as_u64()
                .unwrap();
        for i in 0..3 {
            doc.execute("node.add", json!({"type":"rectangle","parent":destination,"name":format!("D{i}"),"x":i*20,"y":0,"width":10,"height":10}))
                .unwrap();
        }
        let component =
            doc.execute("frame.add", json!({"parent":page,"name":"Source","x":500,"y":200,"width":60,"height":60})).unwrap()["id"].as_u64().unwrap();
        doc.execute("component.add", json!({"ids":[component]})).unwrap();
        let instance = doc.execute("instance.add", json!({"component":component,"parent":frame,"x":200,"y":20})).unwrap()["id"].as_u64().unwrap();
        let style = doc.execute("style.add", json!({"type":"paint","name":"Accent","value":["#f00"]})).unwrap()["id"].clone();
        std::fs::write(workspace.path().join(FILE), doc.save_file(None).unwrap()).unwrap();
        let kit = KitDefinition::from_manifest_text(include_str!("../../../kit.json")).unwrap();
        let kits = KitManager::new(Some(workspace.path().to_path_buf()), vec![(kit, Some(PathBuf::from(env!("CARGO_BIN_EXE_kition-design"))))]);
        Self { kits, workspace, options: CallOptions { guard, ..CallOptions::default() }, frame, a, b, destination, component, instance, style }
    }

    fn bytes(&self) -> Vec<u8> {
        std::fs::read(self.workspace.path().join(FILE)).unwrap()
    }

    fn colleague(&self, commands: &[(&str, Value)]) {
        let mut writer = DesignDocument::open(FILE, &self.bytes()).unwrap();
        for (command, params) in commands {
            writer.execute(command, params.clone()).unwrap_or_else(|error| panic!("colleague {command} {params}: {error}"));
        }
        std::fs::write(self.workspace.path().join(FILE), writer.save_file(None).unwrap()).unwrap();
    }

    async fn call(&self, action: &str, mut input: Value) -> Value {
        input["path"] = json!(FILE);
        match tools::call_as(&self.kits, &format!("design_{action}"), &input, self.options, "observer").await.unwrap() {
            Ok(output) => output.output,
            Err(error) => error.structured(),
        }
    }

    async fn basis(&self) -> Value {
        let seen = self.call("inspect", json!({})).await;
        assert!(seen.get("error").is_none(), "{seen}");
        assert!(seen["revision"].is_string(), "{seen}");
        seen["revision"].clone()
    }

    async fn write(&self, basis: &Value, command: &str, params: Value) -> Value {
        self.call("run", json!({"basis":basis,"steps":[{"command":command,"params":params}]})).await
    }

    fn node(&self, id: u64) -> Value {
        let doc = DesignDocument::open(FILE, &self.bytes()).unwrap();
        serde_json::to_value(doc.session().doc().unwrap().get(kition_design_doc::NodeId(id)).unwrap()).unwrap()
    }

    async fn shutdown(self) {
        self.kits.shutdown().await;
    }
}

fn applied(answer: &Value) {
    assert!(answer.get("error").is_none(), "{answer}");
}

fn differences(left: &Value, right: &Value, path: &str, out: &mut Vec<String>) {
    if left == right || out.len() >= 5 {
        return;
    }
    match (left, right) {
        (Value::Object(a), Value::Object(b)) => {
            let keys: std::collections::BTreeSet<_> = a.keys().chain(b.keys()).collect();
            for key in keys {
                differences(&left[key], &right[key], &format!("{path}.{key}"), out);
            }
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                differences(a, b, &format!("{path}.{i}"), out);
            }
        }
        _ => out.push(format!("{path}: {left} != {right}")),
    }
}

fn conflict(answer: &Value, id: u64, reason: &str) {
    assert_eq!(answer["error"]["code"], "conflict", "{answer}");
    assert!(answer["conflicts"].as_array().unwrap().iter().any(|c| c["id"] == id && c["reason"] == reason), "{answer}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_scalar_id_is_held_after_the_file_changes() {
    let f = Fixture::new(Guard::Premise);
    let basis = f.basis().await;
    f.colleague(&[("node.set", json!({"id":f.a,"props":{"name":"Colleague"}}))]);
    let bytes = f.bytes();
    let answer = f.write(&basis, "node.set", json!({"id":f.a,"props":{"name":"Agent"}})).await;
    conflict(&answer, f.a, "changed");
    assert_eq!(f.bytes(), bytes, "a refused write never saves");
    assert_eq!(f.node(f.a)["name"], "Colleague");
    f.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unrelated_field_on_the_target_applies_and_others_tells_only_that_field() {
    let f = Fixture::new(Guard::Premise);
    let basis = f.basis().await;
    f.colleague(&[("node.set", json!({"id":f.a,"props":{"opacity":0.5}}))]);
    let answer = f.write(&basis, "node.set", json!({"id":f.a,"props":{"name":"Agent"}})).await;
    applied(&answer);
    let others = answer["others"]["changed"].as_array().unwrap();
    assert_eq!(others.len(), 1, "{answer}");
    assert_eq!(others[0]["id"], f.a, "{answer}");
    assert_eq!(others[0]["fields"], json!(["opacity"]), "{answer}");
    assert_eq!(f.node(f.a)["name"], "Agent");
    f.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_deleted_target_is_refused() {
    let f = Fixture::new(Guard::Premise);
    let basis = f.basis().await;
    f.colleague(&[("edit.delete", json!({"ids":[f.a]}))]);
    conflict(&f.write(&basis, "node.set", json!({"id":f.a,"props":{"name":"Agent"}})).await, f.a, "deleted");
    f.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn inserted_before_an_index_is_refused_but_inserted_after_it_applies() {
    for before in [true, false] {
        let f = Fixture::new(Guard::Premise);
        let basis = f.basis().await;
        let mut writer = DesignDocument::open(FILE, &f.bytes()).unwrap();
        let created = writer.execute("node.add", json!({"type":"rectangle","parent":f.destination,"width":10,"height":10})).unwrap()["id"].clone();
        if before {
            writer.execute("node.reorder", json!({"id":created,"index":0})).unwrap();
        }
        std::fs::write(f.workspace.path().join(FILE), writer.save_file(None).unwrap()).unwrap();
        let answer = f.write(&basis, "node.reparent", json!({"ids":[f.a],"parent":f.destination,"index":1})).await;
        assert_eq!(answer["error"]["code"] == "conflict", before, "before={before} {answer}");
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_parent_move_invalidates_world_motion_but_not_a_rename() {
    let f = Fixture::new(Guard::Premise);
    let basis = f.basis().await;
    f.colleague(&[("transform.move", json!({"ids":[f.frame],"dx":300,"dy":0}))]);
    let answer = f.write(&basis, "transform.move", json!({"ids":[f.a],"dx":5,"dy":0})).await;
    assert_eq!(answer["error"]["code"], "conflict", "{answer}");
    applied(&f.write(&basis, "object.rename", json!({"ids":[f.a],"name":"Agent"})).await);
    f.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_moved_point_target_is_refused_but_its_id_still_names_it() {
    let f = Fixture::new(Guard::Premise);
    let basis = f.basis().await;
    f.colleague(&[("transform.move", json!({"ids":[f.instance],"dx":100,"dy":0}))]);
    let answer = f.write(&basis, "instance.swapAt", json!({"component":f.component,"x":210,"y":30})).await;
    assert_eq!(answer["error"]["code"], "conflict", "{answer}");
    applied(&f.write(&basis, "object.rename", json!({"ids":[f.instance],"name":"Agent"})).await);
    f.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn renamed_away_and_swapped_resource_names_are_refused() {
    for swap in [false, true] {
        let f = Fixture::new(Guard::Premise);
        let basis = f.basis().await;
        f.colleague(&[("style.set", json!({"style":f.style,"name":"Away"}))]);
        if swap {
            f.colleague(&[("style.add", json!({"type":"paint","name":"Accent","value":["#0f0"]}))]);
        }
        let answer = f.write(&basis, "style.set", json!({"style":"Accent","description":"Agent"})).await;
        assert_eq!(answer["error"]["code"], "conflict", "{answer}");
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn guard_modes_distinguish_unrelated_and_overlapping_changes() {
    for (guard, overlap, refused) in
        [(Guard::Premise, false, false), (Guard::Premise, true, true), (Guard::Epoch, false, true), (Guard::Off, true, false)]
    {
        let f = Fixture::new(guard);
        let basis = f.basis().await;
        let target = if overlap { f.a } else { f.b };
        f.colleague(&[("object.rename", json!({"ids":[target],"name":"Colleague"}))]);
        let answer = f.write(&basis, "object.rename", json!({"ids":[f.a],"name":"Agent"})).await;
        assert_eq!(answer["error"]["code"] == "conflict", refused, "{guard:?} {answer}");
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn fifty_unrelated_rounds() {
    for guard in [Guard::Premise, Guard::Epoch] {
        let f = Fixture::new(guard);
        let mut refused = 0;
        for round in 0..50 {
            let basis = f.basis().await;
            f.colleague(&[("object.rename", json!({"ids":[f.b],"name":format!("Colleague {round}")}))]);
            let answer = f.write(&basis, "object.rename", json!({"ids":[f.a],"name":format!("Agent {round}")})).await;
            if answer["error"]["code"] == "conflict" {
                refused += 1;
            } else {
                applied(&answer);
            }
        }
        println!("UNRELATED {guard:?} applied={} refused={refused}", 50 - refused);
        assert_eq!(refused, if matches!(guard, Guard::Epoch) { 50 } else { 0 });
        f.shutdown().await;
    }
}

// The necessary-conflict rules are fixed before running the table:
// id rename: A was renamed/deleted; position and unrelated fields do not matter.
// name description: Accent was renamed/reassigned or its description changed.
// point swap: the instance at (210,30), its source component, or its parent's placement changed.
// structural reparent: A was deleted/moved, or either parent's transform/hierarchy changed.
// An applied write is also compared with executing it on the colleague's saved file.
#[tokio::test(flavor = "multi_thread")]
async fn fixed_seed_mixed_table() {
    let mut seed = 0x9e3779b97f4a7c15u64;
    let mut changes: Vec<_> = (0..50).map(|i| i % 10).collect();
    for i in (1..changes.len()).rev() {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        changes.swap(i, (seed as usize) % (i + 1));
    }
    let mut counts = [0usize; 5];
    let mut cells = [[[0usize; 5]; 4]; 10];
    for (round, change) in changes.into_iter().enumerate() {
        for form in 0..4 {
            let f = Fixture::new(Guard::Premise);
            let basis = f.basis().await;
            let (kind, colleague, colleague_params) = match change {
                0 => ("B-name", "object.rename", json!({"ids":[f.b],"name":"Colleague"})),
                1 => ("A-opacity", "node.set", json!({"id":f.a,"props":{"opacity":0.5}})),
                2 => ("A-name", "object.rename", json!({"ids":[f.a],"name":"Colleague"})),
                3 => ("A-delete", "edit.delete", json!({"ids":[f.a]})),
                4 => ("A-move", "transform.move", json!({"ids":[f.a],"dx":10,"dy":0})),
                5 => ("parent-move", "transform.move", json!({"ids":[f.frame],"dx":300,"dy":0})),
                6 => ("style-name", "style.set", json!({"style":f.style,"name":"Away"})),
                7 => ("point-move", "transform.move", json!({"ids":[f.instance],"dx":100,"dy":0})),
                8 => ("style-description", "style.set", json!({"style":f.style,"description":"Colleague"})),
                _ => ("destination-move", "transform.move", json!({"ids":[f.destination],"dx":10,"dy":0})),
            };
            let (write, command, params, necessary) = match form {
                0 => ("id", "node.set", json!({"id":f.a,"props":{"name":"Agent"}}), matches!(change, 2 | 3)),
                1 => ("name", "style.set", json!({"style":"Accent","description":"Agent"}), matches!(change, 6 | 8)),
                2 => ("point", "instance.swapAt", json!({"component":f.component,"x":210,"y":30}), matches!(change, 5 | 7)),
                _ => ("structure", "node.reparent", json!({"ids":[f.a],"parent":f.destination}), matches!(change, 3 | 4 | 5 | 9)),
            };
            f.colleague(&[(colleague, colleague_params)]);
            let before = f.bytes();
            let mut oracle = DesignDocument::open(FILE, &before).unwrap();
            let expected = oracle.execute(command, params.clone());
            let answer = f.write(&basis, command, params).await;
            let class = if answer["error"]["code"] == "conflict" {
                assert_eq!(f.bytes(), before);
                if necessary { 1 } else { 2 }
            } else if answer.get("error").is_some() {
                4
            } else if necessary {
                3
            } else {
                let saved = DesignDocument::open(FILE, &f.bytes()).unwrap();
                let (mut actual, mut expected_doc) =
                    (serde_json::to_value(saved.session().doc().unwrap()).unwrap(), serde_json::to_value(oracle.session().doc().unwrap()).unwrap());
                // Opening allocates a fresh client id range; next_id is allocator state, not KDM content.
                actual.as_object_mut().unwrap().remove("next_id");
                expected_doc.as_object_mut().unwrap().remove("next_id");
                let mut diff = Vec::new();
                differences(&actual, &expected_doc, "doc", &mut diff);
                if !diff.is_empty() {
                    println!("ORACLE {diff:?}");
                }
                if expected.is_err() || actual != expected_doc { 3 } else { 0 }
            };
            counts[class] += 1;
            cells[change][form][class] += 1;
            println!(
                "MIXED {:02} {kind} {write} necessary={necessary} {}",
                round + 1,
                ["applied", "refused-necessary", "refused-false", "applied-wrongly", "failed-at-kit"][class]
            );
            if class >= 2 {
                println!("MIXED-DETAIL {answer}");
            }
            f.shutdown().await;
        }
    }
    for (change, row) in cells.iter().enumerate() {
        for (form, counts) in row.iter().enumerate() {
            println!(
                "MIXED CELL change={change} form={form} applied={} necessary={} false={} wrong={} failed={}",
                counts[0], counts[1], counts[2], counts[3], counts[4]
            );
        }
    }
    println!(
        "MIXED TOTAL applied={} refused-necessary={} refused-false={} applied-wrongly={} failed-at-kit={}",
        counts[0], counts[1], counts[2], counts[3], counts[4]
    );
    assert_eq!(counts[2..], [0, 0, 0]);
}
