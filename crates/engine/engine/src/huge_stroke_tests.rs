//! An absurd stroke weight must not take the renderer down with it: saving (thumbnail) and exporting still work.

use serde_json::{Value, json};

use crate::Session;
use crate::params::id_of;
use kition_design_doc::NodeId;

fn id(v: Value) -> NodeId {
    id_of(&v["id"]).unwrap()
}

#[test]
fn huge_stroke_weight_with_round_caps_still_saves_and_exports() {
    for weight in [1e6, 1e7, 1e10, 1e30] {
        let mut s = Session::new();
        let f = id(s.execute("frame.add", &json!({"x": 0, "y": 0, "width": 100, "height": 100})).unwrap());
        let v = id(s.execute("vector.add", &json!({"parent": f, "path": "M0 0.8333 L7.5 6.25 L15 0.8333"})).unwrap());
        s.execute("node.set", &json!({"id": v, "props": {"stroke": "#ff0000", "strokeWeight": weight, "strokeCap": "round", "strokeJoin": "round"}}))
            .unwrap();
        let path = std::env::temp_dir().join(format!("kition-design-huge-stroke-{}-{weight}.kdesign", std::process::id()));
        let saved = s.execute("file.saveAs", &json!({"path": path.to_string_lossy()}));
        let _ = std::fs::remove_file(&path);
        assert!(saved.is_ok(), "saveAs at weight {weight}: {:?}", saved.err());
        let exported = s.execute("file.export", &json!({"ids": [f], "format": "png"}));
        assert!(exported.is_ok(), "export at weight {weight}: {:?}", exported.err());
    }
}
