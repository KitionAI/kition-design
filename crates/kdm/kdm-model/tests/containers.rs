//! The containers of schema 1 (task schema-freeze): a text layer's `text`, a layer's `fills`,
//! `strokes` and `effects` objects with their bindings, the ballots' keys and the read state kept
//! out of the replica. Edits of a container made one after the other on two replicas of one line
//! of changes all stay, and the replicas end identical.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::sync::Arc;

use kition_design_doc::color::{Color, Paint};
use kition_design_doc::{Comment, Document, ListStyle, Message, NodeId, NodeKind, TextData};
use kition_design_kdm::{DesignSession, read_document, same_content};
use kition_kit::kdm::{ObjectId, Origin, UndoScope, Version};
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

/// `from`'s changes taken into `to` (which has every change before them), then the replicas hold
/// one state and one view, which is what they read.
fn relayed(from: &mut DesignSession, to: &mut DesignSession) {
    let since = to.version();
    to.import_changes(&from.export_changes(&since).unwrap()).unwrap();
    let (a, b) = (from, to);
    assert_eq!(a.state_hash().unwrap(), b.state_hash().unwrap());
    assert!(same_content(a.document(), b.document()));
    for s in [&*a, &*b] {
        let (back, _) = read_document(s.replica(), s.blobs()).unwrap();
        assert!(same_content(&back, s.document()), "the view is what the replica holds");
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
fn recolouring_a_paint_and_adding_one_keep_both() {
    let (doc, _, rect) = doc();
    let (mut a, mut b) = pair(doc);
    let green = Paint::solid(Color::new(0.0, 1.0, 0.0, 1.0));
    let blue = Paint::solid(Color::new(0.0, 0.0, 1.0, 1.0));
    edit(&mut a, |d| d.get_mut(rect).unwrap().fills[0] = green.clone());
    relayed(&mut a, &mut b);
    edit(&mut b, |d| d.get_mut(rect).unwrap().fills.push(blue.clone()));
    relayed(&mut b, &mut a);
    assert_eq!(a.document().node(rect).unwrap().fills, vec![green, blue]);
    let object = a.ids().node_object(rect).unwrap().clone();
    let items = a.replica().list(&object, "fills").unwrap();
    assert_eq!(items.len(), 2, "one Paint object per paint");
    assert_eq!(a.replica().object_type(&items[0]).unwrap(), "Paint");
}

/// The fills of `rect` as the replica holds them: each Paint object's colour binding.
fn fill_bindings(s: &DesignSession, rect: NodeId) -> Vec<Option<String>> {
    let object = s.ids().node_object(rect).unwrap().clone();
    let paints = s.replica().list(&object, "fills").unwrap();
    paints
        .iter()
        .map(|paint| {
            assert_eq!(s.replica().object_type(paint).unwrap(), "Paint");
            let bound = s.replica().map_entries(paint, "bound").unwrap();
            bound.iter().find(|(k, _)| k == "color").and_then(|(_, v)| v.as_str().map(str::to_owned))
        })
        .collect()
}

/// The node's own bindings register holds no paint's binding: that is the Paint object's.
fn no_paint_binding_on_the_node(s: &DesignSession, rect: NodeId) {
    let object = s.ids().node_object(rect).unwrap().clone();
    let bindings = s.replica().get(&object, "bindings").unwrap_or(serde_json::Value::Null);
    assert!(bindings.get("bound").is_none(), "a paint's binding is the paint object's: {bindings}");
}

/// Swaps a layer's two fills as the engine's reorder does: the paints and the bindings that name
/// them by place.
fn swap_fills(d: &mut Document, rect: NodeId) {
    let n = d.get_mut(rect).unwrap();
    n.fills.swap(0, 1);
    let bound: Vec<(String, String)> = std::mem::take(&mut n.bound).into_iter().collect();
    for (key, variable) in bound {
        let key = match key.strip_prefix("fills/") {
            Some(rest) if rest.starts_with("0/") => format!("fills/1/{}", &rest[2..]),
            Some(rest) if rest.starts_with("1/") => format!("fills/0/{}", &rest[2..]),
            _ => key,
        };
        n.bound.insert(key, variable);
    }
}

/// A variable binding lives on its Paint object: a reorder of the paints moves the Paint objects
/// (the binding with its paint), and a recolour of the other paint leaves it.
#[test]
fn a_binding_moves_with_its_paint_on_a_reorder_and_stays_on_a_recolour() {
    let (mut doc, _, rect) = doc();
    doc.get_mut(rect).unwrap().fills.push(Paint::solid(Color::new(0.0, 0.0, 1.0, 1.0)));
    doc.get_mut(rect).unwrap().bound.insert("fills/1/color".into(), "V:brand".into());
    let (mut a, mut b) = pair(doc);
    let blue = a.document().node(rect).unwrap().fills[1].clone();
    assert_eq!(fill_bindings(&a, rect), [None, Some("V:brand".to_owned())]);
    let object = a.ids().node_object(rect).unwrap().clone();
    let mut objects = a.replica().list(&object, "fills").unwrap();
    edit(&mut a, |d| swap_fills(d, rect));
    objects.reverse();
    assert_eq!(a.replica().list(&object, "fills").unwrap(), objects, "the reorder moves the Paint objects");
    let n = a.document().node(rect).unwrap();
    assert_eq!(n.fills[0], blue);
    assert_eq!(n.bound.get("fills/0/color").map(String::as_str), Some("V:brand"), "the binding followed its paint: {:?}", n.bound);
    assert!(!n.bound.contains_key("fills/1/color"));
    assert_eq!(fill_bindings(&a, rect), [Some("V:brand".to_owned()), None]);
    let green = Paint::solid(Color::new(0.0, 1.0, 0.0, 1.0));
    edit(&mut a, |d| d.get_mut(rect).unwrap().fills[1] = green.clone());
    relayed(&mut a, &mut b);
    for s in [&a, &b] {
        let n = s.document().node(rect).unwrap();
        assert_eq!(n.fills, vec![blue.clone(), green.clone()]);
        assert_eq!(n.bound.get("fills/0/color").map(String::as_str), Some("V:brand"), "{:?}", n.bound);
        assert!(!n.bound.contains_key("fills/1/color"), "{:?}", n.bound);
        assert_eq!(fill_bindings(s, rect), [Some("V:brand".to_owned()), None]);
        no_paint_binding_on_the_node(s, rect);
    }
}

/// A draft reorders the paints while the person rebinds the blue paint to another variable: the
/// merge applies (only the draft changed the list, only the document the Paint's binding) and
/// the new binding is on the blue paint at its new place.
#[test]
fn a_draft_reordering_paints_while_the_person_rebinds_keeps_the_binding_on_its_paint() {
    let (mut doc, _, rect) = doc();
    doc.get_mut(rect).unwrap().fills.push(Paint::solid(Color::new(0.0, 0.0, 1.0, 1.0)));
    doc.get_mut(rect).unwrap().bound.insert("fills/1/color".into(), "V:brand".into());
    let (mut a, mut draft) = pair(doc);
    let base = a.version();
    let (red, blue) = {
        let n = a.document().node(rect).unwrap();
        (n.fills[0].clone(), n.fills[1].clone())
    };
    edit(&mut draft, |d| swap_fills(d, rect));
    edit(&mut a, |d| {
        d.get_mut(rect).unwrap().bound.insert("fills/1/color".into(), "V:accent".into());
    });
    a.merge_changes(&draft.export_changes(&base).unwrap(), &person()).unwrap().1.unwrap();
    let n = a.document().node(rect).unwrap();
    assert_eq!(n.fills, vec![blue, red], "the draft's reorder");
    assert_eq!(n.bound.get("fills/0/color").map(String::as_str), Some("V:accent"), "the person's binding, on its paint: {:?}", n.bound);
    assert!(!n.bound.contains_key("fills/1/color"), "{:?}", n.bound);
    assert_eq!(fill_bindings(&a, rect), [Some("V:accent".to_owned()), None]);
    no_paint_binding_on_the_node(&a, rect);
    let (back, _) = read_document(a.replica(), a.blobs()).unwrap();
    assert!(same_content(&back, a.document()), "the view is what the replica holds");
}

/// V13: a paint whose layer is gone. The person adds a paint to the box and takes it back; an
/// agent deletes the box; the person's Redo makes the paint again, but its place in the box's
/// fills went with the box (data model 6.5: an operation whose object is gone is skipped). The
/// repair deletes the paint no live layer lists.
#[test]
fn a_paint_redone_after_its_layer_was_deleted_goes_with_it() {
    let (doc, _, rect) = doc();
    let (mut a, _) = pair(doc);
    edit(&mut a, |d| d.get_mut(rect).unwrap().fills.push(Paint::solid(Color::new(0.0, 0.0, 1.0, 1.0))));
    assert!(a.undo_in(UndoScope::Person).unwrap());
    let mut d = (**a.document()).clone();
    d.remove(rect).unwrap();
    d.take_dirty();
    assert_eq!(a.record(&Origin::agent("agent:test"), &Arc::new(d)).unwrap(), 1);
    let mut paints_before = a.replica().objects_of_type("Paint");
    paints_before.sort();
    assert!(a.redo_in(UndoScope::Person).unwrap(), "the redo makes the paint again");
    assert!(a.document().node(rect).is_none());
    let r = a.replica();
    let listed: Vec<ObjectId> = r.objects_of_type("Node").iter().flat_map(|n| r.list(n, "fills").unwrap()).collect();
    let mut paints = r.objects_of_type("Paint");
    paints.sort();
    assert!(paints.iter().all(|p| listed.contains(p)), "V13 deletes the paints no live layer lists: {paints:?}, listed {listed:?}");
    assert_eq!(paints, paints_before, "only the live layers' paints are left");
    let (back, _) = read_document(r, a.blobs()).unwrap();
    assert!(same_content(&back, a.document()));
}

#[test]
fn every_ballot_is_its_own_key() {
    let (doc, title, rect) = doc();
    let (mut a, mut b) = pair(doc);
    edit(&mut a, |d| {
        d.votes.insert("Ann".into(), vec![title]);
    });
    relayed(&mut a, &mut b);
    edit(&mut b, |d| {
        d.votes.insert("Bo".into(), vec![rect]);
    });
    relayed(&mut b, &mut a);
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
