//! What the style picker and the Local variables window list of the enabled libraries, as
//! upstream's `widgets/style_picker.rs` (`offered`, `Key::of_local`) and
//! `dialogs/variables_panel.rs` (`View::of`) read them: each enabled library's published styles,
//! variable collections and variables (read-only there: a click applies a library style with
//! `library.applyStyle`, "Extend collection" runs `library.extendCollection`), and which of this
//! file's styles, collections and variables are linked copies of them (upstream leaves those out
//! of this file's own sections).

use kition_design_doc::{Document, Variables};
use kition_design_engine::library;
use serde_json::{Map, Value, json};

use super::MAX_LISTED;
use crate::DesignDocument;

/// A library entry as both answers start it: `{ id, name, error }` (`name`: the published name
/// once read; `error`: why its file can't be read now).
fn entry(link: &kition_design_doc::LibraryLink, error: Option<&str>, name: Option<&str>) -> Value {
    json!({ "id": link.id, "name": name.unwrap_or(&link.name), "error": error })
}

/// `styles.libraries { type? }` (`paint`, `text`, `effect` or `grid`):
///
/// ```text
/// { copies: { <local style id>: { library, style } },   // this file's linked library copies
///   libraries: [{ id, name, error,                      // each enabled library, in order
///                 styles: [Style] }] }                  // its published styles of the type, as
///                                                       // `styles` lists them; text styles add
///                                                       // `text` (their properties, camelCase)
/// ```
pub fn styles(doc: &DesignDocument, params: &Value) -> Result<Value, String> {
    let d: &Document = &doc.ds()?.doc;
    let ty = params.get("type").and_then(Value::as_str);
    let mut copies = Map::new();
    for r in &d.libraries.styles {
        if d.style(&r.local).is_some() {
            copies.insert(r.local.clone(), json!({ "library": r.library, "style": r.key }));
        }
    }
    let mut libraries = Vec::new();
    for (link, read) in crate::libraries::enabled(doc) {
        match read {
            Ok(lib) => {
                let rows: Vec<Value> = lib
                    .doc
                    .styles
                    .iter()
                    .filter(|s| lib.manifest.style(&s.id).is_some_and(|x| x.published) && ty.is_none_or(|t| s.kind.type_name() == t))
                    .take(MAX_LISTED)
                    .map(|s| {
                        let mut row = serde_json::to_value(s).unwrap_or(Value::Null);
                        if let kition_design_doc::StyleKind::Text { style } = &s.kind {
                            row["text"] = kition_design_engine::inspect::camel(serde_json::to_value(style).unwrap_or(Value::Null));
                        }
                        row
                    })
                    .collect();
                let mut e = entry(&link, None, Some(&lib.manifest.name));
                e["styles"] = json!(rows);
                libraries.push(e);
            }
            Err(error) => {
                let mut e = entry(&link, Some(&error), None);
                e["styles"] = json!([]);
                libraries.push(e);
            }
        }
    }
    Ok(json!({ "copies": copies, "libraries": libraries }))
}

/// `variables.libraries {}`:
///
/// ```text
/// { copies: { collections: [id], variables: [id] },   // this file's linked library copies
///   libraries: [{ id, name, error,                    // each enabled library, in order
///                 collections: [Collection],          // its published collections and
///                 variables: [Variable] }] }          // variables, as `variables` lists them
/// ```
pub fn variables(doc: &DesignDocument) -> Result<Value, String> {
    let d: &Document = &doc.ds()?.doc;
    let collections: Vec<&str> = d.variables.collections.iter().filter(|c| library::is_remote_collection(d, &c.id)).map(|c| c.id.as_str()).collect();
    let variables: Vec<&str> = d.variables.variables.iter().filter(|v| library::is_remote_variable(d, &v.id)).map(|v| v.id.as_str()).collect();
    let mut libraries = Vec::new();
    for (link, read) in crate::libraries::enabled(doc) {
        match read {
            Ok(lib) => {
                let offered = Variables {
                    collections: library::published_collections(&lib).take(MAX_LISTED).cloned().collect(),
                    variables: library::published_variables(&lib).take(MAX_LISTED).cloned().collect(),
                };
                let mut e = entry(&link, None, Some(&lib.manifest.name));
                let listed = serde_json::to_value(&offered).unwrap_or(Value::Null);
                e["collections"] = listed.get("collections").cloned().unwrap_or_else(|| json!([]));
                e["variables"] = listed.get("variables").cloned().unwrap_or_else(|| json!([]));
                libraries.push(e);
            }
            Err(error) => {
                let mut e = entry(&link, Some(&error), None);
                e["collections"] = json!([]);
                e["variables"] = json!([]);
                libraries.push(e);
            }
        }
    }
    Ok(json!({ "copies": { "collections": collections, "variables": variables }, "libraries": libraries }))
}
