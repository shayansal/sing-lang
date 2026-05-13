use std::process::Command;

#[test]
fn expand_command_prints_compact_json_macro_report() {
    let bin = env!("CARGO_BIN_EXE_sing");
    let output = Command::new(bin)
        .args(["expand", "../../examples/web.sg"])
        .output()
        .expect("sing expand should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("expand output is JSON");
    assert_eq!(json["ok"], true);
    assert_eq!(json["generated_items"], 11);
    assert!(json["source"].as_str().unwrap().contains("U_create"));
    assert!(json["expansions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|expansion| expansion["name"] == "rbac"));
}
