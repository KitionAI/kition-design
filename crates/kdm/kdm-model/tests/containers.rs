//! The containers of schema 1 (task schema-freeze): a text layer's `text`, a layer's `fills`,
//! `strokes` and `effects` objects with their bindings, the ballots' keys and the read state kept
//! out of the replica. Each container has a convergence test: two replicas edit it at once, both
//! edits survive and the replicas end identical.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::sync::Arc;

use kition_design_doc::color::{Color, Paint};
use kition_design_doc::{Comment, Document, Effect, ListStyle, Message, NodeId, NodeKind, TextData};
use kition_design_kdm::{DesignSession, read_document, same_content};
use kition_kit::kdm::{ObjectId, Origin, Version};
use serde_json::json;

fn root_id() -> ObjectId {
    ObjectId::from_string("design")
}

fn person() -> Origin {
    Origin::person("person:test")
}

/// A page with a text layer `Title` ("Hello world") and a rectangle `Box` with one red fill.
fn doc() -> (Document, NodeId, NodeId) {
    let mut doc = Document::new("Containers");
    let page = doc.first_page().unwrap();
    let title = doc.create(page, NodeKind::Text(TextData::new("Hello world")), "Title", 0.0, 0.0, 200.0, 40.0).unwrap();
    let rect = doc.create(page, NodeKind::Rectangle, "Box", 0.0, 100.0, 50.0, 50.0).unwrap();
    doc.get_mut(rect).unwrap().fills = vec![Paint::solid(Color::new(1.0, 0.0, 0.0, 1.0))];
    doc.take_dirty();
    (doc, title, rect)
}

fn pair(doc: Document) -> (DesignSession, DesignSession) {
    let a = DesignSession::create(&Arc::new(doc), &Origin::system("create")).unwrap();
    let b = DesignSession::open(&a.snapshot().unwrap(), &[], a.blobs().clone()).unwrap();
    (a, b)
}

fn edit(s: &mut DesignSession, f: impl FnOnce(&mut Document)) {
    let mut d = (**s.document()).clone();
    f(&mut d);
    s.record(&person(), &Arc::new(d)).unwrap();
}

fn text_of(d: &mut Document, id: NodeId) -> &mut TextData {
    match &mut d.get_mut(id).unwrap().kind {
        NodeKind::Text(t) => t,
        _ => panic!("not a text"),
    }
}

/// Both ways, then the replicas hold one state and one view, which is what they read.
fn merged(a: &mut DesignSession, b: &mut DesignSession) {
    let (va, vb) = (a.version(), b.version());
    a.import_changes(&b.export_changes(&va).unwrap()).unwrap();
    b.import_changes(&a.export_changes(&vb).unwrap()).unwrap();
    assert_eq!(a.state_hash().unwrap(), b.state_hash().unwrap());
    assert!(same_content(a.document(), b.document()));
    for s in [&*a, &*b] {
        let (back, _) = read_document(s.replica(), s.blobs()).unwrap();
        assert!(same_content(&back, s.document()), "the view is what the replica holds");
    }
}

fn characters(s: &DesignSession, id: NodeId) -> String {
    match &s.document().node(id).unwrap().kind {
        NodeKind::Text(t) => t.characters.clone(),
        _ => panic!("not a text"),
    }
}

#[test]
fn a_text_layer_is_a_text_field_with_marks() {
    let (mut doc, title, _) = doc();
    let t = text_of(&mut doc, title);
    t.restyle(6, 11, |s| s.font_style = "Bold".into());
    t.characters.push_str("\nnext");
    t.set_para_lists(13, 13, |p| p.list = ListStyle::Bulleted);
    let (a, _) = pair(doc.clone());
    let object = a.ids().node_object(title).unwrap().clone();
    assert_eq!(a.replica().text(&object, "text").unwrap(), "Hello world\nnext\n", "the characters and the final break");
    let kind = a.replica().get(&object, "kind").unwrap();
    assert!(kind.get("characters").is_none() && kind.get("runs").is_none(), "kind keeps the box settings only: {kind}");
    let runs = a.replica().text_delta(&object, "text").unwrap();
    assert!(runs.iter().any(|r| r.insert.contains("world") && r.attributes.get("fontStyle") == Some(&json!("Bold"))), "{runs:?}");
    assert!(runs.last().unwrap().attributes.contains_key("list"), "the last paragraph's list is on the final break: {runs:?}");
    let (back, _) = read_document(a.replica(), a.blobs()).unwrap();
    assert!(same_content(&back, &doc));
}

#[test]
fn typing_at_once_in_one_text_layer_keeps_both() {
    let (doc, title, _) = doc();
    let (mut a, mut b) = pair(doc);
    edit(&mut a, |d| text_of(d, title).replace_range(5, 5, ", dear"));
    edit(&mut b, |d| text_of(d, title).replace_range(11, 11, "!"));
    merged(&mut a, &mut b);
    assert_eq!(characters(&a, title), "Hello, dear world!");
}

#[test]
fn styling_overlapping_ranges_at_once_applies_both() {
    let (doc, title, _) = doc();
    let (mut a, mut b) = pair(doc);
    edit(&mut a, |d| text_of(d, title).restyle(0, 5, |s| s.font_style = "Bold".into()));
    edit(&mut b, |d| text_of(d, title).restyle(3, 8, |s| s.font_size = 30.0));
    merged(&mut a, &mut b);
    let NodeKind::Text(t) = &a.document().node(title).unwrap().kind else { panic!() };
    assert_eq!((t.style_at(1).font_style.as_str(), t.style_at(1).font_size), ("Bold", 12.0));
    assert_eq!((t.style_at(4).font_style.as_str(), t.style_at(4).font_size), ("Bold", 30.0), "both on the overlap");
    assert_eq!((t.style_at(6).font_style.as_str(), t.style_at(6).font_size), ("Regular", 30.0));
}

#[test]
fn a_paragraph_split_and_typing_at_once_keep_both() {
    let (doc, title, _) = doc();
    let (mut a, mut b) = pair(doc);
    edit(&mut a, |d| {
        let t = text_of(d, title);
        t.replace_range(5, 6, "\n");
        t.set_para_lists(6, 6, |p| p.list = ListStyle::Numbered);
    });
    edit(&mut b, |d| text_of(d, title).replace_range(2, 2, "QQ"));
    merged(&mut a, &mut b);
    assert_eq!(characters(&a, title), "HeQQllo\nworld");
    let NodeKind::Text(t) = &a.document().node(title).unwrap().kind else { panic!() };
    assert_eq!((t.para_list(0).list, t.para_list(1).list), (ListStyle::None, ListStyle::Numbered));
}

#[test]
fn recolouring_a_paint_and_adding_one_at_once_keep_both() {
    let (doc, _, rect) = doc();
    let (mut a, mut b) = pair(doc);
    let green = Paint::solid(Color::new(0.0, 1.0, 0.0, 1.0));
    let blue = Paint::solid(Color::new(0.0, 0.0, 1.0, 1.0));
    edit(&mut a, |d| d.get_mut(rect).unwrap().fills[0] = green.clone());
    edit(&mut b, |d| d.get_mut(rect).unwrap().fills.push(blue.clone()));
    merged(&mut a, &mut b);
    assert_eq!(a.document().node(rect).unwrap().fills, vec![green, blue]);
    let object = a.ids().node_object(rect).unwrap().clone();
    let items = a.replica().list(&object, "fills").unwrap();
    assert_eq!(items.len(), 2, "one Paint object per paint");
    assert_eq!(a.replica().object_type(&items[0]).unwrap(), "Paint");
}

#[test]
fn a_binding_moves_with_its_paint_when_another_person_reorders() {
    let (mut doc, _, rect) = doc();
    doc.get_mut(rect).unwrap().fills.push(Paint::solid(Color::new(0.0, 0.0, 1.0, 1.0)));
    let (mut a, mut b) = pair(doc);
    let blue = a.document().node(rect).unwrap().fills[1].clone();
    // A binds the blue paint's colour (`fills/1/color`) while B moves the blue paint to the bottom.
    edit(&mut a, |d| {
        d.get_mut(rect).unwrap().bound.insert("fills/1/color".into(), "V:brand".into());
    });
    edit(&mut b, |d| d.get_mut(rect).unwrap().fills.swap(0, 1));
    merged(&mut a, &mut b);
    let n = a.document().node(rect).unwrap();
    assert_eq!(n.fills[0], blue, "B's reorder");
    assert_eq!(n.bound.get("fills/0/color").map(String::as_str), Some("V:brand"), "the binding followed its paint: {:?}", n.bound);
    assert!(!n.bound.contains_key("fills/1/color"));
    let object = a.ids().node_object(rect).unwrap().clone();
    let bindings = a.replica().get(&object, "bindings").unwrap();
    assert!(bindings.get("bound").is_none(), "a paint's binding is the paint object's: {bindings}");
}

#[test]
fn adding_strokes_at_once_keeps_both() {
    let (doc, _, rect) = doc();
    let (mut a, mut b) = pair(doc);
    let black = Paint::solid(Color::new(0.0, 0.0, 0.0, 1.0));
    let white = Paint::solid(Color::new(1.0, 1.0, 1.0, 1.0));
    edit(&mut a, |d| d.get_mut(rect).unwrap().strokes.push(black.clone()));
    edit(&mut b, |d| d.get_mut(rect).unwrap().strokes.push(white.clone()));
    merged(&mut a, &mut b);
    let strokes = &a.document().node(rect).unwrap().strokes;
    assert_eq!(strokes.len(), 2, "{strokes:?}");
    assert!(strokes.contains(&black) && strokes.contains(&white));
}

#[test]
fn editing_an_effect_and_adding_one_at_once_keep_both() {
    let (mut doc, _, rect) = doc();
    doc.get_mut(rect).unwrap().effects = vec![Effect::drop_shadow()];
    let (mut a, mut b) = pair(doc);
    edit(&mut a, |d| d.get_mut(rect).unwrap().effects[0].visible = false);
    edit(&mut b, |d| d.get_mut(rect).unwrap().effects.push(Effect::layer_blur()));
    merged(&mut a, &mut b);
    let effects = &a.document().node(rect).unwrap().effects;
    assert_eq!(effects.len(), 2, "{effects:?}");
    assert!(!effects[0].visible, "A's edit");
    assert_eq!(effects[1], Effect::layer_blur(), "B's effect");
}

#[test]
fn a_paint_added_to_a_layer_deleted_at_once_goes_with_it() {
    let (doc, _, rect) = doc();
    let (mut a, mut b) = pair(doc);
    edit(&mut a, |d| {
        d.remove(rect).unwrap();
    });
    edit(&mut b, |d| d.get_mut(rect).unwrap().fills.push(Paint::solid(Color::new(0.0, 0.0, 1.0, 1.0))));
    merged(&mut a, &mut b);
    assert!(a.document().node(rect).is_none());
    let r = a.replica();
    let lists: Vec<String> =
        r.objects_of_type("Node").iter().map(|n| format!("{n} {:?} fills={:?}", r.get(n, "name").unwrap(), r.list(n, "fills").unwrap())).collect();
    assert!(
        r.objects_of_type("Paint").is_empty(),
        "V13 deletes the paints no live layer lists: {:?}; layers: {lists:#?}",
        r.objects_of_type("Paint")
    );
}

#[test]
fn voting_at_once_keeps_every_ballot() {
    let (doc, title, rect) = doc();
    let (mut a, mut b) = pair(doc);
    edit(&mut a, |d| {
        d.votes.insert("Ann".into(), vec![title]);
    });
    edit(&mut b, |d| {
        d.votes.insert("Bo".into(), vec![rect]);
    });
    merged(&mut a, &mut b);
    let votes = &a.document().votes;
    assert_eq!((votes.get("Ann"), votes.get("Bo")), (Some(&vec![title]), Some(&vec![rect])));
    let settings = a.replica().map_entries(&root_id(), "settings").unwrap();
    let keys: Vec<&str> = settings.iter().map(|(k, _)| k.as_str()).filter(|k| k.starts_with("votes")).collect();
    assert_eq!(keys, ["votes/Ann", "votes/Bo"], "one key per voter");
}

#[test]
fn who_has_read_a_thread_is_not_replicated() {
    let (mut doc, _, _) = doc();
    let page = doc.first_page().unwrap();
    doc.comments.push(Comment {
        id: 900,
        page,
        x: 1.0,
        y: 2.0,
        anchor: None,
        first: Message::new("Ann", "Look", 100),
        resolved: false,
        replies: Vec::new(),
        read_by: Some(vec!["Ann".into()]),
    });
    let (mut a, mut b) = pair(doc);
    let thread = a.replica().list(&root_id(), "comments").unwrap()[0].clone();
    assert!(a.replica().get(&thread, "anchor").unwrap().get("readBy").is_none());
    // Bo reads it: nothing is written.
    let before = a.version();
    let mut d = (**a.document()).clone();
    d.comments[0].set_read("Bo", true);
    assert_eq!(a.record(&person(), &Arc::new(d)).unwrap(), 0);
    assert_eq!(a.version(), before);
    assert!(a.document().comments[0].read_by.as_ref().is_some_and(|r| r.iter().any(|u| u == "Bo")));
    // B's view keeps its own read state through A's other edits.
    let read_b = b.document().comments[0].read_by.clone();
    edit(&mut a, |d| d.name = "Renamed".into());
    b.import_changes(&a.export_changes(&Version::empty()).unwrap()).unwrap();
    assert_eq!(b.document().name, "Renamed");
    assert_eq!(b.document().comments[0].read_by, read_b);
    // A reply makes the thread unread for everyone but its author.
    let mut d = (**a.document()).clone();
    d.comments[0].replies.push(Message::new("Ann", "Again", 200));
    a.record(&person(), &Arc::new(d)).unwrap();
    b.import_changes(&a.export_changes(&Version::empty()).unwrap()).unwrap();
    assert!(b.document().comments[0].is_unread("Bo"));
    assert!(!b.document().comments[0].is_unread("Ann"));
}
