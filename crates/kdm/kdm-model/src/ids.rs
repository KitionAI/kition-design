//! Where each engine object lives in the replica: its KDM object (and, for a layer, its tree
//! node). The engine's ids are data in the replica (`key` registers), so every replica has the
//! same keys; these maps are local and rebuilt on every read.

use std::collections::{HashMap, HashSet};

use kition_design_doc::NodeId;
use kition_kit::kdm::{NodeRef, ObjectId};

/// One comment thread: its object and its messages' objects, in order.
#[derive(Clone, Debug, Default)]
pub struct Thread {
    pub object: ObjectId,
    pub messages: Vec<ObjectId>,
}

/// One variable collection: its object and its modes' objects by mode id.
#[derive(Clone, Debug, Default)]
pub struct CollectionIds {
    pub object: ObjectId,
    pub modes: HashMap<String, ObjectId>,
}

#[derive(Clone, Debug, Default)]
pub struct Ids {
    /// Layers: object and tree node.
    pub nodes: HashMap<NodeId, (ObjectId, NodeRef)>,
    /// Object to layer, for reads.
    pub objects: HashMap<ObjectId, NodeId>,
    pub styles: HashMap<String, ObjectId>,
    pub collections: HashMap<String, CollectionIds>,
    pub variables: HashMap<String, ObjectId>,
    /// Images by content hash.
    pub images: HashMap<String, ObjectId>,
    /// Comment threads by comment id.
    pub threads: HashMap<u64, Thread>,
    /// Library links by library id.
    pub libraries: HashMap<String, ObjectId>,
    /// The layers of the library store (`Design.libraryStore`'s nodes; every other layer is
    /// `Design.nodes`').
    pub store: HashSet<NodeId>,
    /// Library copy records (`Design.libraryCopies`) by kind and local id.
    pub copies: HashMap<(String, String), ObjectId>,
}

impl Ids {
    pub fn node_object(&self, id: NodeId) -> Option<&ObjectId> {
        self.nodes.get(&id).map(|(o, _)| o)
    }

    pub fn node_of(&self, object: &ObjectId) -> Option<NodeId> {
        self.objects.get(object).copied()
    }

    pub(crate) fn insert_node(&mut self, id: NodeId, object: ObjectId, node: NodeRef, in_store: bool) {
        self.objects.insert(object.clone(), id);
        self.nodes.insert(id, (object, node));
        if in_store {
            self.store.insert(id);
        } else {
            self.store.remove(&id);
        }
    }

    pub(crate) fn remove_node(&mut self, id: NodeId) {
        if let Some((object, _)) = self.nodes.remove(&id) {
            self.objects.remove(&object);
        }
        self.store.remove(&id);
    }
}
