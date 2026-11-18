//! Expectations fixed before the first run: 25 constraint pairs x 2 parent changes x 3
//! write forms x related/unrelated outside writers = 300 cells.
use super::*;

const HORIZONTAL: [&str; 5] = ["left", "right", "leftRight", "center", "scale"];
const VERTICAL: [&str; 5] = ["top", "bottom", "topBottom", "center", "scale"];

async fn constraint_rows(horizontal: &'static str) -> (usize, usize, usize, usize, usize, Vec<String>) {
    let mut cells = 0;
    let mut wrong = 0;
    let mut false_refusals = 0;
    let mut wrong_reasons = 0;
    let waiting = 0;
    let mut failures = Vec::new();
    for vertical in VERTICAL {
        for change in ["resize", "auto-layout"] {
            for form in ["set-id", "set-on", "resize"] {
                for related in [true, false] {
                    // Parent growth changes every constraint pair except left/top. Changing
                    // auto layout changes the child's positioning/sizing rules in every pair.
                    // The other frame is independent for both kinds of colleague change.
                    let necessary = related && (change == "auto-layout" || horizontal != "left" || vertical != "top");
                    let label = format!("{horizontal}/{vertical}/{change}/{form}/related={related}");
                    let f = Fixture::new(Guard::Premise);
                    rewrite(&f, |d| {
                        d.execute("node.set", json!({"id":f.a,"props":{"constraints":{"horizontal":horizontal,"vertical":vertical}}})).unwrap()
                    });
                    let basis = f.basis().await;
                    let parent = if related { f.frame } else { f.destination };
                    if change == "resize" {
                        f.colleague(&[("node.set", json!({"id":parent,"props":{"width":1000,"height":800}}))]);
                    } else {
                        f.colleague(&[("layout.add", json!({"ids":[parent],"direction":"horizontal"}))]);
                    }
                    let bytes = f.bytes();
                    let answer = match form {
                        "set-id" => f.write(&basis, "node.set", json!({"id":f.a,"props":{"width":60,"height":60}})).await,
                        "set-on" => {
                            f.call(
                                "run",
                                json!({"basis":basis,"steps":[{"command":"node.set","on":[f.a],"params":{"props":{"width":60,"height":60}}}]}),
                            )
                            .await
                        }
                        _ => f.write(&basis, "transform.resize", json!({"ids":[f.a],"width":60,"height":60})).await,
                    };
                    cells += 1;
                    let refused = answer["error"]["code"] == "conflict";
                    if refused {
                        assert_eq!(f.bytes(), bytes, "{label}: refusal saved");
                    }
                    if necessary && !refused && answer.get("error").is_none() {
                        wrong += 1;
                        failures.push(format!("WRONG {label}: {answer}"));
                    } else if !necessary && refused {
                        false_refusals += 1;
                        failures.push(format!("FALSE {label}: {answer}"));
                    } else if answer.get("error").is_some() && !refused {
                        wrong_reasons += 1;
                        failures.push(format!("ERROR {label}: {answer}"));
                    } else if necessary && refused {
                        // Layout is content; constraint propagation also changes spatial
                        // frame registers. Both are real held facts in these cells.
                        let valid =
                            answer["conflicts"].as_array().unwrap().iter().any(|c| matches!(c["reason"].as_str(), Some("changed" | "relocated")));
                        if !valid {
                            wrong_reasons += 1;
                            failures.push(format!("REASON {label}: {answer}"));
                        }
                    }
                    if !refused && answer.get("error").is_none() {
                        assert_eq!(f.node(f.a)["width"], 60.0, "{label}");
                        assert_eq!(f.node(f.a)["height"], 60.0, "{label}");
                    }
                    f.shutdown().await;
                }
            }
        }
    }
    (cells, wrong, false_refusals, wrong_reasons, waiting, failures)
}

#[tokio::test(flavor = "multi_thread")]
async fn waiting_for_sdk_normalisation_constraint_matrix() {
    let tasks = HORIZONTAL.map(|horizontal| tokio::spawn(constraint_rows(horizontal)));
    let (mut cells, mut wrong, mut false_refusals, mut wrong_reasons, mut waiting) = (0, 0, 0, 0, 0);
    let mut failures = Vec::new();
    for task in tasks {
        let (n, w, f, r, s, errors) = task.await.unwrap();
        cells += n;
        wrong += w;
        false_refusals += f;
        wrong_reasons += r;
        waiting += s;
        failures.extend(errors);
    }
    println!("NORMALISATION cells={cells} wrong={wrong} false={false_refusals} wrong-reason={wrong_reasons} waiting_for_sdk={waiting}");
    assert_eq!(cells, 300);
    assert_eq!((wrong, false_refusals, wrong_reasons, waiting), (0, 0, 0, 0));
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
