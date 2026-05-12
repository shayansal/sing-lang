use std::process::Command;

#[test]
fn tokens_command_prints_compact_json_cost() {
    let bin = env!("CARGO_BIN_EXE_sing");
    let output = Command::new(bin)
        .args(["tokens", "../../examples/hello.sg"])
        .output()
        .expect("sing binary should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(!stdout.contains('\n') || stdout.ends_with('\n'));
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("tokens output is JSON");
    assert!(json["bytes"].as_u64().unwrap() > 0);
    assert!(json["lexemes"].as_u64().unwrap() > 0);
    assert!(json["total"].as_u64().unwrap() >= json["llm_tokens"].as_u64().unwrap());
}
