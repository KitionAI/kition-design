use std::sync::Arc;

use kition_design_doc::{Document, NodeId, NodeKind};
use kition_kit::kdm::{Origin, UndoScope, Version};

use crate::{DesignSession, read_document, same_content, schema};

fn sample() -> Document {
    let mut doc = Document::new("Sample");
    let page = doc.first_page().unwrap();
    let frame = doc.create(page, NodeKind::Frame, "Frame 1", 0.0, 0.0, 200.0, 100.0).unwrap();
    doc.create(frame, NodeKind::Rectangle, "Rectangle 1", 10.0, 10.0, 50.0, 40.0).unwrap();
    doc.create(page, NodeKind::Ellipse { start: 0.0, sweep: std::f64::consts::TAU, inner: 0.0 }, "Ellipse", 300.0, 0.0, 40.0, 40.0).unwrap();
    doc.add_page("Page 2");
    doc.take_dirty();
    doc
}

fn person() -> Origin {
    Origin::person("person:test")
}

#[test]
fn the_schema_parses() {
    let schema = schema().unwrap();
    assert_eq!(schema.kit, "kit.design");
    assert_eq!(schema.root, "Design");
}

#[test]
fn a_document_round_trips_through_a_replica() {
    let doc = Arc::new(sample());
    let session = DesignSession::create(&doc, &Origin::system("create")).unwrap();
    let (back, _) = read_document(session.replica(), session.blobs()).unwrap();
    assert!(same_content(&back, &doc), "{:#?}\n---\n{:#?}", back, doc);
}

#[test]
fn structure_edits_round_trip_and_undo_keeps_the_other_scope() {
    let doc = Arc::new(sample());
    let mut s = DesignSession::create(&doc, &Origin::system("create")).unwrap();
    let page = s.document().first_page().unwrap();
    let frame = s.document().nodes().find(|n| n.name == "Frame 1").map(|n| n.id).unwrap();
    let ellipse = s.document().nodes().find(|n| n.name == "Ellipse").map(|n| n.id).unwrap();

    // The person moves the ellipse into the frame.
    let mut d = (**s.document()).clone();
    d.reparent(ellipse, frame, Some(0)).unwrap();
    d.take_dirty();
    let moved = Arc::new(d);
    s.record(&person(), &moved).unwrap();
    let (back, _) = read_document(s.replica(), s.blobs()).unwrap();
    assert!(same_content(&back, &moved));

    // The agent renames the page.
    let mut d = (**s.document()).clone();
    d.get_mut(page).unwrap().name = "Agent's page".into();
    s.record(&Origin::agent("agent:test"), &Arc::new(d)).unwrap();

    // The person's undo takes the move back and keeps the agent's rename.
    assert!(s.undo_in(UndoScope::Person).unwrap());
    let v = s.document();
    assert_eq!(v.node(ellipse).unwrap().parent, Some(page));
    assert_eq!(v.node(page).unwrap().name, "Agent's page");
    // The agent's undo takes back only its rename.
    assert!(s.undo_in(UndoScope::Agent).unwrap());
    assert_eq!(s.document().node(page).unwrap().name, "Page 1");
    assert_eq!(s.document().node(ellipse).unwrap().parent, Some(page));
    assert!(!s.can_undo_in(UndoScope::Agent));
    let _ = NodeId(0);
    let _ = Version::empty();
}

#[test]
fn the_file_entries_reopen_the_replica() {
    let doc = Arc::new(sample());
    let mut s = DesignSession::create(&doc, &Origin::system("create")).unwrap();
    let mut d = (**s.document()).clone();
    let page = d.first_page().unwrap();
    d.get_mut(page).unwrap().name = "Edited".into();
    s.record(&person(), &Arc::new(d)).unwrap();
    let saved = crate::file::save(&s).unwrap();
    assert_eq!(saved.base.version(), &s.version(), "a session saves its own content");
    let names: Vec<&str> = saved.entries.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, [crate::file::META, crate::file::CONTENT], "the content and what identifies it, no change log");
    // An unchanged save writes the same bytes.
    assert_eq!(crate::file::save(&s).unwrap().entries, saved.entries);
    let (back, base) = crate::file::open(&saved.entries, s.blobs().clone()).unwrap();
    assert_eq!(back.doc_id(), s.doc_id());
    assert_eq!(base.doc_id(), s.doc_id().to_string());
    assert_eq!(back.state_hash().unwrap(), s.state_hash().unwrap());
    assert!(same_content(back.document(), s.document()));
}

#[test]
fn the_document_at_a_saved_version_is_read_from_its_snapshot() {
    let doc = Arc::new(sample());
    let mut s = DesignSession::create(&doc, &Origin::system("create")).unwrap();
    let before = s.snapshot().unwrap();
    let mut d = (**s.document()).clone();
    let page = d.first_page().unwrap();
    let was = d.node(page).unwrap().name.clone();
    d.get_mut(page).unwrap().name = "Edited".into();
    s.record(&person(), &Arc::new(d)).unwrap();
    let hash = s.state_hash().unwrap();
    assert_eq!(s.document_at(&before).unwrap().node(page).unwrap().name, was, "the design as it was then");
    assert_eq!(s.document_at(&s.snapshot().unwrap()).unwrap().node(page).unwrap().name, "Edited");
    // A file's content (no version) reads too.
    assert_eq!(s.document_at(&s.replica().content_snapshot().unwrap()).unwrap().node(page).unwrap().name, "Edited");
    assert_eq!(s.state_hash().unwrap(), hash, "reading a version changes nothing");
    // Another document's snapshot is refused.
    let other = DesignSession::create(&doc, &Origin::system("create")).unwrap();
    let error = s.document_at(&other.snapshot().unwrap()).unwrap_err().to_string();
    assert!(error.contains("not-this-document"), "{error}");
}

fn with_library(doc: &Document, id: &str, path: &str) -> Arc<Document> {
    let mut d = doc.clone();
    d.libraries_mut().enabled.push(kition_design_doc::LibraryLink { id: id.into(), name: format!("Library {id}"), path: path.into(), version: 1 });
    Arc::new(d)
}

/// Takes `from`'s changes into `to`, which has every change before them (one line of changes).
fn relayed(from: &DesignSession, to: &mut DesignSession) {
    let since = to.version();
    to.import_changes(&from.export_changes(&since).unwrap()).unwrap();
    assert_eq!(from.state_hash().unwrap(), to.state_hash().unwrap());
}

#[test]
fn library_links_are_objects() {
    let doc = Arc::new(sample());
    let mut a = DesignSession::create(&doc, &Origin::system("create")).unwrap();
    let mut b = DesignSession::open(&a.snapshot().unwrap(), &[], a.blobs().clone()).unwrap();
    // Two people add two libraries, one after the other: both stay, each with its workspace path.
    let da = with_library(a.document(), "lib-a", "../libraries/A.kdesign");
    a.record(&person(), &da).unwrap();
    relayed(&a, &mut b);
    let db = with_library(b.document(), "lib-b", "B.kdesign");
    b.record(&person(), &db).unwrap();
    relayed(&b, &mut a);
    let ids: Vec<&str> = a.document().libraries.enabled.iter().map(|l| l.id.as_str()).collect();
    assert_eq!(ids.len(), 2, "{ids:?}");
    assert!(ids.contains(&"lib-a") && ids.contains(&"lib-b"));
    assert!(same_content(a.document(), b.document()));
    let (back, _) = read_document(a.replica(), a.blobs()).unwrap();
    assert_eq!(back.libraries.enabled, a.document().libraries.enabled);
    assert_eq!(back.libraries.enabled.iter().find(|l| l.id == "lib-a").map(|l| l.path.as_str()), Some("../libraries/A.kdesign"));

    // One removes a library, then the other moves another: both happen.
    let mut da = (**a.document()).clone();
    da.libraries_mut().enabled.retain(|l| l.id != "lib-a");
    a.record(&person(), &Arc::new(da)).unwrap();
    relayed(&a, &mut b);
    let mut db = (**b.document()).clone();
    if let Some(l) = db.libraries_mut().enabled.iter_mut().find(|l| l.id == "lib-b") {
        l.path = "moved/B.kdesign".into();
    }
    b.record(&person(), &Arc::new(db)).unwrap();
    relayed(&b, &mut a);
    let links = &a.document().libraries.enabled;
    assert!(links.iter().all(|l| l.id != "lib-a"));
    assert_eq!(links.iter().find(|l| l.id == "lib-b").map(|l| l.path.as_str()), Some("moved/B.kdesign"));
    // The settings register no longer holds the links.
    let settings = a.replica().map_entries(&crate::root_id(), "settings").unwrap();
    assert!(settings.iter().all(|(k, v)| k != "libraries" || v.get("enabled").is_none_or(|e| e.as_array().is_some_and(Vec::is_empty))));
}

/// A change written straight to the replica by a peer that does not run the engine (a headless
/// agent's `model.batch`, another build): the state of `s`, plus what `f` writes.
fn peer_write(s: &DesignSession, f: impl FnOnce(&mut kition_kit::kdm::Tx<'_>) -> crate::KdmResult<()>) -> Vec<u8> {
    let mut peer = kition_kit::kdm::Replica::load(schema().unwrap(), &s.snapshot().unwrap(), kition_kit::kdm::ActorId::random()).unwrap();
    let since = peer.version();
    peer.transact(&Origin::agent("agent:test"), f).unwrap();
    peer.export_changes(&since).unwrap()
}

fn named(s: &DesignSession, name: &str) -> NodeId {
    s.document().nodes().find(|n| n.name == name).map(|n| n.id).unwrap()
}

/// Review #1: node registers are `json`, so the KDM takes any value. A register (or a setting)
/// the engine cannot read leaves that register unset in the view and the rest of the document as
/// it is, for every replica, instead of failing the whole read (which dropped the replica).
#[test]
fn a_register_that_does_not_read_leaves_the_rest_of_the_document() {
    let doc = Arc::new(sample());
    let mut s = DesignSession::create(&doc, &Origin::system("create")).unwrap();
    let (rect, frame) = (named(&s, "Rectangle 1"), named(&s, "Frame 1"));
    let (ro, fo) = (s.ids().node_object(rect).unwrap().clone(), s.ids().node_object(frame).unwrap().clone());
    let changes = peer_write(&s, |tx| {
        tx.set(&ro, "frame", serde_json::json!({ "width": "wide" }))?;
        tx.set(&ro, "name", serde_json::json!("Kept"))?;
        tx.set(&fo, "kind", serde_json::json!({ "type": "wide" }))?;
        tx.map_set(&crate::root_id(), "settings", "color_profile", serde_json::json!(42))
    });
    s.import_changes(&changes).unwrap();
    let v = s.document().clone();
    let r = v.node(rect).unwrap();
    assert_eq!((r.name.as_str(), r.width), ("Kept", 0.0), "the bad frame is unset, the name read");
    let f = v.node(frame).unwrap();
    assert!(matches!(f.kind, NodeKind::Frame) && f.children.contains(&rect), "an unreadable kind reads as a frame: {:?}", f.kind);
    assert_eq!(v.name, "Sample");
    // The replica reopens (a saved file with the value), and reads the same everywhere.
    let back = DesignSession::open(&s.snapshot().unwrap(), &[], s.blobs().clone()).unwrap();
    assert!(same_content(back.document(), &v));
    // Local edits keep recording.
    let mut d = (*v).clone();
    d.get_mut(named(&s, "Ellipse")).unwrap().opacity = 0.5;
    assert_eq!(s.record(&person(), &Arc::new(d)).unwrap(), 1);
}

fn message(id: u64, author: &str, text: &str, created: u64) -> kition_design_doc::Message {
    kition_design_doc::Message { id, author: author.into(), text: text.into(), created, edited: 0, reactions: Vec::new() }
}

/// Review #12: comment messages keep their objects by identity (their message ids), not by place.
/// A deletes the first reply, then B edits the second: the second reply keeps B's edit.
#[test]
fn deleting_a_reply_keeps_the_next_one_by_identity() {
    let mut doc = sample();
    let page = doc.first_page().unwrap();
    doc.comments.push(kition_design_doc::Comment {
        id: 7,
        page,
        x: 10.0,
        y: 20.0,
        anchor: None,
        first: message(1, "ann", "Look here", 100),
        resolved: false,
        replies: vec![message(2, "bob", "First reply", 200), message(3, "cy", "Second reply", 300)],
        read_by: None,
    });
    let doc = Arc::new(doc);
    let mut a = DesignSession::create(&doc, &Origin::system("create")).unwrap();
    let mut b = DesignSession::open(&a.snapshot().unwrap(), &[], a.blobs().clone()).unwrap();
    let mut d = (**a.document()).clone();
    d.comments[0].replies.remove(0);
    a.record(&person(), &Arc::new(d)).unwrap();
    relayed(&a, &mut b);
    let mut d = (**b.document()).clone();
    let r = &mut d.comments[0].replies[0];
    (r.text, r.edited) = ("Second reply, edited".into(), 400);
    b.record(&person(), &Arc::new(d)).unwrap();
    relayed(&b, &mut a);
    assert!(same_content(a.document(), b.document()));
    let c = &a.document().comments[0];
    assert_eq!(c.first.text, "Look here");
    let replies: Vec<(&str, &str, u64)> = c.replies.iter().map(|m| (m.author.as_str(), m.text.as_str(), m.edited)).collect();
    assert_eq!(replies, vec![("cy", "Second reply, edited", 400)]);
}

/// Limit closed (F18): two replies by one author within one second keep their own
/// objects by their message ids. A deletes the first, then B edits the second: the second keeps
/// B's edit and the first is gone, on both replicas.
#[test]
fn same_second_replies_by_one_author_keep_their_own_objects() {
    let mut doc = sample();
    let page = doc.first_page().unwrap();
    doc.comments.push(kition_design_doc::Comment {
        id: 7,
        page,
        x: 10.0,
        y: 20.0,
        anchor: None,
        first: message(1, "ann", "Look here", 100),
        resolved: false,
        replies: vec![message(2, "bob", "First reply", 200), message(3, "bob", "Second reply", 200)],
        read_by: None,
    });
    let doc = Arc::new(doc);
    let mut a = DesignSession::create(&doc, &Origin::system("create")).unwrap();
    let mut b = DesignSession::open(&a.snapshot().unwrap(), &[], a.blobs().clone()).unwrap();
    let mut d = (**a.document()).clone();
    d.comments[0].replies.remove(0);
    a.record(&person(), &Arc::new(d)).unwrap();
    relayed(&a, &mut b);
    let mut d = (**b.document()).clone();
    let r = &mut d.comments[0].replies[0];
    (r.text, r.edited) = ("Second reply, edited".into(), 400);
    b.record(&person(), &Arc::new(d)).unwrap();
    relayed(&b, &mut a);
    assert!(same_content(a.document(), b.document()));
    let c = &a.document().comments[0];
    let replies: Vec<(&str, &str, u64)> = c.replies.iter().map(|m| (m.author.as_str(), m.text.as_str(), m.edited)).collect();
    assert_eq!(replies, vec![("bob", "Second reply, edited", 400)]);
}

/// A rectangle's registers under `key`, as the writer writes them.
fn layer_fields(key: u64, name: &str) -> serde_json::Map<String, serde_json::Value> {
    let mut d = Document::new("x");
    let page = d.first_page().unwrap();
    let id = d.create(page, NodeKind::Rectangle, name, 0.0, 0.0, 10.0, 10.0).unwrap();
    let mut fields: serde_json::Map<String, serde_json::Value> = crate::fields::node_fields(d.node(id).unwrap(), None)
        .unwrap()
        .into_iter()
        .filter(|(_, v)| !v.is_null())
        .map(|(k, v)| (k.to_owned(), v))
        .collect();
    fields.insert("key".into(), serde_json::json!(key));
    fields
}

fn repair_key(text: &str) -> u64 {
    (crate::validate::REPAIR_SPACE << 32) | u64::from(crate::validate::fnv32(text))
}

/// Review #24 (V3): the key a repair gives is never one a live layer or the library store's root
/// already has; before, the reader dropped one of the two layers (and everything under it).
#[test]
fn a_repaired_key_never_takes_a_live_key() {
    let doc = Arc::new(sample());
    let mut s = DesignSession::create(&doc, &Origin::system("create")).unwrap();
    let page = s.document().first_page().unwrap();
    let page_node = s.ids().nodes.get(&page).unwrap().1.clone();
    let root = crate::root_id();
    let changes = peer_write(&s, |tx| {
        // B and A share a key; C already has the key V3 used to give B.
        for (id, key, name) in [("n-a", 5000, "A"), ("n-b", 5000, "B"), ("n-c", repair_key("n-b"), "C"), ("n-d", 777, "D")] {
            let id = kition_kit::kdm::ObjectId::from_string(id);
            tx.create_with_id("Node", &id, layer_fields(key, name))?;
            tx.tree_create(&root, "nodes", Some(&page_node), &id)?;
        }
        // D has the library store root's key.
        let libraries = kition_design_doc::Libraries { store: Some(NodeId(777)), ..Default::default() };
        tx.map_set(&root, "settings", "libraries", serde_json::to_value(libraries).unwrap())?;
        let id = kition_kit::kdm::ObjectId::from_string("n-s");
        tx.create_with_id("Node", &id, layer_fields(9001, "S"))?;
        tx.tree_create(&root, crate::store::STORE_TREE, None, &id)?;
        Ok(())
    });
    s.import_changes(&changes).unwrap();
    let v = s.document();
    for name in ["A", "B", "C", "D", "S"] {
        assert_eq!(v.nodes().filter(|n| n.name == name).count(), 1, "{name} is read once");
    }
    assert_eq!(v.libraries.store, Some(NodeId(777)));
    let reopened = DesignSession::open(&s.snapshot().unwrap(), &[], s.blobs().clone()).unwrap();
    assert_eq!(reopened.state_hash().unwrap(), s.state_hash().unwrap(), "the repair is done once");
}

/// Review #24 (V8): the page V8 adds gets a key no live layer has (here a library store layer
/// that happens to hold the key V8 used to give).
#[test]
fn the_page_v8_adds_never_takes_a_live_key() {
    let doc = Arc::new(sample());
    let mut s = DesignSession::create(&doc, &Origin::system("create")).unwrap();
    let root = crate::root_id();
    let changes = peer_write(&s, |tx| {
        let mut roots: Vec<String> =
            tx.tree(&root, "nodes")?.into_iter().filter(|e| e.parent.is_none()).map(|e| e.object.as_str().to_owned()).collect();
        roots.sort();
        let page = format!("page-{:016x}", crate::validate::fnv64(&roots.join(",")));
        // Every layer goes, children before their parent: the KDM refuses to delete an object
        // whose node still has live children (`Tx::delete`).
        let entries = tx.tree(&root, "nodes")?;
        let depth = |e: &kition_kit::kdm::TreeEntry| {
            let (mut depth, mut at) = (0, e.parent.clone());
            while let Some(parent) = at {
                depth += 1;
                at = entries.iter().find(|p| p.node == parent).and_then(|p| p.parent.clone());
            }
            depth
        };
        let mut deepest_first: Vec<_> = entries.iter().map(|e| (depth(e), e.object.clone())).collect();
        deepest_first.sort_by_key(|(depth, _)| std::cmp::Reverse(*depth));
        for (_, object) in deepest_first {
            if tx.exists(&object) {
                tx.delete(&object)?;
            }
        }
        let id = kition_kit::kdm::ObjectId::from_string("n-s");
        tx.create_with_id("Node", &id, layer_fields(repair_key(&page), "S"))?;
        tx.tree_create(&root, crate::store::STORE_TREE, None, &id)?;
        Ok(())
    });
    s.import_changes(&changes).unwrap();
    let v = s.document();
    assert_eq!(v.pages.len(), 1);
    assert_eq!(v.nodes().filter(|n| n.name == "S").count(), 1, "the store layer is read");
}

/// Review #24 (V4): a layer that is a root of `Design.nodes` but no page, or that sits under layers
/// none of which can hold children, moves into the first page instead of staying a root no page
/// shows.
#[test]
fn a_layer_with_no_page_moves_into_the_first_page() {
    let doc = Arc::new(sample());
    let mut s = DesignSession::create(&doc, &Origin::system("create")).unwrap();
    let page = s.document().first_page().unwrap();
    let root = crate::root_id();
    let changes = peer_write(&s, |tx| {
        let r = kition_kit::kdm::ObjectId::from_string("n-r");
        tx.create_with_id("Node", &r, layer_fields(6001, "Root rect"))?;
        let rn = tx.tree_create(&root, "nodes", None, &r)?;
        let c = kition_kit::kdm::ObjectId::from_string("n-c");
        tx.create_with_id("Node", &c, layer_fields(6002, "Child rect"))?;
        tx.tree_create(&root, "nodes", Some(&rn), &c)?;
        Ok(())
    });
    s.import_changes(&changes).unwrap();
    let v = s.document();
    for name in ["Root rect", "Child rect"] {
        let n = v.nodes().find(|n| n.name == name).unwrap_or_else(|| panic!("{name} is read"));
        assert_eq!(n.parent, Some(page), "{name} is on the first page");
    }
    assert!(v.node(page).unwrap().children.ends_with(&[NodeId(6001), NodeId(6002)]));
}

/// Review #24 (V3): a style key or comment id a repair gives is never one another object has.
#[test]
fn a_repaired_style_key_or_comment_id_never_takes_a_live_one() {
    let doc = Arc::new(sample());
    let mut s = DesignSession::create(&doc, &Origin::system("create")).unwrap();
    let page = s.document().first_page().unwrap();
    let root = crate::root_id();
    let changes = peer_write(&s, |tx| {
        // Thread B shares A's id; C has the id V3 used to give B.
        for (i, (id, key, text)) in [("t-a", 1, "A"), ("t-b", 1, "B"), ("t-c", repair_key("t-b"), "C")].into_iter().enumerate() {
            let id = kition_kit::kdm::ObjectId::from_string(id);
            let mut fields = serde_json::Map::new();
            // `key` is the thread's tool id: required and creation-only, so a writer gives it
            // with the creation, as the anchor's id (`write.rs`).
            fields.insert("key".into(), serde_json::json!(key));
            fields.insert("anchor".into(), serde_json::json!({ "object": "design", "id": key, "page": page.0, "x": 0.0, "y": 0.0 }));
            fields.insert("resolved".into(), serde_json::json!(false));
            tx.create_with_id("CommentThread", &id, fields)?;
            let mut m = serde_json::Map::new();
            m.insert("author".into(), serde_json::json!("ann"));
            m.insert("created".into(), serde_json::json!("1970-01-01T00:01:40Z"));
            let first = tx.create("Comment", m)?;
            tx.text_insert(&first, "body", 0, text)?;
            tx.list_insert(&id, "comments", 0, &first)?;
            tx.list_insert(&root, "comments", i, &id)?;
        }
        // Styles "x" twice, and one with the key V3 used to give the second.
        let style = |key: &str, name: &str| {
            let s: kition_design_doc::Style =
                serde_json::from_value(serde_json::json!({ "id": key, "name": name, "type": "effect", "effects": [] })).unwrap();
            let mut f: serde_json::Map<String, serde_json::Value> =
                crate::fields::style_fields(&s).unwrap().into_iter().filter(|(_, v)| !v.is_null()).map(|(k, v)| (k.to_owned(), v)).collect();
            f.insert("key".into(), serde_json::json!(key));
            f
        };
        for (i, (id, key, name)) in [("s-a", "x", "SA"), ("s-b", "x", "SB"), ("s-c", "x~sb", "SC")].into_iter().enumerate() {
            let id = kition_kit::kdm::ObjectId::from_string(id);
            tx.create_with_id("Style", &id, style(key, name))?;
            tx.list_insert(&root, "styles", i, &id)?;
        }
        Ok(())
    });
    s.import_changes(&changes).unwrap();
    let v = s.document();
    let mut names: Vec<&str> = v.styles.iter().map(|s| s.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["SA", "SB", "SC"], "every style is read");
    let mut texts: Vec<&str> = v.comments.iter().map(|c| c.first.text.as_str()).collect();
    texts.sort_unstable();
    assert_eq!(texts, ["A", "B", "C"], "every thread is read");
}

/// C4/C5/C6-design (V3): `Node.key` is the layer's tool id, read-only to batches, so a layer a model
/// batch creates comes without one. The repair gives it a key of the repair space that no live
/// layer has, and the reader keeps it, instead of dropping it from the design.
#[test]
fn a_layer_created_without_a_key_gets_one() {
    let doc = Arc::new(sample());
    let mut s = DesignSession::create(&doc, &Origin::system("create")).unwrap();
    let page = s.document().first_page().unwrap();
    let page_node = s.ids().nodes.get(&page).unwrap().1.clone();
    let root = crate::root_id();
    let changes = peer_write(&s, |tx| {
        let id = kition_kit::kdm::ObjectId::from_string("n-keyless");
        let mut fields = layer_fields(0, "Keyless");
        fields.remove("key");
        tx.create_with_id("Node", &id, fields)?;
        tx.tree_create(&root, "nodes", Some(&page_node), &id)?;
        Ok(())
    });
    s.import_changes(&changes).unwrap();
    let made = s.document().nodes().find(|n| n.name == "Keyless").map(|n| n.id).expect("the keyless layer is read");
    assert_eq!(made, NodeId(repair_key("n-keyless")), "its key is the repair's, derived from its object id");
    assert_eq!(s.document().node(made).and_then(|n| n.parent), Some(page));
    let reopened = DesignSession::open(&s.snapshot().unwrap(), &[], s.blobs().clone()).unwrap();
    assert_eq!(reopened.state_hash().unwrap(), s.state_hash().unwrap(), "the repair is done once");
}
