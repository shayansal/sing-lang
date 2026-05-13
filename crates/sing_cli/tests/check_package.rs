use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_dir() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should work")
        .as_nanos();
    std::env::temp_dir().join(format!("sing-cli-check-pkg-{nonce}"))
}

#[test]
fn check_command_accepts_package_root_and_resolves_imported_symbols() {
    let bin = env!("CARGO_BIN_EXE_sing");
    let dir = temp_dir();
    fs::create_dir_all(dir.join("src")).expect("package dir should be writable");
    fs::write(
        dir.join("Sing.toml"),
        "name=\"pkg\"\nversion=\"0.1.0\"\nsources=[\"src/a.sg\",\"src/b.sg\"]\n",
    )
    .expect("manifest should be writable");
    fs::write(
        dir.join("src/a.sg"),
        "m A;t V{x:i4};k num:i4=2;f f()>i4:num",
    )
    .expect("source a should be writable");
    fs::write(dir.join("src/b.sg"), "m B;u A;f g()>i4{v=V{x:f()};v.x+num}")
        .expect("source b should be writable");

    let output = Command::new(bin)
        .arg("check")
        .arg(&dir)
        .output()
        .expect("sing check should run");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("check output should be JSON");

    assert_eq!(json["contract"]["schema_version"], "check.v1");
    assert_eq!(json["ok"], true);
    assert!(json["modules"]
        .as_array()
        .is_some_and(|modules| modules.iter().any(|module| module == "A")
            && modules.iter().any(|module| module == "B")));
    assert!(json["symbols"]
        .as_array()
        .is_some_and(|symbols| symbols.iter().any(|symbol| symbol["qualified"] == "A.V")));

    let _ = fs::remove_dir_all(dir);
}
