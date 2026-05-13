use std::{
    env, fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_out_dir() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should work")
        .as_nanos();
    env::temp_dir().join(format!("sing-cli-build-{nonce}"))
}

#[test]
fn build_command_emits_json_report_and_binary() {
    let bin = env!("CARGO_BIN_EXE_sing");
    let out_dir = temp_out_dir();
    let output = Command::new(bin)
        .args(["build", "../../examples/hello.sg", "--out"])
        .arg(&out_dir)
        .output()
        .expect("sing binary should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("build output is JSON");
    assert_eq!(json["backend"], "cranelift-alpha");
    assert_eq!(json["codegen_strategy"], "oracle-linked-launcher");
    assert_eq!(json["direct_native"], false);
    assert!(json["binary_path"]
        .as_str()
        .is_some_and(|path| !path.is_empty()));

    let _ = fs::remove_dir_all(out_dir);
}
