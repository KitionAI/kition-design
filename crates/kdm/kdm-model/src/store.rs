//! The library store and the records of library copies (schema v3, `docs/features/kdm.md` section 2.3,
//! decision F16).
//!
//! - The store's layers (the copies of library components) are the nodes of their own tree,
//!   `Design.libraryStore`, whose roots are the store's children. The store's root itself is no
//!   object: its id is the `store` of the `libraries` setting and its own registers are the
//!   `libraryStore` setting. So two people who make the store at once never make two roots: their
//!   copies land in the one tree, and the root is a register both write.
//! - Each record of a copy (a component copy's root and layer ids, a style, collection or variable
//!   copy's local id, with its library, asset and version) is one `LibraryCopy` object of
//!   `Design.libraryCopies`, named by its kind and local id, so records taken at once are all kept.

use kition_design_doc::{Document, ImportedComponent, ImportedRecord, Libraries, Node, NodeId};
use serde_json::{Map, Value};

use crate::KdmResult;
use crate::fields::node_fields;
use crate::kdm_err;

/// The tree field of the store's layers.
pub(crate) const STORE_TREE: &str = "libraryStore";
/// The list field of the copies' records.
pub(crate) const COPIES: &str = "libraryCopies";
/// The settings key of the store root's registers.
pub(crate) const STORE_PAGE: &str = "libraryStore";

pub(crate) const COMPONENT: &str = "component";
pub(crate) const STYLE: &str = "style";
pub(crate) const COLLECTION: &str = "collection";
pub(crate) const VARIABLE: &str = "variable";

/// The kinds of copy whose local id is derived from the library asset (`<library>:<key>`), so
/// copies of one asset taken at once have one id (V11).
pub(crate) const BY_ASSET: &[(&str, &str)] = &[(STYLE, "Style"), (COLLECTION, "Collection"), (VARIABLE, "Variable")];

/// The store's id when its layers outlive the setting that names it (V10: an undo of the edit
/// that made the store while another person added copies to it).
pub(crate) fn repair_store_key() -> NodeId {
    NodeId((crate::validate::REPAIR_SPACE << 32) | u64::from(crate::validate::fnv32(STORE_TREE)))
}

/// A store root as the engine makes it, under `id`.
pub(crate) fn default_store(id: NodeId) -> Option<Node> {
    let mut scratch = Document::without_pages("");
    let made = scratch.ensure_library_store();
    let mut node = scratch.node(made).cloned()?;
    node.id = id;
    Some(node)
}

/// The store root's registers, as the `libraryStore` setting holds them.
pub(crate) fn store_page(n: &Node) -> KdmResult<Value> {
    let fields: Map<String, Value> = node_fields(n, None)?.into_iter().filter(|(_, v)| !v.is_null()).map(|(k, v)| (k.to_owned(), v)).collect();
    Ok(Value::Object(fields))
}

/// One copy record: its kind, local id and serde form.
pub(crate) struct Row {
    pub kind: &'static str,
    pub local: String,
    pub record: Value,
}

/// Every copy record of `libs`, in order: components, styles, collections, variables.
pub(crate) fn rows(libs: &Libraries) -> KdmResult<Vec<Row>> {
    let mut out = Vec::with_capacity(libs.components.len() + libs.styles.len() + libs.collections.len() + libs.variables.len());
    for c in &libs.components {
        out.push(Row { kind: COMPONENT, local: c.root.0.to_string(), record: serde_json::to_value(c).map_err(kdm_err)? });
    }
    for (kind, list) in [(STYLE, &libs.styles), (COLLECTION, &libs.collections), (VARIABLE, &libs.variables)] {
        for r in list {
            out.push(Row { kind, local: r.local.clone(), record: serde_json::to_value(r).map_err(kdm_err)? });
        }
    }
    Ok(out)
}

/// Puts records read from the replica (kind, serde form; in list order) into `libs`. A record that
/// does not read is left out.
pub(crate) fn set_rows(libs: &mut Libraries, rows: Vec<(String, Value)>) {
    libs.components.clear();
    libs.styles.clear();
    libs.collections.clear();
    libs.variables.clear();
    for (kind, record) in rows {
        match kind.as_str() {
            COMPONENT => {
                if let Ok(r) = serde_json::from_value::<ImportedComponent>(record) {
                    libs.components.push(r);
                }
            }
            STYLE | COLLECTION | VARIABLE => {
                let Ok(r) = serde_json::from_value::<ImportedRecord>(record) else { continue };
                match kind.as_str() {
                    STYLE => libs.styles.push(r),
                    COLLECTION => libs.collections.push(r),
                    _ => libs.variables.push(r),
                }
            }
            _ => {}
        }
    }
}
