use std::{fs, path::PathBuf};

use sing_sem::check_source;

fn example_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(name)
}

#[test]
fn checks_all_examples_without_diagnostics() {
    for example in ["hello.sg", "dot.sg", "vec.sg", "div.sg", "web.sg"] {
        let src = fs::read_to_string(example_path(example)).expect("example should be readable");
        let checked = check_source(&src);
        assert!(
            checked.diagnostics.is_empty(),
            "{example} diagnostics: {:#?}",
            checked.diagnostics
        );
        assert!(
            checked.hir.is_some(),
            "{example} should lower to a typed HIR program"
        );
    }
}

#[test]
fn reports_unknown_name_with_stable_json_diagnostic() {
    let checked = check_source("f main()>v:missing()");
    assert_eq!(checked.diagnostics.len(), 1);
    let diagnostic = &checked.diagnostics[0];
    assert_eq!(diagnostic.code, "E0201");
    assert!(diagnostic.message.contains("unknown name"));

    let json = serde_json::to_value(diagnostic).expect("diagnostic should serialize");
    assert_eq!(json["code"], "E0201");
    assert_eq!(json["severity"], "error");
    assert!(json["labels"][0]["span"]["end"].as_u64().unwrap() > 0);
}

#[test]
fn reports_unknown_object_field() {
    let checked = check_source("t V{x:f4};f main()>V:V{y:1}");
    assert_eq!(checked.diagnostics[0].code, "E0302");
    assert!(checked.diagnostics[0].message.contains("unknown field"));
}

#[test]
fn reports_return_type_mismatch() {
    let checked = check_source("f bad()>i4:\"nope\"");
    assert_eq!(checked.diagnostics[0].code, "E0401");
    assert!(checked.diagnostics[0].message.contains("return type"));
}

#[test]
fn reports_unlisted_effects() {
    let checked = check_source("f main()>v:out(\"hi\")");
    assert_eq!(checked.diagnostics[0].code, "E0602");
    assert!(checked.diagnostics[0].message.contains("effect"));
}
