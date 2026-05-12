use std::{fs, path::PathBuf};

use sing_parse::parse_file;

fn example_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(name)
}

fn parse_example(name: &str) -> serde_json::Value {
    let src = fs::read_to_string(example_path(name)).expect("example source should be readable");
    let file = parse_file(&src).unwrap_or_else(|errors| panic!("{errors:#?}"));
    serde_json::to_value(file).expect("AST should serialize to JSON")
}

#[test]
fn parses_all_examples_to_json_snapshots() {
    for example in ["hello.sg", "dot.sg", "vec.sg", "div.sg", "web.sg"] {
        let value = parse_example(example);
        let json = serde_json::to_string_pretty(&value).expect("JSON should be printable");
        let snapshot_name = example.trim_end_matches(".sg");
        insta::with_settings!({ snapshot_path => "../../../tests/snapshots" }, {
            insta::assert_snapshot!(snapshot_name, json);
        });
    }
}
