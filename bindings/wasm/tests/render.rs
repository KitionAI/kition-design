//! The agent's preview by explicit view (`file.render`, the kit's `agent.render`; task card
//! C4/C5/C6-design): a function of the design and the call alone. Its view is `target` (a page by id
//! or number, a layer, a frame or an area of a page), the first page without one, never the
//! current page; the same call answers the same bytes, whatever the current page, the selection,
//! the view, the tool or the number of threads.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use base64::Engine as _;
use kition_design_kit::DesignDocument;
use kition_kit::KitDocument;
use serde_json::{Value, json};

struct Design {
    doc: DesignDocument,
    page1: u64,
    page2: u64,
    frame2: u64,
    c: u64,
}

fn id_of(answer: &Value) -> u64 {
    answer["id"].as_u64().unwrap_or_else(|| panic!("no id in {answer}"))
}

/// Page 1: Frame 1 with a purple rectangle. Page 2 ("Second"): Frame 2, 300 x 200 at (1000, 0),
/// with a green rectangle C. The person is on page 2 with C selected, zoomed in, the Ellipse tool on.
fn design() -> Design {
    let mut doc = DesignDocument::create("Design.kdesign", 400, 300).unwrap();
    let inspected = doc.inspect().unwrap();
    let page1 = inspected["pages"][0]["id"].as_u64().unwrap();
    let frame1 = inspected["tree"][0]["id"].as_u64().unwrap();
    doc.execute(
        "node.add",
        json!({ "type": "rectangle", "parent": frame1, "x": 40, "y": 40, "width": 160, "height": 100, "props": { "fill": "#5645D4" } }),
    )
    .unwrap();
    doc.execute("page.add", json!({ "name": "Second" })).unwrap();
    let page2 = doc.inspect().unwrap()["pages"][1]["id"].as_u64().unwrap();
    let frame2 = id_of(
        &doc.execute("node.add", json!({ "type": "frame", "parent": page2, "x": 1000, "y": 0, "width": 300, "height": 200, "name": "Frame 2" }))
            .unwrap(),
    );
    let c = id_of(
        &doc.execute("node.add", json!({ "type": "rectangle", "parent": frame2, "x": 10, "y": 10, "width": 50, "height": 50, "name": "C", "props": { "fill": "#00AA44" } }))
            .unwrap(),
    );
    doc.execute("select.set", json!({ "ids": [c] })).unwrap();
    doc.execute("tool.ellipse", json!({})).unwrap();
    doc.set_viewport(10.0, 10.0, 640.0, 480.0, 3.0);
    Design { doc, page1, page2, frame2, c }
}

fn render(doc: &mut DesignDocument, params: Value) -> (Value, Vec<u8>) {
    let answer = doc.execute("file.render", params.clone()).unwrap_or_else(|error| panic!("{params}: {error}"));
    let png = base64::engine::general_purpose::STANDARD.decode(answer["png"].as_str().unwrap()).unwrap();
    (answer, png)
}

/// Views every test draws: the default, each page, a layer, a frame, an area, at two scales.
fn views(d: &Design) -> Vec<Value> {
    vec![
        json!({}),
        json!({ "target": { "pageId": d.page2 } }),
        json!({ "target": { "page": 1 } }),
        json!({ "target": { "objectId": d.c.to_string() } }),
        json!({ "target": { "frame": d.frame2 } }),
        json!({ "target": { "pageId": d.page2, "area": { "x": 1000, "y": 0, "width": 120, "height": 80 } }, "scale": 2 }),
        json!({ "scale": 0.5, "maxSize": 120 }),
    ]
}

#[test]
fn the_default_view_is_the_first_page_never_the_current_one() {
    let mut d = design();
    assert_eq!(d.doc.inspect().unwrap()["page"], json!(d.page2), "the person is on page 2");
    let (answer, _) = render(&mut d.doc, json!({}));
    assert_eq!((answer["page"].clone(), answer["pageId"].clone(), answer["drew"].clone()), (json!(1), json!(d.page1), json!("page")));
    assert_eq!((answer["width"].clone(), answer["height"].clone()), (json!(400), json!(300)));
    let (answer, _) = render(&mut d.doc, json!({ "target": { "pageId": d.page2 } }));
    assert_eq!((answer["page"].clone(), answer["pageName"].clone()), (json!(2), json!("Second")));
    assert_eq!(answer["bounds"], json!({ "x": 1000.0, "y": 0.0, "width": 300.0, "height": 200.0 }));
    let (answer, _) = render(&mut d.doc, json!({ "target": { "objectId": d.c } }));
    assert_eq!((answer["drew"].clone(), answer["objectId"].clone(), answer["pageId"].clone()), (json!("layer"), json!(d.c), json!(d.page2)));
    let (answer, _) = render(&mut d.doc, json!({ "target": { "pageId": d.page2, "area": { "x": 1000, "y": 0, "width": 120, "height": 80 } } }));
    assert_eq!((answer["drew"].clone(), answer["width"].clone(), answer["height"].clone()), (json!("area"), json!(120), json!(80)));
}

#[test]
fn the_same_call_draws_the_same_bytes_whatever_the_editor_does() {
    let mut d = design();
    let first: Vec<Vec<u8>> = views(&d).into_iter().map(|view| render(&mut d.doc, view).1).collect();
    let again: Vec<Vec<u8>> = views(&d).into_iter().map(|view| render(&mut d.doc, view).1).collect();
    assert_eq!(first, again, "two calls answer the same bytes");
    let editor = d.doc.editor_state();
    d.doc.execute("page.select", json!({ "id": d.page1 })).unwrap();
    d.doc.execute("edit.selectAll", json!({})).unwrap();
    d.doc.execute("tool.rectangle", json!({})).unwrap();
    d.doc.set_viewport(-500.0, 200.0, 300.0, 200.0, 0.25);
    let moved: Vec<Vec<u8>> = views(&d).into_iter().map(|view| render(&mut d.doc, view).1).collect();
    assert_eq!(first, moved, "the current page, the selection, the view and the tool change nothing drawn");
    assert_ne!(d.doc.editor_state(), editor);
}

#[test]
fn the_same_bytes_on_one_thread_and_on_many() {
    let d = design();
    let mut big = d.doc;
    // A view large enough that a banded renderer would split it.
    let call = json!({ "target": { "pageId": d.page2, "area": { "x": 900, "y": -100, "width": 1500, "height": 1000 } }, "scale": 1 });
    let mut on = |threads: usize| {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
        pool.install(|| render(&mut big, call.clone()).1)
    };
    let one = on(1);
    assert_eq!(one, on(8));
    assert_eq!(one, on(3));
}

#[test]
fn scale_and_size_are_kept() {
    let mut d = design();
    let call = |scale: f64| json!({ "target": { "frame": d.frame2 }, "scale": scale });
    let (one, _) = render(&mut d.doc, call(1.0));
    let (two, _) = render(&mut d.doc, call(2.0));
    assert_eq!((one["width"].clone(), two["width"].clone(), two["scale"].clone()), (json!(300), json!(600), json!(2.0)));
    let (fit, _) = render(&mut d.doc, json!({ "target": { "frame": d.frame2 }, "scale": 2, "maxSize": 150 }));
    assert_eq!((fit["width"].clone(), fit["height"].clone(), fit["scale"].clone()), (json!(150), json!(100), json!(0.5)));
}

#[test]
fn refusals_are_typed_and_say_what_is_taken() {
    let mut d = design();
    let refusals = [
        (json!({ "target": { "time": 3 } }), "invalid-params:", "pageId"),
        (json!({ "target": { "pageId": 999_999 } }), "not-found:", "page"),
        (json!({ "target": { "pageId": d.c } }), "invalid-params:", "not a page"),
        (json!({ "target": { "objectId": 999_999 } }), "not-found:", "layer"),
        (json!({ "target": { "objectId": d.c, "pageId": d.page1 } }), "invalid-params:", "page"),
        (json!({ "target": { "frame": d.c } }), "invalid-params:", "frame"),
        (json!({ "target": { "area": { "x": 0, "y": 0, "width": -5, "height": 10 } } }), "invalid-params:", "area"),
        (json!({ "target": { "page": 3 } }), "not-found:", "2 page"),
        (json!({ "scale": 2.5 }), "invalid-params:", "scale"),
        (json!({ "maxSize": 4097 }), "invalid-params:", "maxSize"),
    ];
    for (call, code, says) in refusals {
        let error = d.doc.execute("file.render", call.clone()).unwrap_err();
        assert!(error.starts_with(code) && error.contains(says), "{call}: {error}");
    }
}

#[test]
fn the_render_command_declares_its_view() {
    let d = design();
    let command = d.doc.commands().into_iter().find(|c| c.id == "file.render").unwrap();
    assert!(command.reads.is_empty(), "it reads no editor state");
    let schema = command.schema.unwrap();
    let members: Vec<&String> = schema["target"]["properties"].as_object().unwrap().keys().collect();
    for member in ["page", "pageId", "objectId", "path", "frame", "area"] {
        assert!(members.iter().any(|m| *m == member), "target takes {member}: {members:?}");
    }
}
