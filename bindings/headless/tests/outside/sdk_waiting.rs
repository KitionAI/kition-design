//! Pinned SDK limitations, kept outside the passing C8-design-read gate-case filter.
use super::read::{case, prepare};
use super::*;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "waits for kition-sdk: variable.import's held read must cover only the collections whose names it matches; today a colleague's rename of another collection refuses the import"]
async fn waiting_for_sdk_variable_import_reads_matching_names_and_not_another_collection() {
    let failures = case(
        "variable.import names",
        |f| {
            prepare(f, |d| {
                d.execute("variable.addCollection", json!({"name":"Brand"})).unwrap();
                d.execute("variable.add", json!({"collection":"Brand","name":"Primary","type":"number","value":8})).unwrap();
                let exported = d.execute("variable.export", json!({})).unwrap();
                d.execute("variable.addCollection", json!({"name":"Other"})).unwrap();
                exported
            })
        },
        |_, exported| ("variable.import", json!({"variables":exported["text"].as_str().map(|t| json!(t)).unwrap_or(exported.clone())})),
        |_, _| vec![("variable.renameCollection", json!({"collection":"Brand","name":"Old"}))],
        |_, _| vec![("variable.renameCollection", json!({"collection":"Other","name":"Unrelated"}))],
    )
    .await;
    assert!(failures.is_empty(), "{failures:?}");
}
