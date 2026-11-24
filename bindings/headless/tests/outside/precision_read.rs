//! C8-design-read: branch-specific text and vector dependencies, checked against a real writer.
use super::*;

fn prepare(f: &Fixture, edit: impl FnOnce(&mut DesignDocument) -> Value) -> Value {
    let mut writer = DesignDocument::open(FILE, &f.bytes()).unwrap();
    let made = edit(&mut writer);
    std::fs::write(f.workspace.path().join(FILE), writer.seal_and_save(None).unwrap()).unwrap();
    made
}

fn step(command: &str, params: Value) -> Value {
    json!({"command":command,"params":params})
}

async fn case(
    setup: impl Fn(&Fixture) -> Value,
    steps: impl Fn(&Fixture, &Value) -> Vec<Value>,
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
        let answer = f.call("run", json!({"basis":basis,"steps":steps(&f, &made)})).await;
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
async fn caret_motion_reads_fixed_text_content_and_not_box_height() {
    case(
        |f| prepare(f, |d| {
            d.execute("text.create", json!({"parent":f.frame,"text":"abcd","width":100,"props":{"textAutoResize":"none","height":30}})).unwrap()["id"].clone()
        }),
        |_, text| vec![step("text.edit.begin", json!({"id":text,"caret":2})), step("text.edit.key", json!({"key":"Left"}))],
        |_, text| vec![("node.set", json!({"id":text,"props":{"characters":"efgh"}}))],
        |_, text| vec![("node.set", json!({"id":text,"props":{"height":80}}))],
    ).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn text_styling_outside_auto_layout_does_not_read_minimum_width() {
    case(
        |f| prepare(f, |d| d.execute("text.create", json!({"parent":f.frame,"text":"abcd"})).unwrap()["id"].clone()),
        |_, text| vec![step("text.bold", json!({"ids":[text]}))],
        |_, text| vec![("node.set", json!({"id":text,"props":{"characters":"efgh"}}))],
        |_, text| vec![("node.set", json!({"id":text,"props":{"minWidth":300}}))],
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn drawing_a_rectangle_reads_its_parent_geometry_and_not_paint() {
    case(
        |_| Value::Null,
        |_, _| vec![step("tool.place", json!({"tool":"rectangle","x":550,"y":450}))],
        |f, _| vec![("transform.move", json!({"ids":[f.destination],"dx":100}))],
        |f, _| vec![("node.set", json!({"id":f.destination,"props":{"fill":"#123456"}}))],
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_local_paste_reads_its_copied_main_and_not_unrelated_library_links() {
    case(
        |f| {
            let mut library = DesignDocument::create("Unrelated.kdesign", 100, 100).unwrap();
            let main = library.inspect().unwrap()["tree"][0]["id"].clone();
            library.execute("component.create", json!({"ids":[main]})).unwrap();
            library.execute("library.publish", json!({})).unwrap();
            let bytes = library.seal_and_save(None).unwrap();
            std::fs::write(f.workspace.path().join("Unrelated.kdesign"), &bytes).unwrap();
            prepare(f, |d| {
                d.lend_library("Unrelated.kdesign", Some(&bytes)).unwrap();
                d.execute("library.add", json!({"path":"Unrelated.kdesign"})).unwrap()["id"].clone()
            })
        },
        |f, _| vec![step("edit.copy", json!({"ids":[f.component]})), step("edit.paste", json!({"parent":f.destination}))],
        |f, _| vec![("node.set", json!({"id":f.component,"props":{"opacity":0.5}}))],
        |_, link| vec![("library.remove", json!({"id":link}))],
    )
    .await;
}

fn fitted_text(f: &Fixture) -> Value {
    prepare(f, |d| {
        d.execute("layout.add", json!({"ids":[f.destination],"direction":"vertical"})).unwrap();
        d.execute("text.create", json!({"parent":f.destination,"text":"Fitted"})).unwrap()["id"].clone()
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn refitting_fitted_text_reads_content_and_not_sibling_order() {
    case(
        fitted_text,
        |_, text| vec![step("text.refit", json!({"ids":[text]}))],
        |_, text| vec![("node.set", json!({"id":text,"props":{"characters":"Changed"}}))],
        |_, text| vec![("node.reorder", json!({"id":text,"index":0}))],
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn refitting_a_container_reads_text_membership_and_not_the_container_box() {
    case(
        fitted_text,
        |f, _| vec![step("text.refit", json!({"ids":[f.destination]}))],
        |_, text| vec![("node.set", json!({"id":text,"props":{"characters":"Changed"}}))],
        |f, _| vec![("node.set", json!({"id":f.destination,"props":{"width":300}}))],
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn refitting_a_container_holds_text_membership_and_not_non_text_membership() {
    case(
        fitted_text,
        |f, _| vec![step("text.refit", json!({"ids":[f.destination]}))],
        |f, _| vec![("text.create", json!({"parent":f.destination,"text":"New text"}))],
        |f, _| vec![("shape.rectangle", json!({"parent":f.destination,"width":20,"height":20}))],
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn outlining_an_open_path_reads_its_stroke_and_not_its_fill() {
    case(
        |f| {
            prepare(f, |d| {
                let vector = d.execute("vector.create", json!({"parent":f.frame,"path":"M0 0 L100 0","closed":false})).unwrap()["id"].clone();
                d.execute("stroke.add", json!({"ids":[vector]})).unwrap();
                vector
            })
        },
        |_, vector| vec![step("vector.outlineStroke", json!({"ids":[vector]}))],
        |_, vector| vec![("node.set", json!({"id":vector,"props":{"strokeWeight":8}}))],
        |_, vector| vec![("node.set", json!({"id":vector,"props":{"fills":["#ff0000"]}}))],
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_shape_merge_reads_the_first_picked_regions_style() {
    case(
        |f| {
            prepare(f, |d| {
                let first = d.execute("shape.rectangle", json!({"parent":f.frame,"x":20,"y":300,"width":100,"height":60})).unwrap()["id"].clone();
                let second = d.execute("shape.ellipse", json!({"parent":f.frame,"x":20,"y":400,"width":40,"height":40})).unwrap()["id"].clone();
                json!([first, second])
            })
        },
        |_, shapes| vec![step("shapeBuilder.merge", json!({"ids":shapes,"points":[[50,330],[40,420]]}))],
        |_, shapes| vec![("node.set", json!({"id":shapes[0],"props":{"opacity":0.5}}))],
        |_, shapes| vec![("node.set", json!({"id":shapes[1],"props":{"opacity":0.5}}))],
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn removing_a_stroke_region_reads_the_remainders_appearance() {
    case(
        |f| {
            prepare(f, |d| {
                let line =
                    d.execute("vector.create", json!({"parent":f.frame,"x":300,"y":300,"path":"M0 0 L100 0","closed":false})).unwrap()["id"].clone();
                d.execute("stroke.add", json!({"ids":[line]})).unwrap();
                d.execute("node.set", json!({"id":line,"props":{"strokeWeight":10}})).unwrap();
                let crossing = d.execute("shape.rectangle", json!({"parent":f.frame,"x":340,"y":280,"width":20,"height":40})).unwrap()["id"].clone();
                json!([line, crossing])
            })
        },
        |_, shapes| vec![step("shapeBuilder.remove", json!({"ids":shapes,"points":[[320,300]]}))],
        |_, shapes| vec![("node.set", json!({"id":shapes[0],"props":{"opacity":0.5}}))],
        |_, shapes| vec![("node.set", json!({"id":shapes[0],"props":{"fills":["#ff0000"]}}))],
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn shape_builder_holds_a_groups_sources_when_deletion_keeps_its_bounds() {
    case(
        |f| {
            prepare(f, |d| {
                let first = d.execute("shape.rectangle", json!({"parent":f.frame,"x":300,"y":330,"width":80,"height":40})).unwrap()["id"].clone();
                let second = d.execute("shape.rectangle", json!({"parent":f.frame,"x":300,"y":330,"width":80,"height":40})).unwrap()["id"].clone();
                let group = d.execute("object.group", json!({"ids":[first,second]})).unwrap()["id"].clone();
                json!({"group":group,"second":second})
            })
        },
        |_, made| vec![step("shapeBuilder.merge", json!({"ids":[made["group"]],"points":[[320,350]]}))],
        |_, made| vec![("edit.delete", json!({"ids":[made["second"]]}))],
        |_, made| vec![("object.rename", json!({"ids":[made["group"]],"name":"Colleague"}))],
    )
    .await;
}
