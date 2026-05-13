use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_dir(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should work")
        .as_nanos();
    std::env::temp_dir().join(format!("sing-cli-{name}-{nonce}"))
}

fn write_temp_source(dir: &Path, name: &str, src: &str) -> PathBuf {
    fs::create_dir_all(dir).expect("temp dir should be writable");
    let path = dir.join(name);
    fs::write(&path, src).expect("temp source should be writable");
    path
}

#[test]
fn fmt_and_min_are_parseable_and_min_saves_tokens() {
    let bin = env!("CARGO_BIN_EXE_sing");
    let dir = temp_dir("fmt");
    let file = write_temp_source(&dir, "main.sg", r#"m H;u io;f main()>v !iw:out("hi")"#);

    let fmt = Command::new(bin)
        .arg("fmt")
        .arg(&file)
        .output()
        .expect("sing fmt should run");
    assert!(
        fmt.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&fmt.stderr)
    );
    let formatted = String::from_utf8(fmt.stdout).expect("fmt stdout is utf8");
    assert!(formatted.contains('\n'));

    let min = Command::new(bin)
        .arg("min")
        .arg(&file)
        .output()
        .expect("sing min should run");
    assert!(
        min.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&min.stderr)
    );
    let minimized = String::from_utf8(min.stdout).expect("min stdout is utf8");
    assert!(minimized.len() < formatted.len());
    assert!(!minimized.contains('\n'));
    assert!(minimized.contains("out(\"hi\")"));

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn doc_and_explain_emit_compact_json_for_ai_tools() {
    let bin = env!("CARGO_BIN_EXE_sing");

    let doc = Command::new(bin)
        .args(["doc", "../../examples/hello.sg"])
        .output()
        .expect("sing doc should run");
    assert!(
        doc.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&doc.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&doc.stdout).expect("doc output is JSON");
    assert_eq!(json["module"], "H");
    assert!(json["items"]
        .as_array()
        .is_some_and(|items| !items.is_empty()));
    assert!(json["token_cost"]["total"].as_u64().unwrap() > 0);

    let explain = Command::new(bin)
        .args(["explain", "E0401"])
        .output()
        .expect("sing explain should run");
    assert!(
        explain.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&explain.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&explain.stdout).expect("explain output is JSON");
    assert_eq!(json["code"], "E0401");
    assert!(json["hint"].as_str().unwrap().contains("type"));
}

#[test]
fn test_and_repl_execute_small_sources() {
    let bin = env!("CARGO_BIN_EXE_sing");
    let dir = temp_dir("test");
    let file = write_temp_source(&dir, "tests.sg", "m T;#add:1+1==2;#truth:1");

    let tests = Command::new(bin)
        .arg("test")
        .arg(&file)
        .output()
        .expect("sing test should run");
    assert!(
        tests.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&tests.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&tests.stdout).expect("test output is JSON");
    assert_eq!(json["ok"], true);
    assert_eq!(json["passed"], 2);
    assert_eq!(json["failed"], 0);

    let repl = Command::new(bin)
        .args(["repl", "--eval", "1+2"])
        .output()
        .expect("sing repl should run");
    assert!(
        repl.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&repl.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&repl.stdout), "3\n");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn pkg_reports_manifest_lock_and_runtime_contract() {
    let bin = env!("CARGO_BIN_EXE_sing");
    let dir = temp_dir("pkg");
    fs::create_dir_all(dir.join("src")).expect("temp package dir should be writable");
    fs::write(
        dir.join("Sing.toml"),
        "name=\"tiny\"\nversion=\"0.1.0\"\nentry=\"src/main.sg\"\n",
    )
    .expect("manifest should be writable");
    fs::write(dir.join("src/main.sg"), "RZ f main()>v:0").expect("source should be writable");

    let output = Command::new(bin)
        .arg("pkg")
        .arg(&dir)
        .output()
        .expect("sing pkg should run");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("pkg output is JSON");
    assert_eq!(json["manifest"]["name"], "tiny");
    assert_eq!(json["lock"]["package_hash"].as_str().unwrap().len(), 16);
    assert_eq!(json["runtime"]["panic"], "forbidden");
    assert_eq!(json["runtime"]["runtime"], "none");

    let _ = fs::remove_dir_all(dir);
}
